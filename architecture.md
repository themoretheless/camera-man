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
  EXTENSION_BUNDLE_ID
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
- extension bundle id: `system_extension::EXTENSION_BUNDLE_ID`, reused by `main.rs`
  for the extension's `Info.plist` and `.systemextension` bundle path.

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

- The left panel is a fixed manual column, not a resizable `SidePanel` (it now
  scrolls with `egui::ScrollArea` so content is at least reachable, but a
  fixed-width column is still the wrong tool once the control set grows).
- A stale preview can remain while switching source modes.
- The app does not persist selected layout, fps, sources, or export path.
- The frame-spool path is shown as monospace text but has no copy/reveal action.
- `ExtensionActivationStatus::Requesting` and `Idle` share the same dim color,
  so there is no visual feedback that an async activation request is actually
  in flight versus nothing having happened yet.
- Fixed fps mode gives no feedback when the chosen rate exceeds what the
  camera can actually deliver (Auto mode's caption is the only place that
  surfaces a negotiated rate).

Fixed in this pass:

- The install-extension button used to stay clickable even when activation
  was guaranteed to fail. It is now disabled (with an explanatory hover text)
  unless `extension_capable()` confirms the running bundle is both launched
  from an installed `.app` and signed with the `system-extension.install`
  entitlement.
- The button and its status used to sit directly under the "Output" controls
  with no visual separation; it now has its own "System Extension" label and
  separator, matching how "Output" is grouped.
- The control column had no scroll area: once the fps and system-extension
  controls were added, the bottom of the column (the Output section, the
  Install extension button) was clipped with no way to reach it, even at the
  default window size, not just near `with_min_inner_size`. It now wraps in
  `egui::ScrollArea::vertical()`.

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
- A permanently broken camera in a multi-camera composite used to re-post its
  error every single tick: `set_event` always resets the message's TTL clock,
  so a continuously recurring error never expired and blocked any other
  status-bar message from ever being seen again, and the global
  `capture_error_streak`/auto-stop never engaged because it only sees the
  all-cameras-failing case. Each real source now tracks its own failure
  streak, posts its error once (not every tick), and is auto-dropped from the
  selection once its own streak crosses `MAX_CAPTURE_ERROR_STREAK`.
- If every selected real camera disappeared (or got auto-dropped) while
  running, the status bar kept showing green "Preview running" with nothing
  actually being captured. `render_preview`'s empty-frames path now stops the
  preview and posts an explicit event when that happens while running.

Still open:

- A real camera whose `open()` call itself hangs (a stuck driver, or a device
  `nokhwa` enumerates but never actually streams) is indistinguishable from
  one that is merely warming up: it never reports an error, so it never
  counts toward any failure streak and is never auto-dropped. Fixing this
  needs a bounded open-timeout in `ThreadedNokhwaFrameSource`'s capture
  worker, generous enough not to misclassify a legitimately slow-but-working
  camera as broken.

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
- `stop_streaming` used to only flip a bool; `start_streaming` could then race
  ahead and spawn a second `stream_samples` thread before the first one
  noticed and exited, leaving two threads sending samples to the same
  `CMIOExtensionStream` concurrently. `start_streaming` now joins the previous
  worker thread before spawning a replacement.
- The extension bundle id was duplicated across `main.rs`'s `Info.plist`
  template and the `.systemextension` bundle path; both now derive from
  `system_extension::EXTENSION_BUNDLE_ID`.

Still open:

- Real install needs provisioning profile with system-extension entitlement.
- Frame-spool file transport is too heavy for production.
- CoreMediaIO unsafe callbacks need panic containment.
- Client authorization is logged but not enforced.

## Review Notes

Recent review result:

- `cargo fmt --check` passed before this documentation pass.
- `cargo clippy -- -D warnings` passed before this documentation pass.
- `cargo test` passed with 20 tests before this documentation pass.
- `cargo run -- bundle` creates `target/CameraMan.app`.
- Ad-hoc `CameraMan.app` launches but cannot install the extension.
- Apple Development signing without a matching provisioning profile was verified to trigger `No matching profile found`; the bundler now avoids embedding the restricted entitlement unless a profile path is provided.

Second review pass (after the above):

- `cargo test` actually passes 22 tests, not 20; the count above was stale
  even at the time it was written.
- `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test` were
  re-verified clean against the current tree.
- A 15-agent adversarial review targeted the four areas this document itself
  flagged as needing attention (signing/provisioning, stale documentation,
  app UI state, unsafe extension boundaries) and confirmed nine real issues:
  the extension-host thread-join race, the two `app.rs` status-bar/auto-stop
  bugs, and six stale documentation claims (test count, bundle-id duplication
  claimed as unresolved after it had already been fixed, the install-extension
  button description, and a Module Map omission). All are fixed above or in
  this pass; see the Iteration sections for specifics.
- `security find-identity -v -p codesigning` on this machine shows one
  identity: `Apple Development: d.o.mezhov@gmail.com (VBA8KCMNX7)`, a free
  Xcode personal-team certificate. It can sign a plain app, which is what let
  earlier testing reproduce `No matching profile found` for real; it is not a
  paid Apple Developer Program membership and cannot carry the
  System Extension capability, so it does not change the real-install gap
  below.
- A design-critique pass on the current egui UI (multi-camera checkboxes,
  Target FPS, Install extension button/status) found the control-column
  overflow, the ungrouped extension section, and the always-enabled install
  button described under "Current design issues" and "Fixed in this pass"
  above; the first three were fixed, two remain open.

High-priority fixes next:

- Add deeper provisioning-profile validation.
- Replace file spool with app-group/shared-memory IPC.
- Add integration smoke tests for bundle entitlements.
- Add visual app screenshot checks for the egui UI.
- Add a bounded open-timeout for real cameras that hang during `open()`.
