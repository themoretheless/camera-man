# Isolated resize benchmark

This crate intentionally does not depend on CameraMan, UI frameworks, camera
drivers, or Swift runtime libraries. It compares CameraMan's cached nearest
mapping with `fast_image_resize`, counts full-frame pixel mismatches, and
rejects adoption when the sampling contracts differ even if the adapter is
faster.

```sh
cargo run --release --manifest-path tools/resize-bench/Cargo.toml
cargo run --release --manifest-path tools/resize-bench/Cargo.toml \
  --target x86_64-apple-darwin
```
