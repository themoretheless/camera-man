# CameraMan

CameraMan is a Rust-only macOS camera compositor. It reads an ordered graph of
generated and physical-camera sources, composes them into one BGRA frame,
previews that frame in a desktop app, and publishes the latest composed output
to a Rust CoreMediaIO system extension prototype.

## What Works

- Rust library, CLI, app, and extension binaries compile.
- The app launches with `cargo run`.
- Synthetic sources compose into a 1920x1080 output frame; the display preview is independently capped at 960x540 and 30 fps.
- Real camera discovery and capture use `nokhwa`.
- AVFoundation cameras persist by their stable `uniqueID` (`uid:<value>`); discovery aliases migrate old numeric selections, scene transforms and active workers without duplicating a device.
- Camera open negotiates against the requested output geometry and rate for every decodable format before using bounded fallbacks. A hardware smoke on the LG camera selected 1920x1080 at 30 fps instead of the former 320x240 fallback.
- Cameras and generated sources can be selected together in one ordered scene.
- The app can start/stop preview, switch layout, switch fps mode, export PPM, and publish virtual-output frames.
- Named scenes persist source order, layout, output settings, missing-source policy and typed per-source crop/fit/fill/mirror/rotate/opacity/position transforms.
- Scene edits are transactional with bounded Undo/Redo: a pointer drag is coalesced into one history step instead of consuming the stack one frame at a time.
- Versioned JSON import validates and previews migrations before applying, rejects files above 1 MiB, and exports through an atomic same-directory replacement.
- Camera failures use bounded exponential reconnect with a visible manual Retry instead of deleting the source immediately; opening the device is not treated as recovery until a real frame arrives.
- The Sources -> Preview -> Output workspace has a separate setup window, bounded RU/EN control text, high-contrast state, keyboard reorder, explicit AccessKit names and automated workspace/Setup minimum-window plus Retina fixture captures. Machine-oriented diagnostic payloads remain stable English.
- Virtual Camera Self-Test publishes a color-bar pattern and waits for the extension's exact generation/sequence acknowledgement.
- Composition and virtual-frame publication run on a latest-job worker instead of the egui thread, so slow frames are replaced rather than queued.
- An autonomous Rust `MediaClock` uses the shared absolute-deadline pacer and wakes egui with a coalesced latest-only tick; the window is no longer the stream timer.
- `Frame::clone()` shares immutable pixels through copy-on-write storage; mutation detaches the buffer only when needed.
- `PipelineMetrics` reports source-read, composition and sink-send durations plus received/dropped/error source-frame counts.
- Scaling is selectable as `Fast` nearest-neighbor or `Smooth` bilinear without moving composition back onto the UI thread.
- `FrameLimits` derives the default single-frame budget from physical memory and supports explicit per-call limits.
- Errors expose stable codes, user-facing recovery text and chained diagnostic context.
- Typed source descriptors, source order, layout, scaling, fps, export path and virtual-output preference persist in a bounded, versioned, atomically replaced canonical JSON file with eframe migration fallback; camera handles and frame buffers never do.
- Camera discovery runs on a dedicated worker, so refreshing devices does not block egui.
- A process-local camera lease serializes Stop -> Start for the same id without joining a potentially blocked capture call on egui.
- The control panel is resizable and scrollable; source rows show composition order and real-camera health.
- Selected sources can move earlier/later with keyboard-accessible controls; the first source is primary in the new PiP layout.
- Sticky errors, transient confirmations and derived live state have separate UI slots.
- The resolved RAM/file endpoint has a copy action and the PPM destination is editable.
- `FrameView` accepts borrowed padded rows and converts to a tight owned frame only when requested.
- Pipeline metrics distinguish dropped, errored and repeated stale source frames and report capture-to-sink latency.
- PPM sequences use timestamped filenames and structured JSON metadata sidecars.
- macOS CI runs strict checks, CLI smoke, release bundling, plist validation and codesign verification.
- Supply-chain CI runs pinned `cargo-deny 0.20.2` and `cargo-audit 0.22.2`; every third-party Action is pinned to an immutable commit.
- Production release CI builds host and extension twice, compares unsigned Mach-O bytes, embeds auditable dependency metadata, uses a temporary keychain, hardened runtime, secure timestamps, entitlement allowlists, notarization/stapling, Gatekeeper assessment, SHA-256, SPDX 2.3/3.0.1 SBOMs, final-archive/OSV scans, a machine-readable manifest and separate hosted attestations.
- Repository hygiene tests reject tracked Swift remnants, build/IDE metadata and private signing artifacts.
- App and extension plist files are generated from typed `plist::Value` dictionaries and share the same version/identity constants as runtime code.
- Virtual-camera dimensions, default/range fps, and visible fps presets come from one shared Rust configuration used by the app, transports, and extension.
- Release bundling treats every signing failure as fatal and includes the exact `codesign` command, status and stderr in the error.
- `version`, `bundle --help`, and `diagnose-extension` expose build, signing, provisioning, entitlement and installed-extension state without launching the UI.
- Provisioning and actual code signatures expose Team IDs; installable bundling rejects a certificate/profile mismatch before embedding the profile.
- `SharedFrameSink` publishes through one three-slot mmap protocol: POSIX shared memory for bare development binaries and an App Group container file for signed bundles, with no full-frame file rewrite per tick.
- `cameraman-extension` reads the newest RAM frame and sends CoreMediaIO `CMSampleBuffer` frames.
- The extension reuses a bounded `CVPixelBufferPool`, attaches Rec.709 SDR metadata, paces against absolute monotonic deadlines, expires stale producer frames, and reports generation/sequence discontinuities.
- The mmap reader exposes an RAII borrowed slot, so the extension copies directly into a pooled Core Video buffer without an intermediate owned `Frame`.
- Media contracts now include schema-versioned Rec.709 primaries/transfer/matrix/range/alpha/8-bit semantics, clean aperture, pixel aspect ratio, orientation, mirror state, format epochs, and separate media/monotonic/wall-clock types. mmap v5 and spool v4 fail closed on unknown color contracts.
- Per-source integrity and health contracts link restart generation, sequence, timestamps, discontinuities, drops, jitter, reconnect, negotiated format and consumer acknowledgement without reducing diagnosis to one color.
- Every runtime channel has an explicit bounded slow-consumer policy; typed scene invalidation avoids recomputing output, persistence or undo state for unrelated UI changes.
- Parser limits are centralized for scene/preferences JSON, provisioning profiles, frame spools and benchmark reports before unbounded deserialization or allocation.
- Stable `tracing` spans and native macOS signposts cover capture, compose, publish, consume, pixel-buffer fill, and sample delivery. Bounded diagnostics expose p50/p95/p99/max, typed drop counters, copy/allocation cost per output frame, and a privacy-redacted JSON export.
- The compositor benchmark covers 720p/1080p, 1/2/4/8 sources, Grid/PiP, nearest/bilinear, and both native ARM64 and Rosetta x86_64. Schema 3 reports retain raw samples, distribution statistics, fixture/order metadata, copy costs and before/after environment snapshots; a Rust runner produces process-level bootstrap intervals and rejects AC baseline work on battery.
- The old file spool remains available only as an explicit diagnostic fallback.
- The extension generates placeholder frames when the app has not published a frame yet.
- The app can request system-extension activation and show the activation status.
- The CLI can build `target/CameraMan.app` and embed the `.systemextension`.
- Automated suites cover frame/media contracts, migrations, deterministic reducers, architecture boundaries, pooled/borrowed transport behavior, concurrent no-tearing publication, quality gates, CLI packaging contracts, provisioning, and extension lifecycle/timing rules.
- CI covers meaningful feature powersets on ARM64/x86_64, merged LLVM coverage, focused mutation tests and public-API semver; scheduled maintenance runs pinned `cargo-udeps`, while a quarterly workflow records stale dependencies, duplicates and Rust/objc2/Xcode/macOS SDK drift.
- Two dated research surveys cover 200 relevant repositories, 40 representative deep reads, primary scientific/platform sources, and 200 research-backed recommendations.

