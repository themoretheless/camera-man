# Unsafe invariant register

Every production source containing `unsafe` has one verification owner. Local
`SAFETY` comments explain the immediate precondition; this register names the
test, model, or platform contract that can falsify the wider assumption.

| Source | Unsafe boundary | Verification owner |
|---|---|---|
| `src/diagnostics.rs` | macOS signpost and scalar process counters | `diagnostics::tests`, strict Clippy, macOS CI |
| `src/extension/mod.rs` | retained CMIO objects, sample submission and the worker exit guard | `cameraman-extension` unit tests plus signed real-consumer release gate |
| `src/extension/objects.rs` | objc2 protocol implementations, contained callback bodies, client identity reads and NSError out pointer | `StreamLifecycle`, `client_authorization` and `panic_boundary` tests plus Apple CMIO delegate contract and signed release gate |
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
| `src/system_extension.rs` | OSSystemExtension request/delegate calls and contained delegate callbacks | activation state and `panic_boundary` tests plus paid-profile install release gate |

Adding `unsafe` to a new Rust source without adding a row here fails
`unsafe_sources_have_documented_verification_owner`.

## Panic policy at platform boundaries

No Rust panic may unwind out of an Objective-C callback: the CoreMediaIO
callbacks in `src/extension/objects.rs` and the `OSSystemExtensionRequest`
delegate in `src/system_extension.rs`. objc2 0.6.4 defines every generated class
method as `extern "C-unwind"`, so a panic does not abort at the boundary: it
unwinds into Objective-C frames that own no Rust cleanup and are not required to
handle a foreign unwind. objc2 offers no containment for this; its `catch-all`
feature only converts Objective-C exceptions raised by outgoing message sends
into Rust panics.

The rule is contain, log, degrade:

- Every callback body runs inside `camera_man::contain_panic`, which catches the
  unwind, logs the callback name and the payload, and returns a conservative
  default.
- Conservative means the answer a correct implementation could give knowing
  nothing: `false` for every decision (denied client, refused start or stop,
  rejected property write), an empty collection for every list, an empty
  properties object for every properties getter, and a cancelled replacement for
  the activation delegate. A contained panic can cost one client or one stream
  start; it must never cost the extension process, which would take the camera
  away from every consumer app at once.
- The fallback itself must not panic. It runs contained too, but no return value
  can be fabricated if it fails, so `contain_panic` aborts rather than resuming
  the unwind: an abort is a defined outcome, a foreign unwind is not. Fallbacks
  are therefore a constant or a single framework constructor, and code reachable
  from one logs through `panic_boundary::report_line` instead of `eprintln!`,
  which panics when stderr is gone
  (`a_panicking_fallback_aborts_instead_of_resuming_the_unwind`).
- A callback that mutates stream state repairs it before returning the default,
  so the next callback never starts from the half-finished transition the panic
  interrupted (`reset_after_contained_panic`).
- The `stream_samples` worker keeps the thread boundary as its container, and
  `StreamingExit` clears the streaming flag while unwinding so a worker that is
  gone never leaves the flag claiming it still runs. What reopens the stream is
  the next start: `begin_start_reaping_finished_worker` joins the finished
  worker, reports its payload and forces the lifecycle back to `Ready` before
  `begin_start` runs, because a lifecycle still holding `Running` for a dead
  worker would answer `AlreadyRunning` and wedge the stream until the client
  stopped it.
- Panics before `CMIOExtensionProvider::startServiceWithProvider` are out of
  scope: no client is connected and the process exits.

Containment depends on `panic = "unwind"`. `catch_unwind` is inert under
`panic = "abort"`, so `panic_containment_requires_unwinding_profiles` fails if a
profile in `Cargo.toml` sets it.
`every_objc_callback_body_contains_its_panics` scans every source under `src/`,
so a new callback in a new file is caught too.
