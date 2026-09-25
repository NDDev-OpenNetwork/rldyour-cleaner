//! The platform layer: every OS difference lives under `os/`.
//!
//! Two kinds of surface:
//!
//! * Shared helpers implemented once with `cfg!` branches — home/config/
//!   state/cache dirs, `~` expansion, PATH lookup, filesystem fill level.
//! * The process-liveness probe, `pids_using`, which has no portable shape:
//!   each platform gets its own module and its own honest answer.
//!
//! `pids_using` returns `Option<Vec<u32>>` — `None` means "this platform
//! cannot tell". Callers must treat `None` as in-use and skip: a deletion
//! tool fails closed, never open.

use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::pids_using;

#[cfg(all(unix, not(target_os = "linux")))]
mod unix_lsof;
#[cfg(all(unix, not(target_os = "linux")))]
pub(crate) use unix_lsof::pids_using;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::pids_using;

/// The user's home directory: `%USERPROFILE%` on Windows, `$HOME` elsewhere,
/// with `std::env::home_dir` as the last-resort answer. A machine with no
/// resolvable home gets `/` — config and caches under it simply won't exist.
pub fn home_dir() -> PathBuf {
    let primary = if cfg!(windows) {
        std::env::var_os("USERPROFILE")
    } else {
        std::env::var_os("HOME")
    };
    primary
        .map(PathBuf::from)
        .or_else(std::env::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Expand a leading `~`, `~/` or `~\` against the platform home dir.
/// Anything else passes through unchanged.
pub fn expand_home(raw: &str) -> PathBuf {
    let rest = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"));
    match rest {
        Some(r) => home_dir().join(r),
        None if raw == "~" => home_dir(),
        None => PathBuf::from(raw),
    }
}

/// Per-platform config root:
///   Linux/BSD `$XDG_CONFIG_HOME` (or `~/.config`), macOS
///   `~/Library/Application Support`, Windows `%LOCALAPPDATA%` (Local, not
///   Roaming — a policy full of machine paths must not roam).
pub fn config_dir() -> PathBuf {
    let base = if cfg!(windows) {
        env_path("LOCALAPPDATA").unwrap_or_else(|| home_dir().join(r"AppData\Local"))
    } else if cfg!(target_os = "macos") {
        home_dir().join("Library/Application Support")
    } else {
        env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home_dir().join(".config"))
    };
    base.join("rldyour-cleaner")
}

/// Where `last-run.json` lives: `$XDG_STATE_HOME`/`~/.local/state` on
/// Linux/BSD, `~/Library/Application Support` on macOS, `%LOCALAPPDATA%` on
/// Windows.
pub fn state_dir() -> PathBuf {
    if cfg!(unix) && !cfg!(target_os = "macos") {
        return env_path("XDG_STATE_HOME")
            .unwrap_or_else(|| home_dir().join(".local/state"))
            .join("rldyour-cleaner");
    }
    if cfg!(windows) {
        return env_path("LOCALAPPDATA")
            .unwrap_or_else(|| home_dir().join(r"AppData\Local"))
            .join("rldyour-cleaner");
    }
    config_dir()
}

/// Per-platform cache root — `~/.cache`, `~/Library/Caches`, `%LOCALAPPDATA%`.
pub fn cache_dir() -> PathBuf {
    if cfg!(windows) {
        env_path("LOCALAPPDATA").unwrap_or_else(|| home_dir().join(r"AppData\Local"))
    } else if cfg!(target_os = "macos") {
        home_dir().join("Library/Caches")
    } else {
        env_path("XDG_CACHE_HOME").unwrap_or_else(|| home_dir().join(".cache"))
    }
}

/// Per-platform local data root — `~/.local/share`,
/// `~/Library/Application Support`, `%LOCALAPPDATA%`.
pub fn data_local_dir() -> PathBuf {
    if cfg!(windows) {
        env_path("LOCALAPPDATA").unwrap_or_else(|| home_dir().join(r"AppData\Local"))
    } else if cfg!(target_os = "macos") {
        home_dir().join("Library/Application Support")
    } else {
        env_path("XDG_DATA_HOME").unwrap_or_else(|| home_dir().join(".local/share"))
    }
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// df-style used-percentage (0-100) of the filesystem holding `path`.
/// `fs2::available_space` maps to `statvfs` `f_bavail` on unix and to
/// `GetDiskFreeSpaceEx` on Windows — the number matches what `df`/Explorer
/// show. `None` means unmeasurable; callers then never trigger pressure mode.
pub fn fs_use_pct(path: &Path) -> Option<u64> {
    let total = fs2::total_space(path).ok()?;
    let avail = fs2::available_space(path).ok()?;
    if total == 0 {
        return None;
    }
    Some(((total - avail) as u128 * 100 / total as u128) as u64)
}

/// Worst fill level across every filesystem that matters: each scan root and
/// the home dir (where the tool caches live).
pub fn pressure_level(roots: &[PathBuf]) -> Option<u64> {
    let mut paths: Vec<PathBuf> = roots.to_vec();
    paths.push(home_dir());
    paths.iter().filter_map(|p| fs_use_pct(p)).max()
}

/// `which` without a dependency: does `name` resolve to an executable file
/// on PATH? Windows gets its spellings from `%PATHEXT%`.
pub fn on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path)
        .any(|dir| exe_names(name).iter().any(|n| is_executable(&dir.join(n))))
}

fn exe_names(name: &str) -> Vec<String> {
    if cfg!(windows) {
        let mut v = vec![name.to_string()];
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        for e in pathext.split(';').filter(|e| !e.trim().is_empty()) {
            v.push(format!("{name}{}", e.trim()));
        }
        v
    } else {
        vec![name.to_string()]
    }
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// Stable filesystem identity for the "never cross a mount" scan rule.
/// Unix reports `st_dev`; on Windows a plain tree cannot cross volumes
/// without a reparse point — and reparse points are already refused by the
/// symlink checks — so `None` correctly means "no boundary to check".
pub fn device_id(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path).ok().map(|m| m.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// du-style allocated size where the OS reports it (`st_blocks` on unix),
/// apparent size elsewhere — close enough for a report, not a quota.
pub fn file_size(md: &std::fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let blocks = md.blocks();
        if blocks > 0 {
            return blocks * 512;
        }
    }
    md.len()
}