## Important Limits

- The default bundle is ad-hoc signed. It launches, but macOS will not install the system extension from it.
- Installing the extension requires a provisioning profile that grants `com.apple.developer.system-extension.install`.
- Signing with an Apple Development certificate alone is not enough; without a matching provisioning profile, macOS kills the app with `No matching profile found`.
- A distribution build needs host-app and extension-target profiles with one common exact App Group. The bundler validates and embeds all three inputs, but this machine has no paid profiles with which to exercise a real install.
- The virtual camera declares a 15–60 fps CMIO range; malformed or external transport values are clamped to that contract.
- Shared memory removes disk I/O and the owned-reader copy, but one measured full-frame mmap publish copy and one pooled Core Video upload copy remain. The Rust IOSurface/Metal bridge and wgpu compositor stay experimental until they reduce that ledger on a real consumer path.
- Rec.709 attachments are set in the extension, but propagation through third-party camera consumers still needs a real signed-extension install on hardware with paid host/extension profiles.
- `fast_image_resize` is faster in the isolated ARM64/x86_64 experiment, but its nearest sampling convention differs at every tested output pixel; it is intentionally not the default adapter.
- The checked-in soak command defaults to eight hours, but repository verification runs only a short smoke profile. Release qualification must still execute the full duration on the target hardware.
- The native three-process AC series completed with stable power snapshots,
  9,600 raw samples and no regressions. Rosetta baselines remain unchanged
  because this series did not execute an x86_64 process.
