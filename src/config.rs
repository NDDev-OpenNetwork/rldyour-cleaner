//! User-editable policy. Parsed from `config.toml` under the platform config
//! dir (`os::config_dir`); every field has a default so a missing file means
//! "balanced defaults".

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// How a candidate earns (or loses) the right to be deleted.
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

#[derive(Debug, Clone)]
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
}

#[derive(Debug, Clone)]
pub struct Pressure {
    pub stale_days: u64,
    pub dep_stale_days: u64,
    pub incremental_days: u64,
    pub cache_entry_days: u64,
}

#[derive(Debug, Clone)]
pub struct Categories {
    /// Build/artifact directories found under `roots`.
    pub projects: bool,
    /// `target/*/incremental` gets its own (shorter) age gate.
    pub incremental: bool,
    /// Tool caches under the user's home (`~/.cache/*`, `~/.bun`, ...).
    pub home_caches: bool,
    /// Old `devin/cli/_versions/*` under the platform devin data dir,
    /// except `current`.
    pub devin_versions: bool,
    /// `~/.cargo/registry` — off: cargo's built-in gc owns it (stable since
    /// Rust 1.88). Enable only on toolchains older than that.
    pub cargo_registry: bool,
    /// The platform trash dir (`~/.local/share/Trash`, `~/.Trash`) — off:
    /// deleted files are user data, not cache.
    pub trash: bool,
}

#[derive(Deserialize)]
#[serde(default)]
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
}

#[derive(Deserialize)]
#[serde(default)]
struct FilePressure {
    stale_days: u64,
    dep_stale_days: u64,
    incremental_days: u64,
    cache_entry_days: u64,
}

#[derive(Deserialize)]
#[serde(default)]
struct FileCategories {
    projects: bool,
    incremental: bool,
    home_caches: bool,
    devin_versions: bool,
    cargo_registry: bool,
    trash: bool,
}

impl Default for FilePolicy {
    fn default() -> Self {
        Self {
            roots: vec!["~/Developer".into()],
            protect: Vec::new(),
            stale_days: 14,
            dep_stale_days: 30,
            incremental_days: 7,
            cache_entry_days: 30,
            guard_fresh_minutes: 15,
            pressure_pct: 75,
            pressure: FilePressure::default(),
            categories: FileCategories::default(),
            min_size_bytes: 0,
            extra_cache_paths: Vec::new(),
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
            projects: true,
            incremental: true,
            home_caches: true,
            devin_versions: true,
            cargo_registry: false,
            trash: false,
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
    let file = match std::fs::read_to_string(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Policy::default());
        }
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let parsed: FilePolicy =
        toml::from_str(&file).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
    let policy = policy_from(parsed);
    // The filesystem root is never a legitimate scan root; refuse it outright
    // rather than trust marker files to keep a whole-disk walk harmless.
    if policy
        .roots
        .iter()
        .any(|r| r.canonicalize().is_ok_and(|c| c == Path::new("/")))
    {
        return Err(format!(
            "{}: '/' is not an allowed scan root",
            path.display()
        ));
    }
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
        extra_cache_paths: f.extra_cache_paths.iter().map(|p| expand_home(p)).collect(),
    }
}

/// The default file, written by `install.sh` (and `config --init`) so users
/// see every knob with its meaning next to it.
pub const DEFAULT_CONFIG: &str = r#"# rldyour-cleaner policy — every field is optional and defaults to the
# "balanced" profile below. Days are since last write/use; nothing is ever
# deleted while a process holds it open or wrote it recently.

# Directories scanned for project build artifacts (target/, node_modules/, ...).
roots = ["~/Developer"]

# Extra path substrings that must never be deleted (matched against the
# canonical candidate path; a leading ~ expands like in `roots`).
# Example: protect = ["~/Developer/client-abonmarket-frontend"]
protect = []

# --- routine thresholds (days) ---
stale_days = 14          # derived artifacts: target, build/, .next, __pycache__, ...
dep_stale_days = 30      # dependency dirs: node_modules, .venv, .dart_tool, ...
incremental_days = 7     # target/*/incremental inside active projects
cache_entry_days = 30    # entries inside ~/.cache/* tool caches
guard_fresh_minutes = 15 # never touch anything written this recently

# --- disk pressure ---
# When the filesystem holding any root is this full, the [pressure] ages apply
# instead. Set to 101 to disable pressure mode entirely.
pressure_pct = 75

[pressure]
stale_days = 7
dep_stale_days = 21
incremental_days = 3
cache_entry_days = 14

[categories]
projects = true        # artifact dirs under `roots`
incremental = true     # prune stale incremental caches inside live target dirs
home_caches = true     # uv, go, bun, npm, pub, gradle, playwright, ...
devin_versions = true  # old devin CLI versions under ~/.local/share/devin
cargo_registry = false # cargo >=1.88 garbage-collects ~/.cargo itself
trash = false          # ~/.local/share/Trash is user data, opt in deliberately

# Extra directories treated like `home_caches` entries (age-evicted by files).
extra_cache_paths = []
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_balanced_profile() {
        // Point at a path that cannot exist: the user's real config file must
        // not leak into tests.
        let p = load(Some(Path::new("/nonexistent-rldc-dir/config.toml"))).unwrap();
        assert_eq!(p.stale_days, 14);
        assert_eq!(p.dep_stale_days, 30);
        assert_eq!(p.incremental_days, 7);
        assert_eq!(p.cache_entry_days, 30);
        assert_eq!(p.pressure_pct, 75);
        assert!(p.categories.projects);
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
    fn broken_file_is_an_error_not_defaults() {
        let dir = std::env::temp_dir().join(format!("rldc-badconf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.toml");
        std::fs::write(&p, "stale_days = \"many\"").unwrap();
        assert!(load(Some(&p)).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
