//! The guards that make unattended deletion safe. A candidate is removed only
//! when *every* guard allows it; each denial carries a human-readable reason.
//!
//! Order:
//!   path   — real dir, not a symlink, not protected
//!   fresh  — dir mtime, then a full tree walk: nothing written within
//!            `guard_fresh_minutes` (this IS the pre-rename recheck — the
//!            scan may have walked a 200 GB tree minutes ago)
//!   proc   — no process has the dir (or its project) as exe/cwd/fd/maps target
//!   lock   — cargo's `.cargo-lock` must be acquirable, and is HELD through
//!            the deletion inside `Prepared`, so a racing `cargo build`
//!            simply waits and then recreates the dir

use crate::config::Policy;
use crate::kinds::Kind;
use crate::scan::Candidate;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Why a candidate was refused: a machine-stable `reason` plus detail for
/// humans. `Ok(prepared)` means every guard passed.
#[derive(Debug)]
pub struct Skip {
    pub reason: &'static str,
    pub detail: String,
}

fn skip(reason: &'static str, detail: impl Into<String>) -> Skip {
    Skip {
        reason,
        detail: detail.into(),
    }
}

/// Everything the deletion step needs. `hold_lock` keeps cargo's lock file(s)
/// alive (and exclusively ours) until the drop at the end of `execute`.
pub struct Prepared<'a> {
    pub candidate: &'a Candidate,
    _hold_lock: Vec<File>,
}

/// Refuse instantly on path-shape problems. Returns the canonical path for
/// later checks (process entries resolve symlinks, so we must too).
fn path_guard(c: &Candidate, policy: &Policy) -> Result<PathBuf, Skip> {
    let md = fs::symlink_metadata(&c.path).map_err(|e| skip("gone", e.to_string()))?;
    if md.file_type().is_symlink() {
        return Err(skip("symlink", "never delete through a symlink"));
    }
    if !md.is_dir() {
        return Err(skip("not-dir", "expected a directory"));
    }
    let name = c.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.is_empty() || c.path.components().count() < 4 {
        return Err(skip("shallow", "refusing a suspiciously shallow path"));
    }
    let canon = c
        .path
        .canonicalize()
        .map_err(|e| skip("canonicalize", e.to_string()))?;
    // Round-trip the candidate through its own matching rules: a path whose
    // shape no longer proves its kind must not be deletable. `incremental`
    // and `flutter_build` are emitted by scan's nested-candidate paths, so
    // their expected shape is spelled out rather than re-run through RULES.
    let shape_ok = match c.kind {
        Kind::RustIncremental => {
            name == "incremental"
                && canon
                    .parent()
                    .and_then(Path::parent)
                    .and_then(|g| g.file_name())
                    .is_some_and(|n| n == "target")
        }
        Kind::DartToolBuild => {
            name == "flutter_build"
                && canon
                    .parent()
                    .and_then(|p| p.file_name())
                    .is_some_and(|n| n == ".dart_tool")
        }
        _ => crate::kinds::match_dir(&canon) == Some(c.kind),
    };
    if !shape_ok {
        return Err(skip(
            "shape",
            "path no longer proves its kind (name/marker mismatch)",
        ));
    }
    for pat in &policy.protect {
        if !pat.is_empty() && canon.to_string_lossy().contains(pat.as_str()) {
            return Err(skip(
                "protected",
                format!("matches protect pattern '{pat}'"),
            ));
        }
    }
    Ok(canon)
}

/// Cargo locks `<target>/.cargo-lock` (and the newer `.cargo-build-lock`) for
/// the whole build. Acquiring it exclusively proves no build is running; we
/// keep the File(s) so nothing can start one while we delete. Missing lock
/// files are *not* created — creating one would bump the dir's mtime and make
/// the pre-rename recheck see our own write as activity; their absence simply
/// means no build ever ran here to coordinate with.
fn cargo_lock(target: &Path) -> Result<Vec<File>, Skip> {
    use fs2::FileExt;
    let mut held = Vec::new();
    for name in [".cargo-lock", ".cargo-build-lock"] {
        let p = target.join(name);
        if !p.exists() {
            continue;
        }
        let file = match fs::OpenOptions::new().read(true).write(true).open(&p) {
            Ok(f) => f,
            Err(e) => {
                return Err(skip("lock", format!("cannot open {}: {e}", p.display())));
            }
        };
        if file.try_lock_exclusive().is_err() {
            return Err(skip("locked", format!("{name} is held by a cargo build")));
        }
        held.push(file);
    }
    Ok(held)
}

/// The directory whose cargo locks coordinate this candidate: `target` itself,
/// or — for `target/<profile>/incremental` — the enclosing target dir.
fn lock_dir(c: &Candidate) -> Option<PathBuf> {
    match c.kind {
        Kind::RustTarget => Some(c.path.clone()),
        Kind::RustIncremental => c.project_root.clone(),
        _ => None,
    }
}

/// Which subtree a live process must not sit in for this kind.
fn proc_scope(c: &Candidate) -> &Path {
    if c.kind.guards_project_root()
        && let Some(root) = &c.project_root
    {
        return root;
    }
    &c.path
}

/// PIDs whose exe, cwd, open fd, or memory map lands inside `dir`.
/// Delegates to the platform probe in `os/`; `None` means the platform
/// cannot enumerate (never silently "nobody uses it"). `dir` is
/// canonicalized here so callers cannot forget.
pub(crate) fn pids_using(dir: &Path) -> Option<Vec<u32>> {
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    crate::os::pids_using(&dir)
}

fn dir_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Run every guard for a candidate that already passed its age gate.
/// Called immediately before the rename, so it is also the last-second
/// freshness re-verification.
pub fn guard<'a>(c: &'a Candidate, policy: &Policy) -> Result<Prepared<'a>, Skip> {
    path_guard(c, policy)?;

    // Freshness floor, always: cheap (dir mtime only) and re-run on demand.
    let floor =
        SystemTime::now() - Duration::from_secs(policy.guard_fresh_minutes.saturating_mul(60));
    if dir_mtime(&c.path).is_some_and(|m| m > floor) {
        return Err(skip(
            "warm",
            format!(
                "directory changed within the last {} min",
                policy.guard_fresh_minutes
            ),
        ));
    }

    // The dir-mtime check above only sees immediate children; a build that
    // started after scan time wrote *inside* the tree, so probe once more —
    // bounded by the guard floor, early-exiting on first hit. This runs before
    // lock acquisition deliberately: creating `.cargo-lock` ourselves would
    // bump the dir's mtime and poison the next check.
    if crate::scan::any_newer_than(&c.path, floor) {
        return Err(skip("warm", "written within the guard window"));
    }

    match pids_using(proc_scope(c)) {
        Some(pids) if !pids.is_empty() => {
            return Err(skip(
                "in-use",
                format!(
                    "held by process(es) {}",
                    pids.iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            ));
        }
        // The platform cannot enumerate processes: fail closed. A deletion
        // tool that cannot check who is running deletes nothing that day.
        None => {
            return Err(skip(
                "proc-unknown",
                "process liveness unavailable on this platform run",
            ));
        }
        Some(_) => {}
    }

    // Lock last: it is the interlock that closes the gap between every check
    // above and the rename. A build that started meanwhile holds it and we
    // back out; otherwise we keep it until the directory is gone.
    let hold_lock = match lock_dir(c) {
        Some(d) => cargo_lock(&d)?,
        None => Vec::new(),
    };

    Ok(Prepared {
        candidate: c,
        _hold_lock: hold_lock,
    })
}
