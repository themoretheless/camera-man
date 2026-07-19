#!/usr/bin/env bash
set -euo pipefail

current_tag="${GITHUB_REF_NAME:-}"
baseline_tag="$({
  git tag --list 'v*' --sort=-version:refname |
    while IFS= read -r tag; do
      if [[ -n "${tag}" && "${tag}" != "${current_tag}" ]]; then
        printf '%s\n' "${tag}"
        break
      fi
    done
} || true)"

if [[ -z "${baseline_tag}" ]]; then
  echo "No earlier v* tag exists; semver baseline is not available for the first release."
  exit 0
fi

echo "Checking public Rust API against ${baseline_tag}"
cargo semver-checks check-release --baseline-rev "${baseline_tag}"
