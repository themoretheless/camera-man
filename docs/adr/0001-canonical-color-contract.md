# ADR 0001: Canonical Color Contract

Status: accepted

## Context

Raw `BGRA8` does not identify primaries, transfer, range, alpha semantics,
aperture, pixel aspect, or orientation. Implicit choices produce different
images in different consumers.

## Decision

Compositor output is tightly packed 8-bit BGRA, Rec.709
primaries/transfer/matrix, full range, opaque alpha, square pixels, full clean
aperture and identity transform. The color contract has schema version 1 and a
stable packed wire code; it never relies on Rust enum discriminants. Source
deviations live in `FrameContract` and are consumed once by composition.

Shared-memory protocol v5 and file-spool protocol v4 carry the packed color
contract beside every frame. Readers reject unknown, malformed or noncanonical
contracts before exposing pixels. The CMIO boundary emits
`kCVPixelFormatType_32BGRA` buffers with matching Rec.709 CoreVideo attachments.
Bilinear currently operates on gamma-encoded values for speed; the
`linear-light-experiment` feature is a measured quality path only.

## Consequences

New formats require a schema/protocol review, explicit conversion and
golden/perceptual tests rather than another enum case alone. A wire change must
update `tests/contract_invariants.rs`; old readers fail closed instead of
silently interpreting pixels under a different color model.
