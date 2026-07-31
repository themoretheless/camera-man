use std::ptr::NonNull;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicPtr, Ordering},
};
use std::thread;
use std::time::Duration;

use camera_man::{
    ClientAuthorization, ClientIdentity, CopyStage, DeadlinePacer, DropReason, FrameDiscontinuity,
    FrameIntegrityState, FrameObservation, FrameTransportReader, FrameView, IntegrityDropReason,
    LifecycleError, PipelineStage, PixelFormat, StartAction, StopAction, StreamLifecycle,
    TransportFrameRef, VIRTUAL_CAMERA_DEFAULT_FPS, VIRTUAL_CAMERA_DEVICE_NAME,
    VIRTUAL_CAMERA_DEVICE_UID, VIRTUAL_CAMERA_HEIGHT, VIRTUAL_CAMERA_MAX_FPS,
    VIRTUAL_CAMERA_MIN_FPS, VIRTUAL_CAMERA_STREAM_NAME, VIRTUAL_CAMERA_STREAM_UID,
    VIRTUAL_CAMERA_WIDTH, authorize_client, copy_ledger, monotonic_time_nanos, record_drop,
    record_output_frame, stage_span,
};
#[cfg(test)]
use camera_man::{Frame, classify_frame_integrity};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, define_class, extern_methods, msg_send};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFRetained, CFRunLoop, CFType};
use objc2_core_media::{
    CMFormatDescription, CMSampleBuffer, CMSampleTimingInfo, CMTime, CMVideoFormatDescription,
    CMVideoFormatDescriptionCreate, kCMTimeInvalid,
};
use objc2_core_media_io::{
    CMIOExtensionClient, CMIOExtensionDevice, CMIOExtensionDeviceProperties,
    CMIOExtensionDeviceSource, CMIOExtensionProperty, CMIOExtensionPropertyDeviceModel,
    CMIOExtensionPropertyProviderManufacturer, CMIOExtensionPropertyProviderName,
    CMIOExtensionPropertyStreamActiveFormatIndex, CMIOExtensionProvider,
    CMIOExtensionProviderProperties, CMIOExtensionProviderSource, CMIOExtensionStream,
    CMIOExtensionStreamClockType, CMIOExtensionStreamDirection,
    CMIOExtensionStreamDiscontinuityFlags, CMIOExtensionStreamFormat,
    CMIOExtensionStreamProperties, CMIOExtensionStreamSource,
};
use objc2_core_video::{
    CVAttachmentMode, CVGetCurrentHostTime, CVGetHostClockFrequency, CVPixelBuffer,
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferPool, CVPixelBufferUnlockBaseAddress,
    kCVImageBufferColorPrimaries_ITU_R_709_2, kCVImageBufferColorPrimariesKey,
    kCVImageBufferTransferFunction_ITU_R_709_2, kCVImageBufferTransferFunctionKey,
    kCVImageBufferYCbCrMatrix_ITU_R_709_2, kCVImageBufferYCbCrMatrixKey, kCVPixelBufferHeightKey,
    kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferMetalCompatibilityKey,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferPoolAllocationThresholdKey,
    kCVPixelBufferPoolMinimumBufferCountKey, kCVPixelBufferWidthKey, kCVPixelFormatType_32BGRA,
    kCVReturnWouldExceedAllocationThreshold,
};
use objc2_foundation::{
    NSArray, NSError, NSNumber, NSObject, NSObjectProtocol, NSSet, NSString, NSUUID,
};

mod frame_pump;
mod objects;
mod pixel_buffer;
mod timing;

use frame_pump::*;
use objects::*;
use pixel_buffer::*;
use timing::*;

#[derive(Clone, Copy)]
struct StreamHandle(usize);

impl StreamHandle {
    fn retain(self) -> Option<Retained<CMIOExtensionStream>> {
        // SAFETY: the pointer originates from the retained stream stored in
        // `ExtensionLifetime`, which lives for the process run loop; retaining
        // creates independent ownership for the worker thread.
        unsafe { Retained::retain(self.0 as *mut CMIOExtensionStream) }
    }
}

