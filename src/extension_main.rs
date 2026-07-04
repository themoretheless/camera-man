#[cfg(target_os = "macos")]
mod macos_extension {
    use std::path::PathBuf;
    use std::ptr::NonNull;
    use std::sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicPtr, Ordering},
    };
    use std::thread;
    use std::time::Duration;

    use camera_man::{
        Frame, PixelFormat, TransportFrame, default_frame_spool_path, read_latest_frame,
    };
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2::{AnyThread, DefinedClass, define_class, extern_methods, msg_send};
    use objc2_core_foundation::{CFRetained, CFRunLoop};
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
        CVGetCurrentHostTime, CVGetHostClockFrequency, CVPixelBuffer, CVPixelBufferCreate,
        CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferLockBaseAddress,
        CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_32BGRA,
    };
    use objc2_foundation::{
        NSArray, NSError, NSNumber, NSObject, NSObjectProtocol, NSSet, NSString, NSUUID,
    };

    const PROVIDER_NAME: &str = "CameraMan";
    const MANUFACTURER: &str = "CameraMan Rust";
    const DEVICE_NAME: &str = "CameraMan Virtual Camera";
    const DEVICE_UUID: &str = "7F1D9A42-3C58-4E6B-9D0A-2B6F8C1E5A17";
    const STREAM_NAME: &str = "CameraMan Output";
    const STREAM_UUID: &str = "A3B8C2D1-4E5F-4A6B-8C7D-9E0F1A2B3C4D";
    const OUTPUT_WIDTH: i32 = 1920;
    const OUTPUT_HEIGHT: i32 = 1080;
    /// Nominal ceiling advertised to CMIO clients via `CMIOExtensionStreamFormat`
    /// (used only by `formats()`, which per the CMIOExtensionStreamSource contract
    /// should not change between calls, so this must stay fixed). The rate frames
    /// actually arrive at tracks whatever the app is really producing (see
    /// `FrameSpoolReader`/`stream_samples`): this is a ceiling, not a promise.
    const ADVERTISED_FPS: i32 = 60;
    /// Cadence assumed before the first real frame has arrived from the app (or
    /// if the spool file cannot be read at all), so the placeholder still
    /// animates at a sane rate instead of guessing from `ADVERTISED_FPS`.
    const FALLBACK_FPS: u32 = 30;

    struct StreamSourceIvars {
        stream: AtomicPtr<CMIOExtensionStream>,
        streaming: Arc<AtomicBool>,
        /// The currently running `stream_samples` thread, if any. `start_streaming`
        /// joins this before spawning a replacement so a stop immediately followed
        /// by a start (e.g. a client reconnect) can never leave two threads
        /// calling `sendSampleBuffer_discontinuity_hostTimeInNanoseconds` on the
        /// same `CMIOExtensionStream` at once.
        worker: Mutex<Option<thread::JoinHandle<()>>>,
    }

    impl Default for StreamSourceIvars {
        fn default() -> Self {
            Self {
                stream: AtomicPtr::new(std::ptr::null_mut()),
                streaming: Arc::new(AtomicBool::new(false)),
                worker: Mutex::new(None),
            }
        }
    }

    define_class!(
        #[unsafe(super = NSObject)]
        #[name = "CameraManRustProviderSource"]
        struct ProviderSource;

        unsafe impl NSObjectProtocol for ProviderSource {}

        unsafe impl CMIOExtensionProviderSource for ProviderSource {
            #[unsafe(method(connectClient:error:))]
            fn connect_client_error(
                &self,
                client: &CMIOExtensionClient,
                _out_error: *mut *mut NSError,
            ) -> bool {
                // The bindings this crate uses only expose an opaque per-connection
                // clientID (a UUID minted by CMIO), not the caller's code-signing
                // identity, so this cannot actually verify who is connecting; it
                // only makes connections visible in logs. Any local process can
                // still attach. See recommendation.md item 514.
                eprintln!("CameraMan: client connected ({})", unsafe {
                    client.clientID()
                });
                true
            }

            #[unsafe(method(disconnectClient:))]
            fn disconnect_client(&self, client: &CMIOExtensionClient) {
                eprintln!("CameraMan: client disconnected ({})", unsafe {
                    client.clientID()
                });
            }

            #[unsafe(method_id(availableProperties))]
            fn available_properties(&self) -> Retained<NSSet<CMIOExtensionProperty>> {
                unsafe {
                    NSSet::from_slice(&[
                        CMIOExtensionPropertyProviderName,
                        CMIOExtensionPropertyProviderManufacturer,
                    ])
                }
            }

            #[unsafe(method_id(providerPropertiesForProperties:error:))]
            fn provider_properties_for_properties_error(
                &self,
                _properties: &NSSet<CMIOExtensionProperty>,
                _out_error: *mut *mut NSError,
            ) -> Retained<CMIOExtensionProviderProperties> {
                let properties = unsafe { CMIOExtensionProviderProperties::new() };
                unsafe {
                    properties.setName(Some(&NSString::from_str(PROVIDER_NAME)));
                    properties.setManufacturer(Some(&NSString::from_str(MANUFACTURER)));
                }
                properties
            }

            #[unsafe(method(setProviderProperties:error:))]
            fn set_provider_properties_error(
                &self,
                _provider_properties: &CMIOExtensionProviderProperties,
                _out_error: *mut *mut NSError,
            ) -> bool {
                true
            }
        }
    );

    impl ProviderSource {
        extern_methods!(
            #[unsafe(method(new))]
            fn new() -> Retained<Self>;
        );
    }

    define_class!(
        #[unsafe(super = NSObject)]
        #[name = "CameraManRustDeviceSource"]
        struct DeviceSource;

        unsafe impl NSObjectProtocol for DeviceSource {}

        unsafe impl CMIOExtensionDeviceSource for DeviceSource {
            #[unsafe(method_id(availableProperties))]
            fn available_properties(&self) -> Retained<NSSet<CMIOExtensionProperty>> {
                unsafe { NSSet::from_slice(&[CMIOExtensionPropertyDeviceModel]) }
            }

            #[unsafe(method_id(devicePropertiesForProperties:error:))]
            fn device_properties_for_properties_error(
                &self,
                _properties: &NSSet<CMIOExtensionProperty>,
                _out_error: *mut *mut NSError,
            ) -> Retained<CMIOExtensionDeviceProperties> {
                let properties = unsafe { CMIOExtensionDeviceProperties::new() };
                unsafe {
                    properties.setModel(Some(&NSString::from_str(DEVICE_NAME)));
                    properties.setSuspended(Some(&NSNumber::new_bool(false)));
                }
                properties
            }

            #[unsafe(method(setDeviceProperties:error:))]
            fn set_device_properties_error(
                &self,
                _device_properties: &CMIOExtensionDeviceProperties,
                _out_error: *mut *mut NSError,
            ) -> bool {
                true
            }
        }
    );

    impl DeviceSource {
        extern_methods!(
            #[unsafe(method(new))]
            fn new() -> Retained<Self>;
        );
    }

    define_class!(
        #[unsafe(super = NSObject)]
        #[name = "CameraManRustStreamSource"]
        #[ivars = StreamSourceIvars]
        struct StreamSource;

        unsafe impl NSObjectProtocol for StreamSource {}

        unsafe impl CMIOExtensionStreamSource for StreamSource {
            #[unsafe(method_id(formats))]
            fn formats(&self) -> Retained<NSArray<CMIOExtensionStreamFormat>> {
                match stream_format() {
                    Some(format) => NSArray::from_slice(&[&*format]),
                    None => NSArray::new(),
                }
            }

            #[unsafe(method_id(availableProperties))]
            fn available_properties(&self) -> Retained<NSSet<CMIOExtensionProperty>> {
                unsafe { NSSet::from_slice(&[CMIOExtensionPropertyStreamActiveFormatIndex]) }
            }

            #[unsafe(method_id(streamPropertiesForProperties:error:))]
            fn stream_properties_for_properties_error(
                &self,
                _properties: &NSSet<CMIOExtensionProperty>,
                _out_error: *mut *mut NSError,
            ) -> Retained<CMIOExtensionStreamProperties> {
                let properties = unsafe { CMIOExtensionStreamProperties::new() };
                unsafe {
                    properties.setActiveFormatIndex(Some(&NSNumber::new_u8(0)));
                }
                properties
            }

            #[unsafe(method(setStreamProperties:error:))]
            fn set_stream_properties_error(
                &self,
                _stream_properties: &CMIOExtensionStreamProperties,
                _out_error: *mut *mut NSError,
            ) -> bool {
                true
            }

            #[unsafe(method(authorizedToStartStreamForClient:))]
            fn authorized_to_start_stream_for_client(&self, client: &CMIOExtensionClient) -> bool {
                // See the comment on connect_client_error: this only logs, it
                // does not actually verify the caller. Any local process can
                // start the stream. See recommendation.md item 514.
                eprintln!(
                    "CameraMan: stream start authorized for client ({})",
                    unsafe { client.clientID() }
                );
                true
            }

            #[unsafe(method(startStreamAndReturnError:))]
            fn start_stream_and_return_error(&self, _out_error: *mut *mut NSError) -> bool {
                eprintln!("CameraMan virtual stream start requested.");
                self.start_streaming();
                true
            }

            #[unsafe(method(stopStreamAndReturnError:))]
            fn stop_stream_and_return_error(&self, _out_error: *mut *mut NSError) -> bool {
                eprintln!("CameraMan virtual stream stop requested.");
                self.stop_streaming();
                true
            }
        }
    );

    impl StreamSource {
        fn new() -> Retained<Self> {
            let this = Self::alloc().set_ivars(StreamSourceIvars::default());
            unsafe { msg_send![super(this), init] }
        }

        fn set_stream(&self, stream: &CMIOExtensionStream) {
            self.ivars().stream.store(
                (stream as *const CMIOExtensionStream).cast_mut(),
                Ordering::SeqCst,
            );
        }

        fn start_streaming(&self) {
            if self.ivars().streaming.swap(true, Ordering::SeqCst) {
                return;
            }

            // A previous stop_streaming() only flips the flag; the old
            // stream_samples thread notices and exits on its own schedule
            // (up to one frame period later). Join it now, before spawning a
            // replacement, so the two threads can never overlap.
            let previous = self
                .ivars()
                .worker
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            if let Some(previous) = previous {
                let _ = previous.join();
            }

            let raw_stream = self.ivars().stream.load(Ordering::SeqCst);
            if raw_stream.is_null() {
                eprintln!("CameraMan stream start requested before stream was attached.");
                self.ivars().streaming.store(false, Ordering::SeqCst);
                return;
            }

            let handle = StreamHandle(raw_stream as usize);
            let streaming = Arc::clone(&self.ivars().streaming);
            let worker = thread::spawn(move || stream_samples(handle, streaming));
            *self
                .ivars()
                .worker
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(worker);
        }

        fn stop_streaming(&self) {
            self.ivars().streaming.store(false, Ordering::SeqCst);
        }
    }

    #[derive(Clone, Copy)]
    struct StreamHandle(usize);

    impl StreamHandle {
        fn retain(self) -> Option<Retained<CMIOExtensionStream>> {
            unsafe { Retained::retain(self.0 as *mut CMIOExtensionStream) }
        }
    }

    struct FrameSpoolReader {
        path: PathBuf,
        last_frame: Option<TransportFrame>,
        last_error: Option<String>,
    }

    impl FrameSpoolReader {
        fn new(path: PathBuf) -> Self {
            Self {
                path,
                last_frame: None,
                last_error: None,
            }
        }

        /// Re-reads the spool and returns the latest known frame together
        /// with its producer-reported fps. Both come from the same read, in
        /// one `&mut self` call, so callers never observe a frame from one
        /// tick paired with an fps from a different tick (a two-method split
        /// here would also fight the borrow checker: the `&Frame` returned
        /// would keep `self` mutably borrowed for as long as it is held).
        fn poll(&mut self) -> (Option<&Frame>, u32) {
            match read_latest_frame(&self.path) {
                Ok(Some(frame)) => {
                    self.last_error = None;
                    self.last_frame = Some(frame);
                }
                Ok(None) => {}
                Err(error) => {
                    let message = error.to_string();
                    if self.last_error.as_ref() != Some(&message) {
                        eprintln!("CameraMan frame spool read failed: {message}");
                        self.last_error = Some(message);
                    }
                }
            }
            let fps = self
                .last_frame
                .as_ref()
                .map_or(FALLBACK_FPS, |transport| transport.fps.max(1));
            (
                self.last_frame.as_ref().map(|transport| &transport.frame),
                fps,
            )
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
        let lifetime = match unsafe { create_extension() } {
            Ok(lifetime) => lifetime,
            Err(error) => {
                let message = error.localizedDescription().to_string();
                eprintln!("CameraMan: failed to create CoreMediaIO extension: {message}");
                std::process::exit(1);
            }
        };
        unsafe {
            CMIOExtensionProvider::startServiceWithProvider(&lifetime.provider);
        }

        eprintln!("CameraMan CoreMediaIO provider service started.");
        keep_alive(lifetime);
    }

    unsafe fn create_extension() -> Result<ExtensionLifetime, Retained<NSError>> {
        let provider_source = ProviderSource::new();
        let device_source = DeviceSource::new();
        let stream_source = StreamSource::new();

        let provider = unsafe {
            CMIOExtensionProvider::providerWithSource_clientQueue(
                ProtocolObject::from_ref(&*provider_source),
                None,
            )
        };

        let device = unsafe {
            CMIOExtensionDevice::deviceWithLocalizedName_deviceID_legacyDeviceID_source(
                &NSString::from_str(DEVICE_NAME),
                &uuid(DEVICE_UUID),
                Some(&NSString::from_str("com.cameraman.rust.virtual-camera")),
                ProtocolObject::from_ref(&*device_source),
            )
        };

        let stream = unsafe {
            CMIOExtensionStream::streamWithLocalizedName_streamID_direction_clockType_source(
                &NSString::from_str(STREAM_NAME),
                &uuid(STREAM_UUID),
                CMIOExtensionStreamDirection::Source,
                CMIOExtensionStreamClockType::HostTime,
                ProtocolObject::from_ref(&*stream_source),
            )
        };
        stream_source.set_stream(&stream);

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
        let frame_duration = unsafe { CMTime::new(1, ADVERTISED_FPS) };
        Some(unsafe {
            CMIOExtensionStreamFormat::streamFormatWithFormatDescription_maxFrameDuration_minFrameDuration_validFrameDurations(
                &description,
                frame_duration,
                frame_duration,
                None,
            )
        })
    }

    /// The format never changes (fixed 1920x1080 BGRA/30fps), so it is built
    /// once and reused. `formats()` is queried by clients and
    /// `create_sample_buffer` calls this once per frame (30 times a second);
    /// recreating a `CMVideoFormatDescription` on every call both violates the
    /// CMIOExtensionStreamSource contract that formats do not change between
    /// calls and, per the Core Foundation Create Rule, leaked one reference
    /// per call since nothing here ever released the previous one.
    fn cached_video_format_description() -> Option<CFRetained<CMFormatDescription>> {
        static DESCRIPTION: OnceLock<Option<CFRetained<CMFormatDescription>>> = OnceLock::new();
        DESCRIPTION.get_or_init(video_format_description).clone()
    }

    fn video_format_description() -> Option<CFRetained<CMFormatDescription>> {
        let mut raw_description: *const CMVideoFormatDescription = std::ptr::null();
        let status = unsafe {
            CMVideoFormatDescriptionCreate(
                None,
                kCVPixelFormatType_32BGRA,
                OUTPUT_WIDTH,
                OUTPUT_HEIGHT,
                None,
                NonNull::from(&mut raw_description),
            )
        };

        if status != 0 {
            eprintln!("failed to create CMVideoFormatDescription: OSStatus {status}");
            return None;
        }

        let description = NonNull::new(raw_description.cast_mut())
            .map(|ptr| unsafe { CFRetained::<CMFormatDescription>::from_raw(ptr) })?;
        Some(description)
    }

    /// Polls the frame spool and forwards samples at whatever cadence the
    /// app is actually producing (`TransportFrame.fps`), not a fixed
    /// constant: a 60fps-capable camera is no longer artificially capped at
    /// 30 just because that used to be the only number anywhere in this
    /// pipeline. Falls back to `FALLBACK_FPS` before the first real frame
    /// arrives, or if the app has not written one at all (placeholder-only).
    fn stream_samples(handle: StreamHandle, streaming: Arc<AtomicBool>) {
        let Some(stream) = handle.retain() else {
            streaming.store(false, Ordering::SeqCst);
            return;
        };

        let mut sequence = 0_u64;
        let mut frame_reader = FrameSpoolReader::new(default_frame_spool_path());
        while streaming.load(Ordering::SeqCst) {
            let (app_frame, fps) = frame_reader.poll();
            let host_time_nanos = host_time_nanoseconds();
            if let Some(sample) = create_sample_buffer(sequence, fps, host_time_nanos, app_frame) {
                unsafe {
                    stream.sendSampleBuffer_discontinuity_hostTimeInNanoseconds(
                        &sample,
                        CMIOExtensionStreamDiscontinuityFlags::None,
                        host_time_nanos,
                    );
                }
            }
            sequence = sequence.wrapping_add(1);
            thread::sleep(Duration::from_nanos(1_000_000_000 / u64::from(fps)));
        }
    }

    fn create_sample_buffer(
        sequence: u64,
        fps: u32,
        host_time_nanos: u64,
        app_frame: Option<&Frame>,
    ) -> Option<CFRetained<CMSampleBuffer>> {
        let pixel_buffer = create_pixel_buffer(sequence, app_frame)?;
        let format_description = cached_video_format_description()?;
        let duration = unsafe { CMTime::new(1, fps.min(i32::MAX as u32) as i32) };
        let mut timing = CMSampleTimingInfo {
            duration,
            // Real host-clock nanoseconds, not sequence/fps: that stays
            // monotonic even across an fps change mid-stream, whereas a
            // frame-count-scaled timestamp would jump backwards the instant
            // fps changes (e.g. sequence 100 at 30fps is 3.33s in, but the
            // same sequence at 60fps would be 1.67s: a step backwards).
            presentationTimeStamp: unsafe { CMTime::new(host_time_nanos as i64, 1_000_000_000) },
            decodeTimeStamp: unsafe { kCMTimeInvalid },
        };
        let mut raw_sample: *mut CMSampleBuffer = std::ptr::null_mut();
        let status = unsafe {
            CMSampleBuffer::create_ready_with_image_buffer(
                None,
                &pixel_buffer,
                &format_description,
                NonNull::from(&mut timing),
                NonNull::from(&mut raw_sample),
            )
        };
        if status != 0 {
            eprintln!("failed to create CMSampleBuffer: OSStatus {status}");
            return None;
        }

        NonNull::new(raw_sample).map(|ptr| unsafe { CFRetained::<CMSampleBuffer>::from_raw(ptr) })
    }

    fn create_pixel_buffer(
        sequence: u64,
        app_frame: Option<&Frame>,
    ) -> Option<CFRetained<CVPixelBuffer>> {
        let mut raw_buffer: *mut CVPixelBuffer = std::ptr::null_mut();
        let status = unsafe {
            CVPixelBufferCreate(
                None,
                OUTPUT_WIDTH as usize,
                OUTPUT_HEIGHT as usize,
                kCVPixelFormatType_32BGRA,
                None,
                NonNull::from(&mut raw_buffer),
            )
        };
        if status != 0 {
            eprintln!("failed to create CVPixelBuffer: CVReturn {status}");
            return None;
        }

        let buffer = NonNull::new(raw_buffer)
            .map(|ptr| unsafe { CFRetained::<CVPixelBuffer>::from_raw(ptr) })?;
        let flags = CVPixelBufferLockFlags::empty();
        let lock_status = unsafe { CVPixelBufferLockBaseAddress(&buffer, flags) };
        if lock_status != 0 {
            eprintln!("failed to lock CVPixelBuffer: CVReturn {lock_status}");
            return None;
        }

        let base = CVPixelBufferGetBaseAddress(&buffer).cast::<u8>();
        let bytes_per_row = CVPixelBufferGetBytesPerRow(&buffer);
        if base.is_null() {
            unsafe {
                CVPixelBufferUnlockBaseAddress(&buffer, flags);
            }
            eprintln!("CVPixelBuffer returned a null base address");
            return None;
        }

        if let Some(frame) = app_frame.filter(|frame| frame.pixel_format() == PixelFormat::Bgra8) {
            fill_from_frame(base, bytes_per_row, frame);
        } else {
            fill_placeholder_frame(base, bytes_per_row, sequence);
        }
        let unlock_status = unsafe { CVPixelBufferUnlockBaseAddress(&buffer, flags) };
        if unlock_status != 0 {
            eprintln!("failed to unlock CVPixelBuffer: CVReturn {unlock_status}");
            return None;
        }

        Some(buffer)
    }

    fn fill_from_frame(base: *mut u8, bytes_per_row: usize, frame: &Frame) {
        let source_width = frame.width() as usize;
        let source_height = frame.height() as usize;
        if source_width == 0 || source_height == 0 {
            return;
        }

        let output_width = OUTPUT_WIDTH as usize;
        let output_height = OUTPUT_HEIGHT as usize;
        let source_stride = source_width * PixelFormat::Bgra8.bytes_per_pixel();
        for y in 0..output_height {
            let source_y = y * source_height / output_height;
            let source_row_start = source_y * source_stride;
            let row = unsafe { base.add(y * bytes_per_row) };
            for x in 0..output_width {
                let source_x = x * source_width / output_width;
                let source_offset = source_row_start + source_x * 4;
                let target_offset = x * 4;
                let source_pixel = &frame.data()[source_offset..source_offset + 4];
                unsafe {
                    std::ptr::copy_nonoverlapping(source_pixel.as_ptr(), row.add(target_offset), 4);
                }
            }
        }
    }

    fn fill_placeholder_frame(base: *mut u8, bytes_per_row: usize, sequence: u64) {
        let phase = (sequence % OUTPUT_WIDTH as u64) as usize;
        for y in 0..OUTPUT_HEIGHT as usize {
            let row = unsafe { base.add(y * bytes_per_row) };
            for x in 0..OUTPUT_WIDTH as usize {
                let offset = x * 4;
                let band = ((x + phase) / 160).is_multiple_of(2);
                let vertical = (y / 120).is_multiple_of(2);
                let pulse = ((sequence * 3 + x as u64 + y as u64) % 255) as u8;
                let (b, g, r) = if band ^ vertical {
                    (48, 132, pulse.saturating_add(40))
                } else {
                    (pulse / 3, 58, 96)
                };
                unsafe {
                    *row.add(offset) = b;
                    *row.add(offset + 1) = g;
                    *row.add(offset + 2) = r;
                    *row.add(offset + 3) = 255;
                }
            }
        }
    }

    fn host_time_nanoseconds() -> u64 {
        let ticks = CVGetCurrentHostTime();
        let frequency = CVGetHostClockFrequency();
        if frequency <= 0.0 {
            return 0;
        }
        ((ticks as f64 / frequency) * 1_000_000_000.0) as u64
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
}

#[cfg(target_os = "macos")]
fn main() {
    macos_extension::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("CameraMan CoreMediaIO extension is available only on macOS.");
}
