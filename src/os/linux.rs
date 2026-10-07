//! Linux XDG conventions. Relative XDG values are ignored per the specification.
pub use super::unix::*;
use std::path::PathBuf;
pub fn home_dir() -> PathBuf {
    super::resolved_home("HOME")
}
pub fn config_dir() -> PathBuf {
    super::absolute_env("XDG_CONFIG_HOME")
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("rldyour-cleaner")
}
pub fn state_dir() -> PathBuf {
    super::absolute_env("XDG_STATE_HOME")
        .unwrap_or_else(|| home_dir().join(".local/state"))
        .join("rldyour-cleaner")
}
pub fn cache_dir() -> PathBuf {
    super::absolute_env("XDG_CACHE_HOME").unwrap_or_else(|| home_dir().join(".cache"))
}
pub fn data_local_dir() -> PathBuf {
    super::absolute_env("XDG_DATA_HOME").unwrap_or_else(|| home_dir().join(".local/share"))
}
pub fn executable_names(name: &str) -> Vec<String> {
    vec![name.into()]
}
pub fn npm_cache_dir() -> PathBuf {
    home_dir().join(".npm")
}
pub fn pnpm_cache_paths() -> Vec<PathBuf> {
    vec![
        data_local_dir().join("pnpm/store"),
        cache_dir().join("pnpm"),
        home_dir().join(".pnpm-store"),
    ]
}
pub fn browser_cache_paths() -> Vec<PathBuf> {
    vec![
        cache_dir().join("google-chrome"),
        cache_dir().join("google-chrome-beta"),
        cache_dir().join("chromium"),
    ]
}
