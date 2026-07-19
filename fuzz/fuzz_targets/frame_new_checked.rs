#![no_main]

use camera_man::{Frame, PixelFormat};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    let Some(dimensions) = input.get(..8) else {
        return;
    };
    let width = u32::from_le_bytes(dimensions[..4].try_into().unwrap());
    let height = u32::from_le_bytes(dimensions[4..].try_into().unwrap());
    let pixels = input[8..].to_vec();

    let _ = Frame::new_checked(width, height, PixelFormat::Bgra8, pixels);
});
