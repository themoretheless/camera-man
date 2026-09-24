use super::*;

/// The pixel format and dimensions never change, so the description is
/// built once and reused. `formats()` is queried by clients and
/// `create_sample_buffer` calls this once per delivered frame;
/// recreating a `CMVideoFormatDescription` on every call both violates the
/// CMIOExtensionStreamSource contract that formats do not change between
/// calls and, per the Core Foundation Create Rule, leaked one reference
/// per call since nothing here ever released the previous one.
pub(super) fn cached_video_format_description() -> Option<CFRetained<CMFormatDescription>> {
    static DESCRIPTION: OnceLock<Option<CFRetained<CMFormatDescription>>> = OnceLock::new();
    DESCRIPTION.get_or_init(video_format_description).clone()
}

pub(super) fn video_format_description() -> Option<CFRetained<CMFormatDescription>> {
    let mut raw_description: *const CMVideoFormatDescription = std::ptr::null();
    // SAFETY: dimensions are positive shared constants and the out-parameter
    // points to writable storage for one retained Core Foundation object.
    let status = unsafe {
        CMVideoFormatDescriptionCreate(
            None,
            kCVPixelFormatType_32BGRA,
            VIRTUAL_CAMERA_WIDTH as i32,
            VIRTUAL_CAMERA_HEIGHT as i32,
            None,
            NonNull::from(&mut raw_description),
        )
    };

    if status != 0 {
        eprintln!("failed to create CMVideoFormatDescription: OSStatus {status}");
        return None;
    }

    let description = NonNull::new(raw_description.cast_mut())?;
    // SAFETY: successful CMVideoFormatDescriptionCreate returns this non-null
    // object at +1 ownership, transferred exactly once into `CFRetained`.
    Some(unsafe { CFRetained::<CMFormatDescription>::from_raw(description) })
}

pub(super) struct OutputPixelBufferPool {
    pool: CFRetained<CVPixelBufferPool>,
    allocation_attributes: CFRetained<CFDictionary<CFType, CFType>>,
}

impl OutputPixelBufferPool {
    pub(super) fn new() -> Option<Self> {
        const MINIMUM_BUFFER_COUNT: i32 = 3;
        const MAXIMUM_OUTSTANDING_BUFFERS: i32 = 6;
        Self::new_with_limits(
            VIRTUAL_CAMERA_WIDTH,
            VIRTUAL_CAMERA_HEIGHT,
            MINIMUM_BUFFER_COUNT,
            MAXIMUM_OUTSTANDING_BUFFERS,
        )
    }

    pub(super) fn new_with_limits(
        width_pixels: u32,
        height_pixels: u32,
        minimum_buffer_count: i32,
        maximum_outstanding_buffers: i32,
    ) -> Option<Self> {
        debug_assert!(minimum_buffer_count > 0);
        debug_assert!(maximum_outstanding_buffers >= minimum_buffer_count);
        let minimum_count = CFNumber::new_i32(minimum_buffer_count);
        // SAFETY: key/value slices have equal lengths and contain valid live
        // Core Foundation objects for the duration of dictionary creation.
        let pool_attributes = unsafe {
            CFDictionary::<CFType, CFType>::from_slices(
                &[kCVPixelBufferPoolMinimumBufferCountKey.as_ref()],
                &[minimum_count.as_ref()],
            )
        };

        let width = CFNumber::new_i64(i64::from(width_pixels));
        let height = CFNumber::new_i64(i64::from(height_pixels));
        let pixel_format = CFNumber::new_i64(i64::from(kCVPixelFormatType_32BGRA));
        let io_surface_properties = CFDictionary::<CFType, CFType>::empty();
        let metal_compatible = CFBoolean::new(true);
        // SAFETY: the five key/value entries are length-matched valid CF
        // objects; the dictionary retains everything it stores.
        let pixel_attributes = unsafe {
            CFDictionary::<CFType, CFType>::from_slices(
                &[
                    kCVPixelBufferWidthKey.as_ref(),
                    kCVPixelBufferHeightKey.as_ref(),
                    kCVPixelBufferPixelFormatTypeKey.as_ref(),
                    kCVPixelBufferIOSurfacePropertiesKey.as_ref(),
                    kCVPixelBufferMetalCompatibilityKey.as_ref(),
                ],
                &[
                    width.as_ref(),
                    height.as_ref(),
                    pixel_format.as_ref(),
                    io_surface_properties.as_ref(),
                    metal_compatible.as_ref(),
                ],
            )
        };

        let mut raw_pool: *mut CVPixelBufferPool = std::ptr::null_mut();
        // SAFETY: both dictionaries are valid and the out-parameter points to
        // writable storage for one pool pointer.
        let status = unsafe {
            CVPixelBufferPool::create(
                None,
                Some(erase_dictionary_types(&pool_attributes)),
                Some(erase_dictionary_types(&pixel_attributes)),
                NonNull::from(&mut raw_pool),
            )
        };
        if status != 0 {
            eprintln!("failed to create CVPixelBufferPool: CVReturn {status}");
            return None;
        }
        let pool = NonNull::new(raw_pool)?;
        // SAFETY: successful CVPixelBufferPoolCreate returns this non-null
        // pool at +1 ownership.
        let pool = unsafe { CFRetained::from_raw(pool) };
        let allocation_threshold = CFNumber::new_i32(maximum_outstanding_buffers);
        // SAFETY: the key/value slices have equal lengths and contain live
        // Core Foundation objects; the dictionary retains the threshold.
        let allocation_attributes = unsafe {
            CFDictionary::<CFType, CFType>::from_slices(
                &[kCVPixelBufferPoolAllocationThresholdKey.as_ref()],
                &[allocation_threshold.as_ref()],
            )
        };
        Some(Self {
            pool,
            allocation_attributes,
        })
    }

