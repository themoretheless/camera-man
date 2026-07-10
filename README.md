# CameraMan

CameraMan is a Rust-only macOS camera compositor. It reads selected camera
sources, composes them into one BGRA frame, previews that frame in a desktop
app, and publishes the latest composed output to a Rust CoreMediaIO system
extension prototype.

## What Works

- Rust library, CLI, app, and extension binaries compile.
- The app launches with `cargo run`.
- Synthetic sources compose into a 1920x1080 preview.
- Real camera discovery and capture use `nokhwa`.
- Real mode supports multiple selected cameras.
- The app can start/stop preview, switch layout, switch fps mode, export PPM, and write virtual-output frames.
- `FrameSpoolSink` publishes the latest composed frame to the OS temp directory or `CAMERAMAN_FRAME_SPOOL`.
- `cameraman-extension` reads the frame spool and sends CoreMediaIO `CMSampleBuffer` frames.
- The extension generates placeholder frames when the app has not published a frame yet.
- The app can request system-extension activation and show the activation status.
- The CLI can build `target/CameraMan.app` and embed the `.systemextension`.
- 31 tests cover frame validation, layout, rendering, PPM output, pipeline behavior, capture classification, frame transport, activation-state guards, and provisioning validation.

## Important Limits

- The default bundle is ad-hoc signed. It launches, but macOS will not install the system extension from it.
- Installing the extension requires a provisioning profile that grants `com.apple.developer.system-extension.install`.
- Signing with an Apple Development certificate alone is not enough; without a matching provisioning profile, macOS kills the app with `No matching profile found`.
- The file-based frame spool is a development bridge, not the final production IPC.
- The CoreMediaIO extension is a working prototype and still needs unsafe-boundary hardening.

## Run

Launch the app:

```bash
cargo run
```

Launch explicitly:

```bash
cargo run -- app
```

Print status:

```bash
cargo run -- status
```

List cameras:

```bash
cargo run -- list-cameras
```

Capture one real camera frame:

```bash
cargo run -- capture-demo
```

Render a synthetic demo frame:

```bash
cargo run -- demo
```

Render three pipeline frames:

```bash
cargo run -- pipeline-demo
```

Build the app bundle:

```bash
cargo run -- bundle
open target/CameraMan.app
```

Build only the extension bundle:

```bash
cargo run -- bundle-extension
```

Run checks:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## System Extension Install

For local UI and frame-spool testing, the ad-hoc bundle is enough:

```bash
cargo run -- bundle
open target/CameraMan.app
```

For a real system-extension install, the app must be:

- built as a `.app`;
- launched from an installed bundle such as `/Applications/CameraMan.app`;
- signed with a valid Apple signing identity;
- embedded with a provisioning profile matching `com.cameraman.rust`;
- granted `com.apple.developer.system-extension.install`;
- approved by the user in System Settings after activation is requested.

`cargo run -- bundle` validates the supplied provisioning profile before it
embeds it: the profile must decode with `security cms`, match
`com.cameraman.rust`, and grant `com.apple.developer.system-extension.install`.

Build with explicit signing inputs:

```bash
CODESIGN_IDENTITY="Apple Development: Name (TEAMID)" \
CAMERAMAN_PROVISIONING_PROFILE="/path/to/CameraMan.provisionprofile" \
cargo run -- bundle
```

The app exposes an `Install extension` button, but it stays disabled (with an
explanatory tooltip) unless the running bundle is both launched from an
installed `.app` and actually signed with the `system-extension.install`
entitlement; macOS will still reject activation itself until signing and
provisioning are correct beyond that.

## Repository Layout

```text
src/main.rs             CLI entry point, demos, bundling, signing helpers
src/app.rs              egui desktop app and preview workflow
src/lib.rs              public module exports
src/config.rs           output and virtual camera config
src/error.rs            shared error and capture classification
src/frame.rs            BGRA frame and metadata model
src/layout.rs           row, column, grid cell calculation
src/render.rs           pure Rust compositor
src/camera.rs           camera discovery/source traits and synthetic source
src/capture.rs          real camera discovery/capture through nokhwa
src/virtual_camera.rs   virtual camera sink trait and memory sink
src/frame_transport.rs  file-based frame-spool bridge
src/pipeline.rs         source -> compose -> sink orchestration
src/ppm.rs              PPM writer and PPM sequence sink
src/system_extension.rs OSSystemExtensionRequest activation bridge
src/provisioning.rs     structured provisioning-profile validation
src/extension_main.rs   Rust CoreMediaIO provider/device/stream process

architecture.md         architecture, SOLID/DRY split, design notes, 3 iterations
recommendation.md       exactly 500 review items, improvements, problems, and next steps
```

## Architecture

The project is split around stable Rust boundaries:

```text
FrameSource
  -> CapturedFrame
  -> Compositor
  -> Frame
  -> VirtualCameraSink
```

Platform work is isolated:

- real camera input implements `FrameSource`;
- virtual output implements `VirtualCameraSink`;
- app-to-extension transport lives in `frame_transport.rs`;
- system-extension activation lives in `system_extension.rs`;
- CoreMediaIO provider code lives in `extension_main.rs`.

See [architecture.md](architecture.md) for the full SOLID/DRY map and learning path.

## Design Direction

CameraMan should feel like a focused desktop utility:

- dense controls, not a marketing page;
- preview first, diagnostics second;
- checkboxes for sources;
- segmented controls for mode/layout/fps;
- status text for real state, not decorative copy;
- clear warnings for signing, camera access, and extension approval.

Current UI gaps:

- settings are not persisted;
- the left panel is not resizable (it now scrolls, but is still a fixed-width column);
- the frame-spool path has no reveal/copy action;
- visual regression checks are not automated;
- signing diagnostics show capability but do not expose the certificate Team ID yet.

## Three Iterations

Iteration 1: Rust core

- frame model;
- layout calculation;
- compositor;
- pipeline;
- PPM output;
- core tests.

Iteration 2: Real input and app

- camera discovery;
- threaded capture;
- multi-camera real mode;
- app preview;
- fps controls;
- status bar and export.

Iteration 3: Virtual output and packaging

- frame-spool transport;
- Rust CoreMediaIO extension;
- system-extension activation request;
- app and extension bundling;
- development signing;
- documented production signing gap.
