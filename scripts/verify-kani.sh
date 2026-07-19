#!/bin/sh
set -eu

cargo metadata --locked --format-version 1 >/dev/null
cargo kani --lib \
  --harness successful_mapping_contains_every_slot_start \
  --harness frame_length_is_total_and_overflow_safe \
  --harness oversized_frame_length_is_rejected \
  --harness nearest_coordinate_stays_inside_nonempty_source \
  --harness encoded_reader_state_round_trips
