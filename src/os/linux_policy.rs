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
    let mut autoclean_days = None;
    if let Some(binary) = os::executable("systemctl") {
        let output = process::run(
            Command::new(binary)
                .args([
                    "--system",
                    "show",
                    "apt-daily.timer",
                    "apt-daily-upgrade.timer",
                    "systemd-tmpfiles-clean.timer",
                    "logrotate.timer",
                    "fstrim.timer",
                    "apt-daily-upgrade.service",
                    "--property=Id,LoadState,ActiveState,Unit,Result,ExecMainStatus,ExecMainExitTimestampMonotonic",
                ])
                .env("SYSTEMD_PAGER", "cat")
                .env("LC_ALL", "C"),
            timeout,
        );
        match output {
            Ok(out) if !out.stdout_truncated => {
                let units = properties(&out.stdout);
                for (unit, id, purpose) in [
                    ("apt-daily.timer", "apt-scheduler", "package index/download activities"),
                    ("apt-daily-upgrade.timer", "apt-clean-scheduler", "APT install-mode maintenance, including native autoclean"),
                    ("systemd-tmpfiles-clean.timer", "os-temp", "native temporary-file policy"),
                    ("logrotate.timer", "logrotate", "native log rotation"),
                    ("fstrim.timer", "ssd-trim", "unused-block discard, not file deletion"),
                ] {
                    let active = units.get(unit).is_some_and(|v| v.get("LoadState").map(String::as_str)==Some("loaded") && v.get("ActiveState").map(String::as_str)==Some("active"));
                    results.push(observation(id, if active { "active" } else { "inactive" }, format!("native {unit}: {purpose}; missing/disabled timers are not assumed active")));
                }
                let service = units.get("apt-daily-upgrade.service");
                let success = service.is_some_and(|v| v.get("LoadState").map(String::as_str)==Some("loaded") && v.get("Result").map(String::as_str)==Some("success") && v.get("ExecMainStatus").map(String::as_str)==Some("0") && v.get("ExecMainExitTimestampMonotonic").and_then(|v|v.parse::<u64>().ok()).is_some_and(|n|n>0));
                results.push(observation("apt-clean-job", if success { "completed" } else { "unknown" }, if success { "last systemd APT install-mode invocation succeeded; wrapper success alone does not prove autoclean ran" } else { "no successful completed APT install-mode invocation could be confirmed" }));
            }
            _ => results.push(observation("os-scheduler", "unknown", "native timer status could not be queried completely; no successful maintenance inferred")),
        }
    }
    if let Some(binary) = os::executable("apt-config") {
        let output = process::run(
            Command::new(binary)
                .args([
                    "shell",
                    "ENABLE",
                    "APT::Periodic::Enable",
                    "INTERVAL",
                    "APT::Periodic::AutocleanInterval",
                    "KEEP_INSTALLED",
                    "APT::Clean-Installed",
                ])
                .env_remove("APT_CONFIG"),
            timeout,
        );
        match output {
            Ok(out) if !out.stdout_truncated => {
                let policy = apt_policy(&out.stdout);
                if policy.status == "configured" {
                    autoclean_days = out.stdout.lines().find_map(|line| {
                        line.strip_prefix("INTERVAL='")?
                            .strip_suffix('\'')?
                            .parse::<u64>()
                            .ok()
                    });
                }
                results.push(policy);
            }
            _ => results.push(observation(
                "apt-autoclean",
                "unknown",
                "APT policy query failed or was truncated; no successful maintenance inferred",
            )),
        }
    }
    if Path::new("/usr/lib/apt/apt.systemd.daily").is_file() {
        results.push(autoclean_stamp(
            Path::new("/var/lib/apt/periodic/autoclean-stamp"),
            std::time::SystemTime::now(),
            autoclean_days,
        ));
    }
    results.push(observation("system-logs", "owner-policy", "journald/logrotate own retention and size; cleaner never vacuums diagnostic logs or crash reports"));
    results.push(observation("container-build-cache", "owner-policy", "Docker/BuildKit GC belongs to its daemon policy; images, containers and volumes are never pruned by cleaner"));
    results
}
fn autoclean_stamp(path: &Path, now: std::time::SystemTime, days: Option<u64>) -> SystemPolicy {
    if crate::safety::plain_path(path).is_err() {
        return observation(
            "apt-autoclean-last-success",
            "unknown",
            "native APT completion stamp is redirected or unverifiable",
        );
    }
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => observation(
            "apt-autoclean-last-success",
            "waiting",
            "no native autoclean completion stamp yet; configured policy waits for the install-mode timer; no job is started by this check",
        ),
        Ok(md) if md.is_file() => match md.modified().ok().and_then(|t| now.duration_since(t).ok())
        {
            Some(age)
                if days.is_some_and(|days| {
                    age.as_secs() > days.saturating_add(2).saturating_mul(86400)
                }) =>
            {
                observation(
                    "apt-autoclean-last-success",
                    "unknown",
                    "native autoclean completion stamp is older than the configured interval plus two days; check native job conditions",
                )
            }
            Some(age) => observation(
                "apt-autoclean-last-success",
                "completed",
                format!(
                    "native APT autoclean completion stamp was updated {} seconds ago; stamp metadata only, no log/cache contents read",
                    age.as_secs()
                ),
            ),
            None => observation(
                "apt-autoclean-last-success",
                "unknown",
                "native completion timestamp is unreadable or in the future",
            ),
        },
        _ => observation(
            "apt-autoclean-last-success",
            "unknown",
            "native APT completion stamp is not a readable regular file",
        ),
    }
}
fn properties(output: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    output
        .split("\n\n")
        .filter_map(|block| {
            let values: BTreeMap<_, _> = block
                .lines()
                .filter_map(|line| line.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            Some((values.get("Id")?.clone(), values))
        })
        .collect()
}
fn apt_policy(text: &str) -> SystemPolicy {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line
            .split_once("='")
            .and_then(|(k, v)| v.strip_suffix('\'').map(|v| (k, v)))
        else {
            return observation(
                "apt-autoclean",
                "unknown",
                "unrecognized native APT policy output; no shell evaluation performed",
            );
        };
        if !matches!(key, "ENABLE" | "INTERVAL" | "KEEP_INSTALLED")
            || values.insert(key, value).is_some()
        {
            return observation(
                "apt-autoclean",
                "unknown",
                "unexpected or duplicated APT policy field",
            );
        }
    }
    let enabled = match values.get("ENABLE") {
        None => Some(1),
        Some(value) => value.parse::<u64>().ok(),
    };
    let interval = match values.get("INTERVAL") {
        None => Some(0),
        Some(value) => value.parse::<u64>().ok(),
    };
    if enabled == Some(0) || interval == Some(0) {
        return observation(
            "apt-autoclean",
            "disabled",
            "APT periodic maintenance or autoclean is disabled; interval alone does not enable it",
        );
    }
    if enabled.is_none() || interval.is_none() {
        return observation(
            "apt-autoclean",
            "unknown",
            "unrecognized effective APT interval/enable policy",
        );
    }
    let keep = values
        .get("KEEP_INSTALLED")
        .is_some_and(|s| matches!(*s, "false" | "0" | "no" | "off"));
    observation(
        "apt-autoclean",
        if keep { "configured" } else { "unknown" },
        format!(
            "native autoclean interval {} day(s); installed-package archive protection {}; apt-daily-upgrade.timer owns cleanup execution; this is configuration, not evidence that cleanup ran",
            interval.unwrap(),
            if keep { "enabled" } else { "not confirmed" }
        ),
    )
}

