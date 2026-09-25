# rldyour-cleaner installer — Windows.
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Builds the tool, installs it into %LOCALAPPDATA%\Programs\rldyour-cleaner,
# and registers a daily 03:00 Task Scheduler task (limited rights, current
# user). Linux/macOS use install.sh.
#Requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\rldyour-cleaner'
$Exe = Join-Path $InstallDir 'rldyour-cleaner.exe'
$TaskName = 'rldyour-cleaner'

function Say($msg) { Write-Host "==> $msg" -ForegroundStyle Cyan }

Say 'Building rldyour-cleaner'
& cargo build --release --manifest-path (Join-Path $Root 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

Say "Installing the binary into $InstallDir"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -Force (Join-Path $Root 'target\release\rldyour-cleaner.exe') $Exe

$ConfigDir = Join-Path $env:LOCALAPPDATA 'rldyour-cleaner'
if (-not (Test-Path (Join-Path $ConfigDir 'config.toml'))) {
    Say "Writing the default policy into $ConfigDir"
    & $Exe config --init | Out-Null
}

Say 'Registering the daily task (03:00, current user)'
# Limited-rights daily task; Task Scheduler fires missed runs on wake when
# "run when computer was idle" style catch-up applies via schtasks default.
& schtasks.exe /Create /F /TN $TaskName /SC DAILY /ST 03:00 /RL LIMITED `
    /TR "`"$Exe`" run" | Out-Null
# First run now, so `status` has data before tonight's slot.
& schtasks.exe /Run /TN $TaskName | Out-Null

Say 'Done'
@"

rldyour-cleaner is armed. Useful commands:

    rldyour-cleaner scan            # what would be cleaned, and why
    rldyour-cleaner run --dry-run   # full evaluation, no deletes
    rldyour-cleaner status          # last run's report
    schtasks /Query /TN $TaskName /V /FO LIST

Policy: $ConfigDir\config.toml
Binary: $Exe (add $InstallDir to your PATH for convenience)

"@
