#!/usr/bin/env bash
# Shared install stage: build/select code, validate policy, replace one inode.
# Called by the root dispatcher with ROOT already resolved.
set -euo pipefail
BIN_DIR="${HOME}/.local/bin"
if [[ -x "${ROOT}/rldyour-cleaner" ]]; then
  BINARY="${ROOT}/rldyour-cleaner"
else
  cargo build --release --locked --manifest-path "${ROOT}/Cargo.toml"
  BINARY="${ROOT}/target/release/rldyour-cleaner"
fi
# Validate existing policy before touching the currently installed code.
"${BINARY}" config >/dev/null
if [[ -L "${BIN_DIR}" || -L "${BIN_DIR}/rldyour-cleaner" || -L "${BIN_DIR}/rldyour-cleaner.new" ]]; then
  echo 'Refusing redirected install path' >&2; exit 1
fi
mkdir -p "${BIN_DIR}"
install -m755 "${BINARY}" "${BIN_DIR}/rldyour-cleaner.new"
mv -f "${BIN_DIR}/rldyour-cleaner.new" "${BIN_DIR}/rldyour-cleaner"
