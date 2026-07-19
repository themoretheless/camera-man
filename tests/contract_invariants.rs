use camera_man::{
    COLOR_CONTRACT_SCHEMA_VERSION, Colorimetry, FIXED_OUTPUT_CAPABILITY, FrameContract,
    FrameOwnershipMode, LatencyMode, PixelFormat, SCENE_SCHEMA_VERSION, SHARED_PROTOCOL_MAGIC,
    SHARED_PROTOCOL_SLOT_COUNT, SHARED_PROTOCOL_VERSION, VIRTUAL_CAMERA_HEIGHT,
    VIRTUAL_CAMERA_WIDTH, WirePixelFormat,
};

#[test]
fn wire_and_media_contract_changes_are_explicit_review_events() {
    assert_eq!(SHARED_PROTOCOL_MAGIC, 0x434D_414E);
    assert_eq!(SHARED_PROTOCOL_VERSION, 5);
    assert_eq!(SHARED_PROTOCOL_SLOT_COUNT, 3);
    assert_eq!(WirePixelFormat::Bgra8.code(), 1);
    assert_eq!(SCENE_SCHEMA_VERSION, 3);
    assert_eq!(COLOR_CONTRACT_SCHEMA_VERSION, 1);
    assert_eq!(Colorimetry::BT709_FULL_OPAQUE.schema_version, 1);
    assert_eq!(Colorimetry::BT709_FULL_OPAQUE.bit_depth, 8);
    assert_eq!(
        Colorimetry::from_wire_code(Colorimetry::BT709_FULL_OPAQUE.wire_code()),
        Some(Colorimetry::BT709_FULL_OPAQUE)
    );

    assert_eq!(FIXED_OUTPUT_CAPABILITY.width, VIRTUAL_CAMERA_WIDTH);
    assert_eq!(FIXED_OUTPUT_CAPABILITY.height, VIRTUAL_CAMERA_HEIGHT);
    assert_eq!(FIXED_OUTPUT_CAPABILITY.pixel_format, PixelFormat::Bgra8);
    assert_eq!(
        FIXED_OUTPUT_CAPABILITY.colorimetry,
        Colorimetry::BT709_FULL_OPAQUE
    );
    assert_eq!(
        FIXED_OUTPUT_CAPABILITY.latency_mode,
        LatencyMode::InteractiveLatestFrame
    );
    assert_eq!(
        FIXED_OUTPUT_CAPABILITY.ownership,
        FrameOwnershipMode::BorrowedUntilRelease
    );
    assert!(
        FrameContract::canonical_bgra(VIRTUAL_CAMERA_WIDTH, VIRTUAL_CAMERA_HEIGHT)
            .is_canonical_bgra(VIRTUAL_CAMERA_WIDTH, VIRTUAL_CAMERA_HEIGHT)
    );
}
