use camera_man::{
    AlphaMode, CleanAperture, ColorRange, Colorimetry, CompositionLayout, Compositor,
    FIXED_OUTPUT_FORMAT, FormatEpochCoordinator, Frame, FrameContract, FrameView, PixelAspectRatio,
    PixelFormat, Rotation, ScalingFilter, TransformMetadata, VideoFormat,
};

#[test]
fn generated_contract_combinations_stay_bounded_and_opaque() {
    let rotations = [
        Rotation::Degrees0,
        Rotation::Degrees90,
        Rotation::Degrees180,
        Rotation::Degrees270,
    ];
    let alpha_modes = [
        AlphaMode::Opaque,
        AlphaMode::Straight,
        AlphaMode::Premultiplied,
    ];
    let filters = [ScalingFilter::Nearest, ScalingFilter::Bilinear];

    for rotation in rotations {
        for mirror_horizontal in [false, true] {
            for mirror_vertical in [false, true] {
                for alpha in alpha_modes {
                    for filter in filters {
                        let mut colorimetry = Colorimetry::BT709_FULL_OPAQUE;
                        colorimetry.alpha = alpha;
                        colorimetry.range = ColorRange::Full;
                        let contract = FrameContract {
                            colorimetry,
                            pixel_aspect_ratio: PixelAspectRatio::new(4, 3).unwrap(),
                            clean_aperture: CleanAperture {
                                x: 1,
                                y: 0,
                                width: 2,
                                height: 2,
                            },
                            transform: TransformMetadata {
                                rotation,
                                mirror_horizontal,
                                mirror_vertical,
                            },
                        };
                        let mut pixels = vec![0_u8; 3 * 2 * 4];
                        for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
                            let alpha_byte = if alpha == AlphaMode::Opaque { 255 } else { 128 };
                            pixel.copy_from_slice(&[
                                index as u8 * 11,
                                index as u8 * 7,
                                index as u8 * 3,
                                alpha_byte,
                            ]);
                        }
                        let frame = Frame::new_checked_with_contract(
                            3,
                            2,
                            PixelFormat::Bgra8,
                            contract,
                            pixels,
                        )
                        .unwrap();
                        let compositor = Compositor::new(VideoFormat {
                            width: 7,
                            height: 5,
                            fps: 30,
                            pixel_format: PixelFormat::Bgra8,
                        })
                        .with_scaling_filter(filter);

                        let output = compositor
                            .compose(&[Some(frame)], CompositionLayout::Grid)
                            .unwrap();
                        assert_eq!(output.data().len(), 7 * 5 * 4);
                        assert!(output.data().chunks_exact(4).all(|pixel| pixel[3] == 255));
                        assert!(output.contract().is_canonical_bgra(7, 5));
                    }
                }
            }
        }
    }
}

#[test]
fn generated_odd_strides_copy_only_active_pixels() {
    for width in 1..=9_u32 {
        for height in 1..=7_u32 {
            let active = width as usize * 4;
            let stride = active + (width as usize % 5) + 1;
            let mut bytes = vec![0xEE; stride * height as usize];
            for y in 0..height as usize {
                for (x, pixel) in bytes[y * stride..y * stride + active]
                    .chunks_exact_mut(4)
                    .enumerate()
                {
                    pixel.copy_from_slice(&[x as u8, y as u8, 17, 255]);
                }
            }
            let view =
                FrameView::new_checked(width, height, PixelFormat::Bgra8, stride, &bytes).unwrap();
            let frame = view.to_owned_tightly_packed().unwrap();
            assert_eq!(frame.data().len(), active * height as usize);
            assert!(!frame.data().contains(&0xEE));
        }
    }
}

#[test]
fn generated_format_epochs_commit_only_after_reset() {
    let coordinator = FormatEpochCoordinator::new(FIXED_OUTPUT_FORMAT);
    let mut expected_epoch = 1;
    for fps in [24, 30, 60, 15, 30] {
        let mut format = FIXED_OUTPUT_FORMAT;
        format.fps = fps;
        let before = coordinator.snapshot();
        let transition = coordinator
            .renegotiate(format, |candidate| {
                assert_ne!(candidate.epoch, 0);
                Ok(())
            })
            .unwrap();
        if before.format != format {
            expected_epoch += 1;
            assert_eq!(transition.unwrap().current.epoch, expected_epoch);
        } else {
            assert!(transition.is_none());
        }
        assert_eq!(coordinator.snapshot().format, format);
    }
}
