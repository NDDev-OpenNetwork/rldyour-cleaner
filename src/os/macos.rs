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
