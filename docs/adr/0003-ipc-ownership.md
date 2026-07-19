# ADR 0003: IPC Ownership

Status: accepted

## Context

The app and CMIO extension need low-latency latest-frame exchange without disk
traffic, torn reads, or two live producers.

## Decision

Use a versioned three-slot mmap protocol. A kernel lock grants one writer;
generation nonce plus sequence identifies producer epochs. Readers claim a slot
with an RAII state guard and borrow bytes directly. File spool remains an
explicit fallback with its own versioned DTO.

## Consequences

Wire layouts are adapters, not domain `Frame` types. Protocol changes bump the
version and endpoint name. Crashed reader slots are reclaimable by PID liveness.
