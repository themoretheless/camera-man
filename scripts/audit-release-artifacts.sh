#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ARCHIVE="${1:-${ROOT}/target/release-artifacts/CameraMan.zip}"
readonly SBOM="${2:-${ROOT}/target/release-artifacts/camera-man.spdx.json}"
readonly REPORT_DIR="${3:-${ROOT}/target/release-artifacts}"
readonly OSV_SCANNER="${OSV_SCANNER:-osv-scanner}"
readonly EXTRACTED="$(mktemp -d)"
trap 'rm -rf "${EXTRACTED}"' EXIT

test -f "${ARCHIVE}" || { echo "release archive not found: ${ARCHIVE}" >&2; exit 1; }
test -f "${SBOM}" || { echo "release SBOM not found: ${SBOM}" >&2; exit 1; }
command -v cargo-audit >/dev/null 2>&1 || { echo "cargo-audit is required" >&2; exit 1; }
command -v "${OSV_SCANNER}" >/dev/null 2>&1 || { echo "OSV_SCANNER is unavailable: ${OSV_SCANNER}" >&2; exit 1; }
mkdir -p "${REPORT_DIR}"
cd "${ROOT}"

/usr/bin/zipinfo -1 "${ARCHIVE}" > "${REPORT_DIR}/archive-entries.txt"
if ! awk '
  /^\// || /(^|\/)\.\.($|\/)/ || /(^|\/)(\.env|id_rsa|id_ed25519|AuthKey[^\/]*\.p8|[^\/]*\.p12)$/ { bad=1 }
  $0 !~ /^CameraMan\.app\// { bad=1 }
  END { exit bad }
' "${REPORT_DIR}/archive-entries.txt"; then
  echo "archive contains an unsafe path or private signing material" >&2
  exit 1
fi

/usr/bin/ditto -x -k "${ARCHIVE}" "${EXTRACTED}"
if find "${EXTRACTED}" -type l -print -quit | grep -q .; then
  echo "release archive unexpectedly contains symbolic links" >&2
  exit 1
fi
if find "${EXTRACTED}" -type f \( -name '*.p12' -o -name '*.p8' -o -name 'id_rsa' -o -name 'id_ed25519' -o -name '.env' \) -print -quit | grep -q .; then
  echo "release archive contains private signing material" >&2
  exit 1
fi
readonly APP_BINARY="${EXTRACTED}/CameraMan.app/Contents/MacOS/CameraMan"
readonly EXTENSION_BINARY="${EXTRACTED}/CameraMan.app/Contents/Library/SystemExtensions/com.cameraman.rust.extension.systemextension/Contents/MacOS/CameraManExtension"
test -x "${APP_BINARY}" || { echo "host binary is missing from archive" >&2; exit 1; }
test -x "${EXTENSION_BINARY}" || { echo "extension binary is missing from archive" >&2; exit 1; }

cargo audit bin "${APP_BINARY}" "${EXTENSION_BINARY}" \
  2>&1 | tee "${REPORT_DIR}/binary-audit.txt"
"${OSV_SCANNER}" scan --config "${ROOT}/osv-scanner.toml" --format json -L "${SBOM}" \
  > "${REPORT_DIR}/sbom-osv.json"

echo "Artifact and SBOM scans passed"
