# CameraMan performance baselines

`baselines.json` contains measured compositor baselines keyed by CPU and
target architecture. They are comparison points, not universal performance
claims. Update an entry only after reviewing pixel-quality gates and a full
matrix run on the named hardware. Baseline comparison currently requires AC
Power; a battery run exits with an environment-mismatch error instead of
reporting a false code regression.

The schema 3 compositor harness defaults to five warm-up iterations and 100
measured iterations per case. Each JSON report retains the raw samples plus
average, standard deviation, p50, p95, p99, min, max, throughput, target
architecture, build hash, macOS version, compiler, copy/allocation ledger,
fixture profile, randomized order seed, and environment snapshots before and
after the run.

Run the matrix:

```sh
CAMERAMAN_BENCH_MODE=matrix cargo bench --bench compositor --no-default-features
```

Run the reproducible AC baseline series:

```sh
cargo run --release --locked --bin cameraman-benchmark-series \
  --no-default-features -- \
  --output-dir benchmarks/runs/YYYY-MM-DD-ac
```

The runner refuses to start unless the current source matches the baseline's
required `AC Power`. It launches at least three independent benchmark
processes with saved random seeds, computes a process-level bootstrap 95%
confidence interval, and writes raw reports, summary, exact commands,
environment preflights, Git state, and SHA-256 digests into one manifest.
Battery reports remain exploratory and cannot update the AC baseline.

Summarize existing reports without rerunning them:

```sh
cargo run --release --locked --bin cameraman-benchmark-summary \
  --no-default-features -- \
  --baseline benchmarks/baselines.json \
  --output /tmp/cameraman-summary.json \
  report-1.json report-2.json report-3.json
```

Exercise the shared patterned/odd-size fixture and padded-row materialization:

```sh
CAMERAMAN_BENCH_FIXTURE=representative-patterned-v1 \
CAMERAMAN_BENCH_MODE=matrix \
  cargo bench --locked --bench compositor --no-default-features
```

Measure the separate-process mmap boundary:

```sh
cargo run --release --locked --bin cameraman-e2e-benchmark \
  --no-default-features -- --output /tmp/cameraman-e2e.json
```

That report separates compose and mmap-publish service time, cross-process
queue wait, throughput, and fixture-ready-to-extension-input acknowledgement.
It deliberately excludes physical capture, CoreVideo upload, CoreMediaIO
delivery, and the third-party consumer; those require a signed extension run.

Check representative cases against the local baseline:

```sh
CAMERAMAN_BENCH_MODE=quick CAMERAMAN_BENCH_COMPARE=1 \
  cargo bench --bench compositor --no-default-features
```

For Rosetta x86_64, add `--target x86_64-apple-darwin` to either command.

Useful controls:

- `CAMERAMAN_BENCH_ITERATIONS`: measured samples per case; default 100.
- `CAMERAMAN_BENCH_WARMUP_ITERATIONS`: warm-ups per case; default 5.
- `CAMERAMAN_BENCH_CASE_ROTATION`: left rotation of the selected case order.
- `CAMERAMAN_BENCH_CASE_ORDER`: `rotation` or `random`.
- `CAMERAMAN_BENCH_CASE_SEED`: reproducible random-order seed.
- `CAMERAMAN_BENCH_FIXTURE`: `synthetic-solid-v1` or
  `representative-patterned-v1`.
- `CAMERAMAN_BENCH_BACKGROUND_LOAD`: free-form environment note.
- `CAMERAMAN_BENCH_JSON`: output path for the complete report.
- `CAMERAMAN_BENCH_BASELINE`: alternate baseline file.

Experimental backends have separate correctness gates:

```sh
cargo bench --bench resize_backends --features resize-experiments
cargo bench --bench gpu_compositor --features gpu-compositor-experiment
cargo bench --bench compositor --features parallel-compositor
```

The resize experiment compares every pixel of a patterned frame before it may
recommend an adapter. The GPU experiment compares complete output against the
CPU golden, then measures 30 iterations after warm-up. Neither experiment
changes the production default solely because its isolated kernel is faster.

The sustained profile emits schema 3 process usage and environment data:

```sh
cargo run --release --locked --bin cameraman-soak --no-default-features
```

It defaults to eight hours and reports RSS/footprint, wakeups, pageins, raw
macOS energy counters and per-second rates. A short smoke is a correctness
check only and is not a thermal or energy qualification.

The exploratory battery methodology is recorded in
`runs/2026-07-18/README.md`. The completed native AC schema 3 series and
updated baseline rationale are in `runs/2026-07-18-ac-schema3/README.md`. The
statistical rationale and primary sources are listed in `../research.md`.
