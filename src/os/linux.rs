//! Linux process liveness via `/proc`: a process "uses" a directory when its
//! `exe`, `cwd`, an open `fd`, or a memory map lands inside it. `maps` is
//! what catches a gradle daemon's mmap'd jars — they no longer hold an fd.
//!
//! The probe is per-call and cheap (a few ms of `/proc` reads); that keeps
//! the answer fresh at guard time rather than snapshotted at scan time.
//!
//! `None` = `/proc` unreadable (restricted containers, non-/proc mounts):
//! callers fail closed. Other users' processes are silently unreadable —
//! a documented under-report shared with every userspace liveness check;
//! the freshness floor and cargo-lock interlock bound the residual risk.

use std::fs;
use std::path::Path;

pub fn pids_using(dir: &Path) -> Option<Vec<u32>> {
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let own = std::process::id();
    let needle = dir.to_string_lossy().into_owned();
    let proc_dir = fs::read_dir("/proc").ok()?;
    let mut hits = Vec::new();
    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let Some(s) = name.to_str() else { continue };
        if !s.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(pid) = s.parse::<u32>() else { continue };
        if pid == own {
            continue;
        }
        let base = entry.path();
        let mut hit = false;
        for link in ["exe", "cwd"] {
            if fs::read_link(base.join(link)).is_ok_and(|t| t.starts_with(&dir)) {
                hit = true;
                break;
            }
        }
        if !hit && let Ok(fds) = fs::read_dir(base.join("fd")) {
            for fd in fds.flatten().take(8192) {
                if fs::read_link(fd.path()).is_ok_and(|t| t.starts_with(&dir)) {
                    hit = true;
                    break;
                }
            }
        }
        if !hit && let Ok(maps) = fs::read_to_string(base.join("maps")) {
            // Substring matching can over-report — a false "in-use" merely
            // skips a cleanup pass — but never under-reports.
            hit = maps.lines().any(|l| l.contains(needle.as_str()));
        }
        if hit {
            hits.push(pid);
        }
    }
    Some(hits)
}
