# Contributing

CameraMan is currently an early Rust/macOS prototype. Keep changes small,
reviewable, and inside the module that owns the behavior.

## Development Workflow

1. Read `architecture.md`, then follow its learning path from pure frame/layout
   code toward platform adapters.
2. Add or update focused tests beside the changed behavior.
3. Keep CoreMediaIO and Objective-C work inside the existing macOS adapter
   modules; keep pure frame/render logic platform-independent.
4. Never commit certificates, private keys, provisioning profiles, generated
   bundles, IDE metadata, or `target/` output.
5. Run the checks below before proposing a change.

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --no-fail-fast
cargo doc --no-deps
cargo fmt --manifest-path fuzz/Cargo.toml --check
cargo clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings
```

## Release Checklist

1. Confirm `recommendation.md` still contains exactly 600 sequential items.
2. Update the current test count and known limitations in `README.md`,
   `architecture.md`, `research.md`, and `CHANGELOG.md` when affected.
3. Run all development checks above.
4. Build and inspect the final artifacts.

```bash
cargo run --release -- bundle
plutil -lint target/CameraMan.app/Contents/Info.plist
plutil -lint target/CameraMan.app/Contents/Library/SystemExtensions/com.cameraman.rust.extension.systemextension/Contents/Info.plist
codesign --verify --deep --strict --verbose=2 target/CameraMan.app
cargo run -- diagnose-extension
```

5. Launch `target/CameraMan.app`, verify preview controls at desktop and compact
   window sizes, and confirm no controls overlap or become unreachable.
6. For an installable build, require a matching identity, separate host and
   extension provisioning profiles, restricted entitlement, App Group, and
   successful system-extension approval.
7. Record user-visible changes in `CHANGELOG.md` before creating a release tag.

## License Status

No open-source license has been selected yet. The repository owner must choose
and add a license before public redistribution or accepting external
contributions under explicit terms.
