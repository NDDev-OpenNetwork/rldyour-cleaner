//! What counts as a cleanable artifact, and which marker must sit next to it.
//!
//! A directory name alone is never enough — `build/` under a generic source
//! tree may be checked-in content. Every kind either requires a sibling
//! project marker file or a marker *inside* the directory itself
//! (`CACHEDIR.TAG` is cargo's own tag for a target dir).

use crate::config::Gate;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    RustTarget,
    RustIncremental,
    NodeModules,
    JsFrameworkCache,
    JsOutputDir,
    GenericBuild,
    DartToolDeps,
    DartToolBuild,
    GradleLocal,
    PyVenv,
    PyCache,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::RustTarget => "rust-target",
            Kind::RustIncremental => "rust-incremental",
            Kind::NodeModules => "node-modules",
            Kind::JsFrameworkCache => "js-framework-cache",
            Kind::JsOutputDir => "js-output",
            Kind::GenericBuild => "build-dir",
            Kind::DartToolDeps => "dart-tool-deps",
            Kind::DartToolBuild => "dart-tool-build",
            Kind::GradleLocal => "gradle-local",
            Kind::PyVenv => "python-venv",
            Kind::PyCache => "python-cache",
        }
    }

    /// Deleting a dependency tree under a running app survives the process
    /// (already-loaded code stays mapped) but breaks its next start — so for
    /// these kinds the proc guard checks the whole project root, not just the
    /// candidate dir.
    pub fn guards_project_root(self) -> bool {
        matches!(self, Kind::NodeModules | Kind::PyVenv | Kind::DartToolDeps)
    }

    /// Names so generic they may be checked-in content (`build/` holding pin
    /// files, `dist/` committed bundles). These are only cleanable when git
    /// proves them ignored — the documented sign of reproducible output.
    pub fn needs_gitignore(self) -> bool {
        matches!(self, Kind::GenericBuild | Kind::JsOutputDir)
    }

    pub fn gate(self) -> Gate {
        match self {
            Kind::NodeModules | Kind::PyVenv | Kind::DartToolDeps => Gate::ProjectStale,
            _ => Gate::ArtifactStale,
        }
    }
}

pub struct Rule {
    pub kind: Kind,
    /// Exact directory basenames that can match.
    pub names: &'static [&'static str],
    /// Sibling marker files (in the candidate's parent); empty = any location.
    pub markers: &'static [&'static str],
}

/// Order matters: the first rule whose name *and* marker conditions hold wins.
pub const RULES: &[Rule] = &[
    Rule {
        kind: Kind::RustTarget,
        names: &["target"],
        markers: &["Cargo.toml"],
    },
    Rule {
        kind: Kind::NodeModules,
        names: &["node_modules"],
        markers: &["package.json"],
    },
    Rule {
        kind: Kind::DartToolDeps,
        names: &[".dart_tool"],
        markers: &["pubspec.yaml"],
    },
    Rule {
        kind: Kind::PyVenv,
        names: &[".venv", "venv", ".tox", ".nox"],
        markers: &[
            "pyproject.toml",
            "requirements.txt",
            "setup.py",
            "setup.cfg",
            "Pipfile",
            "uv.lock",
        ],
    },
    Rule {
        kind: Kind::GradleLocal,
        names: &[".gradle"],
        markers: &[
            "settings.gradle",
            "settings.gradle.kts",
            "build.gradle",
            "build.gradle.kts",
            "gradlew",
        ],
    },
    Rule {
        kind: Kind::JsFrameworkCache,
        names: &[
            ".next",
            ".nuxt",
            ".output",
            ".svelte-kit",
            ".turbo",
            ".parcel-cache",
            ".vite",
            ".astro",
            "coverage",
        ],
        markers: &["package.json"],
    },
    Rule {
        kind: Kind::JsOutputDir,
        names: &["dist", "out"],
        markers: &["package.json"],
    },
    Rule {
        kind: Kind::GenericBuild,
        names: &["build"],
        markers: &[
            "pubspec.yaml",
            "settings.gradle",
            "settings.gradle.kts",
            "build.gradle",
            "build.gradle.kts",
            "package.json",
        ],
    },
    Rule {
        kind: Kind::PyCache,
        names: &[
            "__pycache__",
            ".pytest_cache",
            ".mypy_cache",
            ".ruff_cache",
            ".hypothesis",
            ".ipynb_checkpoints",
        ],
        markers: &[],
    },
];

