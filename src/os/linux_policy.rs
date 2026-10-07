use super::{APT_AUTOCLEAN_POLICY, SystemPolicy, observation};
use crate::{os, process, safety};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    process::Command,
    time::Duration,
};

pub(super) fn observe(timeout: Duration) -> Vec<SystemPolicy> {
    let mut results = Vec::new();
    if let Some(binary) = os::executable("systemctl") {
        let output = process::run(
            Command::new(binary)
                .args([
                    "--system",
                    "show",
                    "apt-daily.timer",
                    "systemd-tmpfiles-clean.timer",
                    "logrotate.timer",
                    "fstrim.timer",
                    "--property=Id,LoadState,ActiveState",
                ])
                .env("SYSTEMD_PAGER", "cat")
                .env("LC_ALL", "C"),
            timeout,
        );
        match output {
            Ok(out) => {
                for (id, state) in timer_states(&out.stdout) {
                    let name = match id.as_str() {
                        "apt-daily.timer" => "apt-scheduler",
                        "systemd-tmpfiles-clean.timer" => "os-temp",
                        "logrotate.timer" => "logrotate",
                        "fstrim.timer" => "ssd-trim",
                        _ => continue,
                    };
                    results.push(observation(name, if state == "active" { "active" } else { "inactive" }, format!("native {id}: {state}; cleaner never performs a whole temporary-directory wipe")));
                }
            }
            Err(_) => results.push(observation(
                "os-scheduler",
                "unknown",
                "native timer status could not be queried; no maintenance/deletion inferred",
            )),
        }
    }
    if let Some(binary) = os::executable("apt-config") {
        match process::run(Command::new(binary).args(["shell", "INTERVAL", "APT::Periodic::AutocleanInterval"]), timeout) {
            Ok(out) => match apt_interval(&out.stdout) {
                Some(days) if days > 0 => results.push(observation("apt-autoclean", "configured", format!("native autoclean every {days} day(s); apt-daily timer must also be active; no package uninstall"))),
                Some(_) => results.push(observation("apt-autoclean", "disabled", "downloaded obsolete archives are not periodically pruned; opt-in apt-autoclean --enable installs a weekly policy")),
                None => results.push(observation("apt-autoclean", "unknown", "effective interval is absent or unrecognized; no deletion inferred")),
            },
            Err(_) => results.push(observation("apt-autoclean", "unknown", "APT policy query failed; no deletion inferred")),
        }
    }
    results.push(observation("system-logs", "owner-policy", "journald/logrotate own retention and size; cleaner never vacuums diagnostic logs or crash reports"));
    results.push(observation("container-build-cache", "owner-policy", "Docker/BuildKit GC belongs to its daemon policy; images, containers and volumes are never pruned by cleaner"));
    results
}
fn timer_states(output: &str) -> BTreeMap<String, String> {
    output
        .split("\n\n")
        .filter_map(|block| {
            let values: BTreeMap<_, _> = block
                .lines()
                .filter_map(|line| line.split_once('='))
                .collect();
            if values.get("LoadState") != Some(&"loaded") {
                return None;
            }
            Some((
                values.get("Id")?.to_string(),
                values.get("ActiveState")?.to_string(),
            ))
        })
        .collect()
}
fn apt_interval(text: &str) -> Option<u64> {
    text.trim()
        .strip_prefix("INTERVAL='")?
        .strip_suffix('\'')?
        .parse()
        .ok()
}

pub(super) fn enable_apt_autoclean() -> Result<(), String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(
            "apt-autoclean --enable requires root; the normal installer never requests sudo".into(),
        );
    }
    if !Path::new("/etc/debian_version").is_file()
        || !Path::new("/usr/lib/apt/apt.systemd.daily").is_file()
    {
        return Err("native Debian/Ubuntu APT periodic maintenance is required".into());
    }
    install_policy(Path::new("/etc/apt/apt.conf.d"), true)
}
fn install_policy(parent: &Path, require_root_owner: bool) -> Result<(), String> {
    safety::plain_path(parent).map_err(|e| e.to_string())?;
    let md = parent.symlink_metadata().map_err(|e| e.to_string())?;
    if !md.is_dir() || (require_root_owner && (md.uid() != 0 || md.mode() & 0o022 != 0)) {
        return Err("APT policy directory must be trusted and not writable by other users".into());
    }
    let path = parent.join("90-rldyour-cleaner-autoclean");
    safety::plain_path(&path).map_err(|e| e.to_string())?;
    match fs::symlink_metadata(&path) {
        Ok(md)
            if !md.is_file()
                || md.len() > 4096
                || (require_root_owner && (md.uid() != 0 || md.mode() & 0o022 != 0)) =>
        {
            return Err("existing APT policy is not a trusted regular file".into());
        }
        Ok(_) => {
            let mut content = String::new();
            fs::File::open(&path)
                .and_then(|f| f.take(4097).read_to_string(&mut content))
                .map_err(|e| e.to_string())?;
            return if content == APT_AUTOCLEAN_POLICY {
                Ok(())
            } else {
                Err("existing APT policy differs; refusing to overwrite local changes".into())
            };
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    // Build outside APT's accepted filename set, then publish atomically with
    // hard_link (no replace). An installer racing us can never be overwritten.
    let staging = parent.join(format!(".rldyour-cleaner-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&staging)
        .map_err(|e| e.to_string())?;
    let result = file
        .write_all(APT_AUTOCLEAN_POLICY.as_bytes())
        .and_then(|_| file.sync_all())
        .and_then(|_| fs::hard_link(&staging, &path))
        .and_then(|_| fs::File::open(parent)?.sync_all());
    drop(file);
    let cleanup = fs::remove_file(staging);
    result.and(cleanup).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_known_native_interval_and_loaded_timer_states_are_reported() {
        assert_eq!(apt_interval("INTERVAL='7'\n"), Some(7));
        assert_eq!(apt_interval("INTERVAL='0'"), Some(0));
        assert_eq!(apt_interval("INTERVAL='$(unsafe)'"), None);
        let states = timer_states(
            "ActiveState=active\nId=apt-daily.timer\nLoadState=loaded\n\nId=absent.timer\nLoadState=not-found\nActiveState=inactive",
        );
        assert_eq!(states.len(), 1);
        assert_eq!(states["apt-daily.timer"], "active");
    }
    #[test]
    fn native_apt_policy_is_opt_in_idempotent_and_preserves_local_edits() {
        let dir = std::env::temp_dir().join(format!("rldc-apt-policy-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        install_policy(&dir, false).unwrap();
        install_policy(&dir, false).unwrap();
        let path = dir.join("90-rldyour-cleaner-autoclean");
        fs::write(&path, "local admin policy").unwrap();
        assert!(install_policy(&dir, false).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "local admin policy");
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(dir.join("valuable"), &path).unwrap();
        assert!(install_policy(&dir, false).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
