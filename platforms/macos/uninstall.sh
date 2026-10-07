#!/usr/bin/env bash
set -euo pipefail
launchctl bootout "gui/$(id -u)/io.nddev.rldyour-cleaner" 2>/dev/null || true
file="${HOME}/Library/LaunchAgents/io.nddev.rldyour-cleaner.plist"
if [[ -f "${file}" && ! -L "${file}" ]]; then rm "${file}"; fi
