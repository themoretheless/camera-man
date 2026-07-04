use std::fmt;
use std::sync::{Arc, Mutex};

/// Bundle identifier of the embedded CoreMediaIO extension. `main.rs` reuses
/// this constant for the extension's `Info.plist` (`CFBundleIdentifier` and
/// `CMIOExtensionMachServiceName`) and its `.systemextension` bundle path, so
/// this is the single source of truth for the id.
pub const EXTENSION_BUNDLE_ID: &str = "com.cameraman.rust.extension";

/// Outcome of an `OSSystemExtensionRequest` activation, as reported
/// asynchronously by macOS through `OSSystemExtensionRequestDelegate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionActivationStatus {
    /// No activation request has been submitted yet.
    Idle,
    /// The request was submitted; macOS has not reported an outcome yet.
    Requesting,
    /// macOS needs the user to approve the extension in System Settings
    /// before it can activate. The request stays pending until they do (or
    /// the app quits).
    NeedsApproval,
    /// The extension is active.
    Activated,
    /// The extension will become active after the next reboot.
    WillCompleteAfterReboot,
    /// The request failed. The string is macOS's own error description.
    Failed(String),
}

impl fmt::Display for ExtensionActivationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Extension not requested yet"),
            Self::Requesting => write!(f, "Requesting extension activation..."),
            Self::NeedsApproval => write!(
                f,
                "Approve the extension in System Settings > General > Login Items & Extensions"
            ),
            Self::Activated => write!(f, "Extension activated"),
            Self::WillCompleteAfterReboot => {
                write!(f, "Extension will activate after the next reboot")
            }
            Self::Failed(message) => write!(f, "Extension activation failed: {message}"),
        }
    }
}

/// Requests activation of the bundled CoreMediaIO system extension and
/// tracks the (asynchronous, delegate-driven) outcome.
///
/// This only works when running from an installed `.app` bundle: macOS
/// discovers extensions under `Contents/Library/SystemExtensions` of the
/// *running* application, so `cargo run` (which executes a bare binary, not
/// a bundle) cannot activate anything. Build with `cargo run -- bundle`,
/// copy the result to `/Applications`, and launch it from there.
pub struct ExtensionInstaller {
    status: Arc<Mutex<ExtensionActivationStatus>>,
    #[cfg(target_os = "macos")]
    active: Option<macos::ActiveRequest>,
}

impl Default for ExtensionInstaller {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionInstaller {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(ExtensionActivationStatus::Idle)),
            #[cfg(target_os = "macos")]
            active: None,
        }
    }

    /// Current activation status. Cheap; safe to poll every frame.
    pub fn status(&self) -> ExtensionActivationStatus {
        self.status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Submits (or re-submits) the activation request. Safe to call again
    /// after a failure to retry; macOS treats a repeat request for an
    /// already-active extension as a fast no-op success.
    pub fn activate(&mut self) {
        self.set_status(ExtensionActivationStatus::Requesting);
        self.activate_platform();
    }

    fn set_status(&self, status: ExtensionActivationStatus) {
        let mut guard = self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = status;
    }

    #[cfg(target_os = "macos")]
    fn activate_platform(&mut self) {
        self.active = Some(macos::submit_activation_request(Arc::clone(&self.status)));
    }

    #[cfg(not(target_os = "macos"))]
    fn activate_platform(&mut self) {
        self.set_status(ExtensionActivationStatus::Failed(String::from(
            "system-extension activation is only implemented on macOS",
        )));
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{EXTENSION_BUNDLE_ID, ExtensionActivationStatus};
    use std::sync::{Arc, Mutex};

    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2::{AnyThread, DefinedClass, define_class, msg_send};
    use objc2_foundation::{NSError, NSObject, NSObjectProtocol, NSString};
    use objc2_system_extensions::{
        OSSystemExtensionManager, OSSystemExtensionProperties, OSSystemExtensionReplacementAction,
        OSSystemExtensionRequest, OSSystemExtensionRequestDelegate, OSSystemExtensionRequestResult,
    };

    /// Keeps the request and its delegate alive for as long as activation
    /// might still be pending. `OSSystemExtensionRequest.delegate` is a
    /// *weak* property: if nothing else retained the delegate, it would be
    /// deallocated right after `submitRequest` returns and every subsequent
    /// callback (including "needs approval" and the final result) would
    /// silently land on a dangling weak reference and do nothing.
    pub struct ActiveRequest {
        _request: Retained<OSSystemExtensionRequest>,
        _delegate: Retained<ActivationDelegate>,
    }

    pub fn submit_activation_request(
        status: Arc<Mutex<ExtensionActivationStatus>>,
    ) -> ActiveRequest {
        let delegate = ActivationDelegate::new(status);
        let identifier = NSString::from_str(EXTENSION_BUNDLE_ID);
        let request = unsafe {
            OSSystemExtensionRequest::activationRequestForExtension_queue(
                &identifier,
                dispatch2::DispatchQueue::main(),
            )
        };
        unsafe {
            request.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        }
        unsafe {
            OSSystemExtensionManager::sharedManager().submitRequest(&request);
        }
        ActiveRequest {
            _request: request,
            _delegate: delegate,
        }
    }

    struct DelegateIvars {
        status: Arc<Mutex<ExtensionActivationStatus>>,
    }

    define_class!(
        #[unsafe(super = NSObject)]
        #[name = "CameraManExtensionActivationDelegate"]
        #[ivars = DelegateIvars]
        struct ActivationDelegate;

        unsafe impl NSObjectProtocol for ActivationDelegate {}

        unsafe impl OSSystemExtensionRequestDelegate for ActivationDelegate {
            #[unsafe(method(request:actionForReplacingExtension:withExtension:))]
            fn action_for_replacing_extension(
                &self,
                _request: &OSSystemExtensionRequest,
                _existing: &OSSystemExtensionProperties,
                _ext: &OSSystemExtensionProperties,
            ) -> OSSystemExtensionReplacementAction {
                // Only our own extension is ever requested here, so replacing
                // an older installed copy with this one is always desired.
                OSSystemExtensionReplacementAction::Replace
            }

            #[unsafe(method(requestNeedsUserApproval:))]
            fn request_needs_user_approval(&self, _request: &OSSystemExtensionRequest) {
                self.set_status(ExtensionActivationStatus::NeedsApproval);
            }

            #[unsafe(method(request:didFinishWithResult:))]
            fn request_did_finish_with_result(
                &self,
                _request: &OSSystemExtensionRequest,
                result: OSSystemExtensionRequestResult,
            ) {
                let status = if result == OSSystemExtensionRequestResult::WillCompleteAfterReboot {
                    ExtensionActivationStatus::WillCompleteAfterReboot
                } else {
                    ExtensionActivationStatus::Activated
                };
                self.set_status(status);
            }

            #[unsafe(method(request:didFailWithError:))]
            fn request_did_fail_with_error(
                &self,
                _request: &OSSystemExtensionRequest,
                error: &NSError,
            ) {
                let message = error.localizedDescription().to_string();
                self.set_status(ExtensionActivationStatus::Failed(message));
            }
        }
    );

    impl ActivationDelegate {
        fn new(status: Arc<Mutex<ExtensionActivationStatus>>) -> Retained<Self> {
            let this = Self::alloc().set_ivars(DelegateIvars { status });
            unsafe { msg_send![super(this), init] }
        }

        fn set_status(&self, status: ExtensionActivationStatus) {
            if let Ok(mut guard) = self.ivars().status.lock() {
                *guard = status;
            }
        }
    }
}
