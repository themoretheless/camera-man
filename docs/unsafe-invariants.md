# Unsafe invariant register

Every production source containing `unsafe` has one verification owner. Local
`SAFETY` comments explain the immediate precondition; this register names the
test, model, or platform contract that can falsify the wider assumption.

| Source | Unsafe boundary | Verification owner |
|---|---|---|
| `src/diagnostics.rs` | macOS signpost and scalar process counters | `diagnostics::tests`, strict Clippy, macOS CI |
| `src/extension/mod.rs` | retained CMIO objects and sample submission | `cameraman-extension` unit tests plus signed real-consumer release gate |
| `src/extension/objects.rs` | objc2 protocol implementations and NSError out pointer | `StreamLifecycle` tests plus Apple CMIO delegate contract and signed release gate |
| `src/extension/pixel_buffer.rs` | Core Video ownership, locks, row pointers | extension pixel-writer tests plus Apple CVPixelBuffer lock/base-address contract |
| `src/extension/timing.rs` | Core Media time constants | extension timing/range tests plus Apple CMTime contract |
| `src/frame.rs` | `sysctl` physical-memory query | frame limit tests plus macOS `sysctlbyname` contract |
| `src/media_time.rs` | `clock_gettime` initialization | monotonic/generation tests plus POSIX clock contract |
| `src/metal_interop.rs` | retained Core Video/Metal objects | GPU experiment validation plus Core Video/Metal create-rule contract |
| `src/performance.rs` | process/malloc usage structures | process usage delta tests plus Darwin rusage/malloc structure contract |
| `src/shared_memory_transport/mapped_backend.rs` | mmap lifetime, fd ownership, process identity | Loom, process fault/restart/PID-reuse tests, Kani arithmetic |
| `src/shared_memory_transport/protocol.rs` | page-size query | protocol layout tests and Kani checked mapping arithmetic |
| `src/shared_memory_transport/reader.rs` | borrowed slice over a leased mmap slot | Loom lease model, Miri frame-view profile, process tearing/fault tests |
| `src/shared_memory_transport/writer.rs` | copy into exclusively owned mmap slot | Loom publication model and process tearing tests |
| `src/shared_memory_transport/tests.rs` | POSIX test-object cleanup | shared-memory library tests; test-only pointer is a live `CString` |
| `src/system_extension.rs` | OSSystemExtension request/delegate calls | activation state tests plus paid-profile install release gate |

Adding `unsafe` to a new Rust source without adding a row here fails
`unsafe_sources_have_documented_verification_owner`.
