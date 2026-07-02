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
  UnsupportedVirtualCameraSink

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
  -> preview texture / PPM export
```

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

Next:

- Rust virtual camera backend;
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

## 9. How To Learn The Project

Read in this order:

1. `src/frame.rs`
2. `src/layout.rs`
3. `src/render.rs`
4. `src/camera.rs`
5. `src/virtual_camera.rs`
6. `src/pipeline.rs`
7. `src/ppm.rs`
8. `src/app.rs`
9. `src/main.rs`

That order starts with pure data, then pure math, then orchestration, then entry points.
