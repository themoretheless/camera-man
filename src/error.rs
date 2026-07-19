use std::fmt;

/// Coarse classification of a capture failure, so callers can decide whether
/// retrying makes sense (Disconnected/DeviceBusy) or the user must act first
/// (PermissionDenied). Derived from `nokhwa::NokhwaError` by
/// `CaptureErrorKind::classify`, which is a text-matching heuristic over
/// nokhwa's error messages (не проверено against real AVFoundation failures,
/// since that requires hardware access) rather than a guaranteed mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureErrorKind {
    /// The OS denied camera access (TCC permission not granted).
    PermissionDenied,
    /// Another process is already using the device.
    DeviceBusy,
    /// The requested device id/index does not exist.
    DeviceNotFound,
    /// The device was unplugged or the stream stopped unexpectedly.
    Disconnected,
    /// The backend does not support the requested operation.
    Unsupported,
    /// A bounded capture operation did not finish before its deadline.
    Timeout,
    /// Anything not classified above.
    Other,
}

impl CaptureErrorKind {
    /// Heuristic classification from an error message. Case-insensitive
    /// substring matching over vendor error text; treat this as a best
    /// effort, not a contract. Checks run in the fixed priority order listed
    /// below (first match wins), so a message that plausibly describes two
    /// conditions at once is not disambiguated, it is resolved by priority.
    /// A prior version used a bare "already" keyword for DeviceBusy, which
    /// misclassified messages like "no such device, already removed" as
    /// busy instead of not-found; the phrase is now specific enough to
    /// require an explicit "in use" style qualifier.
    pub fn classify(message: &str) -> Self {
        let lower = message.to_ascii_lowercase();
        if lower.contains("timed out") || lower.contains("timeout") {
            Self::Timeout
        } else if lower.contains("not found") || lower.contains("no such device") {
            Self::DeviceNotFound
        } else if lower.contains("disconnect")
            || lower.contains("unplug")
            || lower.contains("stopped")
        {
            Self::Disconnected
        } else if lower.contains("permission")
            || lower.contains("denied")
            || lower.contains("authoriz")
        {
            Self::PermissionDenied
        } else if lower.contains("busy")
            || lower.contains("in use")
            || lower.contains("already open")
            || lower.contains("already streaming")
            || lower.contains("already capturing")
        {
            Self::DeviceBusy
        } else if lower.contains("not supported") || lower.contains("not implemented") {
            Self::Unsupported
        } else {
            Self::Other
        }
    }
}

