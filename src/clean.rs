//! The deletion itself: rename to a pending sibling, then remove the tree.
//!
//! Rename-first matters: a build or app starting mid-delete sees the artifact
//! *absent* and creates a fresh one, instead of erroring on half-deleted
//! files. Pending dirs whose removal crashed are reaped by a later run —
//! their `.rldyour-cleaner-pending` names make them identifiable.

use crate::safety::Prepared;
use crate::scan::Candidate;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub const PENDING_PREFIX: &str = ".rldyour-cleaner-pending";

fn pending_name(c: &Candidate) -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "{PENDING_PREFIX}-{}-{}-{n}",
        std::process::id(),
        c.path.file_name().and_then(|x| x.to_str()).unwrap_or("dir")
    )
}

/// Why `execute` did not free anything. `Busy` is not an error — it is the
/// OS's own liveness verdict: on Windows a tree that a process sits in,
/// executes from, or holds open simply cannot be renamed (mandatory
/// locking), which is exactly the process guard on that platform.
pub enum ExecuteError {
    /// Refused because the tree is in use — report as a skip, not a failure.
    Busy(String),
    /// A real failure worth reporting as such.
    Failed(String),
}

/// Is this rename refusal the OS telling us the tree is held open?
/// Windows: ERROR_ACCESS_DENIED (5), ERROR_SHARING_VIOLATION (32),
/// ERROR_LOCK_VIOLATION (33). Unix rename has no such semantics — every
/// failure there is a genuine error.
fn is_busy_error(e: &std::io::Error) -> bool {
    cfg!(windows) && matches!(e.raw_os_error(), Some(5) | Some(32) | Some(33))
}

/// Rename the candidate aside and delete it. Takes the `Prepared` — not just
/// the candidate — so cargo's lock file stays held across the rename+remove;
/// that is the whole point of the interlock. Returns freed bytes (the size
/// measured at scan time) or a message describing why nothing was removed.
pub fn execute(p: &Prepared<'_>) -> Result<u64, ExecuteError> {
    let c = p.candidate;
    if !c.path.exists() {
        return Err(ExecuteError::Failed("already gone".into()));
    }
    let pending = c
        .path
        .parent()
        .ok_or_else(|| ExecuteError::Failed("no parent dir".to_string()))?
        .join(pending_name(c));
    fs::rename(&c.path, &pending).map_err(|e| {
        if is_busy_error(&e) {
            ExecuteError::Busy(format!("the OS refuses the rename: {e}"))
        } else {
            ExecuteError::Failed(format!("rename failed: {e}"))
        }
    })?;
    match fs::remove_dir_all(&pending) {
        Ok(()) => Ok(c.size_bytes),
        Err(e) => {
            // Left behind: a `.rldyour-cleaner-pending-*` dir the next run's
            // reaper will finish off.
            Err(ExecuteError::Failed(format!(
                "partially removed ({e}); pending dir left for next run"
            )))
        }
    }
}

/// A pending dir from a crashed run may be deleted outright — it only ever
/// holds content we already decided to remove. Age floor still applies so a
/// half-written rename is not pulled from under a slow `remove_dir_all`.
pub fn is_pending_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(PENDING_PREFIX))
}

/// Remove leftover pending dirs older than `min_age_secs` under `roots`.
/// Returns (dirs removed, per-dir error notes).
pub fn reap_pending(roots: &[PathBuf], min_age_secs: u64) -> (usize, Vec<String>) {
    let mut removed = 0usize;
    let mut notes = Vec::new();
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(min_age_secs);
    for root in roots {
        let mut iter = walkdir::WalkDir::new(root).follow_links(false).into_iter();
        while let Some(entry) = iter.next() {
            let Ok(e) = entry else { continue };
            if e.depth() == 0 || !e.file_type().is_dir() {
                continue;
            }
            let name = e.file_name().to_str().unwrap_or("");
            if name.starts_with(PENDING_PREFIX) {
                iter.skip_current_dir();
                let p = e.into_path();
                let old_enough = fs::metadata(&p)
                    .and_then(|m| m.modified())
                    .is_ok_and(|m| m < cutoff);
                if !old_enough {
                    continue;
                }
                match fs::remove_dir_all(&p) {
                    Ok(()) => removed += 1,
                    Err(err) => notes.push(format!("{}: {err}", p.display())),
                }
                continue;
            }
            // Prune exactly the dirs the scanner never descends into — they
            // can never hold a candidate, hence never a pending sibling. Two
            // exceptions: `target` and `.dart_tool` DO get nested candidates
            // (`incremental`, `flutter_build`), so their pending dirs sit
            // inside and must stay reachable.
            if crate::kinds::SKIP_DIRS.contains(&name) && name != "target" && name != ".dart_tool" {
                iter.skip_current_dir();
            }
        }
    }
    (removed, notes)
}
