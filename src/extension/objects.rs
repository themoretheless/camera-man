use super::*;

const PROVIDER_NAME: &str = "CameraMan";
const MANUFACTURER: &str = "CameraMan Rust";
const STREAM_ERROR_DOMAIN: &str = "com.cameraman.stream";

pub(super) struct StreamSourceIvars {
    stream: AtomicPtr<CMIOExtensionStream>,
    streaming: Arc<AtomicBool>,
    lifecycle: Mutex<StreamLifecycle>,
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
            lifecycle: Mutex::new(StreamLifecycle::default()),
            worker: Mutex::new(None),
        }
    }
}

define_class!(
    #[unsafe(super = NSObject)]
    #[name = "CameraManRustProviderSource"]
    pub(super) struct ProviderSource;

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
            // still attach. See recommendation.md item 359.
            // SAFETY: CMIO supplies a live client object for the entire
            // callback and `clientID` returns an autoreleased value.
            let client_id = unsafe { client.clientID() };
            eprintln!("CameraMan: client connected ({client_id})");
            true
        }

        #[unsafe(method(disconnectClient:))]
        fn disconnect_client(&self, client: &CMIOExtensionClient) {
            // SAFETY: CMIO keeps the callback's client object alive while its
            // identifier is queried.
            let client_id = unsafe { client.clientID() };
            eprintln!("CameraMan: client disconnected ({client_id})");
        }

        #[unsafe(method_id(availableProperties))]
        fn available_properties(&self) -> Retained<NSSet<CMIOExtensionProperty>> {
            // SAFETY: both framework property constants are valid non-null
            // Objective-C objects and the slice length is exact.
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
            // SAFETY: this is the framework-designated constructor for a
            // provider-properties object.
            let properties = unsafe { CMIOExtensionProviderProperties::new() };
            // SAFETY: `properties` is initialized and both NSString values
            // remain alive for each setter call.
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
        pub(super) fn new() -> Retained<Self>;
    );
}

define_class!(
    #[unsafe(super = NSObject)]
    #[name = "CameraManRustDeviceSource"]
    pub(super) struct DeviceSource;

    unsafe impl NSObjectProtocol for DeviceSource {}

    unsafe impl CMIOExtensionDeviceSource for DeviceSource {
        #[unsafe(method_id(availableProperties))]
        fn available_properties(&self) -> Retained<NSSet<CMIOExtensionProperty>> {
            // SAFETY: the framework property constant is a valid non-null
            // Objective-C object and is retained by the returned set.
            unsafe { NSSet::from_slice(&[CMIOExtensionPropertyDeviceModel]) }
        }

        #[unsafe(method_id(devicePropertiesForProperties:error:))]
        fn device_properties_for_properties_error(
            &self,
            _properties: &NSSet<CMIOExtensionProperty>,
            _out_error: *mut *mut NSError,
        ) -> Retained<CMIOExtensionDeviceProperties> {
            // SAFETY: this is the framework-designated constructor for a
            // device-properties object.
            let properties = unsafe { CMIOExtensionDeviceProperties::new() };
            // SAFETY: `properties` is initialized and the temporary values
            // remain alive through their setter calls.
            unsafe {
                properties.setModel(Some(&NSString::from_str(VIRTUAL_CAMERA_DEVICE_NAME)));
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
        pub(super) fn new() -> Retained<Self>;
    );
}

define_class!(
    #[unsafe(super = NSObject)]
    #[name = "CameraManRustStreamSource"]
    #[ivars = StreamSourceIvars]
    pub(super) struct StreamSource;

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
            // SAFETY: the framework property constant is a valid non-null
            // Objective-C object and is retained by the returned set.
            unsafe { NSSet::from_slice(&[CMIOExtensionPropertyStreamActiveFormatIndex]) }
        }

        #[unsafe(method_id(streamPropertiesForProperties:error:))]
        fn stream_properties_for_properties_error(
            &self,
            _properties: &NSSet<CMIOExtensionProperty>,
            _out_error: *mut *mut NSError,
        ) -> Retained<CMIOExtensionStreamProperties> {
            // SAFETY: this is the framework-designated constructor for a
            // stream-properties object.
            let properties = unsafe { CMIOExtensionStreamProperties::new() };
            // SAFETY: `properties` is initialized and the NSNumber remains
            // alive through the setter call.
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
            // start the stream. See recommendation.md item 359.
            // SAFETY: CMIO keeps the callback's client object alive while its
            // identifier is queried.
            let client_id = unsafe { client.clientID() };
            eprintln!(
                "CameraMan: stream start authorized for client ({})",
                client_id
            );
            true
        }

        #[unsafe(method(startStreamAndReturnError:))]
        fn start_stream_and_return_error(&self, out_error: *mut *mut NSError) -> bool {
            eprintln!("CameraMan virtual stream start requested.");
            match self.start_streaming() {
                Ok(()) => true,
                Err(error) => {
                    eprintln!("CameraMan virtual stream start rejected: {error}");
                    // SAFETY: CMIO provided `out_error` for this callback and
                    // permits either null or one writable NSError pointer.
                    unsafe { write_stream_error(out_error, 1) };
                    false
                }
            }
        }

        #[unsafe(method(stopStreamAndReturnError:))]
        fn stop_stream_and_return_error(&self, out_error: *mut *mut NSError) -> bool {
            eprintln!("CameraMan virtual stream stop requested.");
            match self.stop_streaming() {
                Ok(()) => true,
                Err(error) => {
                    eprintln!("CameraMan virtual stream stop rejected: {error}");
                    // SAFETY: CMIO provided `out_error` for this callback and
                    // permits either null or one writable NSError pointer.
                    unsafe { write_stream_error(out_error, 2) };
                    false
                }
            }
        }
    }
);

