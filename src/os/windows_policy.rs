use super::{SystemPolicy, observation};
pub(super) fn observe(_timeout: std::time::Duration) -> Vec<SystemPolicy> {
    vec![observation(
        "os-storage",
        "owner-policy",
        "Storage Sense and Windows servicing own temporary/update-file cleanup; cleaner never changes registry policy, Downloads or Recycle Bin",
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
    let Some(binary) = os::executable("powershell") else {
        return vec![Check::new(
            "cleaner-schedule",
            State::Warning,
            "native PowerShell unavailable; Task Scheduler status could not be checked",
        )];
    };
    // Fixed read-only native command, no user interpolation/scripts/profiles.
    const QUERY: &str = "[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false); $ErrorActionPreference='Stop'; $t=Get-ScheduledTask -TaskName 'rldyour-cleaner' -TaskPath '\\'; $i=Get-ScheduledTaskInfo -InputObject $t; [pscustomobject]@{State=[string]$t.State;Execute=[string]$t.Actions[0].Execute;Arguments=[string]$t.Actions[0].Arguments;ActionCount=@($t.Actions).Count;LastTaskResult=[long]$i.LastTaskResult;HasRun=($i.LastRunTime.Year -gt 2000);HasNextRun=($i.NextRunTime.Year -gt 2000)} | ConvertTo-Json -Compress";
    match process::run(
        std::process::Command::new(binary).args([
            "-NoProfile",
            "-NoLogo",
            "-NonInteractive",
            "-Command",
            QUERY,
        ]),
        timeout,
    ) {
        Ok(out) if !out.stdout_truncated => snapshot(
            &out.stdout,
            &os::data_local_dir().join("Programs/rldyour-cleaner/rldyour-cleaner.exe"),
        ),
        _ => vec![Check::new(
            "cleaner-schedule",
            State::Warning,
            "current task is unavailable or native query failed; no healthy Task Scheduler state inferred",
        )],
    }
}
fn snapshot(output: &str, expected: &std::path::Path) -> Vec<crate::doctor::Check> {
    use crate::doctor::{Check, State};
    let Ok(v) = serde_json::from_str::<serde_json::Value>(output) else {
        return vec![Check::new(
            "cleaner-schedule",
            State::Warning,
            "invalid native task query response",
        )];
    };
    let correct = v["Execute"]
        .as_str()
        .is_some_and(|s| s.eq_ignore_ascii_case(&expected.to_string_lossy()))
        && v["Arguments"].as_str() == Some("run")
        && v["ActionCount"].as_u64() == Some(1);
    let enabled = matches!(v["State"].as_str(), Some("Ready" | "Running" | "Queued"))
        && v["HasNextRun"].as_bool() == Some(true);
    let mut checks = vec![Check::new(
        "cleaner-schedule",
        if correct && enabled {
            State::Ok
        } else {
            State::Error
        },
        if correct && enabled {
            "native task is enabled, has a next run and targets only the installed cleaner run action"
        } else {
            "native task is disabled, has no next run or targets a different action"
        },
    )];
    checks.push(
        if matches!(v["State"].as_str(), Some("Running" | "Queued")) {
            Check::new(
                "cleaner-job",
                State::Info,
                "task is currently running or queued; no completed result inferred",
            )
        } else if v["HasRun"].as_bool() != Some(true) {
            Check::new(
                "cleaner-job",
                State::Warning,
                "task has not completed an actual run",
            )
        } else if v["LastTaskResult"].as_u64() == Some(0) {
            Check::new(
                "cleaner-job",
                State::Ok,
                "last native task completed successfully",
            )
        } else {
            Check::new(
                "cleaner-job",
                State::Error,
                "last native task result failed or is unknown",
            )
        },
    );
    checks
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scheduler_action_disabled_never_run_and_failure_are_distinct() {
        use crate::doctor::State;
        let expected = crate::os::home_dir().join("synthetic.exe");
        let mut v = serde_json::json!({"State":"Ready","Execute":expected,"Arguments":"run","ActionCount":1,"LastTaskResult":0,"HasRun":true,"HasNextRun":true});
        assert!(
            snapshot(&v.to_string(), &expected)
                .iter()
                .all(|c| c.state == State::Ok)
        );
        v["State"] = "Disabled".into();
        assert_eq!(snapshot(&v.to_string(), &expected)[0].state, State::Error);
        v["State"] = "Ready".into();
        v["LastTaskResult"] = 1.into();
        assert_eq!(snapshot(&v.to_string(), &expected)[1].state, State::Error);
        v["HasRun"] = false.into();
        assert_eq!(snapshot(&v.to_string(), &expected)[1].state, State::Warning);
        v["Arguments"] = "other".into();
        assert_eq!(snapshot(&v.to_string(), &expected)[0].state, State::Error);
    }
}
