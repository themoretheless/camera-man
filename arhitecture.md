# CameraMan Rust Architecture

This file keeps the requested filename `arhitecture.md`.

CameraMan is now organized as a Rust-only codebase. The current implementation is a small, testable core rather than a mixed-language prototype.

## 1. Goal

CameraMan should:

1. read frames from one or more camera sources;
2. compose them into a single BGRA output frame;
3. send that output frame to a virtual camera sink;
4. expose a simple UI or CLI around the pipeline.

The key rule: platform-specific work must stay behind Rust traits.

## 2. Current Modules

```text
src/config.rs
  VideoFormat
  VirtualCameraConfig

src/error.rs
  CameraManError

src/frame.rs
  PixelFormat
  Frame
  FrameMetadata
  CapturedFrame

src/frame_transport.rs
  FrameSpoolSink
  TransportFrame
  read_latest_frame
  default_frame_spool_path

src/layout.rs
  CompositionLayout
  GridLayout
  Cell
  GridLayoutCalculator

src/render.rs
  Compositor

src/camera.rs
  CameraDevice
  CameraDiscovery
  FrameSource
  SyntheticFrameSource

src/capture.rs
  NokhwaCameraDiscovery
  NokhwaFrameSource
  ThreadedNokhwaFrameSource
  capture_one_with_timeout

src/virtual_camera.rs
  VirtualCameraSink
  MemorySink

src/ppm.rs
  write_ppm
  PpmSequenceSink

src/pipeline.rs
  PipelineEngine
  RenderReport
  RunSummary

src/main.rs
  CLI entry point
  demo renderer
  pipeline demo
  app/system-extension bundler

src/extension_main.rs
  Rust CoreMediaIO provider process target
  virtual device registration
  1920x1080 BGRA/30fps source stream
  frame-spool reader
  CMSampleBuffer output with placeholder fallback

src/app.rs
  eframe desktop app
  source toggles
  synthetic/real input mode
  layout controls
  preview texture upload
  PPM export
```

## 3. Current Data Flow

```text
FrameSource instances
  -> latest_frame()
  -> CapturedFrame
  -> PipelineEngine
  -> Compositor::compose_captured()
  -> Frame
  -> VirtualCameraSink::send()
```

The current demo uses synthetic sources:

```text
SyntheticFrameSource
  -> PipelineEngine
  -> pure Rust compositor
  -> PpmSequenceSink
  -> PPM files
```

Real camera mode:

```text
NokhwaCameraDiscovery
  -> CameraDevice list
  -> ThreadedNokhwaFrameSource
  -> latest CapturedFrame
  -> Compositor
  -> preview texture / FrameSpoolSink / PPM export
```

System-extension packaging:

```text
cargo run -- bundle
  -> target/CameraMan.app
  -> Contents/Library/SystemExtensions/com.cameraman.rust.extension.systemextension
  -> Rust cameraman-extension binary
```

Virtual camera development backend:

```text
CameraManApp
  -> Compositor
  -> FrameSpoolSink
  -> $TMPDIR/cameraman-virtual-frame.bgra (or $CAMERAMAN_FRAME_SPOOL)
  -> cameraman-extension FrameSpoolReader
  -> CMIOExtensionProvider
  -> CMIOExtensionDevice
  -> CMIOExtensionStream source
  -> BGRA CVPixelBuffer
  -> CMSampleBuffer
  -> sendSampleBuffer()
```

When the app has not published a frame yet, the extension generates a placeholder BGRA frame so the stream can still start predictably.

## 4. SOLID Split

Single Responsibility:

- `Frame` only owns validated pixel data.
- `FrameMetadata` only describes source, sequence, and timestamp.
- `CapturedFrame` only joins a frame with metadata.
- `VideoFormat` only describes output format.
- `GridLayoutCalculator` only calculates cells.
- `Compositor` only renders frames into an output frame.
- `FrameSource` only provides frames.
- `NokhwaCameraDiscovery` only queries camera devices.
- `ThreadedNokhwaFrameSource` only keeps capture off the UI thread.
- `VirtualCameraSink` only receives composed frames.
- `FrameSpoolSink` only publishes the latest composed frame.
- `PipelineEngine` only orchestrates source -> render -> sink.

Open/Closed:

- new layouts should extend layout calculation without changing camera capture;
- new camera backends should implement `FrameSource`;
- new virtual camera backends should implement `VirtualCameraSink`;
- new UIs should use `PipelineEngine` rather than reaching into rendering internals.

Liskov Substitution:

- synthetic sources, real camera sources, file sources, and test sources should all satisfy `FrameSource`;
- memory sinks, debug sinks, and real virtual camera sinks should all satisfy `VirtualCameraSink`.

Interface Segregation:

- rendering does not depend on camera discovery;
- camera discovery does not depend on virtual camera output;
- CLI and desktop app do not depend on pixel-copy internals.

