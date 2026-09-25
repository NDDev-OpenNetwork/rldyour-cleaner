//! Tiny system probes: filesystem fill level (statvfs) and PATH lookup.

use std::path::{Path, PathBuf};

/// df-style used-percentage of the filesystem holding `path` (0-100).
/// `f_bavail` mirrors what `df` calls Avail, so the number matches what the
/// user sees. On non-unix targets there is no statvfs; callers get None and
/// pressure mode simply never triggers.
pub fn fs_use_pct(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        let c_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
        // SAFETY: statvfs writes a plain C struct we own; path is a valid
        // NUL-terminated string.
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c_path.as_ptr(), &mut st) } != 0 {
            return None;
        }
        if st.f_blocks == 0 {
            return None;
        }
        let used = (st.f_blocks - st.f_bavail) as u128;
        Some(((used * 100) / st.f_blocks as u128) as u64)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// Worst fill level across every filesystem that matters: each scan root and
/// $HOME (where the tool caches live).
pub fn pressure_level(roots: &[PathBuf]) -> Option<u64> {
    let mut paths: Vec<PathBuf> = roots.to_vec();
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home));
    }
    paths.iter().filter_map(|p| fs_use_pct(p)).max()
}

/// `which` without the dependency: executable file on PATH.
pub fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p).any(|dir| {
            let f = dir.join(name);
            f.is_file() && is_executable(&f)
        })
    })
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.exists()
}
