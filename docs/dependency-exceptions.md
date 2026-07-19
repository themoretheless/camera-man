# Dependency Exceptions

Exceptions are narrow, owned, dated, and temporary. A release is blocked by a
new RustSec advisory or source-policy failure unless this file and the matching
machine-readable configuration are changed in the same reviewed commit.
OSV findings from the published SPDX SBOM follow the same owner/expiry record;
`osv-scanner.toml` may suppress only IDs documented here.

## RustSec process

1. Confirm the advisory affects the locked graph and CameraMan's compiled macOS path.
2. Prefer upgrading or removing the dependency.
3. If no compatible fix exists, add an advisory ID and reason to `deny.toml`,
   record impact, mitigation, owner, and expiry here, and link the upstream issue.
4. Expiry is at most 90 days. CI continues to print ignored advisories.
5. Remove the exception as soon as the fixed lockfile lands.

### `RUSTSEC-2024-0436` (`paste 1.0.15`, unmaintained)

- **Path:** `nokhwa 0.10.11 -> paste 1.0.15`.
- **Impact:** maintenance advisory; no memory-safety or exploit advisory is
  reported. CameraMan does not call `paste` directly.
- **Mitigation:** no runtime input crosses a `paste` boundary; cargo-deny and
  cargo-audit still gate every other advisory.
- **Owner/review by:** camera capture boundary, 2026-10-12.
- **Removal condition:** a compatible nokhwa release removes or replaces
  `paste`, or CameraMan replaces the capture adapter.

### `spin 0.9.8` (yanked)

- **Path:** `nokhwa 0.10.11 -> flume 0.11.1 -> spin 0.9.8`.
- **Impact:** the locked crate was yanked but has no RustSec vulnerability
  advisory. Cargo refuses to select it for a new resolution, while existing
  lockfiles remain buildable.
- **Mitigation:** preserve and audit `Cargo.lock`; do not broaden the version.
- **Owner/review by:** camera capture boundary, 2026-10-12.
- **Removal condition:** upgrade nokhwa/flume to a graph without the yanked
  release and rerun camera reconnect plus soak tests.

`cargo-audit` cannot ignore one exact yanked package, so its broad yanked check
is disabled in `.cargo/audit.toml`. `cargo-deny` remains the authoritative
yanked-package gate and permits only `spin@0.9.8` with the reason above.

### `RUSTSEC-2026-0194` and `RUSTSEC-2026-0195` (`quick-xml 0.39.4`)

- **Path:** Linux-only `eframe 0.35.0 -> winit/egui-winit -> Wayland ->
  wayland-scanner 0.31.10 -> quick-xml 0.39.4`.
- **Impact:** crafted XML can cause quadratic work or unbounded namespace
  allocation. `cargo tree --target aarch64-apple-darwin` and the x86_64
  equivalent prove this package is absent from CameraMan's macOS build graph;
  Cargo.lock still records target-specific packages, so cargo-audit reports it.
- **Mitigation:** CameraMan is macOS-only; cargo-deny evaluates both supported
  macOS targets and admits no vulnerable package in either graph.
- **Owner/review by:** desktop UI boundary, 2026-08-14.
- **Removal condition:** update eframe/Wayland after upstream accepts
  `quick-xml >=0.41`, then remove both IDs from `.cargo/audit.toml`.

## `block 0.1.6` future incompatibility

- **Observed:** 2026-07-14.
- **Path:** `nokhwa 0.10.11 -> nokhwa-bindings-macos 0.2.4 -> core-video-sys
  0.1.4 -> metal 0.18.0 -> cocoa 0.20.2 -> block 0.1.6`.
- **Impact:** rustc reports future-incompatible code in a transitive macOS
  camera backend; current stable compilation still succeeds.
- **Why not removed now:** `nokhwa 0.10.11` remains the latest compatible
  upstream release. Replacing capture with a new AVFoundation adapter is a
  separate ownership and hardware-validation change, not a dependency bump.
- **Guard:** `scripts/verify-future-incompat.sh` permits this exact crate and
  rejects any additional future-incompatibility report.
- **Owner:** camera capture boundary.
- **Review by:** 2026-10-12, or immediately after a compatible nokhwa release.
- **Removal condition:** upgrade the backend, prove camera enumeration,
  negotiated formats, reconnect, cancellation, and release bundling, then
  delete the exception and guard.
