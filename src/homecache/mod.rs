//! Compile-time provider registry. Only adapters with a coordinated native GC
//! contract may mutate; observations never become deletion candidates.
mod inventory;
mod node;
mod uv;

use crate::{config::Policy, maintenance::Ledger};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Pruned,
    WouldPrune,
    Managed,
    Kept,
    Protected,
    Failed,
    Absent,
    MissingTool,
    Disabled,
    NotDue,
}
impl Action {
    pub fn is_gc(self) -> bool {
        matches!(self, Self::Pruned | Self::WouldPrune)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Pruned => "pruned",
            Self::WouldPrune => "would-prune",
            Self::Managed => "managed",
            Self::Kept => "kept",
            Self::Protected => "protected",
            Self::Failed => "failed",
            Self::Absent => "absent",
            Self::MissingTool => "missing-tool",
            Self::Disabled => "disabled",
            Self::NotDue => "not-due",
        }
    }
}

#[derive(Clone, Serialize)]
pub struct CacheResult {
    pub id: String,
    pub paths: Vec<PathBuf>,
    pub action: Action,
    // Tree sizes/hardlinks/free-space deltas are not reclaimed-byte evidence.
    pub freed_bytes: Option<u64>,
    pub detail: String,
}
fn result(id: &str, paths: Vec<PathBuf>, action: Action, detail: impl Into<String>) -> CacheResult {
    CacheResult {
        id: id.into(),
        paths,
        action,
        freed_bytes: None,
        detail: detail.into(),
    }
}

pub fn evaluate(policy: &Policy, dry_run: bool, cadence: &mut Ledger) -> Vec<CacheResult> {
    if !policy.categories.home_caches {
        return Vec::new();
    }
    let mut results = vec![uv::evaluate(policy, dry_run, cadence)];
    for entry in &results {
        if entry.action == Action::Pruned {
            cadence.record(&entry.id, &entry.paths[0]);
        }
    }
    results.extend(node::evaluate(policy));
    results.extend(inventory::evaluate(policy));
    results
}