Dependency Inversion:

- high-level pipeline code depends on traits;
- platform APIs live in adapters;
- tests can use synthetic sources and memory sinks.

## 5. DRY Rules

Keep these values centralized:

- output width;
- output height;
- fps;
- pixel format;
- virtual camera name;
- virtual camera UID;
- layout rules;
- frame validation;
- error type.

Current source of truth:

- `VideoFormat` for output shape;
- `VirtualCameraConfig` for virtual camera identity;
- `PixelFormat` for pixel memory layout.

## 6. Recommended Future Structure

```text
src/
  main.rs
  lib.rs
  config.rs
  error.rs
  frame.rs
  layout.rs
  render.rs
  pipeline.rs
  capture.rs

  capture/
    mod.rs
    traits.rs
    synthetic.rs
    macos.rs

  virtual_camera/
    mod.rs
    traits.rs
    memory.rs
    macos_coremediaio.rs

  ui/
    mod.rs
    camera_list.rs
    preview.rs
    status_bar.rs

  diagnostics/
    mod.rs
    metrics.rs
    logging.rs
```

Do not add this structure before it is needed. The current flat files are easier to learn from.

## 7. Product Design

CameraMan should be a quiet, practical tool.

The current Rust app uses a dense desktop layout:

- left panel: camera sources and layout controls;
- center: 16:9 preview;
- bottom bar: virtual camera status, fps, dropped frames;
- toolbar: start, stop, refresh, settings;
- dialogs only for permissions, destructive actions, and backend setup.

Good design here means:

- stable dimensions;
- no jumping buttons;
- no decorative backgrounds;
- clear disabled states;
- compact but readable labels;
- errors with recovery actions;
- diagnostics hidden until useful.

## 8. Three Iterations

### Iteration 1: Pure Rust Core

Done:

- remove non-Rust prototype files;
- create Rust modules;
- implement frame validation;
- implement row, column, and grid layouts;
- implement pure Rust BGRA composition;
- add tests;
- add synthetic demo output.
- add frame metadata;
- add pipeline tick reports;
- add PPM sequence sink;
- add pipeline tests.
- add Rust desktop app;
- add preview texture upload;
- add source toggles and layout controls;
- add PPM export from app.
- add real camera discovery;
- add non-blocking real camera capture;
- add macOS app bundler with camera permission metadata.
- add Rust `.systemextension` bundle target;
- embed `.systemextension` inside `CameraMan.app`;
- best-effort ad-hoc sign app and extension bundles.
- add frame-spool transport between app and extension.

Next:

- add more compositor tests;
- add a benchmark;
- add metrics.

### Iteration 2: Real Input And Output

Done:

- Rust camera capture adapter;
- device discovery;
- frame timestamps;
- capture error reporting;
- Rust CoreMediaIO provider;
- virtual device registration;
- source stream format;
- placeholder sample-buffer fallback;
- composed app-frame transport into the extension stream;
- replacement of placeholder frames with pipeline output when app frames are available;

Next:

- harden transport with app-group IPC and entitlements;
- integration smoke tests where possible.

Keep:

- core renderer independent from platform APIs;
- virtual camera output behind `VirtualCameraSink`;
- camera input behind `FrameSource`.

### Iteration 3: Rust Desktop Product

Done:

- Rust GUI;
- preview panel;
- camera selection;
- layout switching;
- status bar;

Next:

- first-run setup;
- packaging;
- release checks.

## 8a. Verification Round (Three-Pass Adversarial Review)

Separately from the three feature iterations above, the core and capture layers went
through a three-pass review-fix-verify cycle (find bugs, fix them, adversarially try to
break the fix). This section records what that cycle actually found and changed, so it
does not get lost in `recommendation.md`'s longer backlog.

Confirmed and fixed:

- `layout.rs`/`render.rs`: more sources than output pixels along one axis produced
  zero-width or zero-height cells, which underflowed `u32` subtraction in
  `paste_aspect_fit` and panicked in debug builds (silently corrupted composition in
  release). Fixed by making `GridLayoutCalculator::cells` compute proportional edges in
  `u64` and having `paste_aspect_fit` clamp destination size to the cell and early-return
  on a zero-size cell.
- `render.rs`: nearest-neighbor sampling multiplied `u32` coordinates directly, which
  overflows for extreme aspect ratios (for example a 2x2,000,000 source). Fixed with a
  `u64`-computed `sample_coordinate` clamped to `source_size - 1`.
- `frame.rs`: `byte_len` computed the pixel buffer size in `u32`, which both rejected
  valid large frames and, per the iteration-3 adversarial pass, can wrap to exactly `0`
  in release builds for pathological width/height pairs, so an oversized frame would be
  accepted with an empty buffer and panic on first pixel access. Fixed with `u64` math
  plus an explicit `MAX_FRAME_BYTES` (1 GiB) cap and a dedicated `BufferTooLarge` error;
  `set_bgra` also got a `debug_assert` on out-of-bounds writes, which is exactly what
  would have caught the original zero-size-cell bug earlier.
