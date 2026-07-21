use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{CameraFormat, CameraIndex, RequestedFormat, RequestedFormatType, Resolution};
use nokhwa::{Camera, FormatDecoder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id = std::env::args().nth(1).unwrap_or_else(|| String::from("0"));
    let index = if let Some(unique_id) = id.strip_prefix("uid:") {
        CameraIndex::String(unique_id.to_owned())
    } else {
        id.parse::<u32>()
            .map(CameraIndex::Index)
            .unwrap_or_else(|_| CameraIndex::String(id.clone()))
    };
    let request = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::None);
    let mut camera = Camera::new(index.clone(), request)?;
    println!("initial {}", camera.camera_format());
    let mut formats = camera.compatible_camera_formats()?;
    formats.sort();
    formats.dedup();
    for format in formats {
        println!("{format}");
    }
    drop(camera);

    for frame_format in RgbAFormat::FORMATS {
        let target = CameraFormat::new(Resolution::new(1920, 1080), *frame_format, 30);
        let request = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::Closest(target));
        match Camera::new(index.clone(), request) {
            Ok(camera) => println!("closest {frame_format}: {}", camera.camera_format()),
            Err(error) => println!("closest {frame_format}: unavailable ({error})"),
        }
    }
    Ok(())
}
