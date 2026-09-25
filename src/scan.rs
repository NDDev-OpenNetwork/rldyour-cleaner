//! Discovery: walk the configured roots, turn matching directories into
//! measured candidates. Measurement is the expensive part (a full metadata
//! walk), so it only runs on directories that are already past their age gate.

use crate::config::{Ages, Gate, Policy};
use crate::kinds::{self, Kind, SKIP_DIRS};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct Candidate {
    pub path: PathBuf,
    pub kind: Kind,
    /// du-style allocated size (st_blocks × 512 where available).
    pub size_bytes: u64,
    /// Newest mtime anywhere inside the candidate.
    pub newest: Option<SystemTime>,
    /// For project-gated kinds: the enclosing project dir — its process
    /// scope. For `RustIncremental`: the enclosing `target` dir — its cargo
    /// lock scope. `None` for kinds that guard themselves.
    pub project_root: Option<PathBuf>,
}

pub struct ScanOutcome {
    /// Candidates that passed their age gate (safety guards still apply).
    pub stale: Vec<Candidate>,
    /// Matched artifact dirs that were still fresh (kept for reporting).
    pub fresh_count: usize,
    pub bytes_seen: u64,
}

/// Directory names excluded from a project-activity walk — the artifact dirs
/// themselves plus purely derived state that says nothing about work.
const ACTIVITY_SKIP: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".venv",
    "venv",
    ".tox",
    ".nox",
    ".dart_tool",
    ".gradle",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".output",
    ".svelte-kit",
    ".turbo",
    ".parcel-cache",
    ".vite",
    ".astro",
    "coverage",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".hypothesis",
    ".ipynb_checkpoints",
    "Pods",
    ".serena",
];

/// (allocated bytes, newest mtime) under `dir`. Symlinks are counted, never
/// followed.
pub fn measure(dir: &Path) -> (u64, Option<SystemTime>) {
    let mut size = 0u64;
    let mut newest: Option<SystemTime> = None;
    for e in WalkDir::new(dir).follow_links(false).into_iter().flatten() {
        let Ok(md) = fs::symlink_metadata(e.path()) else {
            continue;
        };
        size = size.saturating_add(crate::os::file_size(&md));
        if let Ok(m) = md.modified()
            && newest.is_none_or(|n| m > n)
        {
            newest = Some(m);
        }
    }
    (size, newest)
}

/// True when *any* entry under `dir` was modified after `cutoff`.
/// Early-exits on the first hit — this is the cheap "still warm" probe.
pub fn any_newer_than(dir: &Path, cutoff: SystemTime) -> bool {
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .flatten()
        .any(|e| {
            fs::symlink_metadata(e.path())
                .and_then(|m| m.modified())
                .is_ok_and(|m| m > cutoff)
        })
}

/// Has the project been worked on within `days`? Source files are the signal;
/// artifact dirs and agent state are skipped (`.git` is sampled directly
/// first — an index write means work happened even between commits).
fn project_active_within(root: &Path, cutoff: SystemTime) -> bool {
    let git = root.join(".git");
    if git.is_dir() {
        for probe in ["", "index", "HEAD", "packed-refs", "logs/HEAD"] {
            let p = if probe.is_empty() {
                git.clone()
            } else {
                git.join(probe)
            };
            if let Ok(m) = fs::metadata(&p).and_then(|md| md.modified())
                && m > cutoff
            {
                return true;
            }
        }
    }
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir()
                && e.depth() > 0
                && e.file_name()
                    .to_str()
                    .is_some_and(|n| ACTIVITY_SKIP.contains(&n)))
        })
        .flatten()
        .any(|e| {
            fs::symlink_metadata(e.path())
                .and_then(|m| m.modified())
                .is_ok_and(|m| m > cutoff)
        })
}

fn cutoff(days: u64) -> SystemTime {
    SystemTime::now() - Duration::from_secs(days.saturating_mul(86_400))
}