/// Stable machine-readable classification for logs, metrics and UI routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    EmptyInput,
    EmptyFrame,
    Capture(CaptureErrorKind),
    InvalidDimensions,
    InvalidBufferLength,
    InvalidRowStride,
    InvalidMediaContract,
    PixelOutOfBounds,
    BufferTooLarge,
    FrameLimitExceeded,
    Io,
    UnsupportedPixelFormat,
    VirtualCameraUnavailable,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyInput => "empty_input",
            Self::EmptyFrame => "empty_frame",
            Self::Capture(CaptureErrorKind::PermissionDenied) => "capture_permission_denied",
            Self::Capture(CaptureErrorKind::DeviceBusy) => "capture_device_busy",
            Self::Capture(CaptureErrorKind::DeviceNotFound) => "capture_device_not_found",
            Self::Capture(CaptureErrorKind::Disconnected) => "capture_disconnected",
            Self::Capture(CaptureErrorKind::Unsupported) => "capture_unsupported",
            Self::Capture(CaptureErrorKind::Timeout) => "capture_timeout",
            Self::Capture(CaptureErrorKind::Other) => "capture_other",
            Self::InvalidDimensions => "invalid_dimensions",
            Self::InvalidBufferLength => "invalid_buffer_length",
            Self::InvalidRowStride => "invalid_row_stride",
            Self::InvalidMediaContract => "invalid_media_contract",
            Self::PixelOutOfBounds => "pixel_out_of_bounds",
            Self::BufferTooLarge => "buffer_too_large",
            Self::FrameLimitExceeded => "frame_limit_exceeded",
            Self::Io => "io",
            Self::UnsupportedPixelFormat => "unsupported_pixel_format",
            Self::VirtualCameraUnavailable => "virtual_camera_unavailable",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameraManError {
    EmptyInput,
    EmptyFrame,
    Capture {
        kind: CaptureErrorKind,
        message: String,
    },
    InvalidDimensions {
        width: u32,
        height: u32,
    },
    InvalidBufferLength {
        expected: usize,
        actual: usize,
    },
    InvalidRowStride {
        minimum: usize,
        actual: usize,
    },
    InvalidMediaContract(&'static str),
    PixelOutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    BufferTooLarge {
        bytes: u64,
    },
    FrameLimitExceeded {
        bytes: u64,
        limit: u64,
    },
    Io {
        kind: std::io::ErrorKind,
        message: String,
    },
    UnsupportedPixelFormat,
    VirtualCameraUnavailable(&'static str),
    Context {
        operation: String,
        source: Box<CameraManError>,
    },
}

impl CameraManError {
    /// Builds a `Capture` error, classifying the message automatically.
    pub fn capture(message: impl Into<String>) -> Self {
        let message = message.into();
        let kind = CaptureErrorKind::classify(&message);
        Self::Capture { kind, message }
    }

    pub fn capture_with_kind(kind: CaptureErrorKind, message: impl Into<String>) -> Self {
        Self::Capture {
            kind,
            message: message.into(),
        }
    }

    pub fn context(self, operation: impl Into<String>) -> Self {
        Self::Context {
            operation: operation.into(),
            source: Box::new(self),
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::EmptyInput => ErrorCode::EmptyInput,
            Self::EmptyFrame => ErrorCode::EmptyFrame,
            Self::Capture { kind, .. } => ErrorCode::Capture(*kind),
            Self::InvalidDimensions { .. } => ErrorCode::InvalidDimensions,
            Self::InvalidBufferLength { .. } => ErrorCode::InvalidBufferLength,
            Self::InvalidRowStride { .. } => ErrorCode::InvalidRowStride,
            Self::InvalidMediaContract(_) => ErrorCode::InvalidMediaContract,
            Self::PixelOutOfBounds { .. } => ErrorCode::PixelOutOfBounds,
            Self::BufferTooLarge { .. } => ErrorCode::BufferTooLarge,
            Self::FrameLimitExceeded { .. } => ErrorCode::FrameLimitExceeded,
            Self::Io { .. } => ErrorCode::Io,
            Self::UnsupportedPixelFormat => ErrorCode::UnsupportedPixelFormat,
            Self::VirtualCameraUnavailable(_) => ErrorCode::VirtualCameraUnavailable,
            Self::Context { source, .. } => source.code(),
        }
    }

    pub fn user_message(&self) -> &'static str {
        match self {
            Self::EmptyInput => "No input frames are available.",
            Self::EmptyFrame => "A camera returned an empty frame.",
            Self::Capture { kind, .. } => match kind {
                CaptureErrorKind::PermissionDenied => {
                    "Camera access was denied. Allow CameraMan in System Settings > Privacy & Security > Camera."
                }
                CaptureErrorKind::DeviceBusy => "The camera is already in use by another app.",
                CaptureErrorKind::DeviceNotFound => "The selected camera is no longer available.",
                CaptureErrorKind::Disconnected => "The camera disconnected or stopped.",
                CaptureErrorKind::Unsupported => "This camera operation is not supported.",
                CaptureErrorKind::Timeout => "The camera did not respond in time.",
                CaptureErrorKind::Other => "Camera capture failed.",
            },
            Self::InvalidDimensions { .. } => "A video frame had invalid dimensions.",
            Self::InvalidBufferLength { .. } => "A video frame contained incomplete pixel data.",
            Self::InvalidRowStride { .. } => "A video frame used an invalid row stride.",
            Self::InvalidMediaContract(_) => {
                "A video frame used invalid color or geometry metadata."
            }
            Self::PixelOutOfBounds { .. } => "A pixel operation was outside the video frame.",
            Self::BufferTooLarge { .. } | Self::FrameLimitExceeded { .. } => {
                "A video frame exceeds the configured memory budget."
            }
            Self::Io { .. } => "A file or system operation failed.",
            Self::UnsupportedPixelFormat => "The video pixel format is not supported.",
            Self::VirtualCameraUnavailable(_) => "Virtual camera output is unavailable.",
            Self::Context { source, .. } => source.user_message(),
        }
    }

    /// A concrete next step suitable for a status surface. Technical details
    /// remain available through `diagnostic_message` and are not exposed alone.
    pub fn recovery_message(&self) -> &'static str {
        match self {
            Self::EmptyInput => "Select at least one available source, then retry.",
            Self::EmptyFrame => "Reconnect the source or choose another camera, then retry.",
            Self::Capture { kind, .. } => match kind {
                CaptureErrorKind::PermissionDenied => {
                    "Open System Settings > Privacy & Security > Camera, allow CameraMan, then retry."
                }
                CaptureErrorKind::DeviceBusy => {
                    "Close the other app using this camera, then retry."
                }
                CaptureErrorKind::DeviceNotFound | CaptureErrorKind::Disconnected => {
                    "Reconnect the camera, refresh the source list, then retry."
                }
                CaptureErrorKind::Unsupported => {
                    "Choose a supported camera format or a different source."
                }
                CaptureErrorKind::Timeout => {
                    "Check the camera connection and retry; export diagnostics if it repeats."
                }
                CaptureErrorKind::Other => {
                    "Retry once, then export diagnostics if the failure repeats."
                }
            },
            Self::Io { .. } => {
                "Check the destination, permissions, and free disk space, then retry."
            }
            Self::VirtualCameraUnavailable(_) => {
                "Open Setup, complete extension activation, then run the self-test."
            }
            Self::BufferTooLarge { .. } | Self::FrameLimitExceeded { .. } => {
                "Use a smaller source format or reduce the configured frame size."
            }
            Self::Context { source, .. } => source.recovery_message(),
            Self::InvalidDimensions { .. }
            | Self::InvalidBufferLength { .. }
            | Self::InvalidRowStride { .. }
            | Self::InvalidMediaContract(_)
            | Self::PixelOutOfBounds { .. }
            | Self::UnsupportedPixelFormat => {
                "Stop output, retry with another source, and export diagnostics if it repeats."
            }
        }
    }

    pub fn actionable_message(&self, operation: &str) -> String {
        format!(
            "{operation} failed. {} {}",
            self.user_message(),
            self.recovery_message()
        )
    }

    pub fn diagnostic_message(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for CameraManError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "no input frames were provided"),
            Self::EmptyFrame => write!(f, "frame has no pixels"),
            Self::Capture { kind, message } => write!(f, "capture error ({kind:?}): {message}"),
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid frame dimensions: {width}x{height}")
            }
            Self::InvalidBufferLength { expected, actual } => {
                write!(
                    f,
                    "invalid buffer length: expected {expected} bytes, got {actual}"
                )
            }
            Self::InvalidRowStride { minimum, actual } => {
                write!(
                    f,
                    "invalid row stride: minimum {minimum} bytes, got {actual}"
                )
            }
            Self::InvalidMediaContract(reason) => write!(f, "invalid media contract: {reason}"),
            Self::PixelOutOfBounds {
                x,
                y,
                width,
                height,
            } => write!(f, "pixel ({x}, {y}) is outside frame {width}x{height}"),
            Self::BufferTooLarge { bytes } => {
                write!(f, "frame buffer too large: {bytes} bytes")
            }
            Self::FrameLimitExceeded { bytes, limit } => {
                write!(
                    f,
                    "frame needs {bytes} bytes, configured limit is {limit} bytes"
                )
            }
            Self::Io { kind, message } => write!(f, "io error ({kind:?}): {message}"),
            Self::UnsupportedPixelFormat => write!(f, "unsupported pixel format"),
            Self::VirtualCameraUnavailable(reason) => {
                write!(f, "virtual camera unavailable: {reason}")
            }
            Self::Context { operation, source } => write!(f, "{operation}: {source}"),
        }
    }
}

