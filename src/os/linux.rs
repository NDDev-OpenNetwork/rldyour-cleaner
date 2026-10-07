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