- The default frame budget is 1/16 of physical RAM, clamped to 64 MiB–1 GiB; `CAMERAMAN_MAX_FRAME_BYTES` overrides it for controlled deployments.
- Every CoreMediaIO unsafe block now has an enforced local invariant, but
  callback panic containment and enforceable client authorization still need
  production hardening.
- Network capture is not implemented. Its Sans-I/O boundary, threat model and
  fuzz/resource gates are documented so no async/RTP/HTTP stack enters the
  default dependency graph without an approved requirement.
- Self-update is deliberately absent. Network identity, TUF/TLS and updater
  dependencies are architecture-test blocked until rollback, freeze,
  key-rotation and interrupted-install threats have an approved design.
- Distribution is not yet claimed: the full eight-hour soak, paid-profile
  install/notarization, real third-party CMIO consumer and VoiceOver smoke need
  evidence from the same release candidate. See `docs/release-readiness.md`.

## Three-Iteration Source Refactor

1. Scene/preferences schema v4 replaced the global input mode and parallel id lists with ordered typed `SourceDescriptor` values. Sequential migration preserves old camera selections and remaps transform keys.
2. Source selection, ordering, scenes, Undo/Redo and preview preparation now operate on one heterogeneous graph, so generated sources and physical cameras can occupy the same composition.
3. Stream cadence moved from egui elapsed-time checks to an owned Rust `MediaClock` using `DeadlinePacer`; the production binary has one asynchronous render path, while `PipelineEngine` lives in a standalone SDK example.

## Run

Launch the app:

```bash
cargo run
```

Launch explicitly:

```bash
cargo run -- app
```

Print status:

```bash
cargo run -- status
```

Print the package version or extension diagnostics:

```bash
cargo run -- version
cargo run -- diagnose-extension
```

List cameras:

```bash
cargo run -- list-cameras
```

Inspect the formats that the backend can actually open for one stable camera
id (the probe opens the device briefly):

```bash
cargo run --example camera_formats -- 'uid:<value>'
```

Capture one real camera frame:

```bash
cargo run -- capture-demo
```

Render a synthetic demo frame:

```bash
cargo run -- demo
```

Render three frames through the standalone synchronous SDK example:

```bash
cargo run --example pipeline_demo
```

Pipeline PPM files are named `frame-<sequence>-<timestamp>.ppm`; each has a
same-name `.json` sidecar containing source, sequence, timestamp, dimensions
and pixel format.

Build the app bundle:

```bash
cargo run --release -- bundle
open target/CameraMan.app
```

Reproduce the two unsigned release binaries and verify embedded dependency
metadata (requires `cargo-auditable 0.7.5`):

```bash
scripts/build-reproducible-release.sh
cargo audit bin \
  target/reproducibility/first/release/camera-man \
  target/reproducibility/first/release/cameraman-extension
```

The protected release workflow then signs one compared pair and runs
`scripts/audit-release-artifacts.sh` against the final ZIP and SPDX SBOM. See
`docs/release-evidence.md`, `docs/release-rollback.md`, and
`docs/threat-model-self-update.md` before changing that chain.

Show the signing variables and executable release checklist:

```bash
cargo run -- bundle --help
```

