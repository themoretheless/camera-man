#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly OUTPUT="${1:-${ROOT}/target/release-artifacts/camera-man.spdx3.jsonld}"
readonly SPDX2_OUTPUT="${2:-${ROOT}/target/release-artifacts/camera-man.spdx.json}"
readonly PYTHON="${PYTHON:-python3}"
readonly TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TEMP_DIR}"' EXIT

if ! command -v cargo-sbom >/dev/null 2>&1; then
  echo "cargo-sbom 0.10.0 is required: cargo install --locked cargo-sbom --version 0.10.0" >&2
  exit 1
fi
if ! "${PYTHON}" -c 'import spdx_tools' >/dev/null 2>&1; then
  echo "spdx-tools 0.8.5 is required: python3 -m pip install -r scripts/requirements-sbom.txt" >&2
  exit 1
fi

cd "${ROOT}"
mkdir -p "$(dirname "${OUTPUT}")" "$(dirname "${SPDX2_OUTPUT}")"
cargo metadata --locked --format-version 1 >/dev/null
cargo sbom --cargo-package camera-man --output-format spdx_json_2_3 \
  > "${TEMP_DIR}/raw.spdx.json"
"${PYTHON}" scripts/normalize-spdx2.py \
  "${TEMP_DIR}/raw.spdx.json" \
  "${SPDX2_OUTPUT}" \
  Cargo.lock
"${PYTHON}" scripts/convert-spdx3.py \
  "${SPDX2_OUTPUT}" \
  "${OUTPUT}"
"${PYTHON}" scripts/validate-spdx3.py \
  "${OUTPUT}" \
  "${SPDX2_OUTPUT}"
echo "Generated ${SPDX2_OUTPUT}"
echo "Generated ${OUTPUT}"
