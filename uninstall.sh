#!/usr/bin/env bash
# Remove code/schedule while preserving policy and run reports.
set -euo pipefail
case "$(uname -s)" in
  Linux)
    systemctl --user disable --now rldyour-cleaner.timer 2>/dev/null || true
    systemctl --user stop rldyour-cleaner.service 2>/dev/null || true
    for unit in rldyour-cleaner.service rldyour-cleaner.timer; do
      file="${HOME}/.config/systemd/user/${unit}"
      if [[ -f "${file}" && ! -L "${file}" ]]; then rm "${file}"; fi
    done
    systemctl --user daemon-reload ;;
  Darwin)
    launchctl bootout "gui/$(id -u)/io.nddev.rldyour-cleaner" 2>/dev/null || true
    file="${HOME}/Library/LaunchAgents/io.nddev.rldyour-cleaner.plist"
    if [[ -f "${file}" && ! -L "${file}" ]]; then rm "${file}"; fi ;;
  *) echo 'Use uninstall.ps1 on Windows' >&2; exit 1;;
esac
file="${HOME}/.local/bin/rldyour-cleaner"
if [[ -f "${file}" && ! -L "${file}" ]]; then rm "${file}"; fi
printf 'Removed cleaner code and schedule; policy/report preserved. Global tmp policy untouched.\n'