Build only the extension bundle:

```bash
cargo run --release -- bundle-extension
```

Run checks:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo hack check --locked --no-default-features
cargo hack check --locked --feature-powerset --depth 2 --exclude-features all-experiments
cargo +nightly-2026-07-17 udeps --locked --all-targets --all-features
```

Run the full-HD compositor benchmark:

```bash
# Reproducible release comparison; requires AC Power:
cargo run --release --locked --bin cameraman-benchmark-series \
  --no-default-features -- \
  --output-dir benchmarks/runs/YYYY-MM-DD-ac

# One exploratory matrix:
CAMERAMAN_BENCH_MODE=matrix cargo bench --bench compositor --no-default-features

# Patterned odd-size inputs plus padded-row materialization:
CAMERAMAN_BENCH_FIXTURE=representative-patterned-v1 \
CAMERAMAN_BENCH_MODE=matrix \
  cargo bench --locked --bench compositor --no-default-features

# Crop/rotate/mirror/fill/opacity hot paths:
CAMERAMAN_BENCH_MODE=transforms \
CAMERAMAN_BENCH_ITERATIONS=100 \
  cargo bench --locked --bench compositor --no-default-features

# Legacy one-process quick guard:
CAMERAMAN_BENCH_MODE=quick CAMERAMAN_BENCH_COMPARE=1 \
  cargo bench --bench compositor --no-default-features
CAMERAMAN_BENCH_MODE=quick CAMERAMAN_BENCH_COMPARE=1 \
  cargo bench --bench compositor --no-default-features --target x86_64-apple-darwin
```

The matrix defaults to five warm-ups and 100 measured iterations per case.
The series runner starts at least three randomized independent processes,
saves every seed, computes bootstrap 95% intervals, and hashes the reports,
summary, immutable baseline input, and executable into one manifest. Baseline
comparison requires the recorded power source. See
[`benchmarks/README.md`](benchmarks/README.md),
[the exploratory battery run](benchmarks/runs/2026-07-18/README.md), and
[the native AC series](benchmarks/runs/2026-07-18-ac-schema3/README.md) before
interpreting or updating the numbers.

The current bottleneck audit, transform before/after results, copy-bandwidth
budget, remaining risks, and a small-step Rust-only greenfield design are in
[`docs/performance-review.md`](docs/performance-review.md).

Export privacy-redacted versioned diagnostics or run the soak profile:

```bash
cargo run --bin cameraman-diagnostics --no-default-features
cargo run --release --bin cameraman-e2e-benchmark \
  --no-default-features -- --output /tmp/cameraman-e2e.json
cargo run --release --bin cameraman-soak --no-default-features
# Short verification only:
cargo run --release --bin cameraman-soak --no-default-features -- \
  --duration-seconds 2 --sample-seconds 1
```

Compare the shared mmap boundary with the same mutex copy/borrow workload:

```bash
cargo run --release --locked --bin cameraman-transport-benchmark \
  --no-default-features -- --iterations 100 --warmup 20
```

Compare the current nearest scaler with `fast_image_resize` in isolation:

```bash
cargo run --release --manifest-path tools/resize-bench/Cargo.toml
cargo run --release --manifest-path tools/resize-bench/Cargo.toml \
  --target x86_64-apple-darwin
