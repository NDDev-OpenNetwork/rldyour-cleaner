#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "${ROOT}/Cargo.toml" | head -1)"
[[ "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
[[ -z "${1:-}" || "$1" == "v${version}" ]]
grep -q "^## \[${version}\]" "${ROOT}/CHANGELOG.md"
grep -A1 '^name = "rldyour-cleaner"' "${ROOT}/Cargo.lock" | grep -q "\"${version}\""
printf 'Version gate passed: %s\n' "${version}"
