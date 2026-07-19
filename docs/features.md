# Cargo feature contract

The default build is the supported application path. Experimental features are
explicit, non-default and must keep the scalar CPU path as an oracle/fallback.

| Feature | Purpose | Support promise | CI |
|---|---|---|---|
| `camera-capture` | nokhwa/AVFoundation capture and app binary | Default production path | default, all-features, powerset |
| `parallel-compositor` | Thresholded Rayon composition | Experiment; disabled by default until AC ARM64/x86_64 acceptance | powerset, all-features |
| `resize-experiments` | Isolated `fast_image_resize` comparison | Benchmark only | powerset, all-features, benchmark |
| `gpu-compositor-experiment` | wgpu kernel plus measured readback path | Rejected for production while end-to-end slower | powerset, all-features, benchmark |
| `linear-light-experiment` | BT.709 linear-light bilinear reference | Quality experiment only | powerset, all-features |
| `metal-interop-experiment` | Rust CVMetalTextureCache bridge | Zero-readback experiment boundary only | powerset, all-features |
| `all-experiments` | Compile every experiment together | Build/test compatibility, not a production recommendation | all-features |

Required local checks:

```sh
cargo hack check --locked --no-default-features
cargo hack check --locked --feature-powerset --depth 2 --exclude-features all-experiments
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features -- --test-threads=1
```

Delete a feature when it has no user, benchmark owner or CI coverage. Adding a
feature does not authorize new network, updater or remote-identity behavior.
