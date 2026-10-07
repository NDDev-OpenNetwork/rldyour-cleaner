use super::{SystemPolicy, observation};
pub(super) fn observe(_timeout: std::time::Duration) -> Vec<SystemPolicy> {
    vec![observation(
        "os-temp-and-logs",
        "owner-policy",
        "macOS owns temporary files and unified-log retention; cleaner never overrides OS policy or resets system caches",
    )]
}
pub(super) fn enable_apt_autoclean() -> Result<(), String> {
    Err("APT periodic maintenance is available only on Debian/Ubuntu Linux".into())
}

pub(super) fn scheduler_checks(timeout: std::time::Duration) -> Vec<crate::doctor::Check> {
    use crate::{
        doctor::{Check, State},
        os, process,
    };
    let Some(binary) = os::executable("launchctl") else {
        return vec![Check::new(
            "cleaner-schedule",
            State::Warning,
            "launchctl unavailable; user scheduler could not be checked",
        )];
    };
    let label = format!("gui/{}/io.nddev.rldyour-cleaner", unsafe { libc::getuid() });
    match process::run(
        std::process::Command::new(binary).args(["print", &label]),
        timeout,
    ) {
        Ok(out) if !out.stdout_truncated => snapshot(
            &out.stdout,
            &os::home_dir().join(".local/bin/rldyour-cleaner"),
        ),
        _ => vec![Check::new(
            "cleaner-schedule",
            State::Warning,
            "user launch agent is unavailable or query failed; no registered schedule inferred",
        )],
    }
}
fn snapshot(output: &str, expected: &std::path::Path) -> Vec<crate::doctor::Check> {
    use crate::doctor::{Check, State};
    let field = |prefix: &str| {
        output
            .lines()
            .map(str::trim)
            .find_map(|line| line.strip_prefix(prefix))
    };
    let correct = field("program = ") == expected.to_str();
    let mut checks = vec![Check::new(
        "cleaner-schedule",
        if correct { State::Ok } else { State::Error },
        if correct {
            "user launch agent is registered for the installed cleaner; native calendar triggers are owned by launchd"
        } else {
            "registered cleaner agent program differs from installed executable or could not be verified"
        },
    )];
    checks.push(Check::new("cleaner-calendar", if output.contains("stream = com.apple.launchd.calendarinterval") { State::Ok } else { State::Warning },
        if output.contains("stream = com.apple.launchd.calendarinterval") { "registered launch agent has a native calendar trigger" } else { "registered calendar trigger was not found in the native snapshot; schedule is not assumed" }));
    checks.push(match (field("state = "), field("last exit code = ")) {
        (Some("running" | "xpcproxy"), _) => Check::new(
            "cleaner-job",
            State::Info,
            "launchd job is running; no completed result inferred",
        ),
        (_, Some("0")) => Check::new(
            "cleaner-job",
            State::Ok,
            "last launchd run exited successfully; not-running between jobs is normal",
        ),
        (_, Some("(never exited)")) | (_, None) => Check::new(
            "cleaner-job",
            State::Warning,
            "no completed launchd exit result yet",
        ),
        _ => Check::new(
            "cleaner-job",
            State::Error,
            "last launchd exit was unsuccessful",
        ),
    });
    checks
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_success_running_failure_and_program_mismatch_are_distinct() {
        use crate::doctor::State;
        let expected = crate::os::home_dir().join("synthetic-cleaner");
        let good = format!(
            "state = not running\nprogram = {}\nlast exit code = 0\nstate = active\nstream = com.apple.launchd.calendarinterval",
            expected.display()
        );
        assert!(
            snapshot(&good, &expected)
                .iter()
                .all(|c| c.state == State::Ok)
        );
        assert_eq!(
            snapshot(&good.replace("not running", "running"), &expected)[2].state,
            State::Info
        );
        assert_eq!(
            snapshot(
                &good.replace("last exit code = 0", "last exit code = 1"),
                &expected
            )[2]
            .state,
            State::Error
        );
        assert_eq!(
            snapshot(&good, &expected.with_extension("different"))[0].state,
            State::Error
        );
    }
}
