//! End-to-end fixtures: real dirs, real mtimes, real guards. Each test builds
//! a tiny project tree under its own temp dir, ages it with `filetime`, and
//! exercises scan → guard → execute.

use filetime::{FileTime, set_file_mtime, set_file_times};
use rldyour_cleaner::clean;
use rldyour_cleaner::config::{Ages, Policy};
use rldyour_cleaner::safety;
use rldyour_cleaner::scan::{self, Candidate};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Fixture dir that removes itself on drop — tests must not leak into /tmp.
struct TempDir(PathBuf);

impl std::ops::Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> TempDir {
    let dir = std::env::temp_dir().join(format!(
        "rldc-test-{}-{}-{name}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    TempDir(dir)
}

/// Age every entry (files and dirs, bottom-up so parent mtimes land last).
fn age_tree(root: &Path, days_ago: i64) {
    let ft = FileTime::from_unix_time(FileTime::now().unix_seconds() - days_ago * 86_400, 0);
    let mut entries: Vec<PathBuf> = walkdir_collect(root);
    entries.sort_by_key(|e| std::cmp::Reverse(e.components().count()));
    for e in &entries {
        if e.is_dir() {
            let _ = set_file_times(e, ft, ft);
        } else {
            let _ = set_file_mtime(e, ft);
        }
    }
    let _ = set_file_times(root, ft, ft);
}

fn walkdir_collect(root: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    fn rec(p: &Path, v: &mut Vec<PathBuf>) {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                let p = e.path();
                v.push(p.clone());
                if p.is_dir() {
                    rec(&p, v);
                }
            }
        }
    }
    rec(root, &mut v);
    v
}

fn policy_for(root: &Path) -> Policy {
    Policy {
        roots: vec![root.to_path_buf()],
        // Tests set fixture mtimes explicitly; the freshness floor is about
        // *now* writes, not fixture age, so zero it to keep guards honest.
        guard_fresh_minutes: 0,
        ..Policy::default()
    }
}

const AGES: Ages = Ages {
    stale: 14,
    dep_stale: 30,
    incremental: 7,
    cache_entry: 30,
};

fn stale_candidates(policy: &Policy) -> Vec<Candidate> {
    scan::scan(policy, AGES).stale
}

#[test]
fn stale_rust_target_is_a_candidate_and_deletes() {
    let root = fixture("rust-stale");
    let proj = root.join("crate");
    fs::create_dir_all(proj.join("src")).unwrap();
    fs::write(proj.join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
    fs::write(proj.join("src/lib.rs"), "pub fn f() {}").unwrap();
    let target = proj.join("target");
    fs::create_dir_all(target.join("debug/deps")).unwrap();
    fs::write(target.join("debug/deps/libx.rlib"), vec![0u8; 4096]).unwrap();
    fs::write(target.join("CACHEDIR.TAG"), b"tag").unwrap();
    age_tree(&root, 30);

    let policy = policy_for(&root);
    let stale = stale_candidates(&policy);
    assert_eq!(stale.len(), 1, "expected one stale candidate: {stale:?}");
    assert_eq!(stale[0].path, target);
    assert!(stale[0].size_bytes >= 4096);

    let c = &stale[0];
    let prepared = safety::guard(c, &policy).expect("guards should pass");
    assert!(clean::execute(&prepared).is_ok());
    assert!(!target.exists());
}

#[test]
fn fresh_rust_target_is_left_alone() {
    let root = fixture("rust-fresh");
    let proj = root.join("crate");
    fs::create_dir_all(proj.join("target/debug")).unwrap();
    fs::write(proj.join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
    fs::write(proj.join("target/debug/a.o"), b"o").unwrap();
    // Everything is `now` — far inside the 14-day stale window.

    let policy = policy_for(&root);
    assert!(stale_candidates(&policy).is_empty());
}

#[test]
fn node_modules_follows_project_activity_not_its_own_age() {
    let root = fixture("node-stale");
    let proj = root.join("app");
    fs::create_dir_all(proj.join("node_modules/pkg")).unwrap();
    fs::write(proj.join("package.json"), "{}").unwrap();
    fs::write(proj.join("node_modules/pkg/index.js"), b"").unwrap();
    fs::write(proj.join("index.js"), "// src").unwrap();
    age_tree(&root, 40);

    let policy = policy_for(&root);
    let stale = stale_candidates(&policy);
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].kind, rldyour_cleaner::kinds::Kind::NodeModules);

    // Same tree, but the project was touched an hour ago → survives.
    let root2 = fixture("node-active");
    let proj2 = root2.join("app");
    fs::create_dir_all(proj2.join("node_modules/pkg")).unwrap();
    fs::write(proj2.join("package.json"), "{}").unwrap();
    fs::write(proj2.join("index.js"), "// src").unwrap();
    age_tree(&proj2.join("node_modules"), 40); // deps old, project fresh
    let policy2 = policy_for(&root2);
    assert!(stale_candidates(&policy2).is_empty());
}

