# Synthetic Windows installer lifecycle; Task Scheduler calls are mocked.
$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $PSScriptRoot
$Fixture = Join-Path ([IO.Path]::GetTempPath()) ('rldc-install-' + [Guid]::NewGuid())
$PreviousLocal = $env:LOCALAPPDATA
$PreviousProfile = $env:USERPROFILE
function New-ScheduledTaskAction { param($Execute, $Argument) [pscustomobject]@{Execute=$Execute;Argument=$Argument} }
function New-ScheduledTaskTrigger { param([switch]$Daily,[datetime]$At) [pscustomobject]@{Daily=$Daily.IsPresent;Hour=$At.Hour;Minute=$At.Minute} }
function New-ScheduledTaskSettingsSet { param([switch]$StartWhenAvailable,$Priority,$MultipleInstances,[timespan]$ExecutionTimeLimit) [pscustomobject]@{Catchup=$StartWhenAvailable.IsPresent;Priority=$Priority;Instances=$MultipleInstances;Limit=$ExecutionTimeLimit.TotalMinutes} }
function Register-ScheduledTask { param($TaskName,$Action,$Trigger,$Settings,[switch]$Force) $global:RldCleanerTestTask=[pscustomobject]@{Name=$TaskName;Action=$Action;Trigger=$Trigger;Settings=$Settings} }
function Unregister-ScheduledTask { [CmdletBinding(SupportsShouldProcess=$true)] param($TaskName) $global:RldCleanerTestRemoved=$TaskName }
try {
    $Package = Join-Path $Fixture 'package'
    $Profile = Join-Path $Fixture 'home & unicode тест'
    New-Item -ItemType Directory -Force $Package, $Profile | Out-Null
    Copy-Item (Join-Path $RepoRoot 'target/debug/rldyour-cleaner.exe') $Package
    Copy-Item (Join-Path $RepoRoot 'install.ps1'), (Join-Path $RepoRoot 'uninstall.ps1') $Package
    Copy-Item (Join-Path $RepoRoot 'platforms') $Package -Recurse
    $env:USERPROFILE = $Profile
    $env:LOCALAPPDATA = Join-Path $Profile 'local'
    & (Join-Path $Package 'install.ps1')
    $Exe = Join-Path $env:LOCALAPPDATA 'Programs/rldyour-cleaner/rldyour-cleaner.exe'
    $Config = Join-Path $env:LOCALAPPDATA 'rldyour-cleaner/config.toml'
    if (-not (Test-Path $Exe) -or -not (Test-Path $Config)) { throw 'Code/config missing' }
    $Original = Get-Content $Config -Raw
    if ($global:RldCleanerTestTask.Trigger.Hour -ne 3 -or $global:RldCleanerTestTask.Trigger.Minute -ne 0 -or -not $global:RldCleanerTestTask.Settings.Catchup -or $global:RldCleanerTestTask.Settings.Instances -ne 'IgnoreNew' -or $global:RldCleanerTestTask.Settings.Limit -ne 5) { throw 'Wrong scheduler settings' }
    if ($global:RldCleanerTestTask.Action.Execute -ne $Exe -or $global:RldCleanerTestTask.Action.Argument -ne 'run') { throw 'Wrong task action' }
    & (Join-Path $Package 'install.ps1')
    if ((Get-Content $Config -Raw) -ne $Original) { throw 'Policy changed on upgrade' }
    & (Join-Path $Package 'uninstall.ps1')
    if ((Test-Path $Exe) -or (Get-Content $Config -Raw) -ne $Original -or $global:RldCleanerTestRemoved -ne 'rldyour-cleaner') { throw 'Uninstall lifecycle failed' }
    Write-Host 'PASS: Windows install/upgrade/uninstall, spaces/Unicode, catch-up/overlap/deadline; scheduler mocked only'
}
finally {
    $env:LOCALAPPDATA = $PreviousLocal
    $env:USERPROFILE = $PreviousProfile
    Remove-Variable RldCleanerTestTask, RldCleanerTestRemoved -Scope Global -ErrorAction SilentlyContinue
    Remove-Item $Fixture -Recurse -Force -ErrorAction SilentlyContinue
}
