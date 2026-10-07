//! A bounded private completion ledger throttles successful native GC, not
//! files by age. Failures never advance a deadline. Preview never writes.
use crate::{report, safety};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    completions: BTreeMap<String, Completion>,
    #[serde(skip)]
    changed: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Completion {
    cache: PathBuf,
    completed_unix: u64,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
impl Ledger {
    pub fn load(state: &Path) -> Result<Self, String> {
        let path = state.join("maintenance.json");
        safety::plain_path(&path).map_err(|e| e.to_string())?;
        const LIMIT: u64 = 64 * 1024;
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
            Ok(md) if !md.is_file() || md.len() > LIMIT => {
                return Err("maintenance ledger must be a regular file <=64 KiB".into());
            }
            _ => {}
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .and_then(|f| f.take(LIMIT + 1).read_to_end(&mut bytes))
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > LIMIT {
            return Err("maintenance ledger exceeds 64 KiB".into());
        }
        let ledger: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("invalid maintenance ledger: {e}"))?;
        if ledger.completions.len() > 64 {
            return Err("maintenance ledger has too many entries".into());
        }
        Ok(ledger)
    }
    pub fn due(&self, id: &str, cache: &Path, interval_hours: u64) -> bool {
        self.due_at(id, cache, interval_hours, now())
    }
    fn due_at(&self, id: &str, cache: &Path, interval_hours: u64, now: u64) -> bool {
        match self.completions.get(id) {
            Some(last) if last.cache == cache => now
                .checked_sub(last.completed_unix)
                .is_some_and(|elapsed| elapsed >= interval_hours.saturating_mul(3600)),
            _ => true,
        }
    }
    pub fn record(&mut self, id: &str, cache: &Path) {
        self.completions.insert(
            id.into(),
            Completion {
                cache: cache.into(),
                completed_unix: now(),
            },
        );
        self.changed = true;
    }
    pub fn save(&self, state: &Path) -> std::io::Result<()> {
        if !self.changed {
            return Ok(());
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        report::replace_private(state, "maintenance.json", &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadlines_follow_success_cache_identity_and_clock_rollback() {
        let cache = crate::os::home_dir().join("synthetic-cache");
        let other = crate::os::home_dir().join("other-cache");
        let mut ledger = Ledger::default();
        assert!(ledger.due_at("uv", &cache, 24, 100));
        ledger.completions.insert(
            "uv".into(),
            Completion {
                cache: cache.clone(),
                completed_unix: 100,
            },
        );
        assert!(!ledger.due_at("uv", &cache, 24, 99));
        assert!(!ledger.due_at("uv", &cache, 24, 86_499));
        assert!(ledger.due_at("uv", &cache, 24, 86_500));
        assert!(ledger.due_at("uv", &other, 24, 101));
    }
}