    pub(super) fn create_pixel_buffer(&self) -> Option<CFRetained<CVPixelBuffer>> {
        let mut raw_buffer: *mut CVPixelBuffer = std::ptr::null_mut();
        // SAFETY: the pool and auxiliary dictionary are live and correctly
        // typed, and the out-parameter is writable for one retained pointer.
        let status = unsafe {
            CVPixelBufferPool::create_pixel_buffer_with_aux_attributes(
                None,
                &self.pool,
                Some(erase_dictionary_types(&self.allocation_attributes)),
                NonNull::from(&mut raw_buffer),
            )
        };
        if status != 0 {
            if status != kCVReturnWouldExceedAllocationThreshold {
                eprintln!("failed to allocate from CVPixelBufferPool: CVReturn {status}");
            }
            record_drop(DropReason::PoolExhausted, 1, None);
            return None;
        }
        let buffer = NonNull::new(raw_buffer)?;
        // SAFETY: successful pool allocation returns the non-null buffer at
        // +1 ownership, transferred exactly once into `CFRetained`.
        Some(unsafe { CFRetained::<CVPixelBuffer>::from_raw(buffer) })
    }
}

pub(super) fn erase_dictionary_types(dictionary: &CFDictionary<CFType, CFType>) -> &CFDictionary {
    // SAFETY: Core Foundation erases dictionary key/value parameters at the C
    // boundary, and both Rust instantiations have the same transparent layout.
    unsafe { &*std::ptr::from_ref(dictionary).cast::<CFDictionary>() }
}

pub(super) fn create_sample_buffer(
    fps: u32,
    host_time_nanos: u64,
    pixel_buffer: &CVPixelBuffer,
) -> Option<CFRetained<CMSampleBuffer>> {
    let format_description = cached_video_format_description()?;
    let timescale = sample_timescale(fps);
    // SAFETY: value 1 and the clamped positive timescale form a valid CMTime.
    let duration = unsafe { CMTime::new(1, timescale) };
    let presentation_value = i64::try_from(host_time_nanos).unwrap_or(i64::MAX);
    // SAFETY: the nanosecond timescale is positive and the value fits i64.
    let presentation_time = unsafe { CMTime::new(presentation_value, 1_000_000_000) };
    // SAFETY: this immutable CoreMedia constant is valid for direct copying.
    let decode_time = unsafe { kCMTimeInvalid };
    let mut timing = CMSampleTimingInfo {
        duration,
        // Real host-clock nanoseconds, not sequence/fps: that stays
        // monotonic even across an fps change mid-stream, whereas a
        // frame-count-scaled timestamp would jump backwards the instant
        // fps changes (e.g. sequence 100 at 30fps is 3.33s in, but the
        // same sequence at 60fps would be 1.67s: a step backwards).
        presentationTimeStamp: presentation_time,
        decodeTimeStamp: decode_time,
    };
    let mut raw_sample: *mut CMSampleBuffer = std::ptr::null_mut();
    // SAFETY: the image buffer, format description, and timing record remain
    // live through the call; the sample out-parameter is writable.
    let status = unsafe {
        CMSampleBuffer::create_ready_with_image_buffer(
            None,
            pixel_buffer,
            &format_description,
            NonNull::from(&mut timing),
            NonNull::from(&mut raw_sample),
        )
    };
    if status != 0 {
        eprintln!("failed to create CMSampleBuffer: OSStatus {status}");
        return None;
    }

    let sample = NonNull::new(raw_sample)?;
    // SAFETY: successful CMSampleBuffer creation returns this non-null object
    // at +1 ownership.
    Some(unsafe { CFRetained::<CMSampleBuffer>::from_raw(sample) })
}