```

Build or run the frame-validation fuzz target:

```bash
cargo check --manifest-path fuzz/Cargo.toml
cargo install cargo-fuzz
cargo +nightly fuzz run frame_new_checked
```

Run the custom trait examples:

```bash
cargo run --example custom_source
cargo run --example custom_sink
```

## Frame Transport

Bare development binaries use RAM-only POSIX shared memory:

```bash
cargo run
```

The app publishes into a three-slot latest-frame ring named
`/cameraman-frame-v4-<uid>`. The extension maps the same object, locks only the
slot it is reading, and skips stale frames. Protocol v4 pairs the writer PID
with its macOS process-start token so PID reuse cannot revive a crashed writer,
and records the consumer PID/generation/sequence acknowledgement used by the
in-app self-test.
The app UI shows the resolved RAM endpoint under Output.

An installable signed bundle instead maps the same slot protocol through
`CameraMan/frames-v5.mmap` inside the shared App Group container. This avoids
the different user IDs used by the host app and CMIO role process while
keeping writes in mapped pages rather than rewriting a full frame file.

The previous disk transport is retained only for diagnostics:

```bash
CAMERAMAN_FRAME_TRANSPORT=file cargo run
```

`CAMERAMAN_FRAME_SPOOL` changes the fallback file path.
`CAMERAMAN_SHARED_MEMORY_NAME` changes the POSIX shared-memory name for manual
development. Bundled App Group transport takes precedence so the separately
launched system extension cannot diverge from the host environment.

## System Extension Install

For local UI and shared-memory testing, the ad-hoc bundle is enough:

```bash
cargo run --release -- bundle
open target/CameraMan.app
```

For a real system-extension install, the app must be:

- built as a `.app`;
- launched from an installed bundle such as `/Applications/CameraMan.app`;
- signed with a valid Apple signing identity;
- embedded with a provisioning profile matching `com.cameraman.rust`;
- granted `com.apple.developer.system-extension.install`;
- paired with an extension profile that shares an exact App Group;
- approved by the user in System Settings after activation is requested.

`cargo run --release -- bundle` validates the supplied provisioning profile before it
embeds it: the profile must decode with `security cms`, match
`com.cameraman.rust`, grant `com.apple.developer.system-extension.install`,
and expose the same Team ID as the actual app signature.
When an installable app profile is supplied,
`CAMERAMAN_EXTENSION_PROVISIONING_PROFILE` is also mandatory; it must match
`com.cameraman.rust.extension`, grant app sandbox, carry that same Team ID, and
share at least one non-wildcard App Group with the host profile. The selected
group is signed into both bundles, stored in their plist metadata, used for the
group-container mmap path, and prefixes the generated CMIO Mach service name.
`CAMERAMAN_APP_GROUP` optionally selects one when profiles share several.
The extension plist also contains Apple's required system-extension usage
description. The extension profile is embedded before the inner bundle is signed.
Supplying only one profile, or supplying profiles without `CODESIGN_IDENTITY`,
is rejected before the existing app bundle is replaced. The completed nested
bundle is verified with strict `codesign` before success is reported.
It also fails immediately if any release `codesign` step fails. A debug ad-hoc
bundle may continue with a warning so local UI work is still possible; an
explicit `CODESIGN_IDENTITY` is always strict.

Build with explicit signing inputs:

```bash
CODESIGN_IDENTITY="Apple Development: Name (TEAMID)" \
CAMERAMAN_PROVISIONING_PROFILE="/path/to/CameraMan.provisionprofile" \
CAMERAMAN_EXTENSION_PROVISIONING_PROFILE="/path/to/CameraManExtension.provisionprofile" \
CAMERAMAN_APP_GROUP="TEAMID.com.cameraman.shared" \
cargo run --release -- bundle
```

The app exposes an `Install extension` button, but it stays disabled (with an
explanatory tooltip) unless the running bundle is under `/Applications`,
contains the expected extension, passes strict nested-signature verification,
has the signed `system-extension.install` entitlement, contains valid host and
extension profiles, has a valid usage description and App-Group-prefixed Mach
service, and all signature/profile Team IDs agree.

Private `.key`, `.p12`, certificate, CSR and provisioning-profile files are
ignored globally. Keep them outside the repository where possible; the
`signing/` directory is also ignored as a whole.

Inspect the current bundle and macOS registration state:

```bash
cargo run -- diagnose-extension
```

The report includes the selected identity, profile validation result, app,
extension and profile Team IDs/matches, expected and embedded entitlements,
activation-location eligibility, bundle paths, and
`systemextensionsctl list` output. It remains successful when an artifact is
missing so it can describe the broken state.

Platform references: [Apple's camera-extension guide](https://developer.apple.com/documentation/coremediaio/creating-a-camera-extension-with-core-media-i-o),
[required system-extension usage description](https://developer.apple.com/documentation/bundleresources/information-property-list/nssystemextensionusagedescription),
and [App Group container/IPC guidance](https://developer.apple.com/documentation/xcode/configuring-app-groups).

### Troubleshooting `No matching profile found`

This message means the app signature asks for a restricted entitlement that
the embedded provisioning profile does not authorize for the app identifier.
Do not add the entitlement to an ad-hoc or certificate-only build. Use a paid
Apple Developer Program team with the System Extension capability, export a
profile for `com.cameraman.rust`, pass it through
`CAMERAMAN_PROVISIONING_PROFILE`, provide the matching extension-target profile
through `CAMERAMAN_EXTENSION_PROVISIONING_PROFILE`, then rebuild. Confirm
`provisioning.profile=valid` and inspect both entitlement sections with
`cargo run -- diagnose-extension` before requesting activation again.

## Repository Layout

```text
src/main.rs + src/main/ CLI facade; bundle, signing, diagnostics, typed plist
src/app.rs + src/app/   app composition root, reducer, capture/readiness, egui UI
src/app_preferences.rs  bounded persisted UI preferences only
src/camera_discovery_worker.rs nonblocking camera enumeration
src/render_worker.rs    latest-job compositor and virtual-output worker
src/diagnostics.rs      spans/signposts, histograms, drops, event ring, JSON export
src/media_contract.rs   color/alpha/aperture/aspect/transform frame contract
src/media_time.rs       typed clock domains and injectable Clock boundary
src/format_negotiation.rs fixed output contract and atomic format epochs
src/frame_integrity.rs per-source generation/sequence/time/drop classifier
src/source_health.rs   typed health vector and compact non-color summary
src/backpressure.rs    bounded channel capacities and slow-consumer policies
src/invalidation.rs    typed scene dependency/invalidation graph
src/parser_limits.rs   shared pre-deserialization resource budgets
src/scene_schema.rs     versioned scene document, missing policy and migrations
src/source_transform.rs typed crop/fit/fill/mirror/rotate/opacity/position model
src/wire.rs             versioned transport DTO values separate from domain types
src/benchmarking.rs     shared fixtures, environment capture, statistics and reports
src/performance.rs      copy ledger plus resident/peak/allocator usage snapshots
benches/compositor.rs   schema 3 ARM64/x86_64 matrix and guarded baseline comparator
src/bin/cameraman-benchmark-series.rs strict AC process runner and artifact manifest
src/bin/cameraman-benchmark-summary.rs process-level bootstrap summary
src/bin/cameraman-e2e-benchmark.rs separate-process mmap boundary benchmark
src/bin/cameraman-soak.rs sustained thermal, memory, wakeup and energy profile
src/bin/cameraman-transport-benchmark.rs mmap vs mutex reference workload
benchmarks/             raw runs, AC baselines and fixed release acceptance profile
tools/resize-bench/     dependency-isolated ARM64/x86_64 scaler experiment
fuzz/fuzz_targets/      libFuzzer target for frame validation
tests/golden/           tiny compositor golden images
tests/contract_invariants.rs executable wire/media/schema/capability contracts
tests/shared_memory_process.rs crash/restart/truncate/replace/permission faults
examples/               executable custom source and sink implementations
.github/workflows/      strict, mutation and notarized production release CI
docs/ownership-map.md   source-of-truth ownership and newcomer reading route
docs/unsafe-invariants.md unsafe boundary-to-oracle registry
src/lib.rs              public module exports
src/config.rs           output and virtual camera config
src/app_group.rs        signed bundle group discovery and shared container path
src/error.rs            shared error and capture classification
src/frame.rs            tight owned frames, padded borrowed views, metadata
src/layout.rs           row, column, grid cell calculation
src/render.rs           pure Rust compositor
src/camera.rs           camera discovery/source traits and synthetic source
src/capture.rs          real camera discovery/capture through nokhwa
src/virtual_camera.rs   virtual camera sink trait and memory sink
src/frame_transport.rs  explicit file-transport fallback and wire frame
src/shared_memory_transport.rs + directory protocol/mmap/writer/reader facade
src/transport.rs        RAM/file mode selection and app/extension adapters
src/pipeline.rs         source -> compose -> sink orchestration
src/ppm.rs              PPM writer and PPM sequence sink
src/system_extension.rs OSSystemExtensionRequest activation bridge
src/provisioning.rs     structured provisioning-profile validation
src/extension_main.rs + src/extension/ provider objects, pump, timing, pixel pool

