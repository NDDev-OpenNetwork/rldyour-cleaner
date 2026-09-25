//! Tool caches under `$HOME`: content-addressed stores and download caches
//! that regrow on demand. Two strategies:
//!
//! * `AgeEvict` — delete *files* older than `cache_entry_days` inside the
//!   cache, then drop emptied dirs. The cache root stays. This is what
//!   `systemd-tmpfiles` aging does, applied to caches tmpfiles cannot see.
//! * `Command` — delegate to the tool's own GC when the binary is on PATH
//!   (`uv cache prune`, `go clean -modcache`); it knows its own invariants.
//!
//! `~/.cargo/registry` is deliberately absent: cargo ≥1.88 garbage-collects
//! its home itself (see the `cargo_registry` category flag if you're pinned
//! older). Browsers' *live* profiles are never in scope — only the standalone
//! artifact caches listed below.

use crate::config::expand_home;
use crate::sysinfo::on_path;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

pub struct HomeCandidate {
    pub id: &'static str,
    pub path: PathBuf,
    pub strategy: Strategy,
    /// Only process under disk pressure — for caches whose own GC is
    /// scorched-earth (`go clean -modcache` wipes everything, forcing a full
    /// re-download on the next build). Too costly to run on a routine day.
    pub pressure_only: bool,
}

pub enum Strategy {
    /// Evict files older than `days` inside `path`.
    AgeEvict { days: u64 },
    /// Run `argv[0] argv[1..]` when `argv[0]` resolves on PATH; else evict.
    Command {
        argv: &'static [&'static str],
        fallback_days: u64,
    },
    /// Delete sibling version dirs except the `current` symlink's target.
    KeepCurrent { keep_days: u64 },
}

fn hc(id: &'static str, path: PathBuf, strategy: Strategy) -> HomeCandidate {
    HomeCandidate {
        id,
        path,
        strategy,
        pressure_only: false,
    }
}

