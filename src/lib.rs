//! rldyour-cleaner — a policy-driven janitor for stale build artifacts and
//! tool caches. See `README.md` for the model; every module header explains
//! its piece.

pub mod clean;
pub mod cli;
pub mod config;
pub mod homecache;
pub mod kinds;
mod os;
pub mod report;
pub mod safety;
pub mod scan;

use clap::Parser;
use cli::{Cli, Cmd};
use report::{Entry, Report, fmt_bytes};

use std::path::Path;
use std::time::{Instant, SystemTime};

pub fn cli_entry() -> i32 {
    let cli = Cli::parse();
    let policy = match config::load(cli.config.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("rldyour-cleaner: {e}");
            return 2;
        }
    };

    match cli.cmd {
        Cmd::Config { init } => cmd_config(init),
        Cmd::Status => cmd_status(),
        Cmd::Scan { json, verbose } => cmd_run(&policy, Mode::Scan { json, verbose }),
        Cmd::Run { dry_run } => cmd_run(&policy, Mode::Run { dry_run }),
    }
}

enum Mode {
    Scan { json: bool, verbose: bool },
    Run { dry_run: bool },
}

fn cmd_config(init: bool) -> i32 {
    if init {
        let path = config::config_path();
        if path.exists() {
            eprintln!("rldyour-cleaner: {} already exists", path.display());
            return 1;
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::write(&path, config::DEFAULT_CONFIG) {
            Ok(()) => println!("wrote {}", path.display()),
            Err(e) => {
                eprintln!("cannot write {}: {e}", path.display());
                return 1;
            }
        }
    } else {
        print!("{}", config::DEFAULT_CONFIG);
        println!("# active file: {}", config::config_path().display());
    }
    0
}

fn cmd_status() -> i32 {
    let p = os::state_dir().join("last-run.json");
    match std::fs::read_to_string(&p) {
        Ok(s) => {
            println!("{s}");
            0
        }
        Err(_) => {
            eprintln!("no run report yet at {}", p.display());
            1
        }
    }
}

fn age_days(c: &scan::Candidate) -> u64 {
    c.newest
        .and_then(|n| SystemTime::now().duration_since(n).ok())
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0)
}

fn path_allowed(path: &Path, policy: &config::Policy) -> bool {
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    !policy
        .protect
        .iter()
        .any(|pat| !pat.is_empty() && canon.to_string_lossy().contains(pat.as_str()))
}