- `ppm.rs`: `write_ppm` wrote 3 bytes per pixel with an unbuffered `File`, one syscall per
  pixel. Wrapped in `BufWriter`.
- `pipeline.rs`: `PipelineEngine::render_once` used to abort the whole tick if any single
  `FrameSource` errored. It now degrades a failing source to an empty cell and reports
  `(index, error)` pairs via `RenderReport::source_errors`; `start()` is idempotent and
  `render_once()` before `start()` is a typed error instead of silently touching an
  unconnected sink.
- `error.rs`: `CameraManError::Capture` and `Io` were bare `String`s. `Capture` is now a
  struct variant carrying a `CaptureErrorKind` (PermissionDenied / DeviceBusy /
  DeviceNotFound / Disconnected / Unsupported / Other), classified heuristically from the
  message text; `Io` carries `std::io::ErrorKind`. This is explicitly a best-effort
  heuristic over vendor error strings, not a contract, and the adversarial pass found a
  real classification bug (an overly broad `"already"` keyword misclassified `"no such
  device, already removed"` as `DeviceBusy` instead of `DeviceNotFound`); the keyword set
  was tightened and a regression test added. A second ambiguity (a message that plausibly
  names two conditions at once) was judged not solvable by keyword order alone and is
  documented as a known heuristic limitation instead of "fixed."
- `capture.rs`: `ThreadedNokhwaFrameSource` used to fully detach its worker thread with no
  `JoinHandle`, so `Drop` could never confirm the camera was released, and a panic inside
  the capture loop (camera or mutex failure) killed the thread silently, leaving
  `latest_frame()` returning a stale-but-healthy-looking last frame forever. Fixed by
  storing the `JoinHandle` and, on `Drop`, handing it to a short-lived reaper thread that
  joins in the background (so `Drop` itself never blocks the caller), and by wrapping each
  capture iteration in `panic::catch_unwind` so a panic surfaces as a normal `last_error`
  instead of vanishing.

Confirmed but intentionally left open (see `recommendation.md` for the full list):

- **Real race, not yet fixed**: `capture.rs`'s reaper thread means `Drop` returns before
  the camera is guaranteed closed. `app.rs` currently drops the old `ThreadedNokhwaFrameSource`
  and can open a new one for the same device on the very next tick (Stop immediately
  followed by Start, or a fast camera switch), which can race the new open against the
  still-closing old worker. Flagged as a follow-up task rather than fixed inline, because
  `app.rs` was under active concurrent development while this review ran.
- **`src/extension_main.rs` safety review**: this file grew into a real, from-scratch
  CoreMediaIO Camera Extension over `objc2`/`objc2-core-media-io` during this same
  session, in parallel with the review. A dedicated unsafe/FFI review flagged several
  issues worth fixing before this is used for anything beyond local development:
  `create_extension()` calls `.expect(...)` on failure, which panics and kills the whole
  extension host process instead of failing gracefully; `formats()` and the per-frame
  sample-buffer path recreate a `CMVideoFormatDescription` / `CMIOExtensionStreamFormat`
  on every call instead of caching one, which both violates the "formats should not
  change" expectation of the protocol and risks a reference leak per call under the Core
  Foundation Create Rule; a stream handle is stored as a raw `usize` and re-retained by
  address from another thread instead of holding a proper `Retained<CMIOExtensionStream>`;
  `connectClient:error:` and `authorizedToStartStreamForClient:` unconditionally return
  `true`, so any local process can attach; there is no `catch_unwind` around the
  `define_class!` callback bodies, so a Rust panic could unwind across the Objective-C
  runtime boundary and abort the host; and `keep_alive` is a `sleep(60s)` loop rather than
  a real run loop (`CFRunLoopRun` or equivalent). None of this is fixed yet; treat the
  extension as a development-only prototype until it is.
- Everything else the review surfaced that is still open (dropped-frame metrics, render
  loop independent of the UI thread, `Send` bounds for a future multi-threaded pipeline,
  persistence, localization, and the rest) is tracked in `recommendation.md`, which was
  re-audited item by item against the code during this round rather than trusted at face
  value.

## 9. How To Learn The Project

Read in this order:

1. `src/frame.rs`
2. `src/layout.rs`
3. `src/render.rs`
4. `src/camera.rs`
5. `src/virtual_camera.rs`
6. `src/frame_transport.rs`
7. `src/pipeline.rs`
8. `src/ppm.rs`
9. `src/app.rs`
10. `src/main.rs`

That order starts with pure data, then pure math, then orchestration, then entry points.
