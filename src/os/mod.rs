//! Compile-time facade: each OS supplies the same contract, with POSIX-only
//! mechanisms shared by Linux/macOS. Policy/GC/runner have no platform branches.
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(windows)]
pub use windows::*;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
compile_error!("Supported platforms: Linux, macOS, Windows");

pub fn absolute_env(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}
fn resolved_home(var: &str) -> PathBuf {
    absolute_env(var)
        .or_else(|| std::env::home_dir().filter(|p| p.is_absolute()))
        .unwrap_or_default()
}
pub fn validate_home() -> Result<(), String> {
    let home = home_dir();
    if home.is_absolute() && home.parent().is_some() {
        Ok(())
    } else {
        Err("no dedicated absolute user home is available".into())
    }
}
pub fn expand_home(raw: &str) -> PathBuf {
    match raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        Some(rest) => home_dir().join(rest),
        None if raw == "~" => home_dir(),
        None => PathBuf::from(raw),
    }
}
pub fn fs_use_pct(path: &Path) -> Option<u64> {
    let total = fs2::total_space(path).ok()?;
    let available = fs2::available_space(path).ok()?;
    (total != 0).then(|| (total.saturating_sub(available) as u128 * 100 / total as u128) as u64)
}
pub fn pressure_level(roots: &[PathBuf]) -> Option<u64> {
    roots
        .iter()
        .map(PathBuf::as_path)
        .chain(std::iter::once(home_dir().as_path()))
        .filter_map(fs_use_pct)
        .max()
}
pub fn executable(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .filter(|d| d.is_absolute())
        .flat_map(|dir| {
            executable_names(name)
                .into_iter()
                .map(move |name| dir.join(name))
        })
        .find(|p| is_executable(p))
}
