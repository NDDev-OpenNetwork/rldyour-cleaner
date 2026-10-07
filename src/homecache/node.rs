//! Node package stores lack a verified shared install/GC lease. Do not launch
//! Corepack shims (even version/path queries can download software), inspect
//! credentials, or turn a process snapshot into a deletion authorization.
use super::{Action, CacheResult, inventory::observe};
use crate::{config::Policy, os};

pub(super) fn evaluate(_policy: &Policy) -> Vec<CacheResult> {
    let home = os::home_dir();
    let npm = os::absolute_env("npm_config_cache")
        .or_else(|| os::absolute_env("NPM_CONFIG_CACHE"))
        .unwrap_or_else(os::npm_cache_dir);
    let bun = os::absolute_env("BUN_INSTALL_CACHE_DIR")
        .unwrap_or_else(|| home.join(".bun/install/cache"));
    let mut entries = Vec::new();
    for (id, paths, reason) in [
        (
            "npm",
            vec![npm.join("_cacache")],
            "npm verify is manual-maintenance only: current cacache sweeps tmp and rebuilds indexes without a shared installer lease; no scheduled verify or force clean",
        ),
        (
            "npx",
            vec![npm.join("_npx")],
            "npx entries contain runnable installations; no safe automatic unused-entry lease; kept",
        ),
        (
            "npm-logs",
            vec![npm.join("_logs")],
            "npm owns its bounded diagnostic log count; cleaner does not read log contents",
        ),
        (
            "bun",
            vec![bun],
            "Bun provides whole-cache rm or project node_modules prune, not coordinated selective global GC; live shared store/hardlinks may be needed",
        ),
        (
            "pnpm-store",
            os::pnpm_cache_paths(),
            "pnpm prune also removes tmp, metadata and expired dlx installations; no verified shared installer lease; default/env paths are inventory-only, configured paths may differ",
        ),
        (
            "corepack",
            vec![
                os::cache_dir().join("node/corepack"),
                home.join(".cache/node/corepack"),
            ],
            "package-manager distributions can be pinned by projects; no shim invocation or installed-version deletion",
        ),
    ] {
        if let Some(entry) = observe(id, paths, Action::Kept, reason) {
            entries.push(entry);
        }
    }
    entries
}
