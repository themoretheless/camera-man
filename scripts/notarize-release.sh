#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly APP="${1:-${ROOT}/target/CameraMan.app}"
readonly OUTPUT_DIR="${2:-${ROOT}/target/release-artifacts}"
readonly ARCHIVE="${OUTPUT_DIR}/CameraMan.zip"
readonly SUBMISSION="${OUTPUT_DIR}/CameraMan-notarization.zip"
readonly NOTARY_RESULT="${OUTPUT_DIR}/notary-result.json"

: "${APPLE_API_KEY_PATH:?APPLE_API_KEY_PATH must point to a temporary App Store Connect API key}"
: "${APPLE_API_KEY_ID:?APPLE_API_KEY_ID is required}"
: "${APPLE_API_ISSUER:?APPLE_API_ISSUER is required}"

if [[ ! -d "${APP}" ]]; then
  echo "app bundle not found: ${APP}" >&2
  exit 1
fi

mkdir -p "${OUTPUT_DIR}"
rm -f "${ARCHIVE}" "${SUBMISSION}" "${NOTARY_RESULT}" "${ARCHIVE}.sha256"

/usr/bin/codesign --verify --deep --strict --verbose=2 "${APP}"
/usr/bin/ditto -c -k --keepParent "${APP}" "${SUBMISSION}"
xcrun notarytool submit "${SUBMISSION}" \
  --key "${APPLE_API_KEY_PATH}" \
  --key-id "${APPLE_API_KEY_ID}" \
  --issuer "${APPLE_API_ISSUER}" \
  --wait \
  --output-format json > "${NOTARY_RESULT}"

if [[ "$(/usr/bin/plutil -extract status raw "${NOTARY_RESULT}")" != "Accepted" ]]; then
  cat "${NOTARY_RESULT}" >&2
  exit 1
fi

xcrun stapler staple "${APP}"
xcrun stapler validate "${APP}"
/usr/sbin/spctl --assess --type execute --verbose=4 "${APP}"
/usr/bin/ditto -c -k --keepParent "${APP}" "${ARCHIVE}"
(cd "${OUTPUT_DIR}" && /usr/bin/shasum -a 256 "$(basename "${ARCHIVE}")" > "$(basename "${ARCHIVE}").sha256")
rm -f "${SUBMISSION}"

echo "Notarized archive: ${ARCHIVE}"
echo "SHA-256: ${ARCHIVE}.sha256"
