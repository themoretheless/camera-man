#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly PROFILE="${CAMERAMAN_ACCEPTANCE_PROFILE:-${ROOT}/benchmarks/acceptance/apple-m4-max-aarch64.json}"
readonly OUTPUT="${CAMERAMAN_ACCEPTANCE_REPORT:-${ROOT}/target/acceptance/1080p30-4-source.json}"

SMOKE=false
DURATION=28800
SAMPLE=60
if [[ "${1:-}" == "--smoke" ]]; then
  SMOKE=true
  DURATION="${CAMERAMAN_ACCEPTANCE_SECONDS:-3}"
  SAMPLE=1
elif [[ -n "${1:-}" ]]; then
  echo "usage: acceptance-1080p30.sh [--smoke]" >&2
  exit 2
fi

mkdir -p "$(dirname "${OUTPUT}")"
cd "${ROOT}"
cargo run --locked --release --bin cameraman-soak --no-default-features -- \
  --duration-seconds "${DURATION}" \
  --sample-seconds "${SAMPLE}" > "${OUTPUT}"

if [[ "${SMOKE}" == true ]]; then
  python3 scripts/evaluate-acceptance.py "${PROFILE}" "${OUTPUT}" --smoke
else
  python3 scripts/evaluate-acceptance.py "${PROFILE}" "${OUTPUT}"
fi
