#!/usr/bin/env bash
# macOS launchd user scheduler. Does not initiate a cleanup run.
set -euo pipefail
AGENT_DIR="${HOME}/Library/LaunchAgents"
CONFIG_DIR="${HOME}/Library/Application Support/rldyour-cleaner"
LOG_DIR="${HOME}/Library/Logs/rldyour-cleaner"
PLIST="${AGENT_DIR}/io.nddev.rldyour-cleaner.plist"
mkdir -p "${AGENT_DIR}" "${LOG_DIR}"
chmod 700 "${LOG_DIR}"
if [[ -L "${PLIST}" ]]; then echo 'Refusing redirected agent' >&2; exit 1; fi
if [[ ! -e "${CONFIG_DIR}/config.toml" ]]; then "${BIN_DIR}/rldyour-cleaner" config --init; fi
xml_home="$(printf '%s' "${HOME}" | sed 's/&/\&amp;/g; s/</\&lt;/g; s/>/\&gt;/g')"
replacement="$(printf '%s' "${xml_home}" | sed 's/[\\&|]/\\&/g')"
sed "s|@HOME@|${replacement}|g" "${ROOT}/platforms/macos/launchd/io.nddev.rldyour-cleaner.plist" > "${PLIST}"
plutil -lint "${PLIST}"
launchctl bootout "gui/$(id -u)/io.nddev.rldyour-cleaner" 2>/dev/null || true
launchctl bootstrap "gui/$(id -u)" "${PLIST}"
