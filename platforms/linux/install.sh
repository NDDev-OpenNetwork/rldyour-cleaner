#!/usr/bin/env bash
# Linux systemd user scheduler; respects absolute XDG_CONFIG_HOME.
set -euo pipefail
UNIT_DIR="${HOME}/.config/systemd/user"
CONFIG_DIR="${HOME}/.config/rldyour-cleaner"
if [[ "${XDG_CONFIG_HOME:-}" == /* ]]; then CONFIG_DIR="${XDG_CONFIG_HOME}/rldyour-cleaner"; UNIT_DIR="${XDG_CONFIG_HOME}/systemd/user"; fi
mkdir -p "${UNIT_DIR}"
for unit in rldyour-cleaner.service rldyour-cleaner.timer; do
  if [[ -L "${UNIT_DIR}/${unit}" ]]; then echo 'Refusing redirected unit' >&2; exit 1; fi
  install -m644 "${ROOT}/platforms/linux/systemd/${unit}" "${UNIT_DIR}/${unit}"
done
if [[ ! -e "${CONFIG_DIR}/config.toml" ]]; then "${BIN_DIR}/rldyour-cleaner" config --init; fi
systemctl --user daemon-reload
systemctl --user enable --now rldyour-cleaner.timer