pub(super) fn sample_timescale(fps: u32) -> i32 {
    fps.clamp(1, i32::MAX as u32) as i32
}

pub(super) fn create_pixel_buffer(
    sequence: u64,
    app_frame: Option<FrameView<'_>>,
    pixel_pool: &OutputPixelBufferPool,
) -> Option<CFRetained<CVPixelBuffer>> {
    let _pixel_buffer_span = stage_span(
        PipelineStage::PixelBuffer,
        sequence,
        VIRTUAL_CAMERA_WIDTH,
        VIRTUAL_CAMERA_HEIGHT,
    );
    let buffer = pixel_pool.create_pixel_buffer()?;
    attach_rec709_color_tags(&buffer);
    let flags = CVPixelBufferLockFlags::empty();
    // SAFETY: `buffer` is a live mutable pixel buffer and the matching unlock
    // is performed on every path after a successful lock.
    let lock_status = unsafe { CVPixelBufferLockBaseAddress(&buffer, flags) };
    if lock_status != 0 {
        eprintln!("failed to lock CVPixelBuffer: CVReturn {lock_status}");
        return None;
    }

    let base = CVPixelBufferGetBaseAddress(&buffer).cast::<u8>();
    let bytes_per_row = CVPixelBufferGetBytesPerRow(&buffer);
    if base.is_null() {
        // SAFETY: the lock above succeeded and uses these exact flags.
        unsafe {
            CVPixelBufferUnlockBaseAddress(&buffer, flags);
        }
        eprintln!("CVPixelBuffer returned a null base address");
        return None;
    }

    if let Some(frame) = app_frame.filter(|frame| {
        frame.pixel_format() == PixelFormat::Bgra8
            && frame
                .contract()
                .is_canonical_bgra(frame.width(), frame.height())
    }) {
        // SAFETY: the successful lock exposes a writable BGRA allocation with
        // the configured output height and at least `bytes_per_row` per row.
        unsafe { fill_from_frame_view(base, bytes_per_row, frame) };
    } else {
        // SAFETY: the successful lock exposes a writable BGRA allocation with
        // the configured output height and at least `bytes_per_row` per row.
        unsafe { fill_placeholder_frame(base, bytes_per_row, sequence) };
    }
    // SAFETY: the lock above succeeded and no derived pointer is used after
    // this matching unlock.
    let unlock_status = unsafe { CVPixelBufferUnlockBaseAddress(&buffer, flags) };
    if unlock_status != 0 {
        eprintln!("failed to unlock CVPixelBuffer: CVReturn {unlock_status}");
        return None;
    }

    Some(buffer)
}

pub(super) fn attach_rec709_color_tags(buffer: &CVPixelBuffer) {
    let mode = CVAttachmentMode::ShouldPropagate;
    // SAFETY: the buffer and all static attachment keys/values are live Core
    // Foundation objects; set_attachment retains values as required.
    unsafe {
        buffer.set_attachment(
            kCVImageBufferColorPrimariesKey,
            kCVImageBufferColorPrimaries_ITU_R_709_2.as_ref(),
            mode,
        );
        buffer.set_attachment(
            kCVImageBufferTransferFunctionKey,
            kCVImageBufferTransferFunction_ITU_R_709_2.as_ref(),
            mode,
        );
        buffer.set_attachment(
            kCVImageBufferYCbCrMatrixKey,
            kCVImageBufferYCbCrMatrix_ITU_R_709_2.as_ref(),
            mode,
        );
    }
}

