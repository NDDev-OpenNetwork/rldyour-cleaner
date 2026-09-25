//! Non-Linux unix process liveness — macOS and the BSDs have no `/proc`, so
//! `lsof` (which ships with the OS) is the equivalent probe.
//!
//! A full-system `lsof` dump takes ~1s, far too expensive per candidate, so
//! one snapshot is taken per process invocation and prefix-matched per
//! scope. It can lag by at most the run's own duration; the freshness
//! recheck at delete time and the cargo-lock interlock bound that window.
//!
//! `None` = `lsof` could not run — fail closed, every candidate counts as
//! in use.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static SNAP: OnceLock<Option<HashMap<u32, Vec<PathBuf>>>> = OnceLock::new();

fn snapshot() -> Option<&'static HashMap<u32, Vec<PathBuf>>> {
    SNAP.get_or_init(take_snapshot).as_ref()
}

/// `lsof -nP -F pn` emits machine-parseable field lines: `p<pid>` then
/// `n<name>` for each open file, `cwd`, and `txt` (mapped executables and
/// libraries — the unix answer to Linux `maps`). Non-path names (sockets,
/// pipes) never start with `/` and are dropped.
fn take_snapshot() -> Option<HashMap<u32, Vec<PathBuf>>> {
    let out = ["lsof", "/usr/sbin/lsof", "/usr/local/bin/lsof"]
        .iter()
        .find_map(|bin| Command::new(bin).args(["-nP", "-F", "pn"]).output().ok())?;
    // lsof exits 1 on partial permission failures while still printing the
    // rows it could read — only an empty dump is unusable.
    if out.stdout.is_empty() {
        return None;
    }
    let mut map: HashMap<u32, Vec<PathBuf>> = HashMap::new();
    let mut pid = 0u32;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let (tag, val) = line.split_at(1);
        match tag {
            "p" => pid = val.parse().unwrap_or(0),
            "n" if pid != 0 && val.starts_with('/') => {
                map.entry(pid).or_default().push(PathBuf::from(val));
            }
            _ => {}
        }
    }
    Some(map)
}

pub fn pids_using(dir: &Path) -> Option<Vec<u32>> {
    // lsof reports resolved paths; canonicalize so a symlinked root
    // (e.g. /tmp -> private/tmp) still matches.
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let snap = snapshot()?;
    let own = std::process::id();
    let mut hits: Vec<u32> = snap
        .iter()
        .filter(|(pid, paths)| **pid != own && paths.iter().any(|p| p.starts_with(&dir)))
        .map(|(pid, _)| *pid)
        .collect();
    hits.sort_unstable();
    Some(hits)
}
