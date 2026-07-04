# CameraMan Architecture

CameraMan is a Rust-only macOS camera compositor. It combines selected camera
sources into one BGRA output frame, previews that frame in an egui desktop app,
and publishes the latest composed frame to a Rust CoreMediaIO system extension
prototype.

## Current State

What works:

- Rust library and CLI compile.
- The egui app launches from `cargo run`.
- Synthetic sources compose into a 1920x1080 BGRA preview.
- Real camera discovery and background capture use `nokhwa`.
- Real mode supports multiple selected cameras.
- The app writes live composed frames to a frame-spool sink.
- The extension reads that frame spool and emits `CMSampleBuffer` frames.
- The extension falls back to generated placeholder frames when no app frame is available.
- The app can request system-extension activation through `OSSystemExtensionRequest`.
- The CLI can build `target/CameraMan.app` and an embedded `.systemextension`.
- Tests cover frame validation, layout, rendering, PPM output, pipeline ticks, capture classification, and frame transport.

What is still development-only:

- The default app bundle is ad-hoc signed, so it launches but cannot install a system extension.
- A real install requires a provisioning profile that grants `com.apple.developer.system-extension.install`.
- The frame spool is a file-based dev bridge, not a production IPC design.
- The extension still has unsafe CoreMediaIO/Objective-C boundaries that need hardening.

## Module Map

```text
src/config.rs
  VideoFormat
  VirtualCameraConfig

src/error.rs
  CameraManError
  CaptureErrorKind

src/frame.rs
  PixelFormat
  Frame
  FrameMetadata
  CapturedFrame

src/layout.rs
  CompositionLayout
  GridLayout
  GridLayoutCalculator
  Cell

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

src/frame_transport.rs
  FrameSpoolSink
  TransportFrame
  read_latest_frame
  default_frame_spool_path

src/pipeline.rs
  PipelineEngine
  RenderReport
  RunSummary

src/ppm.rs
  write_ppm
  PpmSequenceSink

src/system_extension.rs
  ExtensionInstaller
  ExtensionActivationStatus
  OSSystemExtensionRequest delegate bridge

src/app.rs
  egui desktop app
  source selection
  layout controls
  preview upload
  virtual output toggle
  extension activation UI

src/extension_main.rs
  CoreMediaIO provider process
  virtual device and stream source
  frame-spool reader
  CVPixelBuffer/CMSampleBuffer output

src/main.rs
  CLI commands
  demo renderers
  app/system-extension bundler
  code signing helpers
```

## Data Flow

Synthetic preview:

```text
SyntheticFrameSource
  -> CapturedFrame
  -> Compositor
  -> Frame
  -> egui texture
  -> FrameSpoolSink when live output is enabled
```

Real camera preview:

```text
NokhwaCameraDiscovery
  -> selected CameraDevice ids
  -> ThreadedNokhwaFrameSource per selected id
  -> latest CapturedFrame or None per source
  -> Compositor
  -> egui texture
  -> FrameSpoolSink
```

Virtual camera development backend:

```text
CameraManApp
  -> FrameSpoolSink
  -> $CAMERAMAN_FRAME_SPOOL or $TMPDIR/cameraman-virtual-frame.bgra
  -> cameraman-extension
  -> FrameSpoolReader
  -> CVPixelBuffer
  -> CMSampleBuffer
  -> CMIOExtensionStream::sendSampleBuffer
```

System-extension activation:

```text
CameraMan.app from /Applications
  -> ExtensionInstaller::activate()
  -> OSSystemExtensionRequest
  -> OSSystemExtensionRequestDelegate callbacks
  -> ExtensionActivationStatus
  -> app status label
```

## SOLID Split

Single Responsibility:

- `Frame` validates and owns pixel bytes.
- `FrameMetadata` describes source, sequence, and timestamp.
- `GridLayoutCalculator` calculates cells.
- `Compositor` copies/scales pixels into the output frame.
- `FrameSource` reads one source.
- `VirtualCameraSink` receives one composed output.
- `FrameSpoolSink` publishes the latest composed output.
- `ExtensionInstaller` only requests system-extension activation.
- `PipelineEngine` orchestrates source -> render -> sink.

Open/Closed:

- Add camera backends by implementing `FrameSource`.
- Add sinks by implementing `VirtualCameraSink`.
- Add layouts inside `layout.rs` without touching capture.
- Add UI views on top of the library types without reaching into FFI.

Liskov Substitution:

- Synthetic and real camera sources must both behave as `FrameSource`.
- Memory, PPM, and frame-spool sinks must all behave as `VirtualCameraSink`.
- Test doubles should be able to replace real sources and sinks.

Interface Segregation:

- Capture does not depend on rendering.
- Rendering does not depend on CoreMediaIO.
- Bundling does not depend on app UI internals.
- Extension activation does not depend on frame transport.

