# CameraMan Research Survey

This is a research snapshot, not a dependency wish list. CameraMan remains a
Rust-only source project. Repositories written in C, C++, Objective-C++, C#,
TypeScript, Go, or Shell were studied for architecture, media contracts,
testing, packaging, and product design; none of their source code was copied
into CameraMan.

## Method

- First snapshot date: 2026-07-14; second snapshot date: 2026-07-18.
- GitHub metadata came from the authenticated GraphQL API.
- All 200 top-level READMEs were inspected.
- Architecture, design, profiling, buffer-pool, flow-control, benchmark, fuzz,
  and unsafe-code material was inspected more deeply in 40 representative
  repositories.
- Selection favors direct relevance over raw popularity. The first set has
  1,470,630 total stars, a mean of 14,706, and a median of 5,076.5. Ninety-eight
  repositories have at least 500 stars; the two smaller entries are narrow,
  canonical Rust components relevant to resizing and window interoperability.
- The second set has 1,237,647 total stars, a mean of 12,376, and a median of
  5,326.5. All 100 have at least 500 stars, 53 are primarily Rust, none is
  archived, and `RazrFalcon/memmap2-rs` is explicitly retained as the active
  canonical fork. `olive-editor/olive` is retained as a historical UX reference
  because its last push was in 2024, not as adoption evidence.
- GitHub marks 63 entries in the first set as primarily Rust. Two archived virtual-camera
  projects are retained only as historical migration evidence and are clearly
  marked below.
- Stars are a discovery signal, not proof of quality. Maintenance state,
  architecture, tests, platform fit, licenses, and measured behavior matter
  more when deciding whether to adopt an idea.

Snapshot refresh, replacement and citation rules live in
`docs/research-policy.md`. Supply-chain tools checked for this implementation
snapshot are cargo-deny 0.20.2, cargo-audit 0.22.2, cargo-sbom 0.10.0,
cargo-about 0.9.1 and spdx-tools 0.8.5. The latest compatible camera-backend
line inspected on 2026-07-14 was nokhwa 0.10.11; it still reaches `block 0.1.6`
through its macOS stack, so removal remains conditional on upstream.

## Repositories 1-100

