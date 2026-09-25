#!/usr/bin/env bash
# rldyour-cleaner uninstaller
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Removes the timer, the units and the binary. The policy file and last-run
# report are kept — they are yours; delete them by hand if you want them gone.
set -euo pipefail

BIN_DIR="${HOME}/.local/bin"
UNIT_DIR="${HOME}/.config/systemd/user"

say() { printf '\033[1m==>\033[0m %s\n' "$1"; }

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

say "Done"
echo "Policy and run state kept under ${HOME}/.config/rldyour-cleaner and" \
     "${HOME}/.local/state/rldyour-cleaner."