impl std::error::Error for CameraManError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Context { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CameraManError {
    fn from(error: std::io::Error) -> Self {
        Self::Io {
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_capture_failures() {
        assert_eq!(
            CaptureErrorKind::classify("Permission denied by the OS"),
            CaptureErrorKind::PermissionDenied
        );
        assert_eq!(
            CaptureErrorKind::classify("Device is busy"),
            CaptureErrorKind::DeviceBusy
        );
        assert_eq!(
            CaptureErrorKind::classify("camera not found"),
            CaptureErrorKind::DeviceNotFound
        );
        assert_eq!(
            CaptureErrorKind::classify("stream stopped unexpectedly"),
            CaptureErrorKind::Disconnected
        );
        assert_eq!(
            CaptureErrorKind::classify("no such device, already removed"),
            CaptureErrorKind::DeviceNotFound,
            "a generic 'already' must not shadow a more specific not-found phrase"
        );
        assert_eq!(
            CaptureErrorKind::classify("operation not supported"),
            CaptureErrorKind::Unsupported
        );
        assert_eq!(
            CaptureErrorKind::classify("timed out waiting for frame"),
            CaptureErrorKind::Timeout
        );
        assert_eq!(
            CaptureErrorKind::classify("something weird happened"),
            CaptureErrorKind::Other
        );
    }

    #[test]
    fn structured_code_and_user_message_survive_context() {
        let error = CameraManError::capture_with_kind(
            CaptureErrorKind::PermissionDenied,
            "AVFoundation authorization status denied",
        )
        .context("open camera 0");

        assert_eq!(
            error.code(),
            ErrorCode::Capture(CaptureErrorKind::PermissionDenied)
        );
        assert_eq!(error.code().as_str(), "capture_permission_denied");
        assert_eq!(
            error.user_message(),
            "Camera access was denied. Allow CameraMan in System Settings > Privacy & Security > Camera."
        );
        assert_eq!(
            error.diagnostic_message(),
            "open camera 0: capture error (PermissionDenied): AVFoundation authorization status denied"
        );
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn actionable_message_contains_operation_cause_and_recovery() {
        let message = CameraManError::capture_with_kind(
            CaptureErrorKind::DeviceBusy,
            "AVFoundation reported exclusive ownership",
        )
        .actionable_message("Open Desk Camera");

        assert!(message.starts_with("Open Desk Camera failed."));
        assert!(message.contains("already in use"));
        assert!(message.contains("Close the other app"));
    }
}
