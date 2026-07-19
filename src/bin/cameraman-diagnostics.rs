use std::path::{Path, PathBuf};

use camera_man::{DiagnosticsSnapshot, VideoFormat};

fn main() {
    let paths = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let path_refs = paths.iter().map(PathBuf::as_path).collect::<Vec<&Path>>();
    let snapshot = DiagnosticsSnapshot::capture(&[VideoFormat::hd_1080p_bgra()], &path_refs);
    println!(
        "{}",
        snapshot
            .to_json_pretty()
            .expect("diagnostics snapshot must serialize")
    );
}
