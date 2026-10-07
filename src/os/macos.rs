//! macOS user Library conventions. XDG variables do not change Library roots.
pub use super::unix::*;
use std::path::PathBuf;
pub fn home_dir() -> PathBuf {
    super::resolved_home("HOME")
}
pub fn config_dir() -> PathBuf {
    home_dir().join("Library/Application Support/rldyour-cleaner")
}
pub fn state_dir() -> PathBuf {
    config_dir()
}
pub fn cache_dir() -> PathBuf {
    home_dir().join("Library/Caches")
}
pub fn data_local_dir() -> PathBuf {
    home_dir().join("Library/Application Support")
}
pub fn executable_names(name: &str) -> Vec<String> {
    vec![name.into()]
}
pub fn npm_cache_dir() -> PathBuf {
    home_dir().join(".npm")
}
pub fn pnpm_cache_paths() -> Vec<PathBuf> {
    vec![
        home_dir().join("Library/pnpm/store"),
        cache_dir().join("pnpm"),
        home_dir().join(".pnpm-store"),
    ]
}
pub fn browser_cache_paths() -> Vec<PathBuf> {
    vec![
        cache_dir().join("Google/Chrome"),
        cache_dir().join("Google/Chrome Beta"),
        cache_dir().join("Chromium"),
    ]
}
