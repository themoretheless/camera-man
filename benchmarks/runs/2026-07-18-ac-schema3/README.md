# 2026-07-18 native AC schema 3 series

This is the first process-level schema 3 compositor series on the labelled
Apple M4 Max host.

## Contract

- Target: native `aarch64`
- Power: `AC Power` before and after every process
- Fixture: `synthetic-solid-v1`, matching the baseline contract
- Processes: 3, each with a distinct saved random-order seed
- Cases: 32 per process
- Samples: 100 measured plus 5 warm-ups per case
- Raw samples: 9,600 total
- Bootstrap: 10,000 deterministic resamples, 95% confidence interval

`manifest.json` records structured argv/environment values, Git revision and
dirty state, every preflight, the immutable `baseline-input.json`, and SHA-256
digests for the baseline input, reports, summary, and benchmark executable.

## Compared Cases

| Case | Process mean | 95% CI | Previous baseline | Decision |
|---|---:|---:|---:|---|
| `720p_1_grid_nearest` | 0.355291 ms | 0.348324-0.362588 ms | 0.368 ms | Improvement |
| `1080p_4_grid_nearest` | 1.035713 ms | 1.032324-1.037527 ms | 1.018 ms | No significant change |
| `1080p_4_pip_bilinear` | 6.515521 ms | 6.505763-6.521505 ms | 6.686 ms | Improvement |

There were no regressions. The three native checked-in baselines were updated
to the process means after the full matrix, raw-data validation, environment
checks, and quality/visual gates passed. Rosetta values were not changed
because this series did not execute an x86_64 process.

The baseline is still a fixed point estimate. Ratio intervals in
`compositor-summary.json` include variation from the current three processes,
not uncertainty from the older baseline measurement.
