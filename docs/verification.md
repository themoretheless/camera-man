# Verification profiles

The normal gate is `cargo test --all-targets` plus strict Clippy. The following
profiles cover narrower classes of failure and stay separate so unsupported
macOS FFI is never mistaken for pure-Rust verification.

- `cargo test --test protocol_loom --no-default-features` explores the atomic
  slot claim/publish/read/release interleavings with Loom.
- `scripts/verify-kani.sh` proves checked mmap/frame arithmetic, encoded slot
  ownership and nearest-neighbor coordinate bounds. It names each harness
  explicitly, so a proof added to the tree without also being named there is
  never run; `tests/repository_hygiene.rs` fails on that, and the script runs in
  CI on the daily `proofs` schedule.
- `scripts/verify-miri.sh` executes only protocol layout/metadata and padded
  frame-view tests under Miri; it does not execute CoreMediaIO or mmap FFI, and
  no workflow runs it, so a strict-provenance regression needs someone to
  remember to call it.
- `cargo fuzz run shared_protocol` mutates structured mmap header and slot
  metadata, including version and malformed length fields.
- `cargo fuzz run provisioning_profile` mutates decoded plist/entitlement data.
- `cargo test --test shared_memory_process` uses real child processes for
  crash/restart, generation, producer death, truncate/SIGBUS containment,
  replace, permission changes and fast/slow multi-reader stress.
- `cargo hack check --locked --no-default-features` plus the depth-two feature
  powerset runs on both Apple targets in CI. The pinned local ARM64 and x86_64
  matrices each cover all 15 combinations.
- The LLVM coverage job first removes stale workspace data, then merges library,
  bins and integration/process tests with all features into one LCOV report.
- `scripts/verify-semver.sh` compares the public Rust API with the previous
  `v*` tag before a tagged release; the first release reports a deliberate no-op.
- `cargo +nightly-2026-07-17 udeps --locked --all-targets --all-features`
  checks the complete target/feature graph. The 2026-07-18 local audit was clean.
- Focused mutation CI targets only shared validation, frame-integrity,
  media-contract and provisioning invariants with strong test oracles.
- `cameraman-benchmark-series` is the release-comparison profile. It requires
  AC Power, runs at least three independent randomized compositor processes,
  computes process-level bootstrap intervals and hashes one artifact set.
- `cameraman-e2e-benchmark` uses a separate Rust consumer process to measure
  compose -> file-backed mmap -> borrowed extension-input acknowledgement.
  Physical capture and CoreMediaIO delivery remain outside this local boundary.
- `cameraman-soak` defaults to eight hours and records correctness, memory,
  wakeups, pageins, raw energy counters and environment changes. Schema 4
  separates allocator allocated/in-use/fragmentation from resident/peak memory.
  Short runs verify mechanics only; they do not qualify sustained behavior.
- `cameraman-transport-benchmark` compares the same copy/borrow workload over
  mmap publication and a mutex reference. The 640x360, 100-iteration local run
  measured mmap at 1.062x mutex time, supporting mmap only at the process boundary.
- `scripts/capture-ui-fixtures.sh` plus `verify-ui-fixtures.py` decode and check
  15 deterministic captures: six core states at 1x/2x and pseudo-long text at
  minimum/default/large windows. Manual VoiceOver remains a separate RC gate.
- `representative_quality_fixture` and `summarize_quality` provide exact
  patterned inputs plus deterministic bootstrap 95% intervals. Offline VMAF
  and the subjective checklist remain additive to exact goldens.
- `scripts/build-reproducible-release.sh` performs two clean auditable release
  builds and byte-compares host/extension Mach-O files before signing.
- `scripts/audit-release-artifacts.sh` extracts the final ZIP, runs
  `cargo audit bin` on both embedded binaries and scans normalized SPDX 2.3
  with a checksum-pinned OSV-Scanner. SPDX 3.0.1 is validated separately.
- `cameraman-release-manifest` reads final plist/signature data and indexes
  toolchain, SDK, Team ID and artifact hashes. A successful production run
  requires paid credentials; ad-hoc output intentionally cannot satisfy it.

Kani and Miri are toolchain profiles, not normal Cargo dependencies. Install
them using their upstream instructions before running the scripts.
