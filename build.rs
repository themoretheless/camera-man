use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=CAMERAMAN_BUILD_HASH");
    track_git_metadata();

    let build_hash = std::env::var("CAMERAMAN_BUILD_HASH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(git_build_hash)
        .unwrap_or_else(|| String::from("unknown"));
    println!("cargo:rustc-env=CAMERAMAN_BUILD_HASH={build_hash}");
}

fn track_git_metadata() {
    let Some(head) = git_path("HEAD") else {
        return;
    };
    println!("cargo:rerun-if-changed={}", head.display());

    if let Ok(contents) = fs::read_to_string(&head)
        && let Some(reference) = contents.strip_prefix("ref: ").map(str::trim)
        && let Some(reference_path) = git_path(reference)
    {
        println!("cargo:rerun-if-changed={}", reference_path.display());
    }
    if let Some(packed_refs) = git_path("packed-refs") {
        println!("cargo:rerun-if-changed={}", packed_refs.display());
    }
}

fn git_path(name: &str) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", name])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
        .filter(|path| !path.as_os_str().is_empty())
}

fn git_build_hash() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}