fn cmd_run(policy: &config::Policy, mode: Mode) -> i32 {
    let started = Instant::now();
    let use_pct = os::pressure_level(&policy.roots);
    let pressure = use_pct.is_some_and(|p| p >= policy.pressure_pct);
    let ages = policy.ages(pressure);

    let outcome = scan::scan(policy, ages);
    let dry_run = matches!(mode, Mode::Run { dry_run: true });
    let scan_only = matches!(mode, Mode::Scan { .. });
    let verbose = matches!(mode, Mode::Scan { verbose: true, .. });
    let mut report = Report::new(pressure, dry_run, use_pct);
    report.fresh_matched = outcome.fresh_count;
    report.stale_bytes = outcome.bytes_seen;

    for c in &outcome.stale {
        // `prepared` must stay bound through `execute` — dropping it releases
        // the cargo lock it holds, reopening exactly the race it exists for.
        match safety::guard(c, policy) {
            Ok(prepared) => {
                let bound = if c.kind == kinds::Kind::RustIncremental {
                    ages.incremental
                } else {
                    ages.for_gate(c.kind.gate())
                };
                if dry_run || scan_only {
                    report.push(Entry {
                        path: c.path.display().to_string(),
                        kind: c.kind.label().into(),
                        bytes: c.size_bytes,
                        action: "would-delete",
                        reason: format!("stale {bound}d+"),
                        age_days: age_days(c),
                    });
                } else {
                    match clean::execute(&prepared) {
                        Ok(freed) => report.push(Entry {
                            path: c.path.display().to_string(),
                            kind: c.kind.label().into(),
                            bytes: freed,
                            action: "deleted",
                            reason: String::new(),
                            age_days: age_days(c),
                        }),
                        // Windows locks busy trees; that refusal IS the
                        // process guard there, so it reports as a skip.
                        Err(clean::ExecuteError::Busy(e)) => report.push(Entry {
                            path: c.path.display().to_string(),
                            kind: c.kind.label().into(),
                            bytes: 0,
                            action: "skipped",
                            reason: format!("in-use: {e}"),
                            age_days: age_days(c),
                        }),
                        Err(clean::ExecuteError::Failed(e)) => report.push(Entry {
                            path: c.path.display().to_string(),
                            kind: c.kind.label().into(),
                            bytes: 0,
                            action: "failed",
                            reason: e,
                            age_days: age_days(c),
                        }),
                    }
                }
            }
            Err(s) => {
                if verbose || !scan_only {
                    report.push(Entry {
                        path: c.path.display().to_string(),
                        kind: c.kind.label().into(),
                        bytes: c.size_bytes,
                        action: "skipped",
                        reason: format!("{}: {}", s.reason, s.detail),
                        age_days: age_days(c),
                    });
                }
            }
        }
    }

    if scan_only {
        report.duration_ms = started.elapsed().as_millis() as u64;
        if matches!(mode, Mode::Scan { json: true, .. }) {
            let _ = serde_json::to_writer(std::io::stdout(), &report);
            println!();
        } else {
            print_table(&outcome, &report, verbose);
        }
        return 0;
    }

    // `run`: home caches, then the pending-dir reaper. One loop serves both
    // modes — --dry-run swaps process() for preview(), which answers what
    // each cache *would* free without mutating anything.
    if policy.categories.home_caches {
        let mut specs =
            homecache::specs(ages.cache_entry, ages.dep_stale, &policy.extra_cache_paths);
        if policy.categories.trash
            && let Some(t) = homecache::trash_spec(ages.dep_stale)
        {
            specs.push(t);
        }
        if policy.categories.cargo_registry {
            specs.push(homecache::cargo_registry_spec(ages.cache_entry));
        }
        for spec in &specs {
            if !spec.paths.iter().any(|p| p.is_dir()) {
                continue;
            }
            if spec.pressure_only && !pressure {
                report.caches.push(homecache::CacheResult {
                    id: spec.id.into(),
                    freed_bytes: 0,
                    detail: "reserved for disk-pressure runs".into(),
                });
                continue;
            }
            if spec.paths.iter().any(|p| !path_allowed(p, policy)) {
                report.caches.push(homecache::CacheResult {
                    id: spec.id.into(),
                    freed_bytes: 0,
                    detail: "protected".into(),
                });
                continue;
            }
            let r = if dry_run {
                homecache::preview(spec)
            } else {
                homecache::process(spec)
            };
            report.freed_bytes += r.freed_bytes;
            if r.freed_bytes > 0 {
                report.deleted += 1;
            }
            report.caches.push(r);
        }
    }
    if !dry_run {
        let (reaped, reaped_notes) = clean::reap_pending(&policy.roots, 3600);
        if reaped > 0 {
            println!("reaped {reaped} leftover pending dir(s)");
        }
        for n in reaped_notes {
            eprintln!("reap: {n}");
        }
    }

    report.duration_ms = started.elapsed().as_millis() as u64;
    // A dry-run is a question, not a run — `status` keeps reporting the last
    // real one.
    if !dry_run {
        report.save(&os::state_dir());
    }
    report.print_summary();
    for r in &report.caches {
        println!(
            "  cache {:<16} {} — {}",
            r.id,
            fmt_bytes(r.freed_bytes),
            r.detail
        );
    }
    if report.failed > 0 { 1 } else { 0 }
}

fn print_table(outcome: &scan::ScanOutcome, report: &Report, verbose: bool) {
    println!("{:<22} {:>9} {:>6}  path", "kind", "size", "age");
    for e in &report.entries {
        if !verbose && e.action == "skipped" {
            continue;
        }
        let suffix = if e.action == "skipped" {
            format!("   [{}]", e.reason)
        } else {
            String::new()
        };
        println!(
            "{:<22} {:>9} {:>6}  {}{}",
            e.kind,
            fmt_bytes(e.bytes),
            format!("{}d", e.age_days),
            e.path,
            suffix,
        );
    }
    println!(
        "\n{} matched artifact(s) still fresh; {} stale candidate(s) listed above \
         ({} reclaimable). `rldyour-cleaner run` applies the policy.",
        outcome.fresh_count,
        outcome.stale.len(),
        fmt_bytes(outcome.bytes_seen),
    );
}