Dependency Inversion:

- High-level pipeline code depends on traits.
- Platform APIs stay in adapters.
- Tests use pure Rust doubles.
- The app composes library pieces rather than becoming the core.

## DRY Rules

Keep one source of truth for:

- output dimensions and fps: `VideoFormat`;
- virtual device name and UID: `VirtualCameraConfig`;
- pixel memory layout: `PixelFormat`;
- frame validation: `Frame::new_checked`;
- layout cell math: `GridLayoutCalculator`;
- transport path and header format: `frame_transport.rs`;
- extension bundle id: currently duplicated and should be unified.

Avoid duplicating:

- signing identities and bundle ids across docs and code;
- frame-spool header parsing in app and extension;
- camera open/close logic outside `ThreadedNokhwaFrameSource`;
- CoreMediaIO unsafe code outside `src/extension_main.rs`;
- UI state labels that repeat CLI status text.

## Learning Path

Read the project in small pieces:

1. `src/frame.rs`
2. `src/layout.rs`
3. `src/render.rs`
4. `src/camera.rs`
5. `src/virtual_camera.rs`
6. `src/frame_transport.rs`
7. `src/pipeline.rs`
8. `src/capture.rs`
9. `src/system_extension.rs`
10. `src/app.rs`
11. `src/extension_main.rs`
12. `src/main.rs`

This order starts with pure data and ends with platform integration.

## Design Direction

CameraMan should feel like a focused desktop utility:

- compact left control panel;
- 16:9 preview as the main surface;
- status bar for live state, fps, source count, rendered frames, virtual frames, and layout;
- checkboxes for source selection;
- segmented choices for input mode, layout, and fps;
- clear disabled states;
- no landing page, hero, decorative gradients, or marketing copy;
- errors should tell the user whether the problem is camera access, signing, system approval, or runtime capture.

Current design issues:

- The left panel is a fixed manual column, not a resizable `SidePanel`.
- The install-extension button is visible even when the bundle is ad-hoc signed.
- A stale preview can remain while switching source modes.
- The app does not persist selected layout, fps, sources, or export path.
- The frame-spool path is shown as monospace text but has no copy/reveal action.

## Three Iterations

### Iteration 1: Rust Core

Done:

- Rust-only crate structure.
- Validated BGRA frame model.
- Frame metadata.
- Layout calculation.
- Pure Rust compositor.
- Pipeline orchestration.
- PPM debug output.
- Unit tests for core behavior.

Reviewed and fixed:

- Zero-width/height layout edge cases.
- Overflow-prone frame size math.
- Overflow-prone source sampling math.
- Unbuffered PPM writes.
- Per-source pipeline failure now degrades to an empty cell instead of killing the whole tick.

### Iteration 2: Real Input and App

Done:

- Camera discovery through `nokhwa`.
- Background capture thread.
- Multi-camera real selection.
- App preview with source toggles, layout controls, fps mode, status bar, and PPM export.
- Real camera resources are released on stop/uncheck.

Reviewed and fixed:

- Capture error classification is typed.
- Capture thread panics are isolated.
- Capture thread cleanup has a nonblocking join path.
- Real mode no longer supports only one camera.
- Auto fps can follow negotiated camera rates.

### Iteration 3: Virtual Output and Packaging

Done:

- Frame-spool sink from app to extension.
- Rust CoreMediaIO provider/device/stream prototype.
- Placeholder fallback inside extension.
- System-extension activation request UI.
- `.app` and `.systemextension` bundling.
- Ad-hoc development signing for launchable local bundles.

Reviewed and fixed:

- Extension service uses a run loop instead of a sleep loop.
- Format description is cached instead of recreated per frame.
- Extension creation errors are logged instead of panicking immediately.
- Release bundle now builds the extension in release mode too.

Still open:

- Real install needs provisioning profile with system-extension entitlement.
- Frame-spool file transport is too heavy for production.
- CoreMediaIO unsafe callbacks need panic containment.
- Client authorization is logged but not enforced.
- Bundle id and extension id are duplicated across modules.

## Review Notes

Recent review result:

- `cargo fmt --check` passed before this documentation pass.
- `cargo clippy -- -D warnings` passed before this documentation pass.
- `cargo test` passed with 20 tests before this documentation pass.
- `cargo run -- bundle` creates `target/CameraMan.app`.
- Ad-hoc `CameraMan.app` launches but cannot install the extension.
- Apple Development signing without a matching provisioning profile was verified to trigger `No matching profile found`; the bundler now avoids embedding the restricted entitlement unless a profile path is provided.

High-priority fixes next:

- Add deeper provisioning-profile validation.
- Disable or explain `Install extension` when entitlement/profile is missing.
- Replace file spool with app-group/shared-memory IPC.
- Add integration smoke tests for bundle entitlements.
- Add visual app screenshot checks for the egui UI.
