# Release Acceptance

The production performance gate is one fixed workload, not a portable claim:
1920x1080 BGRA at 30 fps, four 640x480 sources, Grid composition, a three-slot
mmap transport, borrowed reads, and at most one pending render job. The checked
profile is `benchmarks/acceptance/apple-m4-max-aarch64.json`.

Before the timed baseline, 120 frames traverse compose, mmap publish and
borrowed consume. This commits the fixed working set and initializes caches;
RSS budgets then measure growth after warmup. Maximum growth also includes the
macOS process high-water delta, so a transient peak between minute samples is
not hidden.

The gate requires an eight-hour run on an Apple M4 Max arm64 host. It fails on
hardware mismatch, unbounded/changed queue contracts, any torn frame, missing
or discontinuous sequence, busy transport slots, unstable RSS, excessive late
frames, or compose/publish/end-to-end p95 and p99 over budget.

Run release qualification:

```sh
scripts/acceptance-1080p30.sh
```

Run the same invariants for three seconds while developing:

```sh
scripts/acceptance-1080p30.sh --smoke
```

Smoke mode skips the eight-hour minimum and both RSS-growth assertions because
three seconds mostly measures allocator/framework warmup. It does not relax
hardware, frame-byte correctness, queue, drop, discontinuity, or latency
thresholds. Reports are written under `target/acceptance/` and are not accepted
across a different hardware profile. Only the full run qualifies memory.