pub(super) fn scheduler_checks(timeout: Duration) -> Vec<crate::doctor::Check> {
    use crate::doctor::{Check, State};
    let Some(binary) = os::executable("systemctl") else {
        return vec![Check::new(
            "cleaner-schedule",
            State::Warning,
            "systemctl unavailable; scheduler registration cannot be checked",
        )];
    };
    match process::run(Command::new(binary).args(["--user", "show", "rldyour-cleaner.timer", "rldyour-cleaner.service", "--property=Id,LoadState,ActiveState,Unit,NextElapseUSecRealtime,Result,ExecMainStatus,ExecMainExitTimestampMonotonic"]).env("SYSTEMD_PAGER", "cat").env("LC_ALL", "C"), timeout) {
        Ok(out) if !out.stdout_truncated => scheduler_snapshot(&out.stdout),
        _ => vec![Check::new("cleaner-schedule", State::Warning, "user scheduler query failed or was incomplete; no job state invented")],
    }
}
fn scheduler_snapshot(output: &str) -> Vec<crate::doctor::Check> {
    use crate::doctor::{Check, State};
    let units = properties(output);
    let timer = units.get("rldyour-cleaner.timer");
    let active = timer.is_some_and(|v| {
        v.get("LoadState").map(String::as_str) == Some("loaded")
            && v.get("ActiveState").map(String::as_str) == Some("active")
            && v.get("Unit").map(String::as_str) == Some("rldyour-cleaner.service")
            && v.get("NextElapseUSecRealtime")
                .is_some_and(|v| !v.is_empty())
    });
    let mut checks = vec![Check::new(
        "cleaner-schedule",
        if active { State::Ok } else { State::Error },
        if active {
            "user timer is loaded, active, targets cleaner service and has a next calendar run"
        } else {
            "user timer is missing, disabled, targets another service or has no next calendar run"
        },
    )];
    let state = match units.get("rldyour-cleaner.service") {
        None => Check::new(
            "cleaner-job",
            State::Error,
            "cleaner service missing from scheduler snapshot",
        ),
        Some(v) if v.get("LoadState").map(String::as_str) != Some("loaded") => {
            Check::new("cleaner-job", State::Error, "cleaner service is not loaded")
        }
        Some(v) if v.get("ActiveState").map(String::as_str) == Some("activating") => Check::new(
            "cleaner-job",
            State::Info,
            "oneshot is currently running; no completed result inferred",
        ),
        Some(v)
            if !v
                .get("ExecMainExitTimestampMonotonic")
                .and_then(|v| v.parse::<u64>().ok())
                .is_some_and(|n| n > 0) =>
        {
            Check::new(
                "cleaner-job",
                State::Warning,
                "no completed service timestamp could be confirmed in this user-manager session",
            )
        }
        Some(v)
            if v.get("Result").map(String::as_str) == Some("success")
                && v.get("ExecMainStatus").map(String::as_str) == Some("0") =>
        {
            Check::new(
                "cleaner-job",
                State::Ok,
                "last completed oneshot succeeded; inactive between runs is normal",
            )
        }
        Some(_) => Check::new(
            "cleaner-job",
            State::Error,
            "last completed cleaner service result is missing or failed",
        ),
    };
    checks.push(state);
    checks
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
    fn apt_effective_enable_and_installed_archive_protection_are_checked() {
        assert_eq!(
            apt_policy("INTERVAL='7'\nKEEP_INSTALLED='false'").status,
            "configured"
        );
        assert_eq!(
            apt_policy("ENABLE='0'\nINTERVAL='7'\nKEEP_INSTALLED='false'").status,
            "disabled"
        );
        assert_eq!(apt_policy("INTERVAL='0'").status, "disabled");
        assert_eq!(
            apt_policy("INTERVAL='7'\nKEEP_INSTALLED='true'").status,
            "unknown"
        );
        assert_eq!(apt_policy("INTERVAL='$(unsafe)'").status, "unknown");
        assert_eq!(apt_policy("INTERVAL='7'\nINTERVAL='1'").status, "unknown");
    }
    #[test]
    fn timer_result_and_target_are_verified_without_treating_idle_as_failure() {
        let valid = "Id=rldyour-cleaner.timer\nLoadState=loaded\nActiveState=active\nUnit=rldyour-cleaner.service\nNextElapseUSecRealtime=tomorrow\n\nId=rldyour-cleaner.service\nLoadState=loaded\nActiveState=inactive\nResult=success\nExecMainStatus=0\nExecMainExitTimestampMonotonic=100";
        let checks = scheduler_snapshot(valid);
        assert!(checks.iter().all(|c| c.state == crate::doctor::State::Ok));
        assert_eq!(
            scheduler_snapshot(
                &valid.replace("Unit=rldyour-cleaner.service", "Unit=another.service")
            )[0]
            .state,
            crate::doctor::State::Error
        );
        assert_eq!(
            scheduler_snapshot(&valid.replace("Result=success", "Result=exit-code"))[1].state,
            crate::doctor::State::Error
        );
        assert_eq!(
            scheduler_snapshot(&valid.replace(
                "ExecMainExitTimestampMonotonic=100",
                "ExecMainExitTimestampMonotonic=0"
            ))[1]
                .state,
            crate::doctor::State::Warning
        );
        assert_eq!(scheduler_snapshot("")[0].state, crate::doctor::State::Error);
    }
    #[test]
    fn completion_stamp_preserves_missing_custom_interval_and_redirect_semantics() {
        let dir = std::env::temp_dir().join(format!("rldc-apt-stamp-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let stamp = dir.join("stamp");
        let now = std::time::UNIX_EPOCH + Duration::from_secs(100 * 86400);
        assert_eq!(autoclean_stamp(&stamp, now, Some(7)).status, "waiting");
        fs::write(&stamp, "synthetic-metadata-only").unwrap();
        filetime::set_file_mtime(&stamp, filetime::FileTime::from_unix_time(80 * 86400, 0))
            .unwrap();
        assert_eq!(autoclean_stamp(&stamp, now, Some(7)).status, "unknown");
        assert_eq!(autoclean_stamp(&stamp, now, Some(30)).status, "completed");
        fs::remove_file(&stamp).unwrap();
        std::os::unix::fs::symlink(dir.join("other"), &stamp).unwrap();
        assert_eq!(autoclean_stamp(&stamp, now, Some(7)).status, "unknown");
        fs::remove_dir_all(dir).unwrap();
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