#[test]
fn protected_substring_blocks_deletion() {
    let root = fixture("protected");
    let proj = root.join("keepme-app");
    fs::create_dir_all(proj.join("target/debug")).unwrap();
    fs::write(proj.join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
    age_tree(&root, 30);

    let mut policy = policy_for(&root);
    let stale = stale_candidates(&policy);
    assert_eq!(stale.len(), 1);
    policy.protect = vec!["keepme-app".into()];
    match safety::guard(&stale[0], &policy) {
        Err(s) if s.reason == "protected" => {}
        other => panic!("expected protected skip, got {:?}", other.is_ok()),
    }
}

#[test]
fn process_holding_project_blocks_node_modules() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let root = fixture("proc-held");
    let proj = root.join("app");
    fs::create_dir_all(proj.join("node_modules/pkg")).unwrap();
    fs::write(proj.join("package.json"), "{}").unwrap();
    age_tree(&root, 40);

    let mut policy = policy_for(&root);
    // The freshness floor is off for the fixture; the proc guard is the star.
    policy.guard_fresh_minutes = 0;
    let stale = stale_candidates(&policy);
    assert_eq!(stale.len(), 1);

    let child = std::process::Command::new("sleep")
        .arg("10")
        .current_dir(&proj)
        .spawn()
        .expect("spawn sleep");
    std::thread::sleep(std::time::Duration::from_millis(150));
    let verdict = safety::guard(&stale[0], &policy);
    let mut child = child;
    let _ = child.kill();
    let _ = child.wait();
    match verdict {
        Err(s) if s.reason == "in-use" => {}
        _ => panic!("expected in-use skip while a process sits in the project"),
    }
}

#[test]
fn held_cargo_lock_blocks_target_deletion() {
    let root = fixture("cargo-locked");
    let proj = root.join("crate");
    let target = proj.join("target");
    fs::create_dir_all(&target).unwrap();
    fs::write(proj.join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
    let lock_path = target.join(".cargo-lock");
    fs::write(&lock_path, b"").unwrap();
    age_tree(&root, 30);

    let policy = policy_for(&root);
    let stale = stale_candidates(&policy);
    assert_eq!(stale.len(), 1);

    // Hold the lock the way cargo does: exclusive, for the whole build.
    use fs2::FileExt;
    let f = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock_path)
        .unwrap();
    f.lock_exclusive().unwrap();
    let verdict = safety::guard(&stale[0], &policy);
    f.unlock().unwrap();
    match verdict {
        Err(s) if s.reason == "locked" => {}
        _ => panic!("expected locked skip while .cargo-lock is held"),
    }
}

#[test]
fn incremental_inside_warm_target_gets_shorter_leash() {
    let root = fixture("incremental");
    let proj = root.join("crate");
    let inc = proj.join("target/debug/incremental");
    let sub = inc.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(proj.join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
    fs::write(sub.join("work.bin"), b"w").unwrap();
    // The incremental cache is old...
    age_tree(&inc, 10);
    // ...but the rest of target was written today → target survives,
    // incremental (>7d) is offered separately.
    let _ = fs::write(proj.join("target/debug/fresh.o"), b"f");

    let policy = policy_for(&root);
    let stale = stale_candidates(&policy);
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].kind, rldyour_cleaner::kinds::Kind::RustIncremental);
    assert!(stale[0].path.ends_with("incremental"));
}
