//! Full CLI tests in private homes and synthetic caches, never the user's data.
use rldyour_cleaner::{clean::RunLock, process};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
static SEQ: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rldc-native-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn cache(&self) -> PathBuf {
        let path = self.0.join("uv-cache");
        fs::create_dir(&path).unwrap();
        fs::write(
            path.join("CACHEDIR.TAG"),
            b"Signature: 8a477f597d28d172789f06886806bc55",
        )
        .unwrap();
        fs::write(path.join("unused"), b"synthetic-unused").unwrap();
        fs::write(path.join("keep"), b"synthetic-referenced").unwrap();
        path
    }
    fn cli(&self, cache: &Path, policy: &str) -> Command {
        let config = self.0.join("config.toml");
        fs::write(&config, policy).unwrap();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rldyour-cleaner"));
        cmd.args(["--config"])
            .arg(config)
            .env("HOME", &self.0)
            .env("USERPROFILE", &self.0)
            .env("LOCALAPPDATA", self.0.join("local"))
            .env("XDG_STATE_HOME", self.0.join("state"))
            .env("XDG_CACHE_HOME", self.0.join("cache"))
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env("FIXTURE_CACHE", cache)
            .env("FIXTURE_LOG", self.0.join("gc.log"))
            .env("PATH", tool().parent().unwrap())
            .env("PATHEXT", ".EXE")
            .env_remove("FIXTURE_UV_VERSION");
        cmd
    }
    fn state(&self) -> PathBuf {
        if cfg!(windows) {
            self.0.join("local/rldyour-cleaner")
        } else if cfg!(target_os = "macos") {
            self.0.join("Library/Application Support/rldyour-cleaner")
        } else {
            self.0.join("state/rldyour-cleaner")
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn tool() -> &'static Path {
    static TOOL: OnceLock<PathBuf> = OnceLock::new();
    TOOL.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-tools")
            .join(std::process::id().to_string());
        fs::create_dir_all(&dir).unwrap();
        let binary = dir.join(if cfg!(windows) { "uv.exe" } else { "uv" });
        assert!(
            Command::new("rustc")
                .args(["--edition=2024", "tests/fixtures/native_tool.rs", "-o"])
                .arg(&binary)
                .status()
                .unwrap()
                .success()
        );
        binary
    })
}
fn json(output: std::process::Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn scan_and_dry_run_never_call_mutating_gc_or_overwrite_state() {
    let f = Fixture::new();
    let cache = f.cache();
    fs::create_dir_all(f.state()).unwrap();
    fs::write(f.state().join("last-run.json"), b"previous-report").unwrap();
    for args in [vec!["scan", "--json"], vec!["run", "--dry-run", "--json"]] {
        let report = json(f.cli(&cache, "").args(args).output().unwrap());
        assert_eq!(report["caches"][0]["action"], "would-prune");
        assert!(cache.join("unused").exists());
        assert!(!f.0.join("gc.log").exists());
        assert_eq!(
            fs::read(f.state().join("last-run.json")).unwrap(),
            b"previous-report"
        );
    }
}
#[test]
fn native_gc_only_removes_the_owners_dangling_fixture_and_saves_atomic_report() {
    let f = Fixture::new();
    let cache = f.cache();
    let report = json(f.cli(&cache, "").args(["run", "--json"]).output().unwrap());
    assert_eq!(report["caches"][0]["action"], "pruned");
    assert!(report["freed_bytes"].is_null());
    assert!(!cache.join("unused").exists());
    assert!(cache.join("keep").exists());
    assert!(f.state().join("last-run.json").is_file());
    assert!(
        !fs::read_to_string(f.0.join("gc.log"))
            .unwrap()
            .contains("--force")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(f.state()).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(f.state().join("last-run.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
#[test]
fn failed_native_gc_never_falls_back_to_file_deletion() {
    let f = Fixture::new();
    let cache = f.cache();
    fs::write(cache.join("busy.lock"), b"").unwrap();
    let out = f.cli(&cache, "").args(["run", "--json"]).output().unwrap();
    assert!(!out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["failed"], 1);
    assert_eq!(report["caches"][0]["action"], "failed");
    assert!(cache.join("unused").exists());
}
#[test]
fn protected_unmarked_and_old_uv_destinations_never_run_gc() {
    for scenario in ["protected", "unmarked", "old"] {
        let f = Fixture::new();
        let cache = f.cache();
        let policy = if scenario == "protected" {
            format!(
                "protect=[{}]",
                toml::Value::String(cache.to_string_lossy().into())
            )
        } else {
            String::new()
        };
        if scenario == "unmarked" {
            fs::remove_file(cache.join("CACHEDIR.TAG")).unwrap();
        }
        let mut cmd = f.cli(&cache, &policy);
        if scenario == "old" {
            cmd.env("FIXTURE_UV_VERSION", "0.12.16");
        }
        let report = json(cmd.args(["run", "--json"]).output().unwrap());
        assert_ne!(report["caches"][0]["action"], "pruned");
        assert!(cache.join("unused").exists());
        assert!(!f.0.join("gc.log").exists());
    }
}
#[test]
fn overlapping_runs_are_refused_before_native_gc() {
    let f = Fixture::new();
    let cache = f.cache();
    let _lock = RunLock::acquire(&f.state()).unwrap();
    let output = f.cli(&cache, "").args(["run", "--json"]).output().unwrap();
    assert!(!output.status.success());
    assert!(!f.0.join("gc.log").exists());
    assert!(cache.join("unused").exists());
}
#[test]
fn command_output_is_bounded_and_timeout_reaps_the_child() {
    let output = process::run(Command::new(tool()).arg("loud"), Duration::from_secs(5)).unwrap();
    assert_eq!(output.stdout.len(), 16 * 1024);
    assert_eq!(output.stderr.len(), 16 * 1024);
    let start = Instant::now();
    assert!(
        process::run(
            Command::new(tool()).arg("sleep"),
            Duration::from_millis(100)
        )
        .is_err()
    );
    assert!(start.elapsed() < Duration::from_secs(5));
}
#[cfg(unix)]
#[test]
fn redirected_cache_and_report_destinations_are_refused() {
    let f = Fixture::new();
    let cache = f.cache();
    let link = f.0.join("uv-link");
    std::os::unix::fs::symlink(&cache, &link).unwrap();
    let report = json(f.cli(&link, "").args(["run", "--json"]).output().unwrap());
    assert_eq!(report["caches"][0]["action"], "protected");
    assert!(cache.join("unused").exists());
    let destination = f.0.join("valuable");
    fs::write(&destination, b"keep").unwrap();
    fs::remove_file(f.state().join("last-run.json")).unwrap();
    std::os::unix::fs::symlink(&destination, f.state().join("last-run.json")).unwrap();
    assert!(
        rldyour_cleaner::report::Report::new(false, false, None)
            .save(&f.state())
            .is_err()
    );
    assert_eq!(fs::read(destination).unwrap(), b"keep");
}
#[test]
fn legacy_flags_and_pressure_cannot_delete_projects_versions_or_custom_paths() {
    let f = Fixture::new();
    let cache = f.cache();
    let data = f.0.join("valuable");
    fs::create_dir(&data).unwrap();
    fs::write(data.join("old"), b"keep").unwrap();
    let projects = f.0.join("projects");
    let target = projects.join("app/target/debug");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        projects.join("app/Cargo.toml"),
        "[package]\nname=\"fixture\"",
    )
    .unwrap();
    fs::write(target.join("output"), b"keep-build").unwrap();
    let age = filetime::FileTime::from_unix_time(
        filetime::FileTime::now().unix_seconds() - 40 * 86400,
        0,
    );
    for entry in walkdir::WalkDir::new(&projects).contents_first(true) {
        let entry = entry.unwrap();
        filetime::set_file_times(entry.path(), age, age).unwrap();
    }
    let policy = format!(
        "roots=[{}]\nextra_cache_paths=[{}]\npressure_pct=0\n[categories]\nprojects=true\ndevin_versions=true\ncargo_registry=true\ntrash=true",
        toml::Value::String(projects.to_string_lossy().into()),
        toml::Value::String(data.to_string_lossy().into())
    );
    let report = json(
        f.cli(&cache, &policy)
            .args(["run", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(report["pressure"], true);
    assert!(data.join("old").exists());
    assert_eq!(fs::read(target.join("output")).unwrap(), b"keep-build");
    assert!(
        report["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["action"] == "review-only")
    );
    assert!(
        report["caches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == "trash" && r["action"] == "kept")
    );
}
#[test]
fn effective_config_reflects_custom_values_and_initialization_refuses_overwrite() {
    let f = Fixture::new();
    let cache = f.cache();
    let output = f
        .cli(&cache, "command_timeout_seconds=9")
        .arg("config")
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: toml::Value = toml::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(value["command_timeout_seconds"].as_integer(), Some(9));
    let output = f
        .cli(&cache, "command_timeout_seconds=9")
        .args(["config", "--init"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn inherited_output_and_descendants_cannot_outlive_the_owned_command_tree() {
    for mode in ["descendants", "exit-with-descendant"] {
        let f = Fixture::new();
        let marker = f.0.join("heartbeat");
        let start = Instant::now();
        let outcome = process::run(
            Command::new(tool()).arg(mode).arg(&marker),
            Duration::from_secs(2),
        );
        if mode == "descendants" {
            assert!(outcome.is_err());
        } else {
            assert!(outcome.unwrap().stdout.contains("spawned synthetic"));
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(marker.exists());
        std::thread::sleep(Duration::from_millis(100));
        let stopped = fs::read(&marker).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            stopped,
            fs::read(&marker).unwrap(),
            "descendant survived {mode}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn relative_xdg_environment_values_use_absolute_platform_defaults() {
    let f = Fixture::new();
    let cache = f.cache();
    let report = json(
        f.cli(&cache, "")
            .env("XDG_STATE_HOME", "relative-state")
            .env("XDG_CACHE_HOME", "relative-cache")
            .args(["run", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(report["caches"][0]["action"], "pruned");
    assert!(
        f.0.join(".local/state/rldyour-cleaner/last-run.json")
            .is_file()
    );
    assert!(!f.0.join("relative-state").exists());
}

#[test]
fn oversized_policy_is_refused_before_cache_discovery_or_pruning() {
    let f = Fixture::new();
    let cache = f.cache();
    let output = f
        .cli(&cache, &" ".repeat(256 * 1024 + 1))
        .args(["run", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!f.0.join("gc.log").exists());
    assert!(cache.join("unused").exists());
}

#[cfg(unix)]
#[test]
fn explicit_config_initialization_keeps_existing_parent_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let parent = f.0.join("project");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    let config = parent.join("cleaner.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_rldyour-cleaner"))
        .arg("--config")
        .arg(&config)
        .args(["config", "--init"])
        .env("HOME", &f.0)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        fs::metadata(parent).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(config).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
