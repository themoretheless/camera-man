use std::process::{Command, Output};

fn run(command: &str) -> Output {
    run_args(&[command])
}

fn run_args(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_camera-man"))
        .args(arguments)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {arguments:?}: {error}"))
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("CLI stdout must be UTF-8")
}

#[test]
fn status_reports_rust_ram_transport_and_memory_limit() {
    let output = run("status");

    assert!(output.status.success());
    let stdout = stdout(&output);
    assert!(stdout.contains("CameraMan (Rust)"));
    assert!(stdout.contains("via shared memory"));
    assert!(stdout.contains("Frame memory limit:"));
}

#[test]
fn check_prints_normalized_configuration() {
    let output = run("check");

    assert!(output.status.success());
    let stdout = stdout(&output);
    assert!(stdout.contains("config.width=1920"));
    assert!(stdout.contains("config.height=1080"));
    assert!(stdout.contains("config.max_frame_bytes="));
}

#[test]
fn help_lists_app_and_bundle_commands() {
    let output = run("help");

    assert!(output.status.success());
    let stdout = stdout(&output);
    assert!(stdout.contains("app"));
    assert!(stdout.contains("bundle"));
    assert!(stdout.contains("diagnose-extension"));
}

#[test]
fn version_reports_package_version() {
    let output = run("version");

    assert!(output.status.success());
    assert_eq!(
        stdout(&output).trim(),
        format!("CameraMan {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn bundle_help_documents_signing_and_release_checks() {
    let output = run_args(&["bundle", "--help"]);

    assert!(output.status.success());
    let stdout = stdout(&output);
    assert!(stdout.contains("CODESIGN_IDENTITY"));
    assert!(stdout.contains("CAMERAMAN_PROVISIONING_PROFILE"));
    assert!(stdout.contains("CAMERAMAN_EXTENSION_PROVISIONING_PROFILE"));
    assert!(stdout.contains("codesign --verify --deep --strict"));
}

#[test]
fn extension_diagnostics_cover_profile_entitlements_and_system_state() {
    let output = run("diagnose-extension");

    assert!(output.status.success());
    let stdout = stdout(&output);
    assert!(stdout.contains("bundle.app.activation_location="));
    assert!(stdout.contains("provisioning.profile="));
    assert!(stdout.contains("provisioning.profile.team_id="));
    assert!(stdout.contains("provisioning.extension_profile="));
    assert!(stdout.contains("codesign.app.entitlements="));
    assert!(stdout.contains("codesign.extension.entitlements="));
    assert!(stdout.contains("codesign.app.team_id="));
    assert!(stdout.contains("team_id.app_extension_match="));
    assert!(stdout.contains("team_id.extension_profile_match="));
    assert!(stdout.contains("systemextensionsctl.status="));
}
