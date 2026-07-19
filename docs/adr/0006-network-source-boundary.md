# ADR 0006: Network source boundary

Status: accepted, requirement-gated

## Context

CameraMan currently captures local devices and publishes one local CMIO output.
There is no approved remote-source, RTP, discovery, update, or credential use
case. Adding protocol/runtime dependencies now would create untestable states
and an attack surface with no user value.

## Decision

Do not add a `remote-source` feature or network dependency until a written
product requirement names interoperability, latency, recovery, and security
acceptance criteria. When approved, the implementation must have:

1. A Sans-I/O Rust state machine with all protocol mutation behind one
   `&mut self`; socket/timer tasks are thin adapters exchanging typed events.
2. A separate `FrameClock` mapping remote RTP time into host monotonic time.
   Local capture timestamps remain untouched.
3. Typed negotiation through `StreamRequest`/`StreamCapability`, including
   pixel format, dimensions, FPS range, colorimetry, latency, and ownership.
4. An explicit bounded `ChannelBackpressureContract` for every channel.
5. Parser and state-machine fuzz corpora, byte/depth/item/allocation limits,
   and the remote-source threat-model gates before the feature can be enabled.
6. A non-default feature and adapter boundary. Async/TLS/RTP/HTTP dependencies
   require a dependency decision record and measured build/binary/runtime cost.

## Consequences

The local app stays small and synchronous at its domain boundary. `rustls`,
Tokio, RTP, HTTP, discovery, network identity, and remote clock code are absent
by design. This ADR is the interface plan, not a placeholder implementation.
