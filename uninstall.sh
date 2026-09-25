#!/usr/bin/env bash
# rldyour-cleaner uninstaller — Linux + macOS.
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Removes the scheduler registration, the units/plist and the binary. The
# policy file and last-run report are kept — they are yours; delete them by
# hand if you want them gone. Windows uses uninstall.ps1.
set -euo pipefail

BIN_DIR="${HOME}/.local/bin"
OS="$(uname -s)"

say() { printf '\033[1m==>\033[0m %s\n' "$1"; }

if [ "${OS}" = "Linux" ]; then
  UNIT_DIR="${HOME}/.config/systemd/user"

  say "Disabling the timer"
  systemctl --user disable --now rldyour-cleaner.timer 2>/dev/null || true
  systemctl --user stop rldyour-cleaner.service 2>/dev/null || true

  say "Removing units and binary"
  rm -f "${UNIT_DIR}/rldyour-cleaner.service" "${UNIT_DIR}/rldyour-cleaner.timer"
  rm -f "${BIN_DIR}/rldyour-cleaner"
  systemctl --user daemon-reload

  if [ -f /etc/tmpfiles.d/tmp.conf ] && \
     grep -q 'rldyour-cleaner override' /etc/tmpfiles.d/tmp.conf 2>/dev/null; then
    if sudo -n true 2>/dev/null; then
      say "Restoring the stock /tmp tmpfiles policy"
      sudo rm -f /etc/tmpfiles.d/tmp.conf
    else
      echo "NOTE: /etc/tmpfiles.d/tmp.conf still holds the rldyour-cleaner" \
           "override (7d). Remove it with sudo to restore the 30d default."
    fi
  fi

  echo "Policy and run state kept under ${HOME}/.config/rldyour-cleaner and" \
       "${HOME}/.local/state/rldyour-cleaner."

elif [ "${OS}" = "Darwin" ]; then
  AGENT_LABEL="io.nddev.rldyour-cleaner"
  AGENT_PLIST="${HOME}/Library/LaunchAgents/${AGENT_LABEL}.plist"

  say "Removing the launchd agent"
  launchctl bootout "gui/$(id -u)/${AGENT_LABEL}" 2>/dev/null || true
  rm -f "${AGENT_PLIST}"

  say "Removing the binary"
  rm -f "${BIN_DIR}/rldyour-cleaner"

  echo "Policy and logs kept under ${HOME}/Library/Application Support/rldyour-cleaner," \
       "${HOME}/Library/Logs/rldyour-cleaner."

else
  echo "unsupported OS: ${OS} (use uninstall.ps1 on Windows)"; exit 1
fi

say "Done"
