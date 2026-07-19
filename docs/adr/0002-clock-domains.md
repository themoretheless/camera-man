# ADR 0002: Clock Domains

Status: accepted

## Context

Wall time can jump and process-local `Instant` origins cannot be compared over
IPC. Media presentation time can be rebased and has different semantics again.

## Decision

Use boot-relative `CLOCK_MONOTONIC` for pacing, freshness and latency; typed
media timestamps for presentation; Unix wall time only for human diagnostics
and filenames. APIs expose these as distinct types in `media_time.rs`.

## Consequences

No stale/pacing decision may use wall time. Transport carries monotonic and
wall values in separately named wire fields.