/// The fixed cache set. Every path exists or is skipped silently — machines
/// only ever carry a subset of these tools.
pub fn specs(cache_entry_days: u64, dep_stale_days: u64, extra: &[PathBuf]) -> Vec<HomeCandidate> {
    let d = cache_entry_days;
    let mut v = vec![
        hc(
            "uv",
            expand_home("~/.cache/uv"),
            Strategy::Command {
                argv: &["uv", "cache", "prune"],
                fallback_days: d,
            },
        ),
        HomeCandidate {
            id: "go-modcache",
            // `go clean -modcache` knows how to remove go's read-only module
            // dirs, but it wipes *everything* — the next build re-downloads
            // all modules. Pressure-only, and no age-evict fallback: partial
            // rm on read-only trees fails halfway and leaves a mess.
            path: expand_home("~/go/pkg/mod"),
            strategy: Strategy::Command {
                argv: &["go", "clean", "-modcache"],
                fallback_days: 0,
            },
            pressure_only: true,
        },
        hc(
            "go-build",
            expand_home("~/.cache/go-build"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "bun",
            expand_home("~/.bun/install/cache"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "npm",
            expand_home("~/.npm/_cacache"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "pub",
            expand_home("~/.pub-cache"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "gradle",
            expand_home("~/.gradle/caches"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "pip",
            expand_home("~/.cache/pip"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "pre-commit",
            expand_home("~/.cache/pre-commit"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "playwright",
            expand_home("~/.cache/ms-playwright"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "puppeteer",
            expand_home("~/.cache/puppeteer"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "codex-runtimes",
            expand_home("~/.cache/codex-runtimes"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "pnpm-store",
            expand_home("~/.local/share/pnpm/store"),
            Strategy::AgeEvict { days: d },
        ),
        hc(
            "devin-versions",
            expand_home("~/.local/share/devin/cli/_versions"),
            Strategy::KeepCurrent {
                keep_days: dep_stale_days,
            },
        ),
    ];
    for p in extra {
        v.push(hc("extra-cache", p.clone(), Strategy::AgeEvict { days: d }));
    }
    v
}

pub struct CacheResult {
    pub id: String,
    pub freed_bytes: u64,
    pub detail: String,
}

fn file_bytes(md: &fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        md.blocks() * 512
    }
    #[cfg(not(unix))]
    {
        md.len()
    }
}

/// What `age_evict` would free — same walk, no mutation (dry-run preview).
pub fn age_evict_dry(dir: &Path, days: u64) -> (u64, usize) {
    let cutoff = SystemTime::now() - Duration::from_secs(days.saturating_mul(86_400));
    let mut freed = 0u64;
    let mut count = 0usize;
    for e in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if e.depth() == 0 {
            continue;
        }
        let Ok(md) = fs::symlink_metadata(e.path()) else {
            continue;
        };
        if (md.file_type().is_symlink() || md.is_file()) && md.modified().is_ok_and(|m| m < cutoff)
        {
            freed += file_bytes(&md);
            count += 1;
        }
    }
    (freed, count)
}

/// Files older than `cutoff` are removed; emptied subdirs are pruned
/// bottom-up; the cache root itself is kept.
pub fn age_evict(dir: &Path, days: u64) -> (u64, usize) {
    let cutoff = SystemTime::now() - Duration::from_secs(days.saturating_mul(86_400));
    let mut freed = 0u64;
    let mut removed = 0usize;
    let iter = walkdir::WalkDir::new(dir)
        .follow_links(false)
        .contents_first(true)
        .into_iter();
    for entry in iter {
        let Ok(e) = entry else { continue };
        if e.depth() == 0 {
            continue;
        }
        let Ok(md) = fs::symlink_metadata(e.path()) else {
            continue;
        };
        if md.file_type().is_symlink() || md.is_file() {
            let old = md.modified().is_ok_and(|m| m < cutoff);
            if old && fs::remove_file(e.path()).is_ok() {
                freed += file_bytes(&md);
                removed += 1;
            }
        } else if md.is_dir() {
            // contents_first order: children handled before their parent, so
            // remove_dir succeeds exactly when eviction emptied it.
            let _ = fs::remove_dir(e.path());
        }
    }
    (freed, removed)
}

fn run_command(argv: &[&str]) -> Result<String, String> {
    let out = Command::new(argv[0])
        .args(&argv[1..])
        .output()
        .map_err(|e| format!("{}: {e}", argv[0]))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let first = stderr.lines().next().unwrap_or("");
        Err(format!("{} exited {}: {}", argv[0], out.status, first))
    }
}

/// Which sibling version dirs `keep_current` would remove — shared between
/// the real path and the dry-run preview so they can never drift.
fn keep_current_list(dir: &Path, keep_days: u64) -> Vec<PathBuf> {
    let cutoff = SystemTime::now() - Duration::from_secs(keep_days.saturating_mul(86_400));
    // Canonicalize both sides: the symlink target may be relative, absolute,
    // or carry cosmetic components — string equality on joined paths is not
    // a safe "is this the live version" test.
    let keep = fs::read_link(dir.join("current"))
        .ok()
        .and_then(|k| dir.join(k).canonicalize().ok());
    let mut doomed = Vec::new();
    let Ok(read) = fs::read_dir(dir) else {
        return doomed;
    };
    for e in read.flatten() {
        let p = e.path();
        let name = e.file_name();
        if name == "current" || name == "_update.lock" {
            continue;
        }
        if keep
            .as_ref()
            .is_some_and(|k| p.canonicalize().ok().as_ref() == Some(k))
        {
            continue;
        }
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let old = fs::metadata(&p)
            .and_then(|m| m.modified())
            .is_ok_and(|m| m < cutoff);
        if old {
            doomed.push(p);
        }
    }
    doomed
}

/// devin's `_versions/` keeps one `current` symlink; every other version dir
/// older than `keep_days` goes. Liveness is checked *per version dir*: a
/// devin process launched from an old version keeps that version alive while
/// the rest are still pruned.
fn keep_current(dir: &Path, keep_days: u64) -> (u64, usize, Vec<String>) {
    let mut freed = 0u64;
    let mut removed = 0usize;
    let mut notes = Vec::new();
    for p in keep_current_list(dir, keep_days) {
        if let Some(held) = in_use(&p) {
            notes.push(format!("{}: {held}", p.display()));
            continue;
        }
        let (size, _) = crate::scan::measure(&p);
        match fs::remove_dir_all(&p) {
            Ok(()) => {
                freed += size;
                removed += 1;
            }
            Err(err) => notes.push(format!("{}: {err}", p.display())),
        }
    }
    (freed, removed, notes)
}

/// Empty `_download` staging always goes — it only ever holds partial pulls.
fn clear_download(dir: &Path) -> u64 {
    let d = dir.join("_download");
    if !d.is_dir() {
        return 0;
    }
    let (size, _) = crate::scan::measure(&d);
    let _ = fs::remove_dir_all(&d);
    size
}

/// The directory is busy when any live process holds a file, map, exe or cwd
/// inside it — e.g. a gradle daemon with jars mapped, or `pnpm install`
/// mid-hardlink. Skipping beats racing.
fn in_use(path: &Path) -> Option<String> {
    let users = crate::safety::pids_using(path);
    (!users.is_empty()).then(|| {
        format!(
            "in use by pid(s) {}",
            users
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    })
}

/// What `process` would do — same decisions, no mutation (dry-run preview).
pub fn preview(c: &HomeCandidate) -> CacheResult {
    let base = CacheResult {
        id: c.id.to_string(),
        freed_bytes: 0,
        detail: String::new(),
    };
    if !c.path.is_dir() {
        return base;
    }
    // KeepCurrent checks liveness per doomed version dir, not on the root —
    // `_versions` is permanently "in use" while any devin process runs.
    if !matches!(c.strategy, Strategy::KeepCurrent { .. })
        && let Some(detail) = in_use(&c.path)
    {
        return CacheResult { detail, ..base };
    }
    match &c.strategy {
        Strategy::AgeEvict { days } => {
            let (freed, n) = age_evict_dry(&c.path, *days);
            CacheResult {
                freed_bytes: freed,
                detail: format!("would evict {n} entries"),
                ..base
            }
        }
        Strategy::Command {
            argv,
            fallback_days,
        } => {
            if on_path(argv[0]) {
                CacheResult {
                    detail: format!("would run `{}`", argv.join(" ")),
                    ..base
                }
            } else if *fallback_days > 0 {
                let (freed, n) = age_evict_dry(&c.path, *fallback_days);
                CacheResult {
                    freed_bytes: freed,
                    detail: format!(
                        "would evict {n} entries (no {tool} on PATH)",
                        tool = argv[0]
                    ),
                    ..base
                }
            } else {
                CacheResult {
                    detail: format!("{} not on PATH; skipped", argv[0]),
                    ..base
                }
            }
        }
        Strategy::KeepCurrent { keep_days } => {
            let doomed = keep_current_list(&c.path, *keep_days);
            let (mut freed, _) = crate::scan::measure(&c.path.join("_download"));
            let mut held = 0usize;
            for p in &doomed {
                if in_use(p).is_some() {
                    held += 1;
                    continue;
                }
                let (s, _) = crate::scan::measure(p);
                freed += s;
            }
            let mut detail = format!("would remove {} old versions", doomed.len() - held);
            if held > 0 {
                detail.push_str(&format!("; {held} held by running processes"));
            }
            CacheResult {
                freed_bytes: freed,
                detail,
                ..base
            }
        }
    }
}

pub fn process(c: &HomeCandidate) -> CacheResult {
    let base = CacheResult {
        id: c.id.to_string(),
        freed_bytes: 0,
        detail: String::new(),
    };
    if !c.path.is_dir() {
        return base;
    }
    // KeepCurrent checks liveness per doomed version dir, not on the root —
    // `_versions` is permanently "in use" while any devin process runs.
    if !matches!(c.strategy, Strategy::KeepCurrent { .. })
        && let Some(detail) = in_use(&c.path)
    {
        return CacheResult { detail, ..base };
    }
    match &c.strategy {
        Strategy::AgeEvict { days } => {
            let (freed, n) = age_evict(&c.path, *days);
            CacheResult {
                freed_bytes: freed,
                detail: format!("evicted {n} entries"),
                ..base
            }
        }
        Strategy::Command {
            argv,
            fallback_days,
        } => {
            if on_path(argv[0]) {
                match run_command(argv) {
                    Ok(detail) => CacheResult { detail, ..base },
                    Err(e) => CacheResult {
                        detail: format!("tool failed: {e}"),
                        ..base
                    },
                }
            } else if *fallback_days > 0 {
                let (freed, n) = age_evict(&c.path, *fallback_days);
                CacheResult {
                    freed_bytes: freed,
                    detail: format!("evicted {n} entries (no {tool} on PATH)", tool = argv[0]),
                    ..base
                }
            } else {
                CacheResult {
                    detail: format!("{} not on PATH; skipped", argv[0]),
                    ..base
                }
            }
        }
        Strategy::KeepCurrent { keep_days } => {
            let staged = clear_download(&c.path);
            let (freed, n, notes) = keep_current(&c.path, *keep_days);
            let mut detail = format!("removed {n} old versions");
            if !notes.is_empty() {
                detail.push_str(&format!("; {}", notes.join("; ")));
            }
            CacheResult {
                freed_bytes: freed + staged,
                detail,
                ..base
            }
        }
    }
}
