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
        }
    }
    pub fn push(&mut self, entry: Entry) {
        self.skipped += 1;
        self.entries.push(entry);
    }
    pub fn save(&self, state_dir: &Path) -> std::io::Result<()> {
        crate::safety::private_dir(state_dir)?;
        let destination = state_dir.join("last-run.json");
        crate::safety::plain_path(&destination)?;
        let temporary = state_dir.join(format!(
            ".report-{}-{}.tmp",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        write_new_private(&temporary, &bytes)?;
        let result = fs::rename(&temporary, &destination);
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
    pub fn print_summary(&self) {
        println!(
            "rldyour-cleaner {}{}: {} native GC pass(es), {} kept/review-only, {} failed ({}ms)",
            self.version,
            if self.dry_run { " (preview)" } else { "" },
            self.caches
                .iter()
                .filter(|c| matches!(c.action, "pruned" | "would-prune"))
                .count(),
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
            println!("  {}: {} — {}", cache.id, cache.action, cache.detail);
        }
        println!("Reclaimed bytes are reported by the owning tool, not inferred from tree size.");
    }
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
