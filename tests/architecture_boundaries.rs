use std::fs;
use std::path::Path;

const DOMAIN_MODULES: &[&str] = &[
    "src/frame.rs",
    "src/layout.rs",
    "src/media_contract.rs",
    "src/media_time.rs",
    "src/quality.rs",
    "src/scene_schema.rs",
];

const FORBIDDEN_DOMAIN_DEPENDENCIES: &[&str] = &[
    "eframe",
    "objc2",
    "CoreMediaIO",
    "core_media",
    "crate::app",
    "crate::capture",
    "crate::frame_transport",
    "crate::shared_memory_transport",
    "crate::transport",
    "crate::virtual_camera",
    "std::process::Command",
];

#[test]
fn domain_contracts_do_not_depend_on_ui_platform_cli_or_transport() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative_path in DOMAIN_MODULES {
        let source = fs::read_to_string(root.join(relative_path))
            .unwrap_or_else(|error| panic!("cannot read {relative_path}: {error}"));
        for forbidden in FORBIDDEN_DOMAIN_DEPENDENCIES {
            assert!(
                !source.contains(forbidden),
                "domain module {relative_path} crosses the architecture boundary via {forbidden}"
            );
        }
    }
}

#[test]
fn compositor_does_not_know_about_transport_or_ui() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        fs::read_to_string(root.join("src/render.rs")).expect("render.rs must be readable");
    for forbidden in [
        "eframe",
        "CoreMediaIO",
        "crate::frame_transport",
        "crate::shared_memory_transport",
        "crate::transport",
        "crate::virtual_camera",
    ] {
        assert!(
            !source.contains(forbidden),
            "composition crosses the architecture boundary via {forbidden}"
        );
    }
}

#[test]
fn background_workers_exchange_messages_without_camera_man_app_access() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative_path in [
        "src/camera_discovery_worker.rs",
        "src/render_worker.rs",
        "src/capture.rs",
    ] {
        let source = fs::read_to_string(root.join(relative_path)).unwrap();
        assert!(
            !source.contains("CameraManApp"),
            "background worker {relative_path} reaches into CameraManApp"
        );
    }
}

#[test]
fn unsafe_sources_have_documented_verification_owner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let register = fs::read_to_string(root.join("docs/unsafe-invariants.md")).unwrap();
    let mut sources = Vec::new();
    collect_rust_sources(&root.join("src"), root, &mut sources);
    for relative_path in sources {
        let source = fs::read_to_string(root.join(&relative_path)).unwrap();
        if source.contains("unsafe") {
            assert!(
                register.contains(&format!("`{relative_path}`")),
                "unsafe source {relative_path} has no verification owner"
            );
        }
    }
}

#[test]
fn requirement_gated_network_stack_is_not_in_the_local_build() {
    let manifest =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    for dependency in [
        "tokio",
        "rustls",
        "reqwest",
        "hyper",
        "quinn",
        "webrtc",
        "rtp",
        "tough",
        "self_update",
        "ureq",
    ] {
        assert!(
            !manifest.lines().any(|line| {
                line.trim_start()
                    .strip_prefix(dependency)
                    .is_some_and(|rest| rest.trim_start().starts_with('='))
            }),
            "network dependency {dependency} was added before the remote-source requirement"
        );
    }
    for feature in ["remote-source", "self-update", "updater"] {
        assert!(
            !manifest.lines().any(|line| {
                line.trim_start()
                    .strip_prefix(feature)
                    .is_some_and(|rest| rest.trim_start().starts_with('='))
            }),
            "feature {feature} was added before its threat model and requirement"
        );
    }
}

fn collect_rust_sources(directory: &Path, root: &Path, sources: &mut Vec<String>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rust_sources(&path, root, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
}
