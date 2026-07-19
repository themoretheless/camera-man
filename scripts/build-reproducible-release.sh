#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly REQUESTED_OUTPUT_ROOT="${1:-${ROOT}/target/reproducibility}"
mkdir -p "${REQUESTED_OUTPUT_ROOT}"
readonly OUTPUT_ROOT="$(cd "${REQUESTED_OUTPUT_ROOT}" && pwd -P)"
case "${OUTPUT_ROOT}" in
  "${ROOT}/target/"*) ;;
  *)
    echo "reproducibility output must be below ${ROOT}/target: ${OUTPUT_ROOT}" >&2
    exit 1
    ;;
esac
readonly FIRST_TARGET="${OUTPUT_ROOT}/first"
readonly SECOND_TARGET="${OUTPUT_ROOT}/second"
readonly REPORT="${OUTPUT_ROOT}/unsigned-binaries.sha256"
readonly COMMIT="${CAMERAMAN_RELEASE_COMMIT:-$(git -C "${ROOT}" rev-parse HEAD)}"
readonly EPOCH="${SOURCE_DATE_EPOCH:-$(git -C "${ROOT}" show -s --format=%ct "${COMMIT}")}"
readonly REMAP="--remap-path-prefix=${ROOT}=/workspace/camera-man"

command -v cargo-auditable >/dev/null 2>&1 || {
  echo "cargo-auditable 0.7.5 is required" >&2
  exit 1
}

if [[ "${CI:-false}" == "true" ]] && [[ -n "$(git -C "${ROOT}" status --porcelain --untracked-files=all)" ]]; then
  echo "release reproducibility build requires a clean checkout" >&2
  exit 1
fi

rm -rf "${FIRST_TARGET}" "${SECOND_TARGET}"
rm -f "${REPORT}"
mkdir -p "${FIRST_TARGET}" "${SECOND_TARGET}"

build_once() {
  local target_dir=$1
  env \
    CAMERAMAN_BUILD_HASH="${COMMIT}" \
    SOURCE_DATE_EPOCH="${EPOCH}" \
    CARGO_INCREMENTAL=0 \
    CARGO_TARGET_DIR="${target_dir}" \
    RUSTFLAGS="${REMAP} ${RUSTFLAGS:-}" \
    cargo auditable build --locked --release \
      --bin camera-man \
      --bin cameraman-extension
}

cd "${ROOT}"
build_once "${FIRST_TARGET}"
build_once "${SECOND_TARGET}"

for binary in camera-man cameraman-extension; do
  cmp "${FIRST_TARGET}/release/${binary}" "${SECOND_TARGET}/release/${binary}"
done

{
  /usr/bin/shasum -a 256 "${FIRST_TARGET}/release/camera-man"
  /usr/bin/shasum -a 256 "${FIRST_TARGET}/release/cameraman-extension"
} > "${REPORT}"

echo "Reproducible unsigned binaries: ${FIRST_TARGET}/release"
echo "Digest evidence: ${REPORT}"
