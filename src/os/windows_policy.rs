use super::{SystemPolicy, observation};
pub(super) fn observe(_timeout: std::time::Duration) -> Vec<SystemPolicy> {
    vec![observation(
        "os-storage",
        "owner-policy",
        "Storage Sense and Windows servicing own temporary/update-file cleanup; cleaner never changes registry policy, Downloads or Recycle Bin",
    )]
}
pub(super) fn enable_apt_autoclean() -> Result<(), String> {
    Err("APT periodic maintenance is available only on Debian/Ubuntu Linux".into())
}
