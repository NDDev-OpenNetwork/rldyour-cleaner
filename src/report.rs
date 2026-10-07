//! Private atomic run reports. Native reclaim estimates remain text: a cache
//! tree's size does not equal free space, especially with hardlinks or clones.
use crate::homecache::CacheResult;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub const MAX_REPORT_BYTES: u64 = 4 * 1024 * 1024;

pub fn read_status(state_dir: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let path = state_dir.join("last-run.json");
    crate::safety::state_file(&path, MAX_REPORT_BYTES)?;
    let mut options = OpenOptions::new();
    options.read(true);
    crate::os::private_open_options(&mut options);
    let mut content = String::new();
    options
        .open(path)?
        .take(MAX_REPORT_BYTES + 1)
        .read_to_string(&mut content)?;
    if content.len() as u64 > MAX_REPORT_BYTES {
        return Err(std::io::Error::other("report exceeds 4 MiB"));
    }
    let value: serde_json::Value = serde_json::from_str(&content).map_err(std::io::Error::other)?;
    if value.get("tool").and_then(serde_json::Value::as_str) != Some("rldyour-cleaner")
        || value
            .get("version")
            .and_then(serde_json::Value::as_str)
            .is_none()
        || ["started_unix", "duration_ms", "failed"]
            .iter()
            .any(|key| value.get(key).and_then(serde_json::Value::as_u64).is_none())
        || value
            .get("dry_run")
            .and_then(serde_json::Value::as_bool)
            .is_none()
        || ["entries", "caches"].iter().any(|key| {
            value
                .get(key)
                .and_then(serde_json::Value::as_array)
                .is_none()
        })
    {
        return Err(std::io::Error::other("state is not a cleaner report"));
    }
    Ok(content)
}

#[derive(Serialize)]
pub struct Entry {
    pub path: String,
    pub kind: String,
    pub bytes: u64,
    pub action: &'static str,
    pub reason: String,
    pub age_days: u64,
}
#[derive(Serialize)]
pub struct Report {
    pub tool: &'static str,
    pub version: &'static str,
    pub started_unix: u64,
    pub duration_ms: u64,
    pub pressure: bool,
    pub dry_run: bool,
    pub fs_use_pct: Option<u64>,
    pub freed_bytes: Option<u64>,
    pub deleted: usize,
    pub skipped: usize,
    pub failed: usize,
    pub fresh_matched: usize,
    pub stale_bytes: u64,
    pub entries: Vec<Entry>,
    pub caches: Vec<CacheResult>,
    pub system_policies: Vec<crate::system_policy::SystemPolicy>,
}
impl Report {
    pub fn new(pressure: bool, dry_run: bool, fs_use_pct: Option<u64>) -> Self {
        Self {
            tool: "rldyour-cleaner",
            version: env!("CARGO_PKG_VERSION"),
            started_unix: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            duration_ms: 0,
            pressure,
            dry_run,
            fs_use_pct,
            freed_bytes: None,
            deleted: 0,
            skipped: 0,
            failed: 0,
            fresh_matched: 0,
            stale_bytes: 0,
            entries: Vec::new(),
            caches: Vec::new(),
            system_policies: Vec::new(),
        }
    }
    pub fn push(&mut self, entry: Entry) {
        self.skipped += 1;
        self.entries.push(entry);
    }
    pub fn save(&self, state_dir: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        if bytes.len() as u64 > MAX_REPORT_BYTES {
            return Err(std::io::Error::other("report exceeds 4 MiB"));
        }
        replace_private(state_dir, "last-run.json", &bytes)
    }
    pub fn print_summary(&self) {
        println!(
            "rldyour-cleaner {}{}: {} native GC pass(es), {} kept/review-only, {} failed ({}ms)",
            self.version,
            if self.dry_run { " (preview)" } else { "" },
            self.caches.iter().filter(|c| c.action.is_gc()).count(),
            self.skipped,
            self.failed,
            self.duration_ms
        );
        for entry in &self.entries {
            println!(
                "  {} {} {} — {}",
                entry.action,
                entry.kind,
                fmt_bytes(entry.bytes),
                entry.path
            );
        }
        for cache in &self.caches {
            println!(
                "  {}: {} — {}",
                cache.id,
                cache.action.label(),
                cache.detail
            );
        }
        for policy in &self.system_policies {
            println!("  {}: {} — {}", policy.id, policy.status, policy.detail);
        }
        println!("Reclaimed bytes are reported by the owning tool, not inferred from tree size.");
    }
}

pub(crate) fn replace_private(state_dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    crate::safety::private_dir(state_dir)?;
    let destination = state_dir.join(name);
    crate::safety::state_file(&destination, MAX_REPORT_BYTES)?;
    let temporary = state_dir.join(format!(
        ".report-{}-{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    write_new_private(&temporary, bytes)?;
    let result = fs::rename(&temporary, &destination);
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

pub fn write_new_private(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent"))?;
    crate::safety::plain_path(parent)?;
    std::fs::create_dir_all(parent)?;
    crate::safety::plain_path(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    crate::os::private_open_options(&mut options);
    let mut file = options.open(path)?;
    if let Err(e) = file.write_all(content).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(e);
    }
    Ok(())
}

pub fn fmt_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < 4 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}
