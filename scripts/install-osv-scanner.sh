#!/usr/bin/env bash
set -euo pipefail

readonly VERSION="2.3.8"
readonly DESTINATION="${1:-${RUNNER_TEMP:-${TMPDIR:-/tmp}}/osv-scanner}"
readonly DESTINATION_DIR="$(dirname "${DESTINATION}")"

case "$(uname -m)" in
  arm64)
    readonly ASSET="osv-scanner_darwin_arm64"
    readonly EXPECTED_SHA256="a8cd6507b06239f463a7642430cfd2d154882f150f6e30cdc0653e28dfc34216"
    ;;
  x86_64)
    readonly ASSET="osv-scanner_darwin_amd64"
    readonly EXPECTED_SHA256="b8a80a9f14ca4c0cd0fc2d351b28f740da9e6a5b18385ac9f9d083360b5b504e"
    ;;
  *)
    echo "unsupported OSV-Scanner architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

mkdir -p "${DESTINATION_DIR}"
readonly TEMPORARY="$(mktemp "${DESTINATION}.XXXXXX")"
trap 'rm -f "${TEMPORARY}"' EXIT

curl --fail --location --silent --show-error \
  "https://github.com/google/osv-scanner/releases/download/v${VERSION}/${ASSET}" \
  --output "${TEMPORARY}"
printf '%s  %s\n' "${EXPECTED_SHA256}" "${TEMPORARY}" | /usr/bin/shasum -a 256 -c -
chmod 0755 "${TEMPORARY}"
mv -f "${TEMPORARY}" "${DESTINATION}"
"${DESTINATION}" --version
printf '%s\n' "${DESTINATION}"
