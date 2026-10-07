#!/usr/bin/env bash
set -euo pipefail
systemctl --user disable --now rldyour-cleaner.timer 2>/dev/null || true
systemctl --user stop rldyour-cleaner.service 2>/dev/null || true
UNIT_DIR="${HOME}/.config/systemd/user"
if [[ "${XDG_CONFIG_HOME:-}" == /* ]]; then UNIT_DIR="${XDG_CONFIG_HOME}/systemd/user"; fi
for unit in rldyour-cleaner.service rldyour-cleaner.timer; do
  file="${UNIT_DIR}/${unit}"
  if [[ -f "${file}" && ! -L "${file}" ]]; then rm "${file}"; fi
done
systemctl --user daemon-reload
