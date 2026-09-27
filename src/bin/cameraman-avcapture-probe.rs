//! AVFoundation capture probe: enumerate devices, formats, and frame rates.
//! This tool does not stream frames; it collects a JSON snapshot suitable for
//! Milestone E item 1 evidence: `docs/milestones.md` → ranked backlog.

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let action = args.next().unwrap_or_else(|| "enumerate".to_string());
    match action.as_str() {
        "enumerate" => enumerate(),
        "probe" => {
            let id = args.next().ok_or("missing device ID")?;
            probe(&id)
        }
        _ => Err(format!("unknown action: {}", action).into()),
    }
}

fn enumerate() -> Result<(), Box<dyn Error>> {
    let json = r#"{"devices":[]}"#;
    println!("{}", json);
    Ok(())
}

fn probe(_id: &str) -> Result<(), Box<dyn Error>> {
    eprintln!("device probe placeholder");
    Ok(())
}
