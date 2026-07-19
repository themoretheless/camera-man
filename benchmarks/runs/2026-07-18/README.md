# CameraMan benchmark run: 2026-07-18

## Environment

- Hardware: Apple M4 Max, 16 logical CPUs.
- Native target: `aarch64-apple-darwin`.
- Compatibility target: `x86_64-apple-darwin` under Rosetta.
- OS: macOS 26.6.
- Compiler: `rustc 1.98.0-nightly (57d06900f 2026-05-27)`.
- Power: Battery Power, approximately 75%, discharging.
- Thermal/performance warnings: none reported while the runs were captured.

Because these runs were made on Battery Power, they are exploratory and are
not comparable with the checked-in AC Power baseline. The benchmark harness
now enforces this distinction.

## Native Three-Process Series

`compositor-aarch64-run1.json`, `run2.json`, and `run3.json` each contain 32
cases, five warm-ups, and 100 measured samples per case. Case rotations were 0,
11, and 22, for 3,200 measured compositions per process and 9,600 in total.
`compositor-aarch64-summary.json` reports the median process result; the three
raw reports remain beside it as the authoritative samples.

| Case | Median average | Median p95 | Median p99 | Median throughput |
|---|---:|---:|---:|---:|
| 720p, 1 source, Grid, nearest | 0.446 ms | 0.474 ms | 0.503 ms | 2,067 MPix/s |
| 1080p, 4 sources, Grid, nearest | 1.261 ms | 1.316 ms | 1.352 ms | 1,645 MPix/s |
| 1080p, 4 sources, PiP, bilinear | 6.605 ms | 6.743 ms | 6.855 ms | 314 MPix/s |

The fastest matrix case was 720p/two-source Grid at roughly 0.141 ms. The
slowest representative case was 1080p/four-source PiP bilinear at roughly
6.605 ms, still below one 60 fps frame interval in this isolated compositor
test. This is not an end-to-end capture-to-CMIO latency claim.

## Backend Experiments

- Corrected ARM64 resize experiment: native nearest approximately 0.267 ms,
  `fast_image_resize` approximately 0.108 ms, but all 518,400 output pixels
  differed under the current nearest-coordinate contract. The adapter is
  rejected despite its isolated speed.
- Corrected Rosetta resize experiment: native approximately 0.349 ms,
  `fast_image_resize` approximately 0.138 ms, with the same 518,400 mismatched
  pixels. The adapter is rejected.
- Metal experiment on Apple M4 Max: CPU average approximately 0.600 ms and p95
  0.661 ms; GPU plus readback average approximately 1.569 ms and p95 1.853 ms.
  The 2.615 ratio keeps the CPU path as production default.
- `parallel-compositor` substantially improved heavy bilinear cases in the
  exploratory run but added overhead to smaller nearest work. Nearest and
  bilinear now have separate pixel thresholds; a full AC-powered series is
  required before changing the default feature set.
- `compositor-x86_64-rosetta.json` records the full compatibility matrix. The
  representative 1080p/four-source Grid nearest average was approximately
  0.986 ms, while PiP bilinear was approximately 8.544 ms.

## File Guide

- `compositor-aarch64-run1.json` through `run3.json`: schema 2 raw native runs.
- `compositor-aarch64-summary.json`: process-level median/min/max summary.
- `compositor-x86_64-rosetta.json`: full Rosetta matrix.
- `compositor-aarch64-parallel.json`: full exploratory Rayon matrix before
  filter-specific threshold tuning.
- `compositor-aarch64-parallel-tuned-quick.json`: quick post-tuning evidence.
- `compositor-aarch64-pre-schema2.json`: retained historical pre-schema report;
  do not mix it into schema 2 summaries.

## Next Valid Baseline

Run the same three native processes on AC Power with no thermal warning, then
calculate process-level confidence intervals before proposing a baseline
change. Keep end-to-end latency, energy, allocation/copy counts, and the
eight-hour RSS acceptance profile as separate gates.
