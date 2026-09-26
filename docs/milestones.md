# Engineering Milestones

## Milestone A: Core correctness

**Complete.** Bounded `CVPixelBufferPool`, Rec.709 tags, monotonic deadlines,
cadence negotiation, stale/restart generations, process-start identity,
consumer acknowledgement, three-slot ownership, and multi-reader stress tests
are implemented.

## Milestone B: Verification and observability

**Complete.** Loom state-machine models, Kani bounds, Miri helpers,
structure-aware fuzz targets, child-process mmap tests, tracing/signposts,
bounded percentiles, drop reasons, diagnostics export, and scheduled mutation
tests are present.

## Milestone C: Measured CPU work

**Complete for the current CPU path.** Shared immutable frames, reusable render
outputs and preview buffers, persistent mmap mappings, borrowed reads, pooled
Core Video uploads, copy/allocation ledgers, scaler experiments, Rayon feature
gates, and ARM64/x86_64 baselines are implemented. Optional SIMD/Rayon paths do
not become defaults without a matching quality and acceptance win.

## Milestone D: GPU and scene workflow

**Partially complete.** Named scenes, typed transforms, transactional edits,
Undo/Redo, import/export migration, missing-source policy, reconnect, and
wgpu/Metal experiments exist. IOSurface/Metal production publication remains
deferred until it reduces measured copies on a signed real-consumer path and
passes the same acceptance gate.

## Milestone E: Ranked development backlog

Ranked by measured payback per unit of evidence, not by the size of the idea.
One pull request per item, and the next item does not start before the evidence
named in this item's own row is recorded. `docs/performance-review.md` holds the
profiling detail behind these numbers; `docs/release-readiness.md` holds the
wording of the release gates.

| Rank | Item | Measured problem today | Readiness evidence |
|---:|---|---|---|
| 1 | Cancellable capture boundary | `nokhwa::Camera::frame()` cannot be cancelled. `camera_open_timeout()` (`src/capture.rs:52`, 20 s default) and `frame_gap_timeout()` (`src/capture.rs:180`) detect a stall, but nothing returns the parked thread or the device, so Stop -> Start is not bounded | A `FrameSource` adapter (`src/camera.rs:51`) whose `stop()` returns within a stated bound while a read is parked, proven by a hung-read test; existing reconnect tests unchanged |
| 2 | Dependency maintenance | `block 0.1.6` (future-incompatible), `paste 1.0.15` (`RUSTSEC-2024-0436`) and `spin 0.9.8` (yanked) enter the graph only through `nokhwa 0.10.11`, verified with `cargo tree --invert` | All three exceptions retired in `docs/dependency-exceptions.md`, and `scripts/verify-future-incompat.sh` clean |
| 3 | Eight-hour soak report | The gate is open, not unbuilt: `src/bin/cameraman-soak.rs` already defaults to eight hours and reports RSS high-water growth, physical footprint growth and missed deadlines. `scripts/acceptance-1080p30.sh` runs it with `--no-default-features`, so the signed App Group path is unsoaked | Report JSON stored under `benchmarks/runs/<date>/` for the mmap backend, thresholds asserted by `scripts/evaluate-acceptance.py` |
| 4 | Real-consumer profile | No measurement exists for physical camera -> signed CMIO -> third-party consumer; every current end-to-end run stops before the platform-specific segment | Instruments and signpost capture from FaceTime, OBS or QuickTime under a paid Developer ID profile, recorded against one commit. Externally blocked on profile and clean hardware, and a blocked gate stays visible rather than passing on unit tests |
| 5 | IOSurface-backed publication | mmap publish and CoreVideo upload remain full copies: 8.29 MB per frame is the bandwidth floor. GPU composition with a mandatory readback measured 3.79x slower than CPU | The copy ledger shows a measured byte reduction on the signed path with no CPU readback; the CPU compositor stays the oracle and the fallback; only that evidence promotes work out of `metal-interop-experiment` |
| 6 | Headless media runtime | The UI still orchestrates capture and render: `request_render` is called from five files, `refresh_scene_render` from three UI files at twelve sites, `invalidate_render_epoch` at six sites | The media loop advances without an `egui` context and `CameraManApp` keeps only command and observer state; cadence tests unchanged |
| 7 | Preview conversion cost | Up to 2.07 MB of CPU `BGRA` -> `ColorImage` plus a texture upload per preview frame, and no benchmark covers it: `benches/` has compositor, resize and GPU targets only | A benchmark for the preview path and a recorded decision: keep the conversion, downscale inside the render worker, or share the output surface |
| 8 | Compositor cache ownership | The sampling cache sits behind one `Arc<Mutex<_>>` for the whole paste, and overflow clears all 64 maps at once | A microbenchmark isolating the lock, then a worker-owned cache only if the profile justifies it and after item 6 has fixed who owns the worker |

Item 2 follows item 1 rather than preceding it because one adapter removes all
three transitive problems at the same time, while a version bump that keeps
nokhwa keeps the exceptions. Item 6 adds no new structural risk: the aggregate it
needs is already separated, `src/app.rs` holds 621 lines of construction and
lifecycle and the render handshake lives in `src/app/render_coordination.rs`.

Deferred until items 1 through 4 have their evidence: source-name overlays on
rendered cells, and the remote-source boundary of
`docs/adr/0006-network-source-boundary.md`. Not planned: drag and drop in place
of the accessible up/down reorder commands, GPU composition that requires a CPU
readback, and the blend and hoist variants already measured slower and recorded
under "Чего не делать заново" in `docs/performance-review.md`.

