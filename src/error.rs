use std::fmt;

/// Coarse classification of a capture failure, so callers can decide whether
/// retrying makes sense (Disconnected/DeviceBusy) or the user must act first
/// (PermissionDenied). Derived from `nokhwa::NokhwaError` by
/// `CaptureErrorKind::classify`, which is a text-matching heuristic over
/// nokhwa's error messages (не проверено against real AVFoundation failures,
/// since that requires hardware access) rather than a guaranteed mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        if lower.contains("not found") || lower.contains("no such device") {
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
    BufferTooLarge {
        bytes: u64,
    },
    Io {
        kind: std::io::ErrorKind,
        message: String,
    },
    UnsupportedPixelFormat,
    VirtualCameraUnavailable(&'static str),
}

impl CameraManError {
    /// Builds a `Capture` error, classifying the message automatically.
    pub fn capture(message: impl Into<String>) -> Self {
        let message = message.into();
        let kind = CaptureErrorKind::classify(&message);
        Self::Capture { kind, message }
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
            Self::BufferTooLarge { bytes } => {
                write!(f, "frame buffer too large: {bytes} bytes")
            }
            Self::Io { kind, message } => write!(f, "io error ({kind:?}): {message}"),
            Self::UnsupportedPixelFormat => write!(f, "unsupported pixel format"),
            Self::VirtualCameraUnavailable(reason) => {
                write!(f, "virtual camera unavailable: {reason}")
            }
        }
    }
}

impl std::error::Error for CameraManError {}

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
            CaptureErrorKind::classify("something weird happened"),
            CaptureErrorKind::Other
        );
    }
}
