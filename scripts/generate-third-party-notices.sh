#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly OUTPUT="${1:-${ROOT}/THIRD_PARTY_NOTICES.md}"

if ! command -v cargo-about >/dev/null 2>&1; then
  echo "cargo-about 0.9.1 is required: cargo install --locked cargo-about --version 0.9.1" >&2
  exit 1
fi

mkdir -p "$(dirname "${OUTPUT}")"
cd "${ROOT}"
cargo about generate --locked about.hbs > "${OUTPUT}"
echo "Generated ${OUTPUT}"
