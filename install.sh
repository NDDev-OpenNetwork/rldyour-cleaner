#!/usr/bin/env bash
# Install user code and the OS schedule. No sudo, global tmp policy changes,
# first-run cleanup or project mutation occurs during installation.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
OS="$(uname -s)"
case "${OS}" in Linux|Darwin) ;; *) echo "Use install.ps1 on Windows" >&2; exit 1;; esac
if [[ -x "${ROOT}/rldyour-cleaner" ]]; then
  BINARY="${ROOT}/rldyour-cleaner"
else
  cargo build --release --locked --manifest-path "${ROOT}/Cargo.toml"
  BINARY="${ROOT}/target/release/rldyour-cleaner"
fi
# Refuse redirection of code/config files; install via a staged inode so a
# running oneshot can finish its existing binary safely.
if [[ -L "${BIN_DIR}" || -L "${BIN_DIR}/rldyour-cleaner" ]]; then
  echo 'Refusing redirected install path' >&2; exit 1
fi
mkdir -p "${BIN_DIR}"
install -m755 "${BINARY}" "${BIN_DIR}/rldyour-cleaner.new"
mv -f "${BIN_DIR}/rldyour-cleaner.new" "${BIN_DIR}/rldyour-cleaner"
if [[ "${OS}" == Linux ]]; then
  UNIT_DIR="${HOME}/.config/systemd/user"
  CONFIG_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/rldyour-cleaner"
  mkdir -p "${UNIT_DIR}"
  for unit in rldyour-cleaner.service rldyour-cleaner.timer; do
    if [[ -L "${UNIT_DIR}/${unit}" ]]; then echo 'Refusing redirected unit' >&2; exit 1; fi
    install -m644 "${ROOT}/platforms/linux/systemd/${unit}" "${UNIT_DIR}/${unit}"
  done
  if [[ ! -e "${CONFIG_DIR}/config.toml" ]]; then "${BIN_DIR}/rldyour-cleaner" config --init; fi
  "${BIN_DIR}/rldyour-cleaner" config >/dev/null
  systemctl --user daemon-reload
  systemctl --user enable --now rldyour-cleaner.timer
else
  AGENT_DIR="${HOME}/Library/LaunchAgents"
  CONFIG_DIR="${HOME}/Library/Application Support/rldyour-cleaner"
  LOG_DIR="${HOME}/Library/Logs/rldyour-cleaner"
  PLIST="${AGENT_DIR}/io.nddev.rldyour-cleaner.plist"
  mkdir -p "${AGENT_DIR}" "${LOG_DIR}"
  chmod 700 "${LOG_DIR}"
  if [[ -L "${PLIST}" ]]; then echo 'Refusing redirected agent' >&2; exit 1; fi
  if [[ ! -e "${CONFIG_DIR}/config.toml" ]]; then "${BIN_DIR}/rldyour-cleaner" config --init; fi
  "${BIN_DIR}/rldyour-cleaner" config >/dev/null
  # Escape sed replacement syntax even for unusual home directory names.
  replacement="$(printf '%s' "${HOME}" | sed 's/[\\&|]/\\&/g')"
  sed "s|@HOME@|${replacement}|g" "${ROOT}/platforms/macos/launchd/io.nddev.rldyour-cleaner.plist" > "${PLIST}"
  plutil -lint "${PLIST}"
  launchctl bootout "gui/$(id -u)/io.nddev.rldyour-cleaner" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "${PLIST}"
fi
printf 'Installed %s. Daily native GC; project inventory only. Run scan --json before an initial run.\n' "$("${BIN_DIR}/rldyour-cleaner" --version)"