/// Is `dir` ignored by the project's git? For generically-named dirs this is
/// the proof that the content is reproducible output, not source — a real
/// `build/` full of pin files is tracked and answers "no". `git
/// check-ignore` carries the full semantics (nested .gitignore files,
/// info/exclude, worktrees); None when git cannot answer (no git, parent
/// outside a worktree) and we treat that as "not provably output" → keep.
fn git_ignored(dir: &Path) -> Option<bool> {
    let parent = dir.parent()?;
    let name = dir.file_name()?.to_str()?;
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(parent)
        .args(["check-ignore", "-q", "--"])
        .arg(name)
        .output()
        .ok()?;
    match out.status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

fn passes_gate(kind: Kind, dir: &Path, project_root: Option<&Path>, ages: Ages) -> bool {
    match kind.gate() {
        Gate::ArtifactStale => !any_newer_than(dir, cutoff(ages.stale)),
        Gate::ProjectStale => {
            let Some(root) = project_root else {
                return false;
            };
            !project_active_within(root, cutoff(ages.dep_stale))
                // The dep dir itself must also be cold — a fresh `npm install`
                // inside an otherwise untouched project is still activity.
                && !any_newer_than(dir, cutoff(ages.dep_stale))
        }
    }
}

/// Emit `target/<profile>/incremental` children as their own candidates for a
/// target dir that survives the age gate (they get a shorter leash).
fn incremental_candidates(target: &Path, ages: Ages, min_size: u64) -> Vec<Candidate> {
    let mut out = Vec::new();
    let Ok(read) = fs::read_dir(target) else {
        return out;
    };
    for profile in read.flatten() {
        let inc = profile.path().join("incremental");
        let Ok(md) = fs::symlink_metadata(&inc) else {
            continue;
        };
        if !md.is_dir() || md.file_type().is_symlink() {
            continue;
        }
        if !any_newer_than(&inc, cutoff(ages.incremental)) {
            let (size_bytes, newest) = measure(&inc);
            if size_bytes < min_size {
                continue;
            }
            out.push(Candidate {
                path: inc,
                kind: Kind::RustIncremental,
                size_bytes,
                newest,
                project_root: Some(target.to_path_buf()),
            });
        }
    }
    out
}

pub fn scan(policy: &Policy, ages: Ages) -> ScanOutcome {
    let mut outcome = ScanOutcome {
        stale: Vec::new(),
        fresh_count: 0,
        bytes_seen: 0,
    };
    if !policy.categories.projects {
        return outcome;
    }

    for root in &policy.roots {
        if !root.is_dir() {
            continue;
        }
        let root_dev = crate::os::device_id(root);
        let mut warm_targets: Vec<PathBuf> = Vec::new();

        // Manual iteration (cargo-sweep style): `skip_current_dir` yields a
        // matched dir yet never descends into it — filter_entry would hide it
        // from us entirely, which is exactly wrong for artifact dirs.
        let mut iter = WalkDir::new(root).follow_links(false).into_iter();
        while let Some(entry) = iter.next() {
            let Ok(entry) = entry else { continue };
            if entry.depth() == 0 || !entry.file_type().is_dir() {
                continue;
            }
            let name = entry.file_name().to_str().unwrap_or("");
            if name.starts_with(crate::clean::PENDING_PREFIX)
                || crate::os::device_id(entry.path()) != root_dev
            {
                iter.skip_current_dir();
                continue;
            }
            let Some(kind) = kinds::match_dir(entry.path()) else {
                if SKIP_DIRS.contains(&name) {
                    iter.skip_current_dir();
                }
                continue;
            };
            // Generic names must additionally be ignored by git — otherwise
            // a tracked `build/` of pin files would count as an artifact.
            // Not-ignored keeps walking deeper: a nested project may sit
            // inside.
            if kind.needs_gitignore() && git_ignored(entry.path()) != Some(true) {
                continue;
            }
            iter.skip_current_dir();
            let path = entry.into_path();
            let project_root = path.parent().map(Path::to_path_buf);

            if !passes_gate(kind, &path, project_root.as_deref(), ages) {
                outcome.fresh_count += 1;
                if kind == Kind::RustTarget && policy.categories.incremental {
                    // Live target: its incremental caches may still be stale.
                    warm_targets.push(path.clone());
                }
                if kind == Kind::DartToolDeps {
                    // Live project: the pub-resolution side of `.dart_tool`
                    // stays, but `flutter_build` is pure build output with its
                    // own freshness.
                    let fb = path.join("flutter_build");
                    let ok = fs::symlink_metadata(&fb)
                        .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink());
                    if ok && !any_newer_than(&fb, cutoff(ages.stale)) {
                        let (size_bytes, newest) = measure(&fb);
                        if size_bytes < policy.min_size_bytes {
                            continue;
                        }
                        outcome.bytes_seen += size_bytes;
                        outcome.stale.push(Candidate {
                            path: fb,
                            kind: Kind::DartToolBuild,
                            size_bytes,
                            newest,
                            project_root: project_root.clone(),
                        });
                    }
                }
                continue;
            }
            let (size_bytes, newest) = measure(&path);
            if size_bytes < policy.min_size_bytes {
                continue;
            }
            outcome.bytes_seen += size_bytes;
            outcome.stale.push(Candidate {
                path,
                kind,
                size_bytes,
                newest,
                project_root,
            });
        }

        for target in warm_targets {
            for c in incremental_candidates(&target, ages, policy.min_size_bytes) {
                outcome.bytes_seen += c.size_bytes;
                outcome.stale.push(c);
            }
        }
    }
    outcome
}
