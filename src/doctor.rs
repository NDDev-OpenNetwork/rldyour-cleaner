//! Read-only end-to-end maintenance diagnostics. No GC, run lock, directories,
//! report writes or package-manager shim queries are performed here.
use crate::{config::Policy, maintenance::Ledger, os, report, system_policy};
use serde::Serialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    Ok,
    Warning,
    Error,
    Info,
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
#[derive(Serialize)]
pub struct Check {
    pub id: String,
    pub state: State,
    pub detail: String,
}
impl Check {
    pub fn new(id: &str, state: State, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state,
            detail: detail.into(),
        }
    }
}
#[derive(Serialize)]
pub struct DoctorReport {
    pub tool: &'static str,
    pub version: &'static str,
    pub checked_unix: u64,
    pub errors: usize,
    pub warnings: usize,
    pub checks: Vec<Check>,
}
impl DoctorReport {
    pub fn from_checks(checks: Vec<Check>, now: u64) -> Self {
        Self {
            tool: "rldyour-cleaner",
            version: env!("CARGO_PKG_VERSION"),
            checked_unix: now,
            errors: checks.iter().filter(|c| c.state == State::Error).count(),
            warnings: checks.iter().filter(|c| c.state == State::Warning).count(),
            checks,
        }
    }
    pub fn print(&self) {
        println!(
            "rldyour-cleaner doctor {}: {} error(s), {} warning(s)",
            self.version, self.errors, self.warnings
        );
        for check in &self.checks {
            println!("  {:?}: {} — {}", check.state, check.id, check.detail);
        }
    }
}
pub fn inspect(policy: &Policy) -> DoctorReport {
    let now = now();
    let timeout = Duration::from_secs(policy.command_timeout_seconds.min(5));
    let mut checks = system_policy::scheduler_checks(timeout);
    checks.push(match Ledger::load(&os::state_dir()) {
        Ok(_) => Check::new(
            "completion-ledger",
            State::Ok,
            "bounded completion state is valid or has not been created; no write performed",
        ),
        Err(e) => Check::new("completion-ledger", State::Error, e),
    });
    match report::read_status(&os::state_dir()) {
        Ok(content) => {
            let value: serde_json::Value =
                serde_json::from_str(&content).expect("read_status validates JSON");
            checks.push(last_run(&value, now));
            if value["version"].as_str() != Some(env!("CARGO_PKG_VERSION")) {
                checks.push(Check::new("report-version", State::Warning, "saved report comes from another installed version; next normal run will replace it"));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => checks.push(Check::new(
            "last-run",
            State::Warning,
            "no actual cleaner run report yet; doctor does not create one",
        )),
        Err(e) => checks.push(Check::new("last-run", State::Error, e.to_string())),
    }
    checks.extend(system_policy::observe(timeout).into_iter().map(|p| {
        let state = match p.status {
            "active" | "configured" | "completed" => State::Ok,
            "failed" => State::Error,
            "unknown" | "inactive" | "disabled" => State::Warning,
            _ => State::Info,
        };
        Check::new(p.id, state, p.detail)
    }));
    checks.push(Check::new("cache-preservation", if policy.native_gc.uv && policy.native_gc.uv_prune_rebuildable_environments && policy.categories.home_caches { State::Warning } else { State::Ok },
        if policy.native_gc.uv && policy.native_gc.uv_prune_rebuildable_environments && policy.categories.home_caches {
            "explicit uv prune can remove cached/project-linked environments; review cache-dependent interpreter and offline use"
        } else { "cached environments are preserved; Node stores and installed runtimes remain inventory-only" }));
    DoctorReport::from_checks(checks, now)
}
fn last_run(value: &serde_json::Value, now: u64) -> Check {
    let failed = value["failed"].as_u64().unwrap_or(1);
    let started = value["started_unix"].as_u64().unwrap_or(0);
    if value["dry_run"].as_bool() != Some(false) {
        return Check::new(
            "last-run",
            State::Error,
            "saved state is a preview, not evidence of an actual run",
        );
    }
    if failed > 0 {
        return Check::new(
            "last-run",
            State::Error,
            format!("last actual run reported {failed} native/cache failure(s)"),
        );
    }
    match now.checked_sub(started) {
        None => Check::new(
            "last-run",
            State::Warning,
            "report timestamp is in the future; clock rollback may defer maintenance",
        ),
        Some(age) if age > 72 * 3600 => Check::new(
            "last-run",
            State::Warning,
            "last actual run is older than 72 hours; offline/sleep/login state may explain it; check schedule",
        ),
        Some(age) => Check::new(
            "last-run",
            State::Ok,
            format!(
                "last actual run completed without native/cache failures {age} seconds ago; this does not certify all owner-managed caches"
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_stale_future_and_preview_reports_do_not_become_healthy() {
        let mut v = serde_json::json!({"failed":0,"started_unix":100,"dry_run":false});
        assert_eq!(last_run(&v, 101).state, State::Ok);
        assert_eq!(last_run(&v, 99).state, State::Warning);
        assert_eq!(last_run(&v, 100 + 73 * 3600).state, State::Warning);
        v["failed"] = 1.into();
        assert_eq!(last_run(&v, 101).state, State::Error);
        v["failed"] = 0.into();
        v["dry_run"] = true.into();
        assert_eq!(last_run(&v, 101).state, State::Error);
        let r = DoctorReport::from_checks(
            vec![
                Check::new("x", State::Error, "failure"),
                Check::new("y", State::Warning, "unknown"),
            ],
            0,
        );
        assert_eq!((r.errors, r.warnings), (1, 1));
    }
}
