//! User-editable policy. Parsed from `config.toml` under the platform config
//! dir (`os::config_dir`); every field has a default so a missing file means
//! "native-GC defaults".

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Age criteria used only for optional read-only artifact inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// The artifact's own newest mtime must be older than `stale_days`.
    /// Right for outputs that refresh whenever the project is built:
    /// `target/`, `build/`, `.next/`, `__pycache__/`, ...
    ArtifactStale,
    /// The project's own activity must be older than `dep_stale_days`.
    /// Right for dependency trees that are *not* rewritten on use:
    /// `node_modules/`, `.venv`, `.dart_tool`. Their own mtime only moves on
    /// reinstall, so "old" does not mean "unused".
    ProjectStale,
}

#[derive(Debug, Clone, Serialize)]
pub struct Policy {
    pub roots: Vec<PathBuf>,
    pub protect: Vec<String>,
    pub stale_days: u64,
    pub dep_stale_days: u64,
    pub incremental_days: u64,
    pub cache_entry_days: u64,
    pub guard_fresh_minutes: u64,
    pub pressure_pct: u64,
    pub pressure: Pressure,
    pub categories: Categories,
    pub min_size_bytes: u64,
    pub extra_cache_paths: Vec<PathBuf>,
    pub command_timeout_seconds: u64,
    pub native_gc: NativeGc,
}

