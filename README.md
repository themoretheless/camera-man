# CameraMan

CameraMan is now a Rust-only project.

The goal is still the same: combine frames from several cameras into one video frame and expose that frame as `CameraMan Virtual Camera`. The previous non-Rust prototype was removed from the repository so the codebase can grow from a clean Rust foundation.

## Current Status

What works now:

- Rust library and binary compile.
- Core frame model exists.
- Output video format is centralized.
- Grid, row, and column layouts are implemented.
- BGRA compositor is implemented in pure Rust.
- Synthetic demo frames can be rendered.
- The pipeline can write a PPM frame sequence through a Rust sink.
- A Rust desktop app exists through `eframe`/`egui`.
- The app has source toggles, layout switching, start/stop preview, render tick, PPM export, preview area, and status bar.
- The app writes live composed BGRA frames to a Rust frame-spool sink for the virtual-camera extension.
- Real camera discovery exists through `nokhwa`.
- The app has a real-camera mode with non-blocking background capture.
- The CLI can list cameras and attempt a bounded real-frame capture.
- The CLI can create a macOS `.app` bundle with `NSCameraUsageDescription`.
- The CLI can create and embed a Rust `.systemextension` bundle.
- The Rust CoreMediaIO system-extension provider creates a virtual device and source stream.
- The extension declares a 1920x1080 BGRA/30fps stream format.
- The extension reads composed app frames from the Rust frame spool and sends them as `CMSampleBuffer` frames.
- The extension falls back to generated placeholder frames when the app has not published a frame yet.
- Bundles are best-effort ad-hoc signed for local development.
- Unit tests cover frame validation, frame transport, layout calculation, composition, PPM output, and pipeline ticks.

What is intentionally not done yet:

- Production-grade app-group IPC and entitlement hardening for frame transport.
- Signed/notarized release packaging.

Real capture is present, but macOS may require launching the bundled app and approving camera access before frames arrive.
The virtual-camera extension is a development backend: macOS still requires system-extension approval, and production distribution still needs proper signing/notarization.
The default development frame spool path is the `CAMERAMAN_FRAME_SPOOL` environment variable if set, otherwise `cameraman-virtual-frame.bgra` inside the OS temp directory (`std::env::temp_dir()`, i.e. `$TMPDIR` on macOS, not literally `/tmp`).

Known limitations found by review, not yet fixed (see `arhitecture.md` section 8a for detail):

- `src/extension_main.rs` is a first working prototype of a real CoreMediaIO extension, not
  a hardened one: on failure it can panic and kill the whole extension host process, it
  accepts any local client without checking who is connecting, and a couple of internal
  Core Foundation object lifetimes are worth re-checking before this ships beyond local
  development.
- Stopping and immediately restarting the same physical camera (or switching cameras
  quickly) can race the old capture thread's shutdown against the new one's open.
- `CaptureErrorKind::classify` is a keyword heuristic over vendor error text; it is
  documented as best-effort and will misclassify contrived multi-cause messages.

## Run

```bash
cargo run
```

This launches the desktop app.

Launch it explicitly:

```bash
cargo run -- app
```

Render one synthetic demo frame:

```bash
cargo run -- demo
```

The demo writes:

```text
target/camera-man-demo.ppm
```

Render three frames through the full pipeline into a PPM sequence:

```bash
cargo run -- pipeline-demo
```

The pipeline demo writes:

```text
target/camera-man-pipeline-demo/frame-000001.ppm
target/camera-man-pipeline-demo/frame-000002.ppm
target/camera-man-pipeline-demo/frame-000003.ppm
```

Print normalized config:

```bash
cargo run -- check
```

Query real cameras:

```bash
cargo run -- list-cameras
```

Capture one real camera frame with a 5 second timeout:

```bash
cargo run -- capture-demo
```

Create a macOS app bundle with camera permission metadata:

```bash
cargo run -- bundle
open target/CameraMan.app
```

Create only the Rust system-extension bundle:

```bash
cargo run -- bundle-extension
```

Run tests:

```bash
cargo test
```

Format:

```bash
cargo fmt
```

## Repository Layout

```text
src/main.rs             CLI entry point
src/extension_main.rs   Rust CoreMediaIO provider, device, source stream, frame-spool reader
src/app.rs              Rust desktop app
src/lib.rs              public module exports
src/capture.rs          real camera discovery/capture through nokhwa
src/config.rs           video and virtual camera config
src/error.rs            shared error type
src/frame.rs            BGRA frame model
src/layout.rs           row, column, grid cell calculation
src/render.rs           pure Rust compositor
src/camera.rs           camera and frame source traits
src/virtual_camera.rs   virtual camera sink trait and test sinks
src/frame_transport.rs  file-based frame-spool bridge (FrameSpoolSink, read_latest_frame)
src/ppm.rs              PPM writer and PPM sequence sink
src/pipeline.rs         capture -> compose -> sink orchestration

arhitecture.md          architecture notes and Rust-only decomposition
recommendation.md       500 Rust-only recommendations and next steps
```

## Architecture

The project is split around stable boundaries:

```text
FrameSource
  -> CapturedFrame
  -> PipelineEngine
  -> Compositor
  -> VirtualCameraSink
```

The pure Rust core does not know about macOS APIs. Platform-specific work should live behind traits:

- real camera capture implements `FrameSource`;
- macOS virtual camera output stays behind Rust boundaries;
- file/debug output can also implement `VirtualCameraSink`;
- the Rust desktop app uses the same frame, layout, render, and PPM output core as the CLI.

This keeps the hard platform work isolated and lets the frame/layout/rendering code stay easy to test.

## Design Direction

CameraMan should feel like a focused desktop utility, not a marketing page.

The current app is a first usable UI. It shows:

- synthetic source cameras;
- selected layout;
- preview;
- start/stop state;
- output status;
- diagnostics only when needed.

Avoid decorative screens, oversized hero sections, and vague onboarding text. The useful work should be visible immediately.

## Next Implementation Order

1. Keep the pure Rust core green with tests.
2. Add a real camera capture adapter in Rust.
3. Bridge composed app frames into the CoreMediaIO extension stream.
4. Replace the current one-camera real mode with multi-camera real selection.
5. Add release packaging only after capture and sink are stable.

## Important macOS Note

Modern macOS virtual cameras are tied to CoreMediaIO system extension APIs. This project now uses Rust `objc2` bindings for that provider layer; the remaining backend work is the frame transport between the app pipeline and the extension stream.
