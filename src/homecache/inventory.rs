//! Shallow existence inventory; it never walks a cache, follows redirects or
//! claims that age/size proves non-use. Each owner has its own retention rules.
use super::{Action, CacheResult, result};
use crate::{config::Policy, os};
use std::path::PathBuf;

pub(super) fn observe(
    id: &str,
    paths: Vec<PathBuf>,
    action: Action,
    detail: &str,
) -> Option<CacheResult> {
    let mut found = Vec::new();
    for path in paths {
        if crate::safety::plain_path(&path).is_err() {
            return Some(result(
                id,
                vec![path],
                Action::Protected,
                "cache path is redirected or cannot be verified; no traversal or mutation",
            ));
        }
        match path.symlink_metadata() {
            Ok(md) if os::is_redirect(&md) => {
                return Some(result(
                    id,
                    vec![path],
                    Action::Protected,
                    "redirected cache is inventory-only; native destination was not verified",
                ));
            }
            Ok(_) => found.push(path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Some(result(
                    id,
                    vec![path],
                    Action::Failed,
                    format!("cannot inspect cache metadata: {e}"),
                ));
            }
        }
    }
    (!found.is_empty()).then(|| result(id, found, action, detail))
}
pub(super) fn evaluate(policy: &Policy) -> Vec<CacheResult> {
    let home = os::home_dir();
    let cache = os::cache_dir();
    let cargo = os::absolute_env("CARGO_HOME").unwrap_or_else(|| home.join(".cargo"));
    let gradle = os::absolute_env("GRADLE_USER_HOME").unwrap_or_else(|| home.join(".gradle"));
    let go_build = os::absolute_env("GOCACHE").unwrap_or_else(|| cache.join("go-build"));
    let modules = os::absolute_env("GOMODCACHE").unwrap_or_else(|| {
        os::absolute_env("GOPATH")
            .unwrap_or_else(|| home.join("go"))
            .join("pkg/mod")
    });
    let specs = [
        (
            "cargo",
            vec![cargo.join("registry"), cargo.join("git")],
            Action::Managed,
            "Cargo >=1.88 owns global-cache GC during builds/fetch; offline/frozen suppresses it; project target outputs are excluded",
        ),
        (
            "gradle",
            vec![gradle.join("caches")],
            Action::Managed,
            "Gradle owns use-tracked retention during its lifecycle; custom owner policies may disable it",
        ),
        (
            "go-build",
            vec![go_build],
            Action::Managed,
            "Go periodically trims its build cache when used; module downloads are separate",
        ),
        (
            "go-modcache",
            vec![modules],
            Action::Kept,
            "no supported selective unused-module GC; whole-cache clean would remove reusable downloads",
        ),
        (
            "pub",
            vec![
                os::absolute_env("PUB_CACHE").unwrap_or_else(|| home.join(".pub-cache")),
                cache.join("Pub/Cache"),
            ],
            Action::Kept,
            "installed packages can still be needed; no coordinated selective GC adapter",
        ),
        (
            "pip",
            vec![cache.join("pip"), cache.join("pip/cache")],
            Action::Kept,
            "pip purge is a whole-cache reset, not proof of unused packages",
        ),
        (
            "pre-commit",
            vec![os::absolute_env("PRE_COMMIT_HOME").unwrap_or_else(|| cache.join("pre-commit"))],
            Action::Kept,
            "hook environments may still be used by repositories; no verified coordinated adapter",
        ),
        (
            "playwright",
            vec![
                os::absolute_env("PLAYWRIGHT_BROWSERS_PATH")
                    .unwrap_or_else(|| cache.join("ms-playwright")),
            ],
            Action::Managed,
            "Playwright tracks browser clients and performs its own unused-version GC on installation unless disabled; runtimes are not manually removed",
        ),
        (
            "puppeteer",
            vec![
                os::absolute_env("PUPPETEER_CACHE_DIR")
                    .unwrap_or_else(|| home.join(".cache/puppeteer")),
                cache.join("puppeteer"),
            ],
            Action::Kept,
            "browser installations may be active; owner configuration decides their lifecycle",
        ),
        (
            "codex-runtimes",
            vec![cache.join("codex-runtimes")],
            Action::Kept,
            "installed browser runtimes are not disposable age-based cache",
        ),
        (
            "chrome",
            os::browser_cache_paths(),
            Action::Managed,
            "browser owns HTTP/shader cache eviction; profiles, cookies, history and sessions are never cleaned",
        ),
        (
            "homebrew",
            vec![cache.join("Homebrew")],
            Action::Managed,
            "Homebrew owns install/upgrade cleanup; older installed kegs can still support running programs",
        ),
        (
            "ccache",
            vec![os::absolute_env("CCACHE_DIR").unwrap_or_else(|| cache.join("ccache"))],
            Action::Managed,
            "ccache owns its size/use-aware automatic cleanup when invoked",
        ),
        (
            "sccache",
            vec![os::absolute_env("SCCACHE_DIR").unwrap_or_else(|| cache.join("sccache"))],
            Action::Managed,
            "sccache owns size-bounded cache eviction when invoked",
        ),
        (
            "thumbnails",
            vec![cache.join("thumbnails")],
            Action::Managed,
            "desktop owns thumbnail age/size limits; cleaner never deletes desktop state",
        ),
        (
            "search-index",
            vec![cache.join("tracker3"), cache.join("localsearch")],
            Action::Kept,
            "live search indexes are application state, not disposable cache",
        ),
        (
            "gpu-shaders",
            vec![
                cache.join("nvidia"),
                cache.join("mesa_shader_cache"),
                cache.join("mesa_shader_cache_db"),
            ],
            Action::Managed,
            "graphics driver owns shader-cache limits; no reset during running applications",
        ),
    ];
    let mut entries: Vec<_> = specs
        .into_iter()
        .filter_map(|(id, paths, action, reason)| observe(id, paths, action, reason))
        .collect();
    entries.extend(policy.extra_cache_paths.iter().map(|path| {
        result(
            "extra-cache",
            vec![path.clone()],
            Action::Kept,
            "custom paths are inventory-only",
        )
    }));
    if policy.categories.devin_versions {
        entries.push(result(
            "devin-versions",
            vec![os::data_local_dir().join("devin/cli/_versions")],
            Action::Kept,
            "installed versions are not disposable cache",
        ));
    }
    if policy.categories.trash {
        entries.push(result(
            "trash",
            vec![],
            Action::Kept,
            "trash contains user data and is never emptied",
        ));
    }
    entries
}