### Capture and Virtual Cameras

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 1 | [obsproject/obs-studio](https://github.com/obsproject/obs-studio) | 73,849 | Separate sources, filters, outputs, services, rendering, and video-output threads. |
| 2 | [webcamoid/webcamoid](https://github.com/webcamoid/webcamoid) | 2,518 | Treat capture and virtual output as replaceable plugins with explicit format negotiation. |
| 3 | [ZoneMinder/zoneminder](https://github.com/ZoneMinder/zoneminder) | 5,882 | Model long-running camera health, reconnects, and failures as first-class states. |
| 4 | [PipeWire/pipewire](https://github.com/PipeWire/pipewire) | 2,132 | Negotiate graph formats and buffer ownership instead of assuming one universal frame contract. |
| 5 | [v4l2loopback/v4l2loopback](https://github.com/v4l2loopback/v4l2loopback) | 4,203 | Test virtual-device lifecycle, consumer negotiation, exclusive modes, and stale producers. |
| 6 | [CatxFish/obs-virtual-cam](https://github.com/CatxFish/obs-virtual-cam) | 1,832 | Archived: preserve compatibility lessons, but do not copy a legacy driver architecture. |
| 7 | [johnboiles/obs-mac-virtualcam](https://github.com/johnboiles/obs-mac-virtualcam) | 4,035 | Archived: document migration from legacy DAL/CMIO plugins to modern camera extensions. |
| 8 | [dev47apps/droidcam-obs-plugin](https://github.com/dev47apps/droidcam-obs-plugin) | 706 | Make disconnect, reconnect, latency, and remote-source degradation visible. |
| 9 | [nashaofu/xcap](https://github.com/nashaofu/xcap) | 994 | Keep platform capture backends behind a small Rust abstraction. |
| 10 | [scottlamb/moonfire-nvr](https://github.com/scottlamb/moonfire-nvr) | 1,718 | Use monotonic timing and soak tests for continuously running camera processes. |

### Media Pipelines

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 11 | [FFmpeg/FFmpeg](https://github.com/FFmpeg/FFmpeg) | 62,022 | Make pixel format, timestamps, stride, color, and ownership explicit at every boundary. |
| 12 | [GStreamer/gstreamer](https://github.com/GStreamer/gstreamer) | 3,257 | Use buffer pools, allocation negotiation, clocks, tracers, and bounded queues. |
| 13 | [videolan/vlc](https://github.com/videolan/vlc) | 18,985 | Isolate platform modules and keep playback/output clocks separate from UI state. |
| 14 | [mpv-player/mpv](https://github.com/mpv-player/mpv) | 35,976 | Prefer a measured GPU path with clear fallback and compact, inspectable configuration. |
| 15 | [HandBrake/HandBrake](https://github.com/HandBrake/HandBrake) | 23,752 | Validate presets as data and expose background-job progress and failure recovery. |
| 16 | [mifi/lossless-cut](https://github.com/mifi/lossless-cut) | 42,054 | Optimize frequent preview/edit actions and make destructive actions reversible. |
| 17 | [KDE/kdenlive](https://github.com/KDE/kdenlive) | 5,296 | Separate project model, preview proxies, background rendering, and UI commands. |
| 18 | [blender/blender](https://github.com/blender/blender) | 19,152 | Treat composition as a graph and retain deterministic CPU fallbacks for GPU work. |
| 19 | [gpac/gpac](https://github.com/gpac/gpac) | 3,276 | Preserve timestamps and capabilities through each media transform. |
| 20 | [ossrs/srs](https://github.com/ossrs/srs) | 29,045 | Measure low-latency behavior under load, not only single-frame throughput. |

### Image, Codec, and Color Systems

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 21 | [opencv/opencv](https://github.com/opencv/opencv) | 89,880 | Dispatch optimized kernels by hardware while keeping a stable image model. |
| 22 | [google-ai-edge/mediapipe](https://github.com/google-ai-edge/mediapipe) | 36,092 | Limit frames in flight and queued, and record per-stage latency histograms. |
| 23 | [lemenkov/libyuv](https://github.com/lemenkov/libyuv) | 925 | Benchmark color conversion, rotation, and scaling as distinct operations. |
| 24 | [haasn/libplacebo](https://github.com/haasn/libplacebo) | 753 | Treat primaries, transfer functions, range, HDR, and tone mapping as typed data. |
| 25 | [AOMediaCodec/libavif](https://github.com/AOMediaCodec/libavif) | 2,135 | Validate dimensions, allocation limits, bit depth, and color metadata before decoding. |
| 26 | [libjxl/libjxl](https://github.com/libjxl/libjxl) | 3,576 | Combine benchmarks, fuzzing, and strict resource limits for untrusted image data. |
| 27 | [strukturag/libheif](https://github.com/strukturag/libheif) | 2,265 | Preserve ICC/NCLX color information and expose conversion decisions. |
| 28 | [cisco/openh264](https://github.com/cisco/openh264) | 6,096 | Separate real-time deadlines, frame types, and encoder complexity controls. |
| 29 | [webmproject/libvpx](https://github.com/webmproject/libvpx) | 963 | Benchmark real-time deadline modes and profile slow frames, not just averages. |
| 30 | [xiph/rav1e](https://github.com/xiph/rav1e) | 4,137 | Keep safe Rust frame/config APIs around heavily optimized internals. |

### Rust Media Building Blocks

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 31 | [image-rs/image](https://github.com/image-rs/image) | 5,816 | Keep allocation limits and pixel-buffer invariants in the safe API. |
| 32 | [image-rs/imageproc](https://github.com/image-rs/imageproc) | 971 | Separate image storage from processing operations. |
| 33 | [Cykooz/fast_image_resize](https://github.com/Cykooz/fast_image_resize) | 452 | Evaluate pure-Rust SIMD resizing with reproducible ARM64 and x86_64 benchmarks. |
| 34 | [rust-av/rust-av](https://github.com/rust-av/rust-av) | 909 | Use narrow traits for demux, decode, frame, and packet boundaries. |
| 35 | [zmwangx/rust-ffmpeg](https://github.com/zmwangx/rust-ffmpeg) | 1,944 | Encapsulate foreign ownership and lifetimes if FFmpeg ever becomes necessary. |
| 36 | [larksuite/rsmpeg](https://github.com/larksuite/rsmpeg) | 877 | Keep low-level media bindings outside the domain model. |
| 37 | [l1npengtul/nokhwa](https://github.com/l1npengtul/nokhwa) | 796 | Audit CameraMan's current backend for cancellation, format, and macOS failure behavior. |
| 38 | [security-union/videocall-rs](https://github.com/security-union/videocall-rs) | 1,770 | Track end-to-end latency, pacing, reconnects, and security together. |
| 39 | [software-mansion/smelter](https://github.com/software-mansion/smelter) | 704 | Benchmark mixer capacity across input/output counts, resolutions, CPU, and GPU. |
| 40 | [harlanc/xiu](https://github.com/harlanc/xiu) | 2,303 | Keep protocol ingestion independent from the bounded media pipeline. |

### Rust Graphics and Desktop UI

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 41 | [gfx-rs/wgpu](https://github.com/gfx-rs/wgpu) | 17,572 | Build a recoverable GPU adapter with explicit resource lifetime and device-loss handling. |
| 42 | [emilk/egui](https://github.com/emilk/egui) | 29,634 | Work with immediate-mode state flow and expose AccessKit semantics deliberately. |
| 43 | [iced-rs/iced](https://github.com/iced-rs/iced) | 30,965 | Separate application state, messages, updates, and views. |
| 44 | [slint-ui/slint](https://github.com/slint-ui/slint) | 23,192 | Test declarative layout behavior under long text, scaling, and small windows. |
| 45 | [rust-windowing/winit](https://github.com/rust-windowing/winit) | 6,047 | Treat event-loop, scale-factor, suspend, and resume transitions as platform contracts. |
| 46 | [linebender/vello](https://github.com/linebender/vello) | 4,172 | Batch scene work and profile GPU/CPU synchronization. |
| 47 | [linebender/xilem](https://github.com/linebender/xilem) | 5,445 | Keep view state incremental and avoid rebuilding expensive media data with UI state. |
| 48 | [zed-industries/zed](https://github.com/zed-industries/zed) | 86,924 | Use command/action boundaries and split a large desktop app into owned crates/modules. |
| 49 | [lapce/lapce](https://github.com/lapce/lapce) | 38,650 | Isolate configuration, commands, plugins, and editor/view state. |
| 50 | [rerun-io/rerun](https://github.com/rerun-io/rerun) | 11,126 | Use typed time-series data, latest-at queries, wgpu rendering, and immediate-mode profiling. |

### Product and Workflow References

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 51 | [CapSoftware/Cap](https://github.com/CapSoftware/Cap) | 20,130 | Make capture setup, active state, completion, and export feel like one coherent workflow. |
| 52 | [screenpipe/screenpipe](https://github.com/screenpipe/screenpipe) | 19,816 | Bound continuous capture resources and make privacy/local storage explicit. |
| 53 | [GraphiteEditor/Graphite](https://github.com/GraphiteEditor/Graphite) | 26,538 | Separate document graph, command messages, undo history, and panels. |
| 54 | [rustdesk/rustdesk](https://github.com/rustdesk/rustdesk) | 118,168 | Treat reconnect, permissions, encryption, and degraded network state as product features. |
| 55 | [flameshot-org/flameshot](https://github.com/flameshot-org/flameshot) | 30,380 | Keep frequent capture tools compact, keyboard-friendly, and immediately actionable. |
| 56 | [ShareX/ShareX](https://github.com/ShareX/ShareX) | 38,630 | Represent reusable workflows as presets rather than duplicating controls. |
| 57 | [gyroflow/gyroflow](https://github.com/gyroflow/gyroflow) | 9,164 | Keep live preview and final render on the same parameter model. |
| 58 | [ntsc-rs/ntsc-rs](https://github.com/ntsc-rs/ntsc-rs) | 2,379 | Share effect logic between standalone preview and plugin/output paths. |
| 59 | [alacritty/alacritty](https://github.com/alacritty/alacritty) | 64,875 | Prefer a small hot path, measured rendering, and a versioned configuration model. |
| 60 | [helix-editor/helix](https://github.com/helix-editor/helix) | 45,394 | Provide discoverable commands, stable focus, and terse status feedback. |

### Concurrency and IPC

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 61 | [tokio-rs/tokio](https://github.com/tokio-rs/tokio) | 32,525 | Make cancellation, backpressure, and task ownership explicit, even without adopting Tokio. |
| 62 | [crossbeam-rs/crossbeam](https://github.com/crossbeam-rs/crossbeam) | 8,519 | Prefer bounded channels and tested ownership protocols. |
| 63 | [rayon-rs/rayon](https://github.com/rayon-rs/rayon) | 13,152 | Add data parallelism only where independent cells/rows and thresholds are measurable. |
| 64 | [tokio-rs/mio](https://github.com/tokio-rs/mio) | 7,036 | Keep low-level readiness and OS handles behind a narrow adapter. |
| 65 | [tokio-rs/loom](https://github.com/tokio-rs/loom) | 2,751 | Model atomic interleavings; ordinary thread tests cannot prove a lock-free protocol. |
| 66 | [eclipse-iceoryx/iceoryx2](https://github.com/eclipse-iceoryx/iceoryx2) | 2,381 | Use zero-copy loans, explicit service lifecycle, bounded history, and backpressure handlers. |
| 67 | [eclipse-iceoryx/iceoryx](https://github.com/eclipse-iceoryx/iceoryx) | 2,127 | Preallocate shared-memory pools and define QoS before publishing. |
| 68 | [servo/ipc-channel](https://github.com/servo/ipc-channel) | 1,121 | Separate typed messages from OS-specific IPC transport. |
| 69 | [capnproto/capnproto-rust](https://github.com/capnproto/capnproto-rust) | 2,480 | Version schemas and avoid parse-time copies for control-plane data. |
| 70 | [google/flatbuffers](https://github.com/google/flatbuffers) | 26,218 | Keep wire compatibility, validation, and generated DTOs outside domain types. |

### Testing, Benchmarking, and Profiling

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 71 | [bheisler/criterion.rs](https://github.com/bheisler/criterion.rs) | 5,499 | Use statistical baselines, warm-up, outlier handling, and reproducible inputs. |
| 72 | [proptest-rs/proptest](https://github.com/proptest-rs/proptest) | 2,190 | Generate and shrink layout, frame, and protocol invariants. |
| 73 | [rust-fuzz/cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) | 1,845 | Grow multiple focused fuzz targets, corpora, coverage, and minimized regressions. |
| 74 | [nextest-rs/nextest](https://github.com/nextest-rs/nextest) | 3,156 | Isolate slow/flaky tests and make CI execution profiles explicit. |
| 75 | [mitsuhiko/insta](https://github.com/mitsuhiko/insta) | 2,908 | Snapshot stable diagnostics and serialized contracts, not volatile output. |
| 76 | [BurntSushi/quickcheck](https://github.com/BurntSushi/quickcheck) | 2,771 | Keep small property tests near arithmetic-heavy code. |
| 77 | [flamegraph-rs/flamegraph](https://github.com/flamegraph-rs/flamegraph) | 5,973 | Profile before SIMD, parallelism, or GPU rewrites. |
| 78 | [mstange/samply](https://github.com/mstange/samply) | 4,300 | Capture native macOS profiles with symbols and shareable traces. |
| 79 | [tikv/pprof-rs](https://github.com/tikv/pprof-rs) | 1,641 | Make profile capture possible during representative long-running workloads. |
| 80 | [sourcefrog/cargo-mutants](https://github.com/sourcefrog/cargo-mutants) | 1,220 | Check that protocol and rendering tests fail when behavior is intentionally broken. |

### Security and Supply Chain

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 81 | [RustSec/rustsec](https://github.com/RustSec/rustsec) | 1,920 | Gate releases on known Rust dependency advisories. |
| 82 | [EmbarkStudios/cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | 2,367 | Declare license, source, duplicate, and banned-crate policy. |
| 83 | [EmbarkStudios/cargo-about](https://github.com/EmbarkStudios/cargo-about) | 754 | Generate third-party attribution from the actual dependency graph. |
| 84 | [mozilla/sccache](https://github.com/mozilla/sccache) | 7,441 | Cache CI builds without weakening artifact provenance. |
| 85 | [ossf/scorecard](https://github.com/ossf/scorecard) | 5,579 | Audit branch protection, pinned actions, token permissions, and release practices. |
| 86 | [google/oss-fuzz](https://github.com/google/oss-fuzz) | 12,426 | Run continuous fuzzing rather than occasional local fuzz checks. |
| 87 | [AFLplusplus/AFLplusplus](https://github.com/AFLplusplus/AFLplusplus) | 6,651 | Diversify fuzz engines for parsers and protocol state. |
| 88 | [google/sanitizers](https://github.com/google/sanitizers) | 12,423 | Exercise unsafe FFI and mmap code under address/thread sanitizers where supported. |
| 89 | [model-checking/kani](https://github.com/model-checking/kani) | 3,219 | Prove bounds and state invariants for small critical Rust functions. |
| 90 | [rust-lang/miri](https://github.com/rust-lang/miri) | 6,406 | Run pure-Rust unsafe helpers through an interpreter that detects undefined behavior. |

### Rust Architecture and Platform Integration

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 91 | [serde-rs/serde](https://github.com/serde-rs/serde) | 10,718 | Version persisted and wire data separately from runtime objects. |
| 92 | [dtolnay/anyhow](https://github.com/dtolnay/anyhow) | 6,588 | Preserve rich application context while keeping library errors typed. |
| 93 | [dtolnay/thiserror](https://github.com/dtolnay/thiserror) | 5,479 | Keep error variants at ownership boundaries concise and source-linked. |
| 94 | [tokio-rs/tracing](https://github.com/tokio-rs/tracing) | 6,775 | Instrument operations as spans with stable fields rather than ad hoc strings. |
| 95 | [metrics-rs/metrics](https://github.com/metrics-rs/metrics) | 1,459 | Separate metric emission from local presentation/export. |
| 96 | [madsmtm/objc2](https://github.com/madsmtm/objc2) | 981 | Centralize Apple-framework unsafe code and document ownership/thread assumptions. |
| 97 | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | 109,015 | Study signed desktop release/update boundaries without replacing CameraMan's Rust UI. |
| 98 | [tauri-apps/wry](https://github.com/tauri-apps/wry) | 4,857 | Isolate platform-window lifecycle and callbacks from application state. |
| 99 | [rust-windowing/glutin](https://github.com/rust-windowing/glutin) | 2,084 | Handle graphics-context loss and recreation explicitly. |
| 100 | [rust-windowing/raw-window-handle](https://github.com/rust-windowing/raw-window-handle) | 426 | Keep cross-library native-handle interop minimal and auditable. |

## Repositories 101-200

### Real-Time Capture and RTC

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 101 | [pion/webrtc](https://github.com/pion/webrtc) | 16,639 | Keep protocol layers small and testable, and isolate packet interception from media ownership. |
| 102 | [livekit/livekit](https://github.com/livekit/livekit) | 19,823 | Track frame integrity and spatial layers while bounding SFU-style packet and frame buffers. |
| 103 | [bluenviron/mediamtx](https://github.com/bluenviron/mediamtx) | 19,532 | Hide protocol diversity behind common path, session, reader, and publisher contracts. |
| 104 | [meetecho/janus-gateway](https://github.com/meetecho/janus-gateway) | 9,128 | Keep a small session core with plugins at the edge, and fuzz every untrusted parser. |
| 105 | [versatica/mediasoup](https://github.com/versatica/mediasoup) | 7,310 | Give worker, router, transport, producer, and consumer objects explicit ownership. |
| 106 | [coturn/coturn](https://github.com/coturn/coturn) | 14,211 | Treat NAT traversal, credentials, quotas, rate limits, and deployment diagnostics as one security boundary. |
| 107 | [aiortc/aiortc](https://github.com/aiortc/aiortc) | 5,078 | Use readable executable protocol tests and make every media clock conversion explicit. |
| 108 | [webrtc-rs/webrtc](https://github.com/webrtc-rs/webrtc) | 5,086 | Model future protocol logic as runtime-agnostic Sans-I/O state plus thin async adapters. |
| 109 | [OvenMediaLabs/OvenMediaEngine](https://github.com/OvenMediaLabs/OvenMediaEngine) | 3,195 | Publish a latency budget and an exact ingest/output capability matrix before adding live protocols. |
| 110 | [AlexxIT/go2rtc](https://github.com/AlexxIT/go2rtc) | 13,478 | Represent restreaming as a source graph and expose every transcoding fallback. |

### Codec, Color, and Image Systems

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 111 | [Netflix/vmaf](https://github.com/Netflix/vmaf) | 5,415 | Report confidence intervals or bootstrap distributions with perceptual scores, not one magic number. |
| 112 | [videolan/dav1d](https://github.com/videolan/dav1d) | 771 | Keep optimized kernels behind stable runtime dispatch and test adversarial streams. |
| 113 | [rust-av/Av1an](https://github.com/rust-av/Av1an) | 1,951 | Make scene chunking, worker scheduling, retry, and deterministic merge separate concerns. |
| 114 | [xiph/opus](https://github.com/xiph/opus) | 3,246 | State algorithmic delay, pre-skip, frame duration, and packet-loss assumptions in the media contract. |
| 115 | [intel/media-driver](https://github.com/intel/media-driver) | 1,227 | Negotiate hardware capabilities and failure fallback instead of assuming an accelerator exists. |
| 116 | [AcademySoftwareFoundation/OpenColorIO](https://github.com/AcademySoftwareFoundation/OpenColorIO) | 2,074 | Separate reference space, color spaces, transforms, and runtime configuration. |
| 117 | [AcademySoftwareFoundation/openexr](https://github.com/AcademySoftwareFoundation/openexr) | 1,812 | Validate dimensions, channels, metadata, and resource limits before allocating image storage. |
| 118 | [libvips/libvips](https://github.com/libvips/libvips) | 11,495 | Prefer demand-driven image work, low-memory pipelines, and bounded concurrency. |
| 119 | [ImageMagick/ImageMagick](https://github.com/ImageMagick/ImageMagick) | 16,959 | Define an explicit resource and codec policy around all untrusted image input. |
| 120 | [google/highway](https://github.com/google/highway) | 5,685 | Dispatch width-agnostic SIMD at coarse hotspots and retain scalar parity tests. |

### Graphics and Rust UI Foundations

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 121 | [bevyengine/bevy](https://github.com/bevyengine/bevy) | 47,208 | Use owned plugins, schedules, and resources instead of one giant mutable application object. |
| 122 | [servo/servo](https://github.com/servo/servo) | 37,409 | Draw process and component boundaries first; parallelize layout or rendering only after measurement. |
| 123 | [DioxusLabs/dioxus](https://github.com/DioxusLabs/dioxus) | 36,795 | Keep state and view separate, but do not replace the current UI toolkit based on popularity. |
| 124 | [tracel-ai/cubecl](https://github.com/tracel-ai/cubecl) | 2,270 | Introduce backend-neutral compute IR only if several GPU backends justify its cost. |
| 125 | [rust-skia/rust-skia](https://github.com/rust-skia/rust-skia) | 1,795 | Budget FFI ownership, binary size, build time, and platform packaging before adopting a large renderer. |
| 126 | [bitshifter/glam-rs](https://github.com/bitshifter/glam-rs) | 2,003 | Feature-gate SIMD and continuously compare it with the scalar implementation. |
| 127 | [Smithay/smithay](https://github.com/Smithay/smithay) | 3,109 | Keep protocol state, compositor state, and platform adapters distinct. |
| 128 | [AccessKit/accesskit](https://github.com/AccessKit/accesskit) | 1,488 | Maintain one typed semantic tree and translate it through platform accessibility adapters. |
| 129 | [DioxusLabs/taffy](https://github.com/DioxusLabs/taffy) | 3,281 | Benchmark layout computation in isolation before blaming the complete UI frame. |
| 130 | [linebender/tiny-skia](https://github.com/linebender/tiny-skia) | 1,604 | Keep a safe, intentionally scoped CPU renderer with explicit quality limits. |

### Media Desktop Product References

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 131 | [audacity/audacity](https://github.com/audacity/audacity) | 17,376 | Represent edits as undoable commands and move expensive work into visible background jobs. |
| 132 | [Ardour/ardour](https://github.com/Ardour/ardour) | 5,114 | Keep allocation, blocking locks, and UI work out of the real-time media thread. |
| 133 | [mixxxdj/mixxx](https://github.com/mixxxdj/mixxx) | 6,936 | Give high-frequency controls stable positions while allowing expert layouts to be configured. |
| 134 | [LMMS/lmms](https://github.com/LMMS/lmms) | 10,134 | Isolate plugins and version their state instead of letting them mutate the project model directly. |
| 135 | [olive-editor/olive](https://github.com/olive-editor/olive) | 9,086 | Historical only: study node-graph UX, but discount adoption lessons because maintenance is stale. |
| 136 | [NatronGitHub/Natron](https://github.com/NatronGitHub/Natron) | 5,432 | Separate node-graph evaluation, cache invalidation, and interactive controls. |
| 137 | [OpenShot/openshot-qt](https://github.com/OpenShot/openshot-qt) | 6,072 | Keep project state independent from render and export workers. |
| 138 | [NickeManarin/ScreenToGif](https://github.com/NickeManarin/ScreenToGif) | 27,301 | Make capture, trim/edit, review, and export a progressive workflow. |
| 139 | [mltframework/shotcut](https://github.com/mltframework/shotcut) | 14,565 | Put the media engine behind an adapter and store reusable output settings as presets. |
| 140 | [SeaDve/Kooha](https://github.com/SeaDve/Kooha) | 3,443 | Keep a one-task capture surface compact and explain permission or backend failures clearly. |

### Concurrency and Memory

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 141 | [rust-lang/rust](https://github.com/rust-lang/rust) | 114,698 | Tie every unsafe invariant and memory-model assumption to compiler documentation and tests. |
| 142 | [smol-rs/smol](https://github.com/smol-rs/smol) | 5,011 | Compose small executors only when async I/O is actually required; CameraMan does not need one today. |
| 143 | [async-rs/async-std](https://github.com/async-rs/async-std) | 4,068 | Record a maintenance and exit strategy before depending on any runtime. |
| 144 | [zesterer/flume](https://github.com/zesterer/flume) | 3,041 | Compare bounded channel semantics and workloads, not only synthetic message throughput. |
| 145 | [cameron314/concurrentqueue](https://github.com/cameron314/concurrentqueue) | 12,404 | Document reasons not to use a lock-free queue before accepting its proof burden. |
| 146 | [microsoft/mimalloc](https://github.com/microsoft/mimalloc) | 13,195 | Measure bounded allocation latency, metadata overhead, and remote frees under contention. |
| 147 | [jemalloc/jemalloc](https://github.com/jemalloc/jemalloc) | 11,003 | Observe allocated, resident, active, and fragmentation metrics separately. |
| 148 | [RazrFalcon/memmap2-rs](https://github.com/RazrFalcon/memmap2-rs) | 639 | State mapping lifetime and file-mutation hazards at every mmap-backed unsafe boundary. |
| 149 | [Amanieu/parking_lot](https://github.com/Amanieu/parking_lot) | 3,384 | Prefer a well-understood lock when it meets deadlines; audit footprint and fairness if switching. |
| 150 | [xacrimon/dashmap](https://github.com/xacrimon/dashmap) | 4,081 | Remember that sharding does not make multi-key operations atomic or deadlock-free. |

### Transport and Protocol Design

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 151 | [Haivision/srt](https://github.com/Haivision/srt) | 3,557 | Expose latency windows, retransmission, congestion, encryption, and drop metrics together. |
| 152 | [cloudflare/quiche](https://github.com/cloudflare/quiche) | 11,683 | Use buffer pools, explicit session state, and structured qlog-style diagnostics. |
| 153 | [quinn-rs/quinn](https://github.com/quinn-rs/quinn) | 5,177 | Define stream cancellation, backpressure, and connection lifecycle before network capture exists. |
| 154 | [rustls/rustls](https://github.com/rustls/rustls) | 7,524 | Prefer safe protocol defaults and an explicit supported-version policy. |
| 155 | [eclipse-zenoh/zenoh](https://github.com/eclipse-zenoh/zenoh) | 2,997 | Make QoS, congestion, history, and liveliness part of the source contract. |
| 156 | [libp2p/rust-libp2p](https://github.com/libp2p/rust-libp2p) | 5,579 | Version protocol upgrades and bound identities, peers, messages, and resource use. |
| 157 | [zeromq/libzmq](https://github.com/zeromq/libzmq) | 10,940 | Socket patterns hide queues; always set capacity, timeout, and slow-consumer policy. |
| 158 | [nanomsg/nng](https://github.com/nanomsg/nng) | 4,632 | Use typed communication patterns and make reconnect state observable. |
| 159 | [nats-io/nats-server](https://github.com/nats-io/nats-server) | 20,251 | Protect producers from slow consumers with explicit limits and diagnostics. |
| 160 | [tokio-rs/bytes](https://github.com/tokio-rs/bytes) | 2,234 | Use cheap immutable slicing without exposing mutable aliasing across owners. |

### Benchmarking, Profiling, and Telemetry

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 161 | [sharkdp/hyperfine](https://github.com/sharkdp/hyperfine) | 28,499 | Warm up, enforce a minimum run count, prepare caches explicitly, and vary command order. |
| 162 | [google/benchmark](https://github.com/google/benchmark) | 10,289 | Use fixtures, domain counters, complexity checks, and noise-aware repetition. |
| 163 | [google/perfetto](https://github.com/google/perfetto) | 6,235 | Bound trace buffers, synchronize clocks, and report trace data loss and instrumentation overhead. |
| 164 | [KDE/heaptrack](https://github.com/KDE/heaptrack) | 4,124 | Profile allocation count, lifetime, peaks, and RSS as well as CPU time. |
| 165 | [benfred/py-spy](https://github.com/benfred/py-spy) | 15,349 | Prefer low-perturbation sampling and make profiler permissions diagnosable. |
| 166 | [open-telemetry/opentelemetry-rust](https://github.com/open-telemetry/opentelemetry-rust) | 2,647 | Stabilize semantic fields first and keep external export optional. |
| 167 | [tokio-rs/console](https://github.com/tokio-rs/console) | 4,560 | Add task lifecycle telemetry only if an async runtime is later justified. |
| 168 | [tikv/rust-prometheus](https://github.com/tikv/rust-prometheus) | 1,178 | Select metric types deliberately and cap label cardinality. |
| 169 | [grafana/pyroscope](https://github.com/grafana/pyroscope) | 11,551 | Make continuous profiling opt-in and define privacy and retention boundaries. |
| 170 | [iovisor/bcc](https://github.com/iovisor/bcc) | 22,555 | Use latency histograms and account for probe overhead; prefer macOS signposts locally. |

### Test and Public-API Tooling

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 171 | [taiki-e/cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) | 1,423 | Clean stale artifacts and deliberately merge coverage across feature profiles. |
| 172 | [xd009642/tarpaulin](https://github.com/xd009642/tarpaulin) | 2,998 | Record platform limits and never treat one coverage engine as ground truth on macOS. |
| 173 | [obi1kenobi/cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks) | 1,658 | Gate accidental public API breakage before release. |
| 174 | [cargo-public-api/cargo-public-api](https://github.com/cargo-public-api/cargo-public-api) | 564 | Snapshot the intended exported Rust surface and review every delta. |
| 175 | [asomers/mockall](https://github.com/asomers/mockall) | 1,827 | Mock only true ownership boundaries; prefer real value objects elsewhere. |
| 176 | [testcontainers/testcontainers-rs](https://github.com/testcontainers/testcontainers-rs) | 1,103 | Use external-system integration fixtures where relevant, but not as a substitute for real CMIO tests. |
| 177 | [LukeMathWalker/wiremock-rs](https://github.com/LukeMathWalker/wiremock-rs) | 792 | Add deterministic HTTP fixtures only if CameraMan actually gains a network control plane. |
| 178 | [taiki-e/cargo-hack](https://github.com/taiki-e/cargo-hack) | 847 | Check meaningful feature powersets instead of testing only the default build. |
| 179 | [est31/cargo-udeps](https://github.com/est31/cargo-udeps) | 2,123 | Remove dependencies that no target or feature really uses. |
| 180 | [rust-lang/cargo](https://github.com/rust-lang/cargo) | 15,253 | Pin the lockfile and document MSRV, offline, target, and reproducible build commands. |

### Supply Chain and Release Automation

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 181 | [sigstore/cosign](https://github.com/sigstore/cosign) | 6,141 | Sign provenance and attestations separately from the mandatory Apple code signature. |
| 182 | [anchore/syft](https://github.com/anchore/syft) | 9,250 | Cross-check the Rust-generated SBOM against the final bundle filesystem. |
| 183 | [anchore/grype](https://github.com/anchore/grype) | 12,580 | Scan the shipped artifact or SBOM, not merely the source lockfile. |
| 184 | [aquasecurity/trivy](https://github.com/aquasecurity/trivy) | 36,960 | Scan workflows, configuration, secrets, and artifacts with pinned scanner data. |
| 185 | [in-toto/in-toto](https://github.com/in-toto/in-toto) | 1,017 | Attest ordered release steps together with their materials and products. |
| 186 | [sigstore/rekor](https://github.com/sigstore/rekor) | 1,177 | Add transparency only after the release threat model explains what it proves. |
| 187 | [rust-secure-code/cargo-auditable](https://github.com/rust-secure-code/cargo-auditable) | 835 | Embed locked dependency metadata in release binaries for later incident response. |
| 188 | [mozilla/cargo-vet](https://github.com/mozilla/cargo-vet) | 965 | Grow dependency audits incrementally and import trust under an explicit policy. |
| 189 | [axodotdev/cargo-dist](https://github.com/axodotdev/cargo-dist) | 2,070 | Evaluate artifact planning and checksums only after verifying nested macOS bundle support. |
| 190 | [release-plz/release-plz](https://github.com/release-plz/release-plz) | 1,420 | Automate release proposals, but keep publish behind signing and acceptance gates. |

### Architecture and Product Systems

| # | Repository | Stars | CameraMan lesson |
|---:|---|---:|---|
| 191 | [rust-lang/rust-analyzer](https://github.com/rust-lang/rust-analyzer) | 16,664 | Distinguish source inputs from derived state and write architecture invariants beside the code. |
| 192 | [denoland/deno](https://github.com/denoland/deno) | 107,708 | Split owned crates by permissions, resources, runtime, and platform boundaries. |
| 193 | [nushell/nushell](https://github.com/nushell/nushell) | 40,033 | Keep typed commands, plugins, and pipeline values independent from presentation. |
| 194 | [AppFlowy-IO/AppFlowy](https://github.com/AppFlowy-IO/AppFlowy) | 73,960 | Separate local source-of-truth state from optional sync and conflict resolution. |
| 195 | [penpot/penpot](https://github.com/penpot/penpot) | 56,732 | Use explicit events, undoable direct manipulation, and workers around one coherent document model. |
| 196 | [salsa-rs/salsa](https://github.com/salsa-rs/salsa) | 2,903 | Adopt incremental computation only when a real dependency graph justifies invalidation machinery. |
| 197 | [dtolnay/cxx](https://github.com/dtolnay/cxx) | 6,775 | Express foreign ownership as checked bridge contracts; retain `objc2` for the current Apple boundary. |
| 198 | [rust-lang/rust-bindgen](https://github.com/rust-lang/rust-bindgen) | 5,238 | Isolate, pin, and review generated bindings rather than spreading them through domain modules. |
| 199 | [bytecodealliance/wasmtime](https://github.com/bytecodealliance/wasmtime) | 18,362 | Combine differential fuzzing, independent oracles, and a documented security triage path. |
| 200 | [astral-sh/ruff](https://github.com/astral-sh/ruff) | 48,648 | Optimize from profiles while preserving compatibility with a large fixture-driven suite. |

## Scientific and Primary Sources

### Performance Methodology

- Tomas Kalibera and Richard Jones, [Rigorous Benchmarking in Reasonable Time](https://kar.kent.ac.uk/33611/): choose process and in-process repetitions from measured variance and report confidence, not an arbitrary three samples.
- Tomas Kalibera and Richard Jones, [Quantifying Performance Changes with Effect Size Confidence Intervals](https://www.cs.kent.ac.uk/pubs/2012/3233/): compare changes using uncertainty around effect size rather than one percentage threshold alone.
- Charlie Curtsinger and Emery Berger, [Stabilizer](https://people.cs.umass.edu/~emery/pubs/stabilizer-asplos13.pdf): layout and environmental noise can change measured performance enough to produce false conclusions.
- Todd Mytkowicz et al., [Producing Wrong Data Without Doing Anything Obviously Wrong](https://doi.org/10.1145/1508284.1508275): compiler layout and execution environment are experimental variables, so raw benchmark artifacts must retain context.
- Philip Fleming and John Wallace, [How Not to Lie with Statistics: The Correct Way to Summarize Benchmark Results](https://cgi.cse.unsw.edu.au/~cs9242/11/papers/Fleming_Wallace_86.pdf): aggregate normalized ratios with an appropriate geometric mean, while preserving per-case results.

### Real-Time Media and Concurrency

- Maurice Herlihy and Jeannette Wing, [Linearizability: A Correctness Condition for Concurrent Objects](https://www.cs.cmu.edu/~wing/publications/HerlihyWing90.pdf): describe the shared slot as observable histories and prove each operation has a valid linearization point.
- Maged Michael and Michael Scott, [Simple, Fast, and Practical Non-Blocking and Blocking Concurrent Queue Algorithms](https://www.cs.rochester.edu/research/synchronization/pseudocode/queues.html): compare lock-free publication with a simpler blocking baseline and include memory-reclamation obligations.
- Ralf Jung et al., [RustBelt](https://plv.mpi-sws.org/rustbelt/popl18/paper.pdf): unsafe modules must expose safe contracts whose invariants remain valid under composition.
- Mark Batty et al., [The C11 and C++11 Concurrency Model](https://kar.kent.ac.uk/50268/): cross-language atomics require a shared formal memory-model argument, not Rust-only intuition.
- Leslie Lamport, [Concurrent Reading and Writing](https://lamport.azurewebsites.net/pubs/rd-wr.pdf): the mapped-slot protocol needs a stated single-writer model, publication order, and proof obligations.
- Jeffrey Dean and Luiz Andre Barroso, [The Tail at Scale](https://research.google/pubs/the-tail-at-scale/): averages hide the slow frames users actually see; track percentiles and maxima.
- Fouladi et al., [Salsify: Low-Latency Network Video](https://www.usenix.org/conference/nsdi18/presentation/fouladi): media pacing, codec work, and transport decisions must be evaluated as one latency path.
- Apple, [Handling Frame Drops with AVCaptureVideoDataOutput](https://developer.apple.com/library/archive/technotes/tn2445/_index.html): keep the latest frame, report drop reasons, and reduce work/rate when chronically late.
- Rust, [Atomic Ordering](https://doc.rust-lang.org/std/sync/atomic/enum.Ordering.html): every `Acquire`, `Release`, and relaxed metadata access in the mmap protocol needs an explicit happens-before argument.
- Tokio, [Loom](https://github.com/tokio-rs/loom): factor the slot state machine so atomic interleavings can be model-checked independently of `mmap`.
- IETF, [RFC 8834: Media Transport and Use of RTP in WebRTC](https://www.rfc-editor.org/rfc/rfc8834.html): if remote sources arrive, codec, RTP, RTCP, retransmission, and congestion behavior belong in one negotiated profile.
- IETF, [RFC 8836: Congestion Control Requirements for Interactive Real-Time Media](https://www.rfc-editor.org/rfc/rfc8836.html): any network source needs bounded queues and congestion response that protects interactive latency.
- IETF, [RFC 8888: RTP Control Protocol Feedback for Congestion Control](https://www.rfc-editor.org/rfc/rfc8888.html): transport feedback should be typed and rate-limited instead of routed through ad hoc UI events.
- W3C, [WebRTC Statistics API](https://www.w3.org/TR/webrtc-stats/): model health as a vector of loss, jitter, freeze, decode, frame, and transport counters rather than one green/red flag.

### Image Quality, Color, and Core Video

- Wang et al., [Image Quality Assessment: From Error Visibility to Structural Similarity](https://doi.org/10.1109/TIP.2003.819861): exact pixel goldens are useful but insufficient for evaluating resampling quality.
- Bampis et al., [SpatioTemporal Feature Integration and Model Fusion for Full Reference Video Quality Assessment](https://arxiv.org/abs/1804.04813): VMAF can complement SSIM for offline quality gates, but should not become a runtime dependency.
- ITU-R, [BT.709](https://www.itu.int/rec/R-REC-BT.709/en): define the SDR primaries and transfer contract instead of treating BGRA bytes as self-describing.
- ITU-R, [BT.2100](https://www.itu.int/rec/R-REC-BT.2100/en): defer HDR until the pipeline can preserve transfer, primaries, bit depth, and tone-mapping intent end to end.
- Apple, [Tagging media with video color information](https://developer.apple.com/documentation/avfoundation/tagging-media-with-video-color-information): generated pixel buffers need propagating color attachments.
- Apple, [Image Buffer Transfer Function Constants](https://developer.apple.com/documentation/corevideo/image-buffer-transfer-function-constants): most SDR video paths should state the Rec.709 transfer function explicitly.
- Apple, [CVPixelBufferPool](https://developer.apple.com/documentation/corevideo/cvpixelbufferpool-77o): recycle a bounded set of output buffers rather than allocate one every frame.
- Apple, [CVMetalTextureCache](https://developer.apple.com/documentation/corevideo/cvmetaltexturecache-q3j): a future Rust/Metal path can share Core Video image buffers with the GPU.
- Apple, [CMClockGetHostTimeClock](https://developer.apple.com/documentation/coremedia/cmclockgethosttimeclock%28%29): media timestamps and cross-process latency should use a host monotonic clock, not wall time.
- ITU-T, [P.910 Subjective Video Quality Assessment](https://www.itu.int/rec/T-REC-P.910-202310-I/en): calibrate objective quality metrics against controlled viewing tasks and representative content.
- ITU-R, [BT-series recommendations](https://www.itu.int/rec/R-REC-BT): treat primaries, transfer, matrix, range, and bit depth as one versioned color contract.
- W3C, [WebCodecs](https://www.w3.org/TR/webcodecs/all/): timestamp, coded size, visible rectangle, color space, and ownership are independent frame fields worth preserving even without adopting the web API.

### Architecture and Verification

- David Parnas, [On the Criteria To Be Used in Decomposing Systems into Modules](https://doi.org/10.1145/361598.361623): split by hidden design decisions and likely changes, which directly motivates breaking up four oversized files.
- Moseley and Marks, [Out of the Tar Pit](https://curtclifton.net/papers/MoseleyMarks06a.pdf): minimize accidental state and keep derived UI state out of persisted/domain state.
- Manes et al., [The Art, Science, and Engineering of Fuzzing](https://arxiv.org/abs/1812.00140): add structure-aware targets and maintain corpora/coverage rather than treating one target as completion.
- Ding and Le Goues, [An Empirical Study of OSS-Fuzz Bugs](https://arxiv.org/abs/2103.11518): continuous fuzzing finds useful faults, while timeouts, OOMs, and flaky cases need explicit triage.
- Google, [OSS-Fuzz](https://github.com/google/oss-fuzz): continuous, distributed fuzzing now supports Rust and should inform the release quality bar.
- Barbara Liskov and Jeannette Wing, [A Behavioral Notion of Subtyping](https://doi.org/10.1145/197320.197383): adapters must preserve observable source and transport contracts, not merely share method names.

### Human-Computer Interaction and Accessibility

- Paul Fitts, [The Information Capacity of the Human Motor System](https://doi.org/10.1037/h0055392): frequent controls need adequate target size and short pointer travel.
- W. E. Hick, [On the Rate of Gain of Information](https://doi.org/10.1080/17470215208416600): progressive disclosure is preferable to presenting every rare signing control in the main workflow.
- W3C, [WCAG 2.2](https://www.w3.org/TR/WCAG22/): focus visibility, non-color status, keyboard operation, and minimum target size provide useful desktop checks too.
- Apple, [Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines/): use familiar macOS commands, hierarchy, accessibility, and platform behavior.
- Ben Shneiderman, [Direct Manipulation](https://doi.org/10.1109/MC.1983.1654471): source transforms should provide continuous visible feedback, incremental actions, and easy reversal.
- Robert Miller, [Response Time in Man-Computer Conversational Transactions](https://doi.org/10.1145/1476589.1476628): UI feedback must match human response-time expectations even when media or installation work continues asynchronously.

### macOS Distribution and Supply Chain

- Apple, [Creating a camera extension with Core Media I/O](https://developer.apple.com/documentation/coremediaio/creating-a-camera-extension-with-core-media-i-o): provider/device/stream ownership, App Groups, frame-rate contracts, and host packaging are the canonical activation model.
- Apple, [Create camera extensions with Core Media I/O](https://developer.apple.com/videos/play/wwdc2022/10022/): the Mach service must be valid beneath the shared App Group prefix.
- Apple, [Configuring App Groups](https://developer.apple.com/documentation/xcode/configuring-app-groups): host and extension communication belongs in a shared signed container.
- Apple, [NSSystemExtensionUsageDescription](https://developer.apple.com/documentation/bundleresources/information-property-list/nssystemextensionusagedescription): activation must carry a clear user-facing purpose string.
- Apple, [Creating distribution-signed code for macOS](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/): nested code and restricted entitlements need matching identities, profiles, and inside-out signing.
- Apple, [Notarizing macOS software before distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution): production release acceptance includes notarization and stapling, not only local `codesign --verify`.
- [SLSA v1.2](https://slsa.dev/spec/v1.2/): release artifacts should gain build provenance generated by the hosted build platform.
- [SPDX 3.0.1](https://spdx.github.io/spdx-spec/v3.0.1/model/Software/Classes/Sbom/): publish a machine-readable SBOM alongside signed releases.
- NIST, [Secure Software Development Framework 1.1](https://csrc.nist.gov/pubs/sp/800/218/final): release evidence should cover preparation, protected development, well-secured output, and vulnerability response.
- in-toto authors, [in-toto: Providing farm-to-table guarantees for bits and bytes](https://www.usenix.org/conference/usenixsecurity19/presentation/torres-arias): attest materials, commands, actors, and products across the complete release chain.
- The Update Framework, [Specification 1.0.34](https://theupdateframework.io/specification/latest/): do not add a self-updater until rollback, freeze, mix-and-match, key compromise, and threshold signatures are designed.
- Reproducible Builds, [Plans and status](https://reproducible-builds.org/docs/plans/): reproducibility requires deterministic inputs and an independent rebuild comparison, not only a clean local build.
- Sigstore, [Overview](https://docs.sigstore.dev/about/overview/): identity-bound signing and transparency can complement, but do not replace, Apple signing and notarization.

## Synthesis for CameraMan

The strongest evidence does not point to adding a large framework. It points to
making the existing small Rust system more explicit and measurable:

- P0 correctness: bounded `CVPixelBufferPool`, color attachments, monotonic
  cross-process timestamps, drift-free pacing, stale-frame policy, and restart
  generations.
- P1 proof and visibility: model the atomic protocol, add p50/p95/p99 stage
  metrics, OS signposts, child-process crash tests, and structured diagnostics.
- P1 architecture: split `app.rs`, `main.rs`, `extension_main.rs`, and
  `shared_memory_transport.rs` by decisions they hide.
- P2 performance: remove measured frame copies, reuse buffers and resize maps,
  then compare pure-Rust SIMD, Rayon, and wgpu paths behind the same contracts.
- P2 product design: keep the preview primary, move rare installation details
  into a setup surface, add scene/source transforms, and make status accessible
  without relying on color.
- P2 release: add dependency policy, SBOM, provenance, notarization, and an
  explicit project license before public distribution.

The first survey maps to `recommendation.md` items 501-600. The second survey,
primary-source pass, and measured benchmark experiments map to items 601-700.
