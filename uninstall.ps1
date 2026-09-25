# rldyour-cleaner uninstaller — Windows.
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Removes the scheduled task and the binary. The policy file and last-run
# report are kept — they are yours; delete them by hand if you want them
# gone. Linux/macOS use uninstall.sh.
#Requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\rldyour-cleaner'
$ConfigDir = Join-Path $env:LOCALAPPDATA 'rldyour-cleaner'
$TaskName = 'rldyour-cleaner'

function Say($msg) { Write-Host "==> $msg" -ForegroundStyle Cyan }

Say 'Deleting the scheduled task'
Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false `
    -ErrorAction SilentlyContinue

Say 'Removing the binary'
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $InstallDir

Write-Host "Policy and run state kept under $ConfigDir"
Say 'Done'