impl StreamSource {
    pub(super) fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(StreamSourceIvars::default());
        // SAFETY: `this` is a newly allocated NSObject subclass with all Rust
        // ivars initialized exactly once before invoking super init.
        unsafe { msg_send![super(this), init] }
    }

    pub(super) fn set_stream(&self, stream: &CMIOExtensionStream) {
        self.ivars().stream.store(
            (stream as *const CMIOExtensionStream).cast_mut(),
            Ordering::SeqCst,
        );
        let mut lifecycle = self
            .ivars()
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Err(error) = lifecycle.attach() {
            eprintln!("CameraMan stream attach rejected: {error}");
        }
    }

    fn start_streaming(&self) -> Result<(), LifecycleError> {
        let mut lifecycle = self
            .ivars()
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if lifecycle.begin_start()? == StartAction::AlreadyRunning {
            return Ok(());
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
            let _ = lifecycle.finish_start(false);
            return Err(LifecycleError {
                state: lifecycle.state(),
                operation: "start without an attached CMIO stream",
            });
        }

        self.ivars().streaming.store(true, Ordering::SeqCst);
        let handle = StreamHandle(raw_stream as usize);
        let streaming = Arc::clone(&self.ivars().streaming);
        let worker = thread::spawn(move || stream_samples(handle, streaming));
        *self
            .ivars()
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(worker);
        lifecycle.finish_start(true)?;
        Ok(())
    }

    fn stop_streaming(&self) -> Result<(), LifecycleError> {
        {
            let mut lifecycle = self
                .ivars()
                .lifecycle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if lifecycle.begin_stop()? == StopAction::AlreadyStopped {
                return Ok(());
            }
        }
        self.ivars().streaming.store(false, Ordering::SeqCst);
        let worker = self
            .ivars()
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
        self.ivars()
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .finish_stop()
    }
}

/// Writes an autoreleased NSError to a CoreMediaIO callback out-parameter.
///
/// # Safety
///
/// A non-null `out_error` must be writable for one Objective-C object pointer.
unsafe fn write_stream_error(out_error: *mut *mut NSError, code: isize) {
    if out_error.is_null() {
        return;
    }
    let domain = NSString::from_str(STREAM_ERROR_DOMAIN);
    // SAFETY: the domain is a valid NSString, the integer code is unrestricted,
    // and a nil user-info dictionary is accepted by NSError.
    let error = unsafe { NSError::errorWithDomain_code_userInfo(&domain, code, None) };
    // SAFETY: guaranteed by this function's caller; ownership follows the
    // Objective-C autoreleasing out-parameter convention.
    unsafe {
        *out_error = Retained::autorelease_ptr(error);
    }
}
