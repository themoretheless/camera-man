# ADR 0004: Buffer Pools

Status: accepted

## Context

Per-frame full-HD allocation and repeated copies create latency variance and
memory pressure in both the app and extension.

## Decision

Reuse compositor output and preview buffers in bounded pools. The extension
uses a bounded IOSurface-backed `CVPixelBufferPool`. Borrowed mmap frames copy
directly into the pooled CoreVideo destination. Pools are reset with a format
epoch, never resized piecemeal.

## Consequences

Pool exhaustion drops a frame instead of growing memory without bound. Copy and
allocation counters are part of diagnostics and performance acceptance.
