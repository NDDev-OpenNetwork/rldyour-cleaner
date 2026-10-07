//! Cache inventory and one verified native GC adapter. We never age-evict
//! arbitrary files, installed runtimes, dependency trees or browser profiles.
use crate::{config::Policy, os, process, safety};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Serialize)]
pub struct CacheResult {
    pub id: String,
    pub paths: Vec<PathBuf>,
    pub action: &'static str,
    // Native uv reports its own estimate in detail. We do not turn filesystem
    // free-space deltas or cache tree sizes into claimed reclaimed bytes.
    pub freed_bytes: Option<u64>,
    pub detail: String,
}

fn result(
    id: &str,
    paths: Vec<PathBuf>,
    action: &'static str,
    detail: impl Into<String>,
) -> CacheResult {
    CacheResult {
        id: id.into(),
        paths,
        action,
        freed_bytes: None,
        detail: detail.into(),
    }
}

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

fn prune_uv(policy: &Policy, dry_run: bool) -> CacheResult {
    let Some(binary) = os::executable("uv") else {
        return result(
            "uv",
            vec![],
            "missing-tool",
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
                "kept",
                "uv >=0.12.17 with in-use GC locking is required",
            );
        }
        Err(e) => return result("uv", vec![], "failed", e),
    };
    let output = match process::run(
        uv_command(&binary).args(["cache", "dir", "--offline", "--color", "never"]),
        quick,
    ) {
        Ok(output) => output,
        Err(e) => return result("uv", vec![], "failed", e),
    };
    if output.stdout.lines().count() != 1 {
        return result(
            "uv",
            vec![],
            "failed",
            "uv returned an invalid cache directory",
        );
    }
    let path = PathBuf::from(output.stdout);
    if path.symlink_metadata().is_err() {
        return result(
            "uv",
            vec![path],
            "absent",
            "no cache exists; no directory is created",
        );
    }
    let canonical = match safety::cache_allowed(&path, policy) {
        Ok(path) => path,
        Err(e) => return result("uv", vec![path], "protected", e),
    };
    if dry_run {
        return result(
            "uv",
            vec![canonical],
            "would-prune",
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
            "pruned",
            [output.stdout, output.stderr]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("; "),
        ),
        Err(e) => result("uv", vec![canonical], "failed", e),
    }
}

fn existing(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|p| p.symlink_metadata().is_ok())
        .collect()
}

pub fn evaluate(policy: &Policy, dry_run: bool) -> Vec<CacheResult> {
    if !policy.categories.home_caches {
        return Vec::new();
    }
    let home = os::home_dir();
    let cache = os::cache_dir();
    let data = os::data_local_dir();
    let mut results = vec![prune_uv(policy, dry_run)];
    let cargo = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cargo"));
    let gradle = std::env::var_os("GRADLE_USER_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".gradle"));
    let go_build = std::env::var_os("GOCACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| cache.join("go-build"));
    for (id, paths, managed) in [
        (
            "cargo",
            vec![cargo.join("registry"), cargo.join("git")],
            true,
        ),
        ("gradle", vec![gradle.join("caches")], true),
        ("go-build", vec![go_build], true),
        ("go-modcache", vec![home.join("go/pkg/mod")], false),
        ("bun", vec![home.join(".bun/install/cache")], false),
        ("npm", vec![home.join(".npm/_cacache")], false),
        (
            "pub",
            vec![home.join(".pub-cache"), cache.join("Pub/Cache")],
            false,
        ),
        (
            "pip",
            vec![cache.join("pip"), cache.join("pip/cache")],
            false,
        ),
        ("pre-commit", vec![cache.join("pre-commit")], false),
        ("playwright", vec![cache.join("ms-playwright")], false),
        ("puppeteer", vec![cache.join("puppeteer")], false),
        ("codex-runtimes", vec![cache.join("codex-runtimes")], false),
        (
            "pnpm-store",
            vec![data.join("pnpm/store"), home.join("Library/pnpm/store")],
            false,
        ),
    ] {
        let paths = existing(paths);
        if !paths.is_empty() {
            results.push(result(
                id,
                paths,
                if managed { "managed" } else { "kept" },
                if managed {
                    "owner has automatic cache GC; left to that tool"
                } else {
                    "no verified unused-entry GC adapter; age is not proof of non-use"
                },
            ));
        }
    }
    for path in &policy.extra_cache_paths {
        results.push(result(
            "extra-cache",
            vec![path.clone()],
            "kept",
            "custom paths are inventory-only",
        ));
    }
    if policy.categories.devin_versions {
        results.push(result(
            "devin-versions",
            vec![data.join("devin/cli/_versions")],
            "kept",
            "installed versions are not disposable cache",
        ));
    }
    if policy.categories.trash {
        results.push(result(
            "trash",
            vec![],
            "kept",
            "trash contains user data and is never emptied",
        ));
    }
    results
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
