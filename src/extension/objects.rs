use super::*;

const PROVIDER_NAME: &str = "CameraMan";
const MANUFACTURER: &str = "CameraMan Rust";
const STREAM_ERROR_DOMAIN: &str = "com.cameraman.stream";
const STREAM_ERROR_START_REJECTED: isize = 1;
const STREAM_ERROR_STOP_REJECTED: isize = 2;
const STREAM_ERROR_CLIENT_DENIED: isize = 3;
const STREAM_ERROR_CALLBACK_PANICKED: isize = 4;

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
            out_error: *mut *mut NSError,
        ) -> bool {
            contain_panic(
                "connectClient:error:",
                || {
                    // SAFETY: CMIO provided `out_error` for this callback and
                    // permits either null or one writable NSError pointer.
                    unsafe { write_stream_error(out_error, STREAM_ERROR_CALLBACK_PANICKED) };
                    false
                },
                || {
                    let identity = client_identity(client);
                    match authorize_client(&identity) {
                        ClientAuthorization::Allow => {
                            eprintln!("CameraMan: client connected ({})", identity.audit_line());
                            true
                        }
                        ClientAuthorization::Deny(reason) => {
                            eprintln!(
                                "CameraMan: client connection denied, {reason} ({})",
                                identity.audit_line()
                            );
                            // SAFETY: CMIO provided `out_error` for this callback
                            // and permits either null or one writable NSError
                            // pointer.
                            unsafe { write_stream_error(out_error, STREAM_ERROR_CLIENT_DENIED) };
                            false
                        }
                    }
                },
            )
        }

        #[unsafe(method(disconnectClient:))]
        fn disconnect_client(&self, client: &CMIOExtensionClient) {
            contain_panic_unit("disconnectClient:", || {
                let identity = client_identity(client);
                eprintln!("CameraMan: client disconnected ({})", identity.audit_line());
            });
        }

        #[unsafe(method_id(availableProperties))]
        fn available_properties(&self) -> Retained<NSSet<CMIOExtensionProperty>> {
            contain_panic("ProviderSource availableProperties", NSSet::new, || {
                // SAFETY: both framework property constants are valid non-null
                // Objective-C objects and the slice length is exact.
                unsafe {
                    NSSet::from_slice(&[
                        CMIOExtensionPropertyProviderName,
                        CMIOExtensionPropertyProviderManufacturer,
                    ])
                }
            })
        }

        #[unsafe(method_id(providerPropertiesForProperties:error:))]
        fn provider_properties_for_properties_error(
            &self,
            _properties: &NSSet<CMIOExtensionProperty>,
            _out_error: *mut *mut NSError,
        ) -> Retained<CMIOExtensionProviderProperties> {
            contain_panic(
                "providerPropertiesForProperties:error:",
                || {
                    // SAFETY: this is the framework-designated constructor for a
                    // provider-properties object.
                    unsafe { CMIOExtensionProviderProperties::new() }
                },
                || {
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
                },
            )
        }

        #[unsafe(method(setProviderProperties:error:))]
        fn set_provider_properties_error(
            &self,
            _provider_properties: &CMIOExtensionProviderProperties,
            out_error: *mut *mut NSError,
        ) -> bool {
            contain_panic(
                "setProviderProperties:error:",
                || {
                    // SAFETY: CMIO provided `out_error` for this callback and
                    // permits either null or one writable NSError pointer.
                    unsafe { write_stream_error(out_error, STREAM_ERROR_CALLBACK_PANICKED) };
                    false
                },
                || true,
            )
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
            contain_panic("DeviceSource availableProperties", NSSet::new, || {
                // SAFETY: the framework property constant is a valid non-null
                // Objective-C object and is retained by the returned set.
                unsafe { NSSet::from_slice(&[CMIOExtensionPropertyDeviceModel]) }
            })
        }

        #[unsafe(method_id(devicePropertiesForProperties:error:))]
        fn device_properties_for_properties_error(
            &self,
            _properties: &NSSet<CMIOExtensionProperty>,
            _out_error: *mut *mut NSError,
        ) -> Retained<CMIOExtensionDeviceProperties> {
            contain_panic(
                "devicePropertiesForProperties:error:",
                || {
                    // SAFETY: this is the framework-designated constructor for a
                    // device-properties object.
                    unsafe { CMIOExtensionDeviceProperties::new() }
                },
                || {
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
                },
            )
        }

        #[unsafe(method(setDeviceProperties:error:))]
        fn set_device_properties_error(
            &self,
            _device_properties: &CMIOExtensionDeviceProperties,
            out_error: *mut *mut NSError,
        ) -> bool {
            contain_panic(
                "setDeviceProperties:error:",
                || {
                    // SAFETY: CMIO provided `out_error` for this callback and
                    // permits either null or one writable NSError pointer.
                    unsafe { write_stream_error(out_error, STREAM_ERROR_CALLBACK_PANICKED) };
                    false
                },
                || true,
            )
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
            contain_panic("formats", NSArray::new, || match stream_format() {
                Some(format) => NSArray::from_slice(&[&*format]),
                None => NSArray::new(),
            })
        }

        #[unsafe(method_id(availableProperties))]
        fn available_properties(&self) -> Retained<NSSet<CMIOExtensionProperty>> {
            contain_panic("StreamSource availableProperties", NSSet::new, || {
                // SAFETY: the framework property constant is a valid non-null
                // Objective-C object and is retained by the returned set.
                unsafe { NSSet::from_slice(&[CMIOExtensionPropertyStreamActiveFormatIndex]) }
            })
        }

        #[unsafe(method_id(streamPropertiesForProperties:error:))]
        fn stream_properties_for_properties_error(
            &self,
            _properties: &NSSet<CMIOExtensionProperty>,
            _out_error: *mut *mut NSError,
        ) -> Retained<CMIOExtensionStreamProperties> {
            contain_panic(
                "streamPropertiesForProperties:error:",
                || {
                    // SAFETY: this is the framework-designated constructor for a
                    // stream-properties object.
                    unsafe { CMIOExtensionStreamProperties::new() }
                },
                || {
                    // SAFETY: this is the framework-designated constructor for a
                    // stream-properties object.
                    let properties = unsafe { CMIOExtensionStreamProperties::new() };
                    // SAFETY: `properties` is initialized and the NSNumber remains
                    // alive through the setter call.
                    unsafe {
                        properties.setActiveFormatIndex(Some(&NSNumber::new_u8(0)));
                    }
                    properties
                },
            )
        }

        #[unsafe(method(setStreamProperties:error:))]
        fn set_stream_properties_error(
            &self,
            _stream_properties: &CMIOExtensionStreamProperties,
            out_error: *mut *mut NSError,
        ) -> bool {
            contain_panic(
                "setStreamProperties:error:",
                || {
                    // SAFETY: CMIO provided `out_error` for this callback and
                    // permits either null or one writable NSError pointer.
                    unsafe { write_stream_error(out_error, STREAM_ERROR_CALLBACK_PANICKED) };
                    false
                },
                || true,
            )
        }

        #[unsafe(method(authorizedToStartStreamForClient:))]
        fn authorized_to_start_stream_for_client(&self, client: &CMIOExtensionClient) -> bool {
            contain_panic(
                "authorizedToStartStreamForClient:",
                || false,
                || {
                    let identity = client_identity(client);
                    match authorize_client(&identity) {
                        ClientAuthorization::Allow => {
                            eprintln!(
                                "CameraMan: stream start authorized ({})",
                                identity.audit_line()
                            );
                            true
                        }
                        ClientAuthorization::Deny(reason) => {
                            eprintln!(
                                "CameraMan: stream start denied, {reason} ({})",
                                identity.audit_line()
                            );
                            false
                        }
                    }
                },
            )
        }

        #[unsafe(method(startStreamAndReturnError:))]
        fn start_stream_and_return_error(&self, out_error: *mut *mut NSError) -> bool {
            contain_panic(
                "startStreamAndReturnError:",
                || {
                    self.repair_after_panic();
                    // SAFETY: CMIO provided `out_error` for this callback and
                    // permits either null or one writable NSError pointer.
                    unsafe { write_stream_error(out_error, STREAM_ERROR_CALLBACK_PANICKED) };
                    false
                },
                || {
                    eprintln!("CameraMan virtual stream start requested.");
                    match self.start_streaming() {
                        Ok(()) => true,
                        Err(error) => {
                            eprintln!("CameraMan virtual stream start rejected: {error}");
                            // SAFETY: CMIO provided `out_error` for this callback
                            // and permits either null or one writable NSError
                            // pointer.
                            unsafe { write_stream_error(out_error, STREAM_ERROR_START_REJECTED) };
                            false
                        }
                    }
                },
            )
        }

        #[unsafe(method(stopStreamAndReturnError:))]
        fn stop_stream_and_return_error(&self, out_error: *mut *mut NSError) -> bool {
            contain_panic(
                "stopStreamAndReturnError:",
                || {
                    self.repair_after_panic();
                    // SAFETY: CMIO provided `out_error` for this callback and
                    // permits either null or one writable NSError pointer.
                    unsafe { write_stream_error(out_error, STREAM_ERROR_CALLBACK_PANICKED) };
                    false
                },
                || {
                    eprintln!("CameraMan virtual stream stop requested.");
                    match self.stop_streaming() {
                        Ok(()) => true,
                        Err(error) => {
                            eprintln!("CameraMan virtual stream stop rejected: {error}");
                            // SAFETY: CMIO provided `out_error` for this callback
                            // and permits either null or one writable NSError
                            // pointer.
                            unsafe { write_stream_error(out_error, STREAM_ERROR_STOP_REJECTED) };
                            false
                        }
                    }
                },
            )
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
        let (action, reaped) = begin_start_reaping_finished_worker(
            &mut lifecycle,
            &self.ivars().streaming,
            self.reap_finished_worker(),
        )?;
        if let Some(interrupted) = reaped {
            eprintln!(
                "CameraMan stream worker ended while {interrupted:?}; stream reset for restart."
            );
        }
        if action == StartAction::AlreadyRunning {
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
            Self::join_worker(previous);
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
            Self::join_worker(worker);
        }
        self.ivars()
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .finish_stop()
    }

    /// Joins one worker and reports a panic that the thread boundary contained.
    fn join_worker(worker: thread::JoinHandle<()>) {
        if let Err(payload) = worker.join() {
            eprintln!(
                "CameraMan stream worker thread panicked: {}",
                panic_message(payload)
            );
        }
    }

    /// Joins a worker that ended on its own: a panic contained at the thread
    /// boundary, or an early return when the stream retain or the pixel pool
    /// failed. The caller hands the answer to
    /// `begin_start_reaping_finished_worker`, which owns the state repair.
    fn reap_finished_worker(&self) -> bool {
        let finished = self
            .ivars()
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take_if(|worker| worker.is_finished());
        match finished {
            Some(worker) => {
                Self::join_worker(worker);
                true
            }
            None => false,
        }
    }

    /// Runs on the uncontained fallback path of the start and stop callbacks,
    /// so nothing here may panic: `report_line` replaces `eprintln!`, which
    /// panics when stderr is gone.
    fn repair_after_panic(&self) {
        let interrupted = reset_after_contained_panic(
            &mut self
                .ivars()
                .lifecycle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            &self.ivars().streaming,
        );
        report_line(&format!(
            "CameraMan stream state reset from {interrupted:?} after a contained panic."
        ));
    }
}

/// Snapshots everything CMIO can report about a client so the pure policy in
/// `camera_man::client_authorization` decides without touching Objective-C.
fn client_identity(client: &CMIOExtensionClient) -> ClientIdentity {
    // SAFETY: CMIO supplies a live client object for the entire callback;
    // `clientID` and `signingID` return autoreleased objects and `pid` is a
    // plain scalar read.
    let (client_id, signing_id, pid) =
        unsafe { (client.clientID(), client.signingID(), client.pid()) };
    ClientIdentity {
        client_id: client_id.to_string(),
        signing_id: signing_id.map(|value| value.to_string()),
        pid: (pid > 0).then_some(pid),
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