architecture.md         architecture, SOLID/DRY split, design notes, 3 iterations
research.md             200 repositories, primary sources, evidence synthesis
recommendation.md       exactly 700 review items, improvements, problems, and next steps
CHANGELOG.md            unreleased milestone and known limitations
CONTRIBUTING.md         small-change workflow and executable release checklist
```

CameraMan is dual-licensed under `MIT OR Apache-2.0`. The canonical texts are
`LICENSE-MIT` and `LICENSE-APACHE`; `THIRD_PARTY_NOTICES.md` is generated from
the locked macOS dependency graph with `cargo-about`.

## Supply Chain And Release

Run local dependency gates and regenerate distribution metadata:

```bash
cargo deny check --hide-inclusion-graph
cargo audit
scripts/generate-third-party-notices.sh
python3 -m venv target/spdx-tools-venv
target/spdx-tools-venv/bin/python -m pip install -r scripts/requirements-sbom.txt
PYTHON=target/spdx-tools-venv/bin/python scripts/generate-sbom.sh
```

Temporary exceptions, ownership, expiry and removal conditions are recorded in
`docs/dependency-exceptions.md`. The production workflow requires Developer ID
host/extension profiles with one shared App Group and App Store Connect notary
credentials. It never writes those materials into the repository. See
`docs/acceptance.md` for the fixed 1080p30/four-source release gate and
`docs/architecture-decisions.md` for dependency decisions.

## Architecture

The project is split around stable Rust boundaries:

```text
FrameSource
  -> CapturedFrame
  -> latest RenderJob
  -> Compositor worker
  -> Frame
  -> VirtualCameraSink
