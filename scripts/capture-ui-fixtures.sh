#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output=${1:-"$root/target/ui-fixtures"}
mkdir -p "$output"

capture() {
  name=$1
  state=$2
  locale=$3
  size=$4
  scale=$5
  rm -f \
    "$output/$name.state.ron" \
    "$output/$name.preferences.json" \
    "$output/$name.png"
  CAMERAMAN_UI_FIXTURE_STATE=$state \
  CAMERAMAN_UI_LOCALE=$locale \
  CAMERAMAN_UI_FIXTURE_SIZE=$size \
  CAMERAMAN_UI_FIXTURE_SCALE=$scale \
  CAMERAMAN_UI_SCREENSHOT_TO="$output/$name.png" \
    cargo run --locked --quiet --manifest-path "$root/Cargo.toml" --bin camera-man
  rm -f "$output/$name.state.ron" "$output/$name.preferences.json"
}

for scale in 1 2; do
  capture "workspace-${scale}x" workspace en 1120x720 "$scale"
  capture "setup-${scale}x" setup ru 920x560 "$scale"
  capture "empty-${scale}x" empty pseudo-long 920x560 "$scale"
  capture "disconnected-${scale}x" disconnected ru 1120x720 "$scale"
  capture "install-error-${scale}x" install-error en 920x560 "$scale"
  capture "running-${scale}x" running en 1440x900 "$scale"
done

# String-expansion coverage at all supported design checkpoints.
capture pseudo-minimum workspace pseudo-long 920x560 1
capture pseudo-default workspace pseudo-long 1120x720 1
capture pseudo-large workspace pseudo-long 1440x900 1

python3 "$root/scripts/verify-ui-fixtures.py" "$output"
