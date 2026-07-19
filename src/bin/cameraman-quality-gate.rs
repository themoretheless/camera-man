use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 {
        eprintln!("usage: cameraman-quality-gate <reference-video> <candidate-video>");
        return ExitCode::from(2);
    }
    let reference = PathBuf::from(&arguments[0]);
    let candidate = PathBuf::from(&arguments[1]);
    if !reference.is_file() || !candidate.is_file() {
        eprintln!("both VMAF inputs must be existing files");
        return ExitCode::from(2);
    }

    let filter = String::from(
        "[0:v]setpts=PTS-STARTPTS[ref];[1:v]setpts=PTS-STARTPTS[dist];[dist][ref]libvmaf=log_fmt=json:log_path=-",
    );
    match Command::new("ffmpeg")
        .args(["-hide_banner", "-nostdin", "-i"])
        .arg(reference)
        .arg("-i")
        .arg(candidate)
        .args(["-lavfi", &filter, "-f", "null", "-"])
        .status()
    {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!("could not run offline ffmpeg/libvmaf: {error}");
            ExitCode::FAILURE
        }
    }
}
