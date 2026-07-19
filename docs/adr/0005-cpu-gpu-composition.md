# ADR 0005: CPU and GPU Composition

Status: accepted for CPU; GPU remains experimental

## Context

GPU composition can avoid copies only when the surrounding pipeline remains on
GPU/IOSurface. A compute pass followed by CPU readback can be slower than the
optimized CPU scaler.

## Decision

Keep the reusable CPU compositor as production default. Rayon is thresholded
and feature-gated. `fast_image_resize` needs wins on ARM64 and x86_64 before
adoption. The pure-Rust `wgpu` path has pixel parity and mandatory CPU fallback
but remains behind an experiment feature. Metal/CoreVideo interop uses Rust
`objc2` bindings; no Swift source is introduced.

## Evidence

On Apple M4 Max, 1920x1080 to 960x540 nearest measured 0.240 ms native versus
0.141 ms `fast_image_resize`, but the candidate uses a different nearest-pixel
contract. A 2026-07-18 optimized 30-sample rerun of the 1920x1080 -> 1280x720
`wgpu` path measured 1.780 ms with readback versus 0.470 ms CPU, a 3.79x ratio.
Debug custom-benchmark output from `cargo test --all-targets` is not accepted
as performance evidence. Mandatory readback still erases the GPU benefit.
