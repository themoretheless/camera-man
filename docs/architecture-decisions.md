# Architecture Decisions

## ADR-001: Owned threads, no Tokio runtime

**Status:** accepted. Capture, discovery, render, and CMIO workers have owned
threads, explicit stop paths, replaceable latest work, and bounded state. Tokio
is banned by `deny.toml`. Reconsider only for an approved async I/O feature
whose cancellation and load tests beat the current design.

## ADR-002: No FFmpeg or GStreamer runtime

**Status:** accepted. CameraMan captures canonical BGRA and publishes CMIO
frames; it does not decode containers or encode streams. FFmpeg may remain an
offline VMAF tool. A runtime media framework requires a concrete unsupported
codec/container feature, a narrow Rust adapter, license review, and measured
binary/startup impact.

## ADR-003: Retain egui

**Status:** accepted. The app state, commands, scene model, capture coordination,
and UI modules are separated. AccessKit, keyboard traversal, RU/EN fixtures,
minimum-window captures, and a dedicated setup window address measured needs.
A replacement requires usability evidence that cannot be solved within these
boundaries and a migration cost estimate.

## ADR-004: Narrow App Group/CMIO IPC

**Status:** accepted. The mmap protocol owns exactly one use case: latest BGRA
frame publication plus producer/consumer progress. It borrows proven ideas such
as bounded slots and explicit generations without adopting a generic zero-copy
framework, service discovery layer, allocator, or QoS model that the product
does not need.
