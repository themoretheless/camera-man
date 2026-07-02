# CameraMan

CameraMan is now a Rust-only project.

The goal is still the same: combine frames from several cameras into one video frame and eventually expose that frame as `CameraMan Virtual Camera`. The previous non-Rust prototype was removed from the repository so the codebase can grow from a clean Rust foundation.

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
- Real camera discovery exists through `nokhwa`.
- The app has a real-camera mode with non-blocking background capture.
- The CLI can list cameras and attempt a bounded real-frame capture.
- The CLI can create a macOS `.app` bundle with `NSCameraUsageDescription`.
- Unit tests cover frame validation, layout calculation, composition, PPM output, and pipeline ticks.

What is intentionally not done yet:

- Real macOS virtual camera backend.
- Signed/notarized release packaging.

Real capture is present, but macOS may require launching the bundled app and approving camera access before frames arrive.

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
- macOS virtual camera output implements `VirtualCameraSink`;
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
3. Add a Rust macOS virtual camera backend behind `VirtualCameraSink`.
4. Replace the current one-camera real mode with multi-camera real selection.
5. Add release packaging only after capture and sink are stable.

## Important macOS Note

Modern macOS virtual cameras are tied to CoreMediaIO system extension APIs. The Rust version should still target the same platform behavior, but the backend must be implemented through Rust FFI/bindings rather than a non-Rust source layer.