/// Directory names the walker never descends into: matched artifacts are
/// handled whole, dependency trees are opaque, `.git` is never an artifact
/// (its activity is sampled separately in `project_activity`).
pub const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".venv",
    "venv",
    ".tox",
    ".nox",
    ".dart_tool",
    ".gradle",
    "__pycache__",
    ".svn",
    ".hg",
    "Pods",
    ".rldyour-cleaner-pending",
];

/// Does `dir` look like a cargo target directory? `Cargo.toml` as a sibling is
/// the primary signal; `CACHEDIR.TAG` inside is cargo's own stamp and also
/// covers target dirs redirected via `CARGO_TARGET_DIR`/`.cargo/config.toml`.
pub fn is_cargo_target(dir: &Path) -> bool {
    dir.parent()
        .map(|p| p.join("Cargo.toml").is_file())
        .unwrap_or(false)
        || dir.join("CACHEDIR.TAG").is_file()
        || dir.join(".rustc_info.json").is_file()
}

/// Any rule matching `name` whose markers exist beside `dir`.
pub fn match_dir(dir: &Path) -> Option<Kind> {
    let name = dir.file_name()?.to_str()?;
    let parent = dir.parent()?;
    for rule in RULES {
        if !rule.names.contains(&name) {
            continue;
        }
        // For `target`, cargo's own stamps (CACHEDIR.TAG / .rustc_info.json
        // inside) count alongside the Cargo.toml sibling — target dirs are
        // relocatable via CARGO_TARGET_DIR and may sit away from sources.
        let marker_ok = if rule.kind == Kind::RustTarget {
            is_cargo_target(dir)
        } else {
            rule.markers.is_empty() || rule.markers.iter().any(|m| parent.join(m).is_file())
        };
        if marker_ok {
            return Some(rule.kind);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// Fixture dir that removes itself on drop — tests must not leak /tmp.
    struct TempDir(std::path::PathBuf);
    impl std::ops::Deref for TempDir {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn proj(files: &[&str], dirs: &[&str]) -> TempDir {
        // Unique per call — tests run in parallel threads and fixture paths
        // derived from contents would collide (remove_dir_all racing builds).
        let root = std::env::temp_dir().join(format!(
            "rldc-kinds-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        for d in dirs {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        for f in files {
            fs::write(root.join(f), b"x").unwrap();
        }
        TempDir(root)
    }

    #[test]
    fn target_needs_a_cargo_marker() {
        let p = proj(&["Cargo.toml"], &["target"]);
        assert_eq!(match_dir(&p.join("target")), Some(Kind::RustTarget));

        // A bare `target` with no marker is not cargo's — leave it alone.
        let q = proj(&[], &["target"]);
        assert_eq!(match_dir(&q.join("target")), None);

        // ... unless cargo itself tagged it.
        let r = proj(&[], &["target"]);
        fs::write(r.join("target/CACHEDIR.TAG"), b"sig").unwrap();
        assert_eq!(match_dir(&r.join("target")), Some(Kind::RustTarget));
    }

    #[test]
    fn node_modules_needs_package_json() {
        let p = proj(&["package.json"], &["node_modules"]);
        assert_eq!(match_dir(&p.join("node_modules")), Some(Kind::NodeModules));
        let q = proj(&[], &["node_modules"]);
        assert_eq!(match_dir(&q.join("node_modules")), None);
    }

    #[test]
    fn build_only_under_known_markers() {
        let f = proj(&["pubspec.yaml"], &["build"]);
        assert_eq!(match_dir(&f.join("build")), Some(Kind::GenericBuild));
        let g = proj(&["settings.gradle.kts"], &["build"]);
        assert_eq!(match_dir(&g.join("build")), Some(Kind::GenericBuild));
        let bare = proj(&["README.md"], &["build"]);
        assert_eq!(match_dir(&bare.join("build")), None);
    }

    #[test]
    fn pycache_matches_anywhere() {
        let p = proj(&[], &["pkg/__pycache__"]);
        assert_eq!(match_dir(&p.join("pkg/__pycache__")), Some(Kind::PyCache));
    }
}