/// Fields are never read after construction; they exist only so `Drop`
/// does not run on the retained ObjC objects while `keep_alive` blocks
/// forever in `CFRunLoop::run()`.
#[allow(dead_code)]
struct ExtensionLifetime {
    provider_source: Retained<ProviderSource>,
    device_source: Retained<DeviceSource>,
    stream_source: Retained<StreamSource>,
    provider: Retained<CMIOExtensionProvider>,
    device: Retained<CMIOExtensionDevice>,
    stream: Retained<CMIOExtensionStream>,
}

pub fn run() {
    eprintln!("CameraMan Rust CoreMediaIO extension process started.");

    // A panic here would abort the whole extension host process (there is
    // nothing above us to catch it), so a setup failure is reported and
    // the process exits cleanly instead of via `.expect()`.
    let lifetime = match create_extension() {
        Ok(lifetime) => lifetime,
        Err(error) => {
            let message = error.localizedDescription().to_string();
            eprintln!("CameraMan: failed to create CoreMediaIO extension: {message}");
            std::process::exit(1);
        }
    };
    // SAFETY: the provider is fully configured, retained by `lifetime`, and
    // remains alive while the process services its run loop.
    unsafe {
        CMIOExtensionProvider::startServiceWithProvider(&lifetime.provider);
    }

    eprintln!("CameraMan CoreMediaIO provider service started.");
    keep_alive(lifetime);
}

fn create_extension() -> Result<ExtensionLifetime, Retained<NSError>> {
    let provider_source = ProviderSource::new();
    let device_source = DeviceSource::new();
    let stream_source = StreamSource::new();

    // SAFETY: the source implements CMIOExtensionProviderSource and is retained
    // in `ExtensionLifetime`; nil selects the framework-managed client queue.
    let provider = unsafe {
        CMIOExtensionProvider::providerWithSource_clientQueue(
            ProtocolObject::from_ref(&*provider_source),
            None,
        )
    };

    // SAFETY: names and UUIDs are valid retained Objective-C values, and the
    // source implements the required device protocol for the device lifetime.
    let device = unsafe {
        CMIOExtensionDevice::deviceWithLocalizedName_deviceID_legacyDeviceID_source(
            &NSString::from_str(VIRTUAL_CAMERA_DEVICE_NAME),
            &uuid(VIRTUAL_CAMERA_DEVICE_UID),
            Some(&NSString::from_str("com.cameraman.rust.virtual-camera")),
            ProtocolObject::from_ref(&*device_source),
        )
    };

    // SAFETY: names and UUIDs are valid, HostTime is a supported clock, and
    // the retained source implements CMIOExtensionStreamSource.
    let stream = unsafe {
        CMIOExtensionStream::streamWithLocalizedName_streamID_direction_clockType_source(
            &NSString::from_str(VIRTUAL_CAMERA_STREAM_NAME),
            &uuid(VIRTUAL_CAMERA_STREAM_UID),
            CMIOExtensionStreamDirection::Source,
            CMIOExtensionStreamClockType::HostTime,
            ProtocolObject::from_ref(&*stream_source),
        )
    };
    stream_source.set_stream(&stream);

    // SAFETY: provider, device, and stream are fully initialized and retained;
    // CoreMediaIO validates the hierarchy and reports failures as NSError.
    unsafe {
        device.addStream_error(&stream)?;
        provider.addDevice_error(&device)?;
    }

    Ok(ExtensionLifetime {
        provider_source,
        device_source,
        stream_source,
        provider,
        device,
        stream,
    })
}

fn uuid(value: &str) -> Retained<NSUUID> {
    let string = NSString::from_str(value);
    NSUUID::from_string(&string).unwrap_or_else(|| panic!("invalid UUID: {value}"))
}

fn stream_format() -> Option<Retained<CMIOExtensionStreamFormat>> {
    let description = cached_video_format_description()?;
    let (max_frame_duration, min_frame_duration) = advertised_frame_duration_range();
    // SAFETY: the format description is retained and both positive duration
    // bounds are ordered max-duration to min-duration as CMIO requires.
    Some(unsafe {
        CMIOExtensionStreamFormat::streamFormatWithFormatDescription_maxFrameDuration_minFrameDuration_validFrameDurations(
            &description,
            max_frame_duration,
            min_frame_duration,
            None,
        )
    })
}

