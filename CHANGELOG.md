# Changelog

All notable CameraMan changes are recorded here. The project has not published
a stable release yet, so current work remains under `Unreleased`.

## Unreleased

### Added

- Rust-only egui desktop compositor with synthetic and multi-camera input.
- Latest-only background rendering with nearest and bilinear scaling.
- Three-slot transport with POSIX shm for bare development, App Group container
  mmap for signed bundles, and an explicit diagnostic file fallback.
- Rust CoreMediaIO system-extension prototype and activation UI.
- Structured app/extension bundling, signing diagnostics, and provisioning checks.
- Bounded persisted UI preferences and nonblocking camera discovery.
- Resizable controls, editable export path, endpoint copy, source order/health,
  waiting overlay, and separate sticky/transient status channels.
- Explicit source reorder, first-source-primary picture-in-picture composition,
  Team ID/profile matching, and copyable in-app signing diagnostics.
- Separate host/extension provisioning-profile validation and inside-out embedding.
- Same-id capture-worker leases that prevent Stop -> Start reopen races.
- Mandatory system-extension usage metadata, exact `/Applications` preflight,
  shared App Group validation, and App-Group-prefixed CMIO Mach service generation.
- Frame limits, borrowed padded views, structured errors, pipeline latency metrics,
  PPM/JSON debug artifacts, benchmarks, fuzzing, and macOS CI.
- Two dated surveys of 200 relevant repositories and primary research/platform
  sources, with recommendations 501-700 derived from the evidence.
- Reusable Core Video pools, Rec.709 attachments, absolute-deadline pacing,
  stale-frame expiry, discontinuity reporting and borrowed mmap consumption.
- Named scenes, typed source transforms, bounded transactional Undo/Redo,
  reconnect/retry policies, virtual-camera self-test and RU/EN accessible UI.
- Loom/Kani/Miri profiles, structured protocol/provisioning fuzzing,
  child-process crash/restart tests, SPDX 3.0.1 SBOM and fixed-hardware soak gate.
- Schema 3 compositor reports with shared solid/patterned fixtures, odd and
  padded inputs, raw samples, seeded case order, copy costs and before/after
  environment snapshots.
- Strict AC benchmark-series orchestration with independent processes,
  bootstrap confidence intervals, executable/report SHA-256 hashes, Git state
  and exact command manifests.
- Separate-process mmap benchmark with distinct compose/publish service time,
  queue wait, throughput and fixture-to-extension-input acknowledgement.
- Shared process-usage instrumentation and schema 4 soak reports with allocator,
  resident/peak, wakeup, pagein, raw energy-rate and environment fields.
- Per-source frame-integrity and health vectors, explicit channel backpressure,
  typed scene invalidation and centralized parser resource limits.
- Executable cross-schema contracts, mmap fault/process histories, unsafe-oracle
  registry, ownership map and an mmap-versus-mutex reference benchmark.
- Pinned Rust feature/coverage/API/dependency maintenance workflows using
  cargo-hack, cargo-llvm-cov, cargo-semver-checks and cargo-udeps.
- Stable accessibility identities, centralized UI tokens/geometry contracts,
  cancel-safe discovery/self-test/export workers, canonical atomic preferences
  and 15 decoded 1x/2x/pseudo-long UI fixture variants.
- Schema-versioned Rec.709/8-bit color wire contract in mmap v5 and spool v4,
  representative quality fixtures, bootstrap confidence intervals and isolated
  linear-light/Metal experiments.
- Reproducible auditable release builds, final-ZIP binary audit, SPDX 2.3 OSV
  scan alongside SPDX 3.0.1, Rust release manifest, layered attestations and
  rollback/self-update threat-model documentation.

### Changed

- Preview upload is capped independently at 960x540 and 30 fps while virtual
  output remains full HD.
- Frame clones use copy-on-write storage.
- Release signing failures are fatal and include actionable command context.
- The documented next milestones now prioritize the full eight-hour and
  notarized-consumer evidence, upstream dependency maintenance, and only then
  measured IOSurface/wgpu copy removal.
- Scene import is capped at 1 MiB and export uses a synced atomic replacement;
  camera recovery requires a real frame before reconnect backoff resets.
- Unsafe FFI/mmap contracts are documented and enforced by strict Clippy; raw
  Core Video writers are explicit unsafe APIs and sample fps cannot create a
  zero or wrapped CoreMedia timescale.
- File-spool protocol is v4 and shared-memory protocol is v5; both reject an
  unknown or noncanonical color contract before exposing frame pixels.

### Known Limitations

- A real extension install requires paid Apple provisioning with the System
  Extension capability and matching App Group entitlements.
- The CPU path still performs one measured mmap publish copy and one pooled
  Core Video upload copy; IOSurface/wgpu remains evidence-gated.
- The full eight-hour RSS qualification and a notarized third-party consumer
  test require the production hardware and Apple credentials.
- Any local CMIO client can connect because the available binding exposes only
  an opaque client UUID, not a code-signing identity suitable for authorization.
- The transitive nokhwa macOS stack still pulls `block 0.1.6`; an exact guard
  prevents the reviewed future-incompat exception from silently expanding.
- Self-update/network identity remains intentionally absent pending an approved
  rollback, freeze, key-rotation and partial-install threat model.
