#!/bin/sh
set -eu

export MIRIFLAGS="${MIRIFLAGS:--Zmiri-strict-provenance}"
cargo metadata --locked --format-version 1 >/dev/null
CARGO_MIRI="$(rustup which --toolchain nightly cargo-miri)"
"$CARGO_MIRI" miri test --no-default-features --lib \
  shared_memory_transport::protocol::tests
"$CARGO_MIRI" miri test --no-default-features --lib \
  shared_memory_transport::validation::tests
"$CARGO_MIRI" miri test --no-default-features --lib \
  frame::tests::padded_frame_view_converts_to_tightly_packed_pixels
