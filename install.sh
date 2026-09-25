#!/usr/bin/env bash
# rldyour-cleaner installer
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Builds the tool, installs it plus a daily systemd --user timer, and — when
# sudo is available — drops the /tmp aging override into tmpfiles.d. All user
# state stays under $HOME; only the tmpfiles drop-in touches the system.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
UNIT_DIR="${HOME}/.config/systemd/user"
CONF_DIR="${HOME}/.config/rldyour-cleaner"
TMPFILES_DROPIN="${ROOT}/tmpfiles.d/tmp.conf"

say() { printf '\033[1m==>\033[0m %s\n' "$1"; }

say "Building rldyour-cleaner"
cargo build --release --manifest-path "${ROOT}/Cargo.toml"

say "Installing the binary into ${BIN_DIR}"
install -Dm755 "${ROOT}/target/release/rldyour-cleaner" \
  "${BIN_DIR}/rldyour-cleaner"

say "Installing the user units into ${UNIT_DIR}"
install -Dm644 "${ROOT}/systemd/rldyour-cleaner.service" \
  "${UNIT_DIR}/rldyour-cleaner.service"
install -Dm644 "${ROOT}/systemd/rldyour-cleaner.timer" \
  "${UNIT_DIR}/rldyour-cleaner.timer"

if [ ! -f "${CONF_DIR}/config.toml" ]; then
  say "Writing the default policy into ${CONF_DIR}"
  mkdir -p "${CONF_DIR}"
  "${BIN_DIR}/rldyour-cleaner" config --init
fi

say "Enabling the daily timer"
systemctl --user daemon-reload
systemctl --user enable --now rldyour-cleaner.timer

if sudo -n true 2>/dev/null; then
  say "Installing the /tmp aging override (7d) into /etc/tmpfiles.d"
  sudo install -Dm644 "${TMPFILES_DROPIN}" /etc/tmpfiles.d/tmp.conf
  sudo systemd-tmpfiles --clean tmp.conf >/dev/null 2>&1 || true
else
  cat <<'NOTE'

No passwordless sudo — the /tmp aging override was NOT installed. To age
/tmp entries out after 7 days (distro default is 30), run once:

    sudo install -Dm644 tmpfiles.d/tmp.conf /etc/tmpfiles.d/tmp.conf
    sudo systemd-tmpfiles --clean

NOTE
fi

say "Done"
cat <<'NOTE'

rldyour-cleaner is armed. Useful commands:

    rldyour-cleaner scan            # what would be cleaned, and why
    rldyour-cleaner run --dry-run   # full evaluation, no deletes
    rldyour-cleaner status          # last run's report
    systemctl --user list-timers rldyour-cleaner.timer

Policy: ~/.config/rldyour-cleaner/config.toml

NOTE
