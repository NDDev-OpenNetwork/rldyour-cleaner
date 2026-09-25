#!/usr/bin/env bash
# rldyour-cleaner installer — Linux + macOS.
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Builds the tool, installs it into ~/.local/bin, arms the OS scheduler
# (systemd --user timer on Linux, launchd agent on macOS), and on Linux —
# when sudo is available — drops the /tmp aging override into tmpfiles.d.
# All user state stays under $HOME; only the tmpfiles drop-in touches the
# system. Windows uses install.ps1.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
OS="$(uname -s)"

say() { printf '\033[1m==>\033[0m %s\n' "$1"; }

case "${OS}" in
  Linux|Darwin) ;;
  *) echo "unsupported OS: ${OS} (use install.ps1 on Windows)"; exit 1 ;;
esac

REPO="NDDev-OpenNetwork/rldyour-cleaner"

platform_slug() {
  case "${OS}:$(uname -m)" in
    Linux:x86_64)              echo linux-x86_64 ;;
    Linux:aarch64|Linux:arm64) echo linux-aarch64 ;;
    Darwin:arm64)              echo macos-aarch64 ;;
    Darwin:x86_64)             echo macos-x86_64 ;;
    *) return 1 ;;
  esac
}

# No Rust toolchain (or RLDYOUR_CLEANER_USE_RELEASE=1): fetch the latest
# release archive for this platform and verify its SHA-256 before it ever
# reaches the filesystem permanently.
install_from_release() {
  command -v curl >/dev/null || return 1
  local slug tag asset tmp
  slug="$(platform_slug)" || return 1
  tag="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)"
  [ -n "${tag}" ] || return 1
  asset="rldyour-cleaner-${tag}-${slug}.tar.gz"
  tmp="$(mktemp -d)"
  trap 'rm -rf "${tmp}"' RETURN
  say "Downloading ${asset}"
  curl -fsSL "https://github.com/${REPO}/releases/download/${tag}/${asset}" \
    -o "${tmp}/${asset}"
  curl -fsSL "https://github.com/${REPO}/releases/download/${tag}/${asset}.sha256" \
    -o "${tmp}/${asset}.sha256"
  (cd "${tmp}" && if command -v sha256sum >/dev/null; then
    sha256sum -c "${asset}.sha256"
  else
    shasum -a 256 -c "${asset}.sha256"
  fi)
  tar -xzf "${tmp}/${asset}" -C "${tmp}"
  mkdir -p "${BIN_DIR}"
  install -m755 "${tmp}/rldyour-cleaner" "${BIN_DIR}/rldyour-cleaner"
}

if command -v cargo >/dev/null && [ "${RLDYOUR_CLEANER_USE_RELEASE:-0}" != "1" ]; then
  say "Building rldyour-cleaner"
  cargo build --release --manifest-path "${ROOT}/Cargo.toml"
  say "Installing the binary into ${BIN_DIR}"
  # `install -D` is GNU coreutils only — mkdir + install -m works on BSD/macOS.
  mkdir -p "${BIN_DIR}"
  install -m755 "${ROOT}/target/release/rldyour-cleaner" \
    "${BIN_DIR}/rldyour-cleaner"
else
  say "Installing the binary into ${BIN_DIR} from the latest release"
  install_from_release || {
    echo "could not fetch a release binary for ${OS} $(uname -m);" \
      "install Rust (https://rustup.rs) and re-run to build from source" >&2
    exit 1
  }
fi

if [ "${OS}" = "Linux" ]; then
  UNIT_DIR="${HOME}/.config/systemd/user"
  CONF_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/rldyour-cleaner"
  TMPFILES_DROPIN="${ROOT}/platforms/linux/tmpfiles.d/tmp.conf"

  say "Installing the user units into ${UNIT_DIR}"
  mkdir -p "${UNIT_DIR}"
  install -m644 "${ROOT}/platforms/linux/systemd/rldyour-cleaner.service" \
    "${UNIT_DIR}/rldyour-cleaner.service"
  install -m644 "${ROOT}/platforms/linux/systemd/rldyour-cleaner.timer" \
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
    sudo install -m644 "${TMPFILES_DROPIN}" /etc/tmpfiles.d/tmp.conf
    sudo systemd-tmpfiles --clean tmp.conf >/dev/null 2>&1 || true
  else
    cat <<'NOTE'

No passwordless sudo — the /tmp aging override was NOT installed. To age
/tmp entries out after 7 days (distro default is 30), run once:

    sudo install -m644 platforms/linux/tmpfiles.d/tmp.conf /etc/tmpfiles.d/tmp.conf
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

elif [ "${OS}" = "Darwin" ]; then
  AGENT_LABEL="io.nddev.rldyour-cleaner"
  AGENT_PLIST="${HOME}/Library/LaunchAgents/${AGENT_LABEL}.plist"
  LOG_DIR="${HOME}/Library/Logs/rldyour-cleaner"

  mkdir -p "${HOME}/Library/LaunchAgents" "${LOG_DIR}"

  say "Installing the launchd agent (daily 03:00)"
  sed "s|@HOME@|${HOME}|g" \
    "${ROOT}/platforms/macos/launchd/${AGENT_LABEL}.plist" > "${AGENT_PLIST}"

  if [ ! -f "${HOME}/Library/Application Support/rldyour-cleaner/config.toml" ]; then
    say "Writing the default policy"
    "${BIN_DIR}/rldyour-cleaner" config --init
  fi

  launchctl bootout "gui/$(id -u)/${AGENT_LABEL}" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "${AGENT_PLIST}"
  # First run now, so `status` has data before tonight's 03:00 slot.
  launchctl kickstart -p "gui/$(id -u)/${AGENT_LABEL}" 2>/dev/null || true

  say "Done"
  cat <<NOTE

rldyour-cleaner is armed. Useful commands:

    rldyour-cleaner scan            # what would be cleaned, and why
    rldyour-cleaner run --dry-run   # full evaluation, no deletes
    rldyour-cleaner status          # last run's report
    launchctl print gui/$(id -u)/${AGENT_LABEL}

Policy: ${HOME}/Library/Application Support/rldyour-cleaner/config.toml
Logs:   ${LOG_DIR}/out.log

NOTE
fi
