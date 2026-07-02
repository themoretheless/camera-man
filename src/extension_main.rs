use std::thread;
use std::time::Duration;

fn main() {
    eprintln!("CameraMan Rust CoreMediaIO extension process started.");
    eprintln!(
        "Provider implementation is not active yet; this binary is packaged as the Rust system-extension target."
    );

    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
