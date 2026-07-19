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