/// Only verified adapters are configurable. Unknown providers fail closed.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct NativeGc {
    pub uv: bool,
    pub interval_hours: u64,
}
impl Default for NativeGc {
    fn default() -> Self {
        Self {
            uv: true,
            interval_hours: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Pressure {
    pub stale_days: u64,
    pub dep_stale_days: u64,
    pub incremental_days: u64,
    pub cache_entry_days: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Categories {
    /// Build/artifact directories found under `roots`.
    pub projects: bool,
    /// `target/*/incremental` gets its own (shorter) age gate.
    pub incremental: bool,
    /// Tool caches under the user's home (`~/.cache/*`, `~/.bun`, ...).
    pub home_caches: bool,
    /// Old `devin/cli/_versions/*` under the platform devin data dir,
    /// report-only; no version is removed.
    pub devin_versions: bool,
    /// `~/.cargo/registry` — off: cargo's built-in gc owns it (stable since
    /// Rust 1.88). This flag adds visibility, not a deletion capability.
    pub cargo_registry: bool,
    /// The platform trash dir (`~/.local/share/Trash`, `~/.Trash`) — off:
    /// deleted files are user data, not cache.
    pub trash: bool,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FilePolicy {
    roots: Vec<String>,
    protect: Vec<String>,
    stale_days: u64,
    dep_stale_days: u64,
    incremental_days: u64,
    cache_entry_days: u64,
    guard_fresh_minutes: u64,
    pressure_pct: u64,
    pressure: FilePressure,
    categories: FileCategories,
    min_size_bytes: u64,
    extra_cache_paths: Vec<String>,
    command_timeout_seconds: u64,
    native_gc: NativeGc,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FilePressure {
    stale_days: u64,
    dep_stale_days: u64,
    incremental_days: u64,
    cache_entry_days: u64,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileCategories {
    projects: bool,
    incremental: bool,
    home_caches: bool,
    devin_versions: bool,
    cargo_registry: bool,
    trash: bool,
    // 0.0.x generated this key under [categories] by mistake. Accept and
    // migrate it into the top-level report-only list; never delete it.
    extra_cache_paths: Vec<String>,
}

impl Default for FilePolicy {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            protect: Vec::new(),
            stale_days: 14,
            dep_stale_days: 30,
            incremental_days: 7,
            cache_entry_days: 30,
            guard_fresh_minutes: 15,
            pressure_pct: 101,
            pressure: FilePressure::default(),
            categories: FileCategories::default(),
            min_size_bytes: 0,
            extra_cache_paths: Vec::new(),
            command_timeout_seconds: 60,
            native_gc: NativeGc::default(),
        }
    }
}

impl Default for FilePressure {
    fn default() -> Self {
        Self {
            stale_days: 7,
            dep_stale_days: 21,
            incremental_days: 3,
            cache_entry_days: 14,
        }
    }
}

impl Default for FileCategories {
    fn default() -> Self {
        Self {
            projects: false,
            incremental: true,
            home_caches: true,
            devin_versions: false,
            cargo_registry: false,
            trash: false,
            extra_cache_paths: Vec::new(),
        }
    }
}

impl Policy {
    /// Effective ages for the current disk-pressure state.
    pub fn ages(&self, under_pressure: bool) -> Ages {
        let p = &self.pressure;
        Ages {
            stale: if under_pressure {
                p.stale_days
            } else {
                self.stale_days
            },
            dep_stale: if under_pressure {
                p.dep_stale_days
            } else {
                self.dep_stale_days
            },
            incremental: if under_pressure {
                p.incremental_days
            } else {
                self.incremental_days
            },
            cache_entry: if under_pressure {
                p.cache_entry_days
            } else {
                self.cache_entry_days
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Ages {
    pub stale: u64,
    pub dep_stale: u64,
    pub incremental: u64,
    pub cache_entry: u64,
}

impl Ages {
    /// The age bound that actually decides a candidate with this gate.
    pub fn for_gate(self, gate: Gate) -> u64 {
        match gate {
            Gate::ArtifactStale => self.stale,
            Gate::ProjectStale => self.dep_stale,
        }
    }
}

/// Expand a leading `~` against the platform home dir — see `os::expand_home`.
/// Kept re-exported here because every config string flows through it.
pub fn expand_home(raw: &str) -> PathBuf {
    crate::os::expand_home(raw)
}

/// `config.toml` under the per-platform config root
/// (`~/.config/rldyour-cleaner`, `~/Library/Application Support/…`,
/// `%LOCALAPPDATA%\…`).
pub fn config_path() -> PathBuf {
    crate::os::config_dir().join("config.toml")
}

/// Load the policy file, or fall back to defaults when it is absent.
/// A present-but-broken file is an error — silently applying defaults to a
/// mistyped policy is how deletions surprise people.
pub fn load(path: Option<&Path>) -> Result<Policy, String> {
    let path = path.map(PathBuf::from).unwrap_or_else(config_path);
    const LIMIT: u64 = 256 * 1024;
    match std::fs::symlink_metadata(&path) {
        Ok(md) if !md.is_file() || md.len() > LIMIT => {
            return Err("policy must be a regular file <=256 KiB".into());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Policy::default()),
        Err(e) => return Err(e.to_string()),
        _ => {}
    }
    use std::io::Read;
    let file = match std::fs::File::open(&path).and_then(|file| {
        let mut text = String::new();
        file.take(LIMIT + 1).read_to_string(&mut text)?;
        if text.len() as u64 > LIMIT {
            return Err(std::io::Error::other("policy exceeds 256 KiB"));
        }
        Ok(text)
    }) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Policy::default());
        }
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let parsed: FilePolicy =
        toml::from_str(&file).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
    let policy = policy_from(parsed);
    validate(&policy).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(policy)
}

impl Default for Policy {
    /// The balanced profile — the same values the shipped default file
    /// documents.
    fn default() -> Self {
        policy_from(FilePolicy::default())
    }
}

fn policy_from(f: FilePolicy) -> Policy {
    Policy {
        roots: f.roots.iter().map(|r| expand_home(r)).collect(),
        // Protect patterns are substrings of canonical paths — expand `~`
        // the same way `roots` does, or "~/x" would silently never match.
        protect: f
            .protect
            .iter()
            .map(|p| expand_home(p).to_string_lossy().into_owned())
            .collect(),
        stale_days: f.stale_days,
        dep_stale_days: f.dep_stale_days,
        incremental_days: f.incremental_days,
        cache_entry_days: f.cache_entry_days,
        guard_fresh_minutes: f.guard_fresh_minutes,
        // No clamp: >100 legitimately means "never" (use% tops out at 100).
        pressure_pct: f.pressure_pct,
        pressure: Pressure {
            stale_days: f.pressure.stale_days,
            dep_stale_days: f.pressure.dep_stale_days,
            incremental_days: f.pressure.incremental_days,
            cache_entry_days: f.pressure.cache_entry_days,
        },
        categories: Categories {
            projects: f.categories.projects,
            incremental: f.categories.incremental,
            home_caches: f.categories.home_caches,
            devin_versions: f.categories.devin_versions,
            cargo_registry: f.categories.cargo_registry,
            trash: f.categories.trash,
        },
        min_size_bytes: f.min_size_bytes,
        extra_cache_paths: f
            .extra_cache_paths
            .iter()
            .chain(f.categories.extra_cache_paths.iter())
            .map(|p| expand_home(p))
            .collect(),
        command_timeout_seconds: f.command_timeout_seconds,
        native_gc: f.native_gc,
    }
}

/// The default file, written by `install.sh` (and `config --init`) so users
/// see every knob with its meaning next to it.
pub fn validate(policy: &Policy) -> Result<(), String> {
    if !(1..=8760).contains(&policy.native_gc.interval_hours) {
        return Err("native_gc.interval_hours must be 1..=8760".into());
    }
    if !(1..=300).contains(&policy.command_timeout_seconds) {
        return Err("command_timeout_seconds must be 1..=300".into());
    }
    for days in [
        policy.stale_days,
        policy.dep_stale_days,
        policy.incremental_days,
        policy.cache_entry_days,
        policy.pressure.stale_days,
        policy.pressure.dep_stale_days,
        policy.pressure.incremental_days,
        policy.pressure.cache_entry_days,
    ] {
        if days > 36_500 {
            return Err("age thresholds must be <=36500 days".into());
        }
    }
    if policy.guard_fresh_minutes > 525_600 {
        return Err("guard_fresh_minutes must be <=525600".into());
    }
    for root in policy.roots.iter().chain(&policy.extra_cache_paths) {
        if !root.is_absolute()
            || root.parent().is_none()
            || root
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!("invalid scan root: {}", root.display()));
        }
    }
    Ok(())
}

pub const DEFAULT_CONFIG: &str = r#"# Only native unused-cache GC mutates content. Age-based project discovery
# is optional and report-only; legacy destructive category flags never delete.
roots = []
protect = []
extra_cache_paths = []   # inventory only, never age-evicted
command_timeout_seconds = 60
stale_days = 14
dep_stale_days = 30
incremental_days = 7
cache_entry_days = 30    # compatibility field; no manual age eviction
guard_fresh_minutes = 15
pressure_pct = 101      # pressure is reported; never enables destructive GC

[native_gc]
uv = true
interval_hours = 20     # daily jitter tolerance; not a file-age deletion rule

[pressure]
stale_days = 7
dep_stale_days = 21
incremental_days = 3
cache_entry_days = 14

[categories]
projects = false        # opt-in report of stale artifacts under roots
incremental = true      # report nested incremental candidates
home_caches = true      # native locked GC; inventory other caches
devin_versions = false # report only; installed versions are not cache
cargo_registry = false # report only; Cargo >=1.88 owns its cache GC
trash = false           # report only; never empty user trash
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_native_gc_only() {
        // Point at a path that cannot exist: the user's real config file must
        // not leak into tests.
        let p = load(Some(Path::new("/nonexistent-rldc-dir/config.toml"))).unwrap();
        assert_eq!(p.stale_days, 14);
        assert_eq!(p.dep_stale_days, 30);
        assert_eq!(p.incremental_days, 7);
        assert_eq!(p.cache_entry_days, 30);
        assert_eq!(p.pressure_pct, 101);
        assert!(!p.categories.projects);
        assert!(p.roots.is_empty());
        assert!(!p.categories.trash);
        assert!(!p.categories.cargo_registry);
        let routine = p.ages(false);
        let press = p.ages(true);
        assert!(press.stale < routine.stale);
        assert!(press.dep_stale < routine.dep_stale);
    }

    #[test]
    fn partial_file_keeps_defaults_elsewhere() {
        let f: FilePolicy = toml::from_str("stale_days = 3\nroots = [\"~/src\"]").unwrap();
        assert_eq!(f.stale_days, 3);
        assert_eq!(f.dep_stale_days, 30);
        assert_eq!(f.roots, vec!["~/src".to_string()]);
    }

    #[test]
    fn protect_patterns_expand_tilde() {
        let f: FilePolicy = toml::from_str("protect = [\"~/keep-this\", \"literal-sub\"]").unwrap();
        let p = policy_from(f);
        assert_eq!(
            p.protect[0],
            crate::os::home_dir().join("keep-this").to_string_lossy()
        );
        assert_eq!(p.protect[1], "literal-sub");
    }

    #[test]
    fn generated_policy_roundtrips_and_legacy_nested_paths_migrate() {
        let file: FilePolicy = toml::from_str(DEFAULT_CONFIG).unwrap();
        let policy = policy_from(file);
        let rendered = toml::to_string_pretty(&policy).unwrap();
        let decoded: FilePolicy = toml::from_str(&rendered).unwrap();
        assert!(!decoded.categories.projects);
        let legacy: FilePolicy =
            toml::from_str("[categories]\nextra_cache_paths = [\"~/known-cache\"]").unwrap();
        assert_eq!(
            policy_from(legacy).extra_cache_paths,
            vec![expand_home("~/known-cache")]
        );
    }

    #[test]
    fn typos_extreme_ages_and_root_traversal_fail_closed() {
        assert!(toml::from_str::<FilePolicy>("[categories]\nhome_cache = true").is_err());
        for text in [
            "stale_days = 999999999",
            "command_timeout_seconds = 0",
            "[native_gc]\ninterval_hours = 0",
            "roots = [\"/\"]",
            "roots = [\"/tmp/../\"]",
        ] {
            let file: FilePolicy = toml::from_str(text).unwrap();
            assert!(validate(&policy_from(file)).is_err(), "{text}");
        }
    }

    #[test]
    fn broken_file_is_an_error_not_defaults() {
        let dir = std::env::temp_dir().join(format!("rldc-badconf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.toml");
        std::fs::write(&p, "stale_days = \"many\"").unwrap();
        assert!(load(Some(&p)).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
