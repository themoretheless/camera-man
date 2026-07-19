#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly LOG="$(mktemp)"
trap 'rm -f "${LOG}"' EXIT

cd "${ROOT}"
cargo check --locked --all-targets --all-features 2>&1 | tee "${LOG}"

CRATES="$({
  sed -n 's/.*following packages contain code that will be rejected by a future version of Rust: //p' "${LOG}" |
    tr ',' '\n' |
    sed 's/^[[:space:]]*//;s/[[:space:]]*$//' |
    sed '/^$/d' |
    sort -u
} || true)"

if [[ -z "${CRATES}" ]]; then
  echo "No future-incompatibility reports. Remove the block exception documentation."
  exit 0
fi

if [[ "${CRATES}" != "block v0.1.6" ]]; then
  printf 'Unexpected future-incompatible crates:\n%s\n' "${CRATES}" >&2
  exit 1
fi

echo "Only the reviewed block v0.1.6 exception remains."
