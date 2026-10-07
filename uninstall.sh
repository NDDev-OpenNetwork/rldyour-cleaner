#!/usr/bin/env bash
# Platform scheduler teardown plus one common code removal; policy/state stay.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
case "$(uname -s)" in
  Linux) PLATFORM=linux;;
  Darwin) PLATFORM=macos;;
  *) echo 'Use uninstall.ps1 on Windows' >&2; exit 1;;
esac
# Platform scripts are checked individually by CI.
# shellcheck source=/dev/null
source "${ROOT}/platforms/${PLATFORM}/uninstall.sh"
file="${HOME}/.local/bin/rldyour-cleaner"
if [[ -f "${file}" && ! -L "${file}" ]]; then rm "${file}"; fi
printf 'Removed cleaner code and schedule; policy/report preserved. Global tmp policy untouched.\n'