/// Polls the app transport and forwards samples at whatever cadence the
/// app is actually producing (`TransportFrame.fps`), not a fixed
/// constant: a 60fps-capable camera is no longer artificially capped at
/// 30 just because that used to be the only number anywhere in this
/// pipeline. Falls back to `VIRTUAL_CAMERA_DEFAULT_FPS` before the first
/// real frame arrives, or if the app has not written one at all.
fn stream_samples(handle: StreamHandle, streaming: Arc<AtomicBool>) {
    let Some(stream) = handle.retain() else {
        streaming.store(false, Ordering::SeqCst);
        return;
    };

    let Some(pixel_pool) = OutputPixelBufferPool::new() else {
        streaming.store(false, Ordering::SeqCst);
        return;
    };
    let mut sequence = 0_u64;
    let mut frame_reader = LatestFrameReader::new();
    let mut pacer = DeadlinePacer::new(VIRTUAL_CAMERA_DEFAULT_FPS);
    let mut pacing_fps = VIRTUAL_CAMERA_DEFAULT_FPS;
    while streaming.load(Ordering::SeqCst) {
        // Wait before reading and uploading so the sample sent at this
        // deadline contains the newest available producer frame. Polling
        // first would make that frame sit in a CVPixelBuffer for a complete
        // output interval.
        let pacing = pacer.plan(monotonic_time_nanos(), pacing_fps);
        record_drop(DropReason::Late, pacing.skipped_deadlines, Some(sequence));
        if pacing.sleep_nanos > 0 {
            thread::sleep(Duration::from_nanos(pacing.sleep_nanos));
        }
        if !streaming.load(Ordering::SeqCst) {
            break;
        }
        let poll = frame_reader.poll(&pixel_pool, sequence);
        pacing_fps = poll.fps;
        let host_time_nanos = host_time_nanoseconds();
        let pixel_buffer = poll
            .pixel_buffer
            .or_else(|| create_pixel_buffer(sequence, None, &pixel_pool));
        if let Some(sample) = pixel_buffer
            .as_ref()
            .and_then(|pixel_buffer| create_sample_buffer(poll.fps, host_time_nanos, pixel_buffer))
        {
            let discontinuity = cmio_discontinuity(
                poll.discontinuity,
                pacing.skipped_deadlines,
                pacing.clock_discontinuity,
            );
            if poll.discontinuity != FrameDiscontinuity::None
                || pacing.skipped_deadlines > 0
                || pacing.clock_discontinuity
            {
                record_drop(DropReason::Discontinuity, 1, Some(sequence));
            }
            {
                let _send_sample_span = stage_span(
                    PipelineStage::SendSample,
                    sequence,
                    VIRTUAL_CAMERA_WIDTH,
                    VIRTUAL_CAMERA_HEIGHT,
                );
                // SAFETY: `stream` and `sample` are independently retained for
                // the call, and host time uses the stream's declared clock.
                unsafe {
                    stream.sendSampleBuffer_discontinuity_hostTimeInNanoseconds(
                        &sample,
                        discontinuity,
                        host_time_nanos,
                    );
                }
            }
            record_output_frame();
        }
        sequence = sequence.wrapping_add(1);
    }
}

