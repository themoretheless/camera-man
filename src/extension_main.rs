#[cfg(target_os = "macos")]
#[path = "extension/mod.rs"]
mod macos_extension;

#[cfg(target_os = "macos")]
fn main() {
    macos_extension::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("CameraMan CoreMediaIO extension is available only on macOS.");
}
