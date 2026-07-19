#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly APP="${1:-${ROOT}/target/CameraMan.app}"
readonly EXTENSION="${APP}/Contents/Library/SystemExtensions/com.cameraman.rust.extension.systemextension"
readonly APP_GROUP="${CAMERAMAN_APP_GROUP:?CAMERAMAN_APP_GROUP must name the signed shared group}"
readonly TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TEMP_DIR}"' EXIT

extract_entitlements() {
  local bundle=$1
  local output=$2
  /usr/bin/codesign --verify --strict --verbose=2 "${bundle}"
  /usr/bin/codesign --display --entitlements :- "${bundle}" > "${output}" 2>/dev/null
  /usr/bin/plutil -lint "${output}"
}

extract_entitlements "${APP}" "${TEMP_DIR}/host.plist"
extract_entitlements "${EXTENSION}" "${TEMP_DIR}/extension.plist"

python3 "${ROOT}/scripts/compare-entitlements.py" \
  "${TEMP_DIR}/host.plist" \
  "${ROOT}/config/entitlements/host-allowlist.plist" \
  "${APP_GROUP}"
python3 "${ROOT}/scripts/compare-entitlements.py" \
  "${TEMP_DIR}/extension.plist" \
  "${ROOT}/config/entitlements/extension-allowlist.plist" \
  "${APP_GROUP}"

echo "Host and extension entitlements match their minimal allowlists."