/// Copies or scales one validated BGRA frame into a locked output buffer.
///
/// # Safety
///
/// `base` must point to a writable locked allocation containing at least
/// `VIRTUAL_CAMERA_HEIGHT * bytes_per_row` bytes, and every row must hold at
/// least `VIRTUAL_CAMERA_WIDTH * 4` bytes for the duration of this call.
pub(super) unsafe fn fill_from_frame_view(
    base: *mut u8,
    bytes_per_row: usize,
    frame: FrameView<'_>,
) {
    let source_width = frame.width() as usize;
    let source_height = frame.height() as usize;
    if source_width == 0 || source_height == 0 {
        return;
    }

    let output_width = VIRTUAL_CAMERA_WIDTH as usize;
    let output_height = VIRTUAL_CAMERA_HEIGHT as usize;
    let source_stride = source_width * PixelFormat::Bgra8.bytes_per_pixel();
    if source_width == output_width && source_height == output_height {
        for y in 0..output_height {
            // SAFETY: guaranteed by this function's destination contract;
            // FrameView validates each complete source row.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    frame
                        .row(y as u32)
                        .expect("frame view row was validated")
                        .as_ptr(),
                    base.add(y * bytes_per_row),
                    source_stride,
                );
            }
        }
        copy_ledger().record_copy(
            CopyStage::CoreVideoUpload,
            source_stride.saturating_mul(output_height),
        );
        return;
    }
    for y in 0..output_height {
        // The shared nearest sampler proves the row index stays inside the
        // source, so `row` cannot return `None` here.
        let source_y =
            nearest_source_coordinate(y as u32, source_height as u32, output_height as u32)
                as usize;
        let source_row = frame
            .row(source_y as u32)
            .expect("frame view row was validated");
        // SAFETY: the destination contract covers every output row offset.
        let row = unsafe { base.add(y * bytes_per_row) };
        for x in 0..output_width {
            let source_x =
                nearest_source_coordinate(x as u32, source_width as u32, output_width as u32)
                    as usize;
            let source_offset = source_x * 4;
            let target_offset = x * 4;
            let source_pixel = &source_row[source_offset..source_offset + 4];
            // SAFETY: source slicing proves four readable bytes and the
            // destination row contract proves four writable bytes.
            unsafe {
                std::ptr::copy_nonoverlapping(source_pixel.as_ptr(), row.add(target_offset), 4);
            }
        }
    }
    copy_ledger().record_copy(
        CopyStage::CoreVideoUpload,
        output_width.saturating_mul(output_height).saturating_mul(4),
    );
}

/// Writes the animated placeholder into a locked output buffer.
///
/// # Safety
///
/// `base` must point to a writable locked allocation containing at least
/// `VIRTUAL_CAMERA_HEIGHT * bytes_per_row` bytes, and every row must hold at
/// least `VIRTUAL_CAMERA_WIDTH * 4` bytes for the duration of this call.
pub(super) unsafe fn fill_placeholder_frame(base: *mut u8, bytes_per_row: usize, sequence: u64) {
    let phase = (sequence % VIRTUAL_CAMERA_WIDTH as u64) as usize;
    for y in 0..VIRTUAL_CAMERA_HEIGHT as usize {
        // SAFETY: the destination contract covers every output row offset.
        let row = unsafe { base.add(y * bytes_per_row) };
        for x in 0..VIRTUAL_CAMERA_WIDTH as usize {
            let offset = x * 4;
            let band = ((x + phase) / 160).is_multiple_of(2);
            let vertical = (y / 120).is_multiple_of(2);
            let pulse = ((sequence * 3 + x as u64 + y as u64) % 255) as u8;
            let (b, g, r) = if band ^ vertical {
                (48, 132, pulse.saturating_add(40))
            } else {
                (pulse / 3, 58, 96)
            };
            // SAFETY: `x` is bounded by output width and the row contract
            // provides four writable bytes for every output pixel.
            unsafe {
                *row.add(offset) = b;
                *row.add(offset + 1) = g;
                *row.add(offset + 2) = r;
                *row.add(offset + 3) = 255;
            }
        }
    }
}
