use super::{SystemPolicy, observation};
pub(super) fn observe(_timeout: std::time::Duration) -> Vec<SystemPolicy> {
    vec![observation(
        "os-temp-and-logs",
        "owner-policy",
        "macOS owns temporary files and unified-log retention; cleaner never overrides OS policy or resets system caches",
    )]
}
pub(super) fn enable_apt_autoclean() -> Result<(), String> {
    Err("APT periodic maintenance is available only on Debian/Ubuntu Linux".into())
}
