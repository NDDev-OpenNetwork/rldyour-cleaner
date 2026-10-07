//! Evaluation and reporting share one pipeline. Scans/dry runs never mutate
//! content or overwrite the last actual run; real runs own a private lock.
use crate::{
    clean::RunLock,
    config::Policy,
    homecache,
    maintenance::Ledger,
    os,
    report::{Entry, Report},
    scan,
};
use std::time::{Instant, SystemTime};

pub fn evaluate(policy: &Policy, dry_run: bool, inspect_projects: bool) -> Result<Report, String> {
    let _lock = if dry_run {
        None
    } else {
        Some(
            RunLock::acquire(&os::state_dir())
                .map_err(|e| format!("cannot acquire cleaner run lock: {e}"))?,
        )
    };
    let started = Instant::now();
    let mut cadence = Ledger::load(&os::state_dir())?;
    // Refuse redirected report destinations before invoking any native GC.
    crate::safety::plain_path(&os::state_dir().join("last-run.json")).map_err(|e| e.to_string())?;
    let use_pct = os::pressure_level(&policy.roots);
    let pressure = use_pct.is_some_and(|p| p >= policy.pressure_pct);
    let mut report = Report::new(pressure, dry_run, use_pct);
    if inspect_projects && policy.categories.projects {
        let outcome = scan::scan(policy, policy.ages(pressure));
        report.fresh_matched = outcome.fresh_count;
        report.stale_bytes = outcome.bytes_seen;
        for c in outcome.stale {
            report.push(Entry {
                path: c.path.display().to_string(),
                kind: c.kind.label().into(),
                bytes: c.size_bytes,
                action: "review-only",
                reason: "age/mtime and no open process cannot prove an artifact is unused".into(),
                age_days: c
                    .newest
                    .and_then(|n| SystemTime::now().duration_since(n).ok())
                    .map(|d| d.as_secs() / 86_400)
                    .unwrap_or(0),
            });
        }
    }
    report.caches = homecache::evaluate(policy, dry_run, &mut cadence);
    report.system_policies = crate::system_policy::observe(std::time::Duration::from_secs(
        policy.command_timeout_seconds.min(5),
    ));
    report.failed += report
        .caches
        .iter()
        .filter(|c| c.action == homecache::Action::Failed)
        .count();
    report.skipped += report
        .caches
        .iter()
        .filter(|c| !c.action.is_gc() && c.action != homecache::Action::Failed)
        .count();
    report.duration_ms = started.elapsed().as_millis() as u64;
    if !dry_run {
        cadence.save(&os::state_dir()).map_err(|e| e.to_string())?;
        report.save(&os::state_dir()).map_err(|e| e.to_string())?;
    }
    Ok(report)
}
