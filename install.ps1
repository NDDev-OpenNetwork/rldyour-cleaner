#Requires -Version 5.1
[CmdletBinding()]
param([string]$Version = '0.1.1')
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'platforms\windows\install.ps1') -Version $Version