```

Platform work is isolated:

- real camera input implements `FrameSource`;
- virtual output implements `VirtualCameraSink`;
- app-to-extension mode selection lives in `transport.rs`;
- RAM transport facade and its protocol/mmap/writer/reader modules live under `shared_memory_transport`;
- UI-independent composition and transport publication live in `render_worker.rs`;
- the old file fallback stays isolated in `frame_transport.rs`;
- system-extension activation lives in `system_extension.rs`;
- CoreMediaIO provider code lives under `src/extension/`; `extension_main.rs` is only the platform entry point.

See [architecture.md](architecture.md) for the full SOLID/DRY map and learning
path, [research.md](research.md) for the evidence survey, and
[recommendation.md](recommendation.md) for the 700-item backlog.

## Design Direction

CameraMan should feel like a focused desktop utility:

- dense controls, not a marketing page;
- preview first, diagnostics second;
- checkboxes for sources;
- segmented controls for mode/layout/scaling/fps;
- status text for real state, not decorative copy;
- clear warnings for signing, camera access, and extension approval.

Current UI limits:

- reorder deliberately uses accessible up/down commands rather than drag and drop;
- rendered cells have no optional source-name overlays;
- a camera driver that hangs inside an underlying `open()` call cannot be cancelled by nokhwa.

Implemented research-backed design:

- preserve the Sources -> Preview -> Output hierarchy;
- move rare signing/install details into a dedicated setup surface;
- keep status compact but expose p95 latency, drop and stale state;
- edit source transforms through one selected-source inspector;
- preserve keyboard traversal, visible focus, non-color status and 28x28 frequent targets;
- gate RU/EN workspace/Setup, Retina and compact-window visual fixtures in macOS CI.

## Next Milestones

1. Release evidence: execute the eight-hour fixed-hardware acceptance gate and a notarized paid-profile install through a real third-party consumer.
2. Dependency maintenance: remove `block 0.1.6` when a compatible camera backend release exists; keep the exact exception guard until then.
3. Copy reduction: use the ledger to evaluate IOSurface/wgpu without weakening the fixed CPU correctness fallback.
4. Capture boundary: replace or wrap the backend only if hardware tests can provide a genuinely cancelable camera-open operation.

## Three Iterations

Iteration 1: Rust core

- frame model;
- layout calculation;
- compositor;
- pipeline;
- PPM output;
- core tests.

Iteration 2: Real input and app

- background camera discovery;
- threaded capture;
- multi-camera real mode;
- nonblocking app preview with a 960x540/30-fps display path;
- persisted user preferences and a resizable control panel;
- explicit source reorder and first-source-primary PiP layout;
- fps controls;
- status bar and export.

Iteration 3: Virtual output and packaging

- three-slot shared-memory transport with explicit file fallback;
- Rust CoreMediaIO extension;
- system-extension activation request;
- app and extension bundling;
- development signing;
- documented production signing gap.

### Earlier Three-Pass Audit: Items 551-600

This completed milestone was split into three reviewable passes:

- Pass one completed items 551-600: protocol model checking, Kani/Miri/fuzz
  profiles, cross-process tests, the three-column workspace, scenes,
  reconnect, accessibility, release security, SBOM and acceptance tooling.
- Pass two exercised adversarial states and fixed bounded/atomic scene I/O,
  drag history coalescing, reconnect reset before the first real frame,
  warmup-aware acceptance accounting, isolated UI fixture storage and a real
  Setup-window screenshot path.
- Pass three reviewed the resulting diff, documented every Rust unsafe block,
  made raw pixel-buffer writers explicitly unsafe, denied future undocumented
  unsafe blocks, and clamped CoreMedia sample timescales so zero fps cannot
  cross the FFI boundary.

The final local matrix passes 120 library tests, 46 app tests, 7 extension
tests, 17 integration/process/property tests and 1 benchmark-runner test
(191 total). Five Kani
harnesses and six Miri tests pass; all three fuzz targets completed 2,000-run
smokes. Four EN/RU, minimum-window, Setup and Retina fixtures pass structural
and visual review. `cargo-deny`, RustSec, strict Clippy, rustdoc, shell/Python
syntax, workflow YAML, plist validation and strict bundle signature checks all
pass. The latest 1080p30/four-source smoke produced 90 complete frames, zero
torn frames and 5.89 ms p95 end-to-end latency.

`target/CameraMan.app` is therefore buildable and launchable as an arm64 ad-hoc
bundle. Installing its system extension remains a production-signing task:
the full eight-hour RSS gate, paid host/extension provisioning profiles,
notarization credentials and a real third-party consumer cannot be replaced by
a local ad-hoc run.

### Additional Three-Pass Audit: Items 601-700

- Pass one validated the second 100-repository set, sequential 1-700 numbering,
  benchmark percentile math and every Rust target/feature. It fixed effective
  case rotation in schema 2 JSON and incorrect experimental feature names in
  the benchmark guide.
- Pass two recomputed all statistics from 9,600 raw samples, checked local
  document links and reproduced the intentional AC-baseline rejection on
  Battery Power. Exploratory battery results remain separate from release
  baselines.
- Pass three ran strict Clippy/rustdoc, rebuilt and verified the arm64 app and
  extension, scanned for non-Rust source, and recaptured four Retina UI states.
  Visual review removed the meaningless collapsible state from Setup and found
  no text overlap in the workspace, compact Russian, or Setup fixtures.

The second survey resolves all 100 repositories without duplicates, archives,
or entries below 500 stars. It contributes 1,237,647 stars as a discovery
signal, not an adoption score; 53 entries are primarily Rust. The complete
methodology and measured results are in [research.md](research.md),
[the exploratory battery run](benchmarks/runs/2026-07-18/README.md), and
[the native AC schema 3 series](benchmarks/runs/2026-07-18-ac-schema3/README.md).

### Benchmark Hardening: Items 611-620

- Iteration one ran the complete CI test profile and all 32 compositor cases.
  Together with the final runner-contract test it passed 191 tests. Targeted
  release smokes also validated the shared patterned fixture, padded-row
  accounting, separate-process acknowledgement, and schema 3 soak report.
- Iteration two passed `cargo fmt --all --check`, all-targets/all-features
  Clippy with warnings denied, and rustdoc with warnings denied.
- Iteration three rebuilt the arm64 app and extension, validated both plist
  files and the deep strict ad-hoc signature, then recaptured and visually
  reviewed four 2x EN/RU/default/minimum/Setup UI fixtures.
- Final review stopped process aggregation from combining different build
  hashes, compilers, OS versions, iteration policies, or an in-run power-source
  change. It also moved copy-stage attribution out of generic `FrameView` code
  so owned mmap materialization cannot count one physical copy twice.

The native AC series completed three randomized processes with 9,600 raw
samples. All report, summary, baseline-input and executable hashes verify; two
of three compared cases improved, one showed no significant change, and none
regressed. The three native M4 Max baselines were updated to process means.
Physical capture -> CoreVideo -> CoreMediaIO -> third-party-consumer timing
remains gated on a properly signed and provisioned extension.