/// Keeps every retained object alive and runs the current thread's
/// CFRunLoop, which is what Apple's own Camera Extension sample code
/// does after starting the provider service: it is what actually
/// services the mach-port sources CMIO registers for client
/// connections, not just a way to avoid exiting. A plain sleep loop
/// left those unserviced.
fn keep_alive(lifetime: ExtensionLifetime) -> ! {
    let _lifetime = lifetime;
    CFRunLoop::run();
    unreachable!("CFRunLoop::run() does not return");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transport_frame(
        generation: u64,
        sequence: u64,
        monotonic_timestamp_nanos: u64,
    ) -> FrameSnapshot {
        FrameSnapshot {
            width: 2,
            height: 2,
            pixel_format: PixelFormat::Bgra8,
            sequence,
            generation,
            monotonic_timestamp_nanos,
            fps: 30,
        }
    }

    #[test]
    fn transport_fps_stays_inside_the_advertised_range() {
        assert_eq!(normalized_transport_fps(None), VIRTUAL_CAMERA_DEFAULT_FPS);
        assert_eq!(normalized_transport_fps(Some(1)), VIRTUAL_CAMERA_MIN_FPS);
        assert_eq!(normalized_transport_fps(Some(24)), 24);
        assert_eq!(normalized_transport_fps(Some(120)), VIRTUAL_CAMERA_MAX_FPS);
    }

    #[test]
    fn advertised_duration_range_is_reciprocal_of_fps_bounds() {
        let (max_duration, min_duration) = advertised_frame_duration_range();
        let max_value = max_duration.value;
        let max_timescale = max_duration.timescale;
        let min_value = min_duration.value;
        let min_timescale = min_duration.timescale;

        assert_eq!(
            (max_value, max_timescale),
            (1, VIRTUAL_CAMERA_MIN_FPS as i32)
        );
        assert_eq!(
            (min_value, min_timescale),
            (1, VIRTUAL_CAMERA_MAX_FPS as i32)
        );
    }

    #[test]
    fn pixel_buffer_pool_bounds_outstanding_frame_memory() {
        let pool = OutputPixelBufferPool::new_with_limits(16, 16, 3, 6)
            .expect("CoreVideo pool must be available");
        let mut buffers = (0..6)
            .map(|_| {
                pool.create_pixel_buffer()
                    .expect("the configured pool budget must admit six buffers")
            })
            .collect::<Vec<_>>();

        assert!(pool.create_pixel_buffer().is_none());
        buffers.pop();
        assert!(pool.create_pixel_buffer().is_some());
    }

    #[test]
    fn sample_timescale_never_reaches_core_media_as_zero_or_wrapped() {
        assert_eq!(sample_timescale(0), 1);
        assert_eq!(sample_timescale(60), 60);
        assert_eq!(sample_timescale(u32::MAX), i32::MAX);
    }

    #[test]
    fn transport_restart_and_sequence_gap_are_distinct_discontinuities() {
        let first = transport_frame(10, 4, 100);
        let restarted = transport_frame(11, 0, 200);
        let dropped = transport_frame(10, 7, 200);

        assert_eq!(
            classify_discontinuity(Some(&first), &restarted, false),
            FrameDiscontinuity::WriterRestarted
        );
        assert_eq!(
            classify_discontinuity(Some(&first), &dropped, false),
            FrameDiscontinuity::FramesDropped
        );
    }

    #[test]
    fn stale_threshold_allows_at_least_five_frame_periods() {
        assert_eq!(stale_timeout_nanos(60), 500_000_000);
        assert_eq!(stale_timeout_nanos(1), 5_000_000_000);
    }

    #[test]
    fn restart_maps_to_time_and_drop_flags() {
        let flags = cmio_discontinuity(FrameDiscontinuity::WriterRestarted, 0, false);
        assert!(flags.contains(CMIOExtensionStreamDiscontinuityFlags::Time));
        assert!(flags.contains(CMIOExtensionStreamDiscontinuityFlags::SampleDropped));
    }

    #[test]
    fn clock_reset_maps_to_time_and_drop_flags() {
        let flags = cmio_discontinuity(FrameDiscontinuity::None, 0, true);
        assert!(flags.contains(CMIOExtensionStreamDiscontinuityFlags::Time));
        assert!(flags.contains(CMIOExtensionStreamDiscontinuityFlags::SampleDropped));
    }

    #[test]
    fn unsafe_pixel_writers_respect_the_validated_padded_destination() {
        let row_bytes = VIRTUAL_CAMERA_WIDTH as usize * 4 + 32;
        let mut destination = vec![0_u8; row_bytes * VIRTUAL_CAMERA_HEIGHT as usize];
        let source = Frame::solid_bgra(2, 2, [7, 11, 13, 255]).unwrap();

        // SAFETY: `destination` has exactly the documented padded output
        // geometry for the duration of both writer calls.
        unsafe {
            fill_from_frame_view(destination.as_mut_ptr(), row_bytes, source.as_view());
        }
        assert_eq!(&destination[..4], &[7, 11, 13, 255]);
        let last = (VIRTUAL_CAMERA_HEIGHT as usize - 1) * row_bytes
            + (VIRTUAL_CAMERA_WIDTH as usize - 1) * 4;
        assert_eq!(&destination[last..last + 4], &[7, 11, 13, 255]);

        // SAFETY: the same live allocation and row-stride contract still hold.
        unsafe {
            fill_placeholder_frame(destination.as_mut_ptr(), row_bytes, 9);
        }
        assert_eq!(destination[3], 255);
        assert_eq!(destination[last + 3], 255);
    }
}
