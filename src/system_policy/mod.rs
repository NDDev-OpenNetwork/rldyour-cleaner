//! System maintenance remains owned by the OS/package manager. Observation
//! and the explicitly requested root APT policy are separate from user GC.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
use serde::Serialize;
#[cfg(windows)]
use windows as platform;

pub const APT_AUTOCLEAN_POLICY: &str = "// Managed by rldyour-cleaner; opt-in native APT maintenance.\n// Removes only obsolete downloaded archives; retains installed-package archives.\n// No apt clean, autoremove, kernel removal, reboot or separate root daemon.\nAPT::Periodic::AutocleanInterval \"7\";\nAPT::Clean-Installed \"false\";\n";

#[derive(Serialize)]
pub struct SystemPolicy {
    pub id: &'static str,
    pub status: &'static str,
    pub detail: String,
}
fn observation(id: &'static str, status: &'static str, detail: impl Into<String>) -> SystemPolicy {
    SystemPolicy {
        id,
        status,
        detail: detail.into(),
    }
}
pub fn observe(timeout: std::time::Duration) -> Vec<SystemPolicy> {
    platform::observe(timeout)
}
pub fn enable_apt_autoclean() -> Result<(), String> {
    platform::enable_apt_autoclean()
}
