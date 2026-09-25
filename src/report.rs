//! Run report — printed to stdout (the scheduler's log picks it up) and
//! persisted to `os::state_dir()/last-run.json` for `status`.

use serde::Serialize;
use std::path::Path;
use std::time::SystemTime;

#[derive(Serialize)]
pub struct Entry {
    pub path: String,
    pub kind: String,
    pub bytes: u64,
    /// deleted | skipped | would-delete | evicted | failed
    pub action: &'static str,
    pub reason: String,
    /// Days since the newest write inside the candidate (for the scan table).
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
    pub freed_bytes: u64,
    pub deleted: usize,
    pub skipped: usize,
    pub failed: usize,
    pub entries: Vec<Entry>,
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Report {
    pub fn new(pressure: bool, dry_run: bool, fs_use_pct: Option<u64>) -> Self {
        Self {
            tool: "rldyour-cleaner",
            version: env!("CARGO_PKG_VERSION"),
            started_unix: now_unix(),
            duration_ms: 0,
            pressure,
            dry_run,
            fs_use_pct,
            freed_bytes: 0,
            deleted: 0,
            skipped: 0,
            failed: 0,
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, e: Entry) {
        match e.action {
            "deleted" | "evicted" | "would-delete" => {
                self.deleted += 1;
                self.freed_bytes += e.bytes;
            }
            "failed" => self.failed += 1,
            _ => self.skipped += 1,
        }
        self.entries.push(e);
    }

    /// Persist for `rldyour-cleaner status`; best-effort, state dir is ours.
    pub fn save(&self, state_dir: &Path) {
        let _ = std::fs::create_dir_all(state_dir);
        let p = state_dir.join("last-run.json");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, json);
        }
    }

    pub fn print_summary(&self) {
        if self.dry_run {
            println!(
                "rldyour-cleaner {} (dry-run): would free {} across {} item(s), {} skipped",
                self.version,
                fmt_bytes(self.freed_bytes),
                self.deleted,
                self.skipped,
            );
        } else {
            println!(
                "rldyour-cleaner {}: freed {} across {} item(s), {} skipped, {} failed ({}s, pressure={})",
                self.version,
                fmt_bytes(self.freed_bytes),
                self.deleted,
                self.skipped,
                self.failed,
                self.duration_ms / 1000,
                self.pressure,
            );
        }
    }
}

pub fn fmt_bytes(b: u64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if b == 0 {
        return "0 B".into();
    }
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", U[i])
}
