#!/usr/bin/env bash
# Unix entrypoint; shared code stage and OS-native scheduler are separate.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
case "$(uname -s)" in
  Linux) PLATFORM=linux;;
  Darwin) PLATFORM=macos;;
  *) echo 'Supported Unix platforms: Linux/macOS; use install.ps1 on Windows' >&2; exit 1;;
esac
# shellcheck source=platforms/common/install.sh
source "${ROOT}/platforms/common/install.sh"
# Platform scripts are checked individually by CI.
# shellcheck source=/dev/null
source "${ROOT}/platforms/${PLATFORM}/install.sh"
printf 'Installed %s. Daily native GC; project inventory only.\n' "$("${BIN_DIR}/rldyour-cleaner" --version)"
