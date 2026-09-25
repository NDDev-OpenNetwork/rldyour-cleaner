//! Windows process liveness — deliberately not a port of `/proc`.
//!
//! Windows has no cheap userspace "who holds this tree" API, and does not
//! need one: its mandatory file locking already refuses to rename or remove
//! a directory that a process sits in, executes from, or holds open. The
//! deletion attempt itself is the probe — `clean::execute` maps those
//! refusals (sharing violation, access denied) to a safe skip, and the
//! freshness floor plus the cargo-lock interlock do the rest.
//!
//! Returning `Some(vec![])` is therefore honest, not a gap: "no users
//! *enumerated*" is true, and the OS enforces what enumeration would miss.

use std::path::Path;

pub fn pids_using(_dir: &Path) -> Option<Vec<u32>> {
    Some(Vec::new())
}
