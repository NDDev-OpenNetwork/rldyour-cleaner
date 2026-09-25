# rldyour-cleaner installer — Windows.
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Builds the tool, installs it into %LOCALAPPDATA%\Programs\rldyour-cleaner,
# and registers a daily 03:00 Task Scheduler task (current user, lowest
# priority, catch-up for missed runs — the analogue of the systemd timer's
# Persistent=true). Linux/macOS use install.sh.
#Requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\rldyour-cleaner'
$Exe = Join-Path $InstallDir 'rldyour-cleaner.exe'
$TaskName = 'rldyour-cleaner'
$Repo = 'NDDev-OpenNetwork/rldyour-cleaner'

function Say($msg) { Write-Host "==> $msg" -ForegroundStyle Cyan }

# No Rust toolchain (or RLDYOUR_CLEANER_USE_RELEASE=1): fetch the latest
# release zip and
# verify its SHA-256 before the binary lands in $InstallDir.
function Install-FromRelease {
    $rel = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
    $tag = $rel.tag_name
    if (-not $tag) { throw 'could not resolve the latest release tag' }
    $asset = "rldyour-cleaner-$tag-windows-x86_64.zip"
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    try {
        $zip = Join-Path $tmp $asset
        Say "Downloading $asset"
        $base = "https://github.com/$Repo/releases/download/$tag"
        Invoke-WebRequest "$base/$asset" -OutFile $zip -UseBasicParsing
        $expected = ((Invoke-RestMethod "$base/$asset.sha256") -split '\s+')[0].ToLower()
        $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
        if ($expected -ne $actual) {
            throw "checksum mismatch for $asset ($actual != $expected)"
        }
        Expand-Archive $zip -DestinationPath $tmp -Force
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        Copy-Item -Force (Join-Path $tmp 'rldyour-cleaner.exe') $Exe
    }
    finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}

if ((Get-Command cargo -ErrorAction SilentlyContinue) -and -not $env:RLDYOUR_CLEANER_USE_RELEASE) {
    Say 'Building rldyour-cleaner'
    & cargo build --release --manifest-path (Join-Path $Root 'Cargo.toml')
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    Say "Installing the binary into $InstallDir"
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Force (Join-Path $Root 'target\release\rldyour-cleaner.exe') $Exe
}
else {
    Say "Installing the binary into $InstallDir from the latest release"
    Install-FromRelease
}

$ConfigDir = Join-Path $env:LOCALAPPDATA 'rldyour-cleaner'
if (-not (Test-Path (Join-Path $ConfigDir 'config.toml'))) {
    Say "Writing the default policy into $ConfigDir"
    & $Exe config --init | Out-Null
}

Say 'Registering the daily task (03:00, current user)'
$action = New-ScheduledTaskAction -Execute $Exe -Argument 'run'
$trigger = New-ScheduledTaskTrigger -Daily -At 03:00
# StartWhenAvailable is the Persistent=true analogue: a laptop that was off
# at 03:00 runs the job at next boot/logon. Priority 9 = near-idle: a
# janitor must never contend with the builds it cleans up after.
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -Priority 9
Register-ScheduledTask -TaskName $TaskName -Action $action `
    -Trigger $trigger -Settings $settings -Force | Out-Null
# First run now, so `status` has data before tonight's slot.
Start-ScheduledTask -TaskName $TaskName

Say 'Done'
@"

rldyour-cleaner is armed. Useful commands:

    rldyour-cleaner scan            # what would be cleaned, and why
    rldyour-cleaner run --dry-run   # full evaluation, no deletes
    rldyour-cleaner status          # last run's report
    Get-ScheduledTask -TaskName $TaskName | Format-List

Policy: $ConfigDir\config.toml
Binary: $Exe (add $InstallDir to your PATH for convenience)

"@
