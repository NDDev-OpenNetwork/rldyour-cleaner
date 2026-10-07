//! Locked native uv GC; discovery and mutation pin the same validated cache.
use super::{Action, CacheResult, result};
use crate::{config::Policy, os, process, safety};
use std::{path::Path, path::PathBuf, process::Command, time::Duration};

fn uv_command(binary: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .current_dir(os::home_dir())
        .env("UV_LOCK_TIMEOUT", "5")
        .env_remove("UV_NO_CACHE")
        .env_remove("UV_PROJECT")
        .env_remove("UV_WORKING_DIR");
    command
}

fn supported_uv(text: &str) -> bool {
    let Some(version) = text
        .strip_prefix("uv ")
        .and_then(|s| s.split_whitespace().next())
    else {
        return false;
    };
    let fields: Vec<_> = version.split('.').collect();
    if fields.len() != 3 {
        return false;
    }
    let parsed: Option<Vec<u64>> = fields.iter().map(|s| s.parse().ok()).collect();
    parsed.is_some_and(|v| (v[0], v[1], v[2]) >= (0, 12, 17))
}

pub(super) fn evaluate(
    policy: &Policy,
    dry_run: bool,
    cadence: &crate::maintenance::Ledger,
) -> CacheResult {
    if !policy.native_gc.uv {
        return result("uv", vec![], Action::Disabled, "disabled by native_gc.uv");
    }
    let Some(binary) = os::executable("uv") else {
        return result(
            "uv",
            vec![],
            Action::MissingTool,
            "uv is not installed; no manual fallback",
        );
    };
    let quick = Duration::from_secs(policy.command_timeout_seconds.min(5));
    let version = match process::run(uv_command(&binary).arg("--version"), quick) {
        Ok(output) if supported_uv(&output.stdout) => output.stdout,
        Ok(_) => {
            return result(
                "uv",
                vec![],
                Action::Kept,
                "uv >=0.12.17 with in-use GC locking is required",
            );
        }
        Err(e) => return result("uv", vec![], Action::Failed, e),
    };
    let output = match process::run(
        uv_command(&binary).args(["cache", "dir", "--offline", "--color", "never"]),
        quick,
    ) {
        Ok(output) => output,
        Err(e) => return result("uv", vec![], Action::Failed, e),
    };
    if output.stdout.lines().count() != 1 {
        return result(
            "uv",
            vec![],
            Action::Failed,
            "uv returned an invalid cache directory",
        );
    }
    let path = PathBuf::from(output.stdout);
    match path.symlink_metadata() {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return result(
                "uv",
                vec![path],
                Action::Absent,
                "no cache exists; no directory is created",
            );
        }
        Err(e) => {
            return result(
                "uv",
                vec![path],
                Action::Failed,
                format!("cache metadata is unreadable: {e}"),
            );
        }
    }
    let canonical = match safety::cache_allowed(&path, policy) {
        Ok(path) => path,
        Err(e) => return result("uv", vec![path], Action::Protected, e),
    };
    if !cadence.due("uv", &canonical, policy.native_gc.interval_hours) {
        return result(
            "uv",
            vec![canonical],
            Action::NotDue,
            "successful native GC is still within its interval; no cache scan or prune",
        );
    }
    if dry_run {
        return result(
            "uv",
            vec![canonical],
            Action::WouldPrune,
            format!("{version}: native unused-cache GC; size is unknown until uv runs"),
        );
    }
    // Pin the validated destination explicitly: discovery and mutation cannot
    // choose different locations through config or working-directory changes.
    let mut command = uv_command(&binary);
    command
        .args([
            "cache",
            "prune",
            "--offline",
            "--no-config",
            "--color",
            "never",
            "--no-progress",
            "--cache-dir",
        ])
        .arg(&canonical);
    match process::run(
        &mut command,
        Duration::from_secs(policy.command_timeout_seconds),
    ) {
        Ok(output) => result(
            "uv",
            vec![canonical],
            Action::Pruned,
            [output.stdout, output.stderr]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("; "),
        ),
        Err(e) => result("uv", vec![canonical], Action::Failed, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_known_uv_release_versions_enable_locked_gc() {
        assert!(supported_uv("uv 0.12.17 (release)"));
        assert!(supported_uv("uv 1.0.0"));
        assert!(!supported_uv("uv 0.12.16"));
        assert!(!supported_uv("uv 0.12.17-dev"));
        assert!(!supported_uv("something 4.0.0"));
    }
}
