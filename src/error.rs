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
    /// Stable code, user-facing cause and recovery step for this kind, each
    /// defined once. Row order is the classification priority.
    pub(crate) const fn profile(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Timeout => (
                "capture_timeout",
                "The camera did not respond in time.",
                "Check the camera connection and retry; export diagnostics if it repeats.",
            ),
            Self::DeviceNotFound => (
                "capture_device_not_found",
                "The selected camera is no longer available.",
                Self::RECONNECT,
            ),
            Self::Disconnected => (
                "capture_disconnected",
                "The camera disconnected or stopped.",
                Self::RECONNECT,
            ),
            Self::PermissionDenied => (
                "capture_permission_denied",
                "Camera access was denied. Allow CameraMan in System Settings > Privacy & Security > Camera.",
                "Open System Settings > Privacy & Security > Camera, allow CameraMan, then retry.",
            ),
            Self::DeviceBusy => (
                "capture_device_busy",
                "The camera is already in use by another app.",
                "Close the other app using this camera, then retry.",
            ),
            Self::Unsupported => (
                "capture_unsupported",
                "This camera operation is not supported.",
                "Choose a supported camera format or a different source.",
            ),
            Self::Other => (
                "capture_other",
                "Camera capture failed.",
                "Retry once, then export diagnostics if the failure repeats.",
            ),
        }
    }

    const RECONNECT: &'static str = "Reconnect the camera, refresh the source list, then retry.";

    const CLASSIFIERS: &'static [(Self, &'static [&'static str])] = &[
        (Self::Timeout, &["timed out", "timeout"]),
        (Self::DeviceNotFound, &["not found", "no such device"]),
        (Self::Disconnected, &["disconnect", "unplug", "stopped"]),
        (
            Self::PermissionDenied,
            &["permission", "denied", "authoriz"],
        ),
        (
            Self::DeviceBusy,
            &[
                "busy",
                "in use",
                "already open",
                "already streaming",
                "already capturing",
            ],
        ),
        (Self::Unsupported, &["not supported", "not implemented"]),
    ];

    /// Heuristic classification from an error message. Case-insensitive
    /// substring matching over vendor error text; treat this as a best
    /// effort, not a contract. See [`Self::CLASSIFIERS`] for the order the
    /// needles are checked in.
    /// A prior version used a bare "already" keyword for DeviceBusy, which
    /// misclassified messages like "no such device, already removed" as
    /// busy instead of not-found; the phrase is now specific enough to
    /// require an explicit "in use" style qualifier.
    pub fn classify(message: &str) -> Self {
        let lower = message.to_ascii_lowercase();
        Self::CLASSIFIERS
            .iter()
            .find(|(_, needles)| needles.iter().any(|needle| lower.contains(needle)))
            .map_or(Self::Other, |(kind, _)| *kind)
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
    /// Stable code, user-facing cause and recovery step, each defined once per
    /// code. Neither half reads the error payload, so the profile is fully
    /// determined by the code.
    pub(crate) const fn profile(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::EmptyInput => (
                "empty_input",
                "No input frames are available.",
                "Select at least one available source, then retry.",
            ),
            Self::EmptyFrame => (
                "empty_frame",
                "A camera returned an empty frame.",
                "Reconnect the source or choose another camera, then retry.",
            ),
            Self::Capture(kind) => kind.profile(),
            Self::InvalidDimensions => (
                "invalid_dimensions",
                "A video frame had invalid dimensions.",
                Self::FRAME_DEFECT,
            ),
            Self::InvalidBufferLength => (
                "invalid_buffer_length",
                "A video frame contained incomplete pixel data.",
                Self::FRAME_DEFECT,
            ),
            Self::InvalidRowStride => (
                "invalid_row_stride",
                "A video frame used an invalid row stride.",
                Self::FRAME_DEFECT,
            ),
            Self::InvalidMediaContract => (
                "invalid_media_contract",
                "A video frame used invalid color or geometry metadata.",
                Self::FRAME_DEFECT,
            ),
            Self::PixelOutOfBounds => (
                "pixel_out_of_bounds",
                "A pixel operation was outside the video frame.",
                Self::FRAME_DEFECT,
            ),
            Self::UnsupportedPixelFormat => (
                "unsupported_pixel_format",
                "The video pixel format is not supported.",
                Self::FRAME_DEFECT,
            ),
            Self::BufferTooLarge => ("buffer_too_large", Self::OVER_BUDGET, Self::SMALLER_SOURCE),
            Self::FrameLimitExceeded => (
                "frame_limit_exceeded",
                Self::OVER_BUDGET,
                Self::SMALLER_SOURCE,
            ),
            Self::Io => (
                "io",
                "A file or system operation failed.",
                "Check the destination, permissions, and free disk space, then retry.",
            ),
            Self::VirtualCameraUnavailable => (
                "virtual_camera_unavailable",
                "Virtual camera output is unavailable.",
                "Open Setup, complete extension activation, then run the self-test.",
            ),
        }
    }

    const FRAME_DEFECT: &'static str =
        "Stop output, retry with another source, and export diagnostics if it repeats.";
    const OVER_BUDGET: &'static str = "A video frame exceeds the configured memory budget.";
    const SMALLER_SOURCE: &'static str =
        "Use a smaller source format or reduce the configured frame size.";

    pub const fn as_str(self) -> &'static str {
        self.profile().0
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
        self.code().profile().1
    }

    /// A concrete next step suitable for a status surface. Technical details
    /// remain available through `diagnostic_message` and are not exposed alone.
    pub fn recovery_message(&self) -> &'static str {
        self.code().profile().2
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
    fn every_classifier_row_can_actually_fire() {
        let rows = CaptureErrorKind::CLASSIFIERS;
        for (index, (kind, needles)) in rows.iter().enumerate() {
            for needle in *needles {
                // Only the message is case-folded, so a needle carrying uppercase
                // would silently stop matching instead of failing loudly.
                assert_eq!(
                    needle.to_ascii_lowercase(),
                    *needle,
                    "needle {needle} of {kind:?} does not match its own folded form"
                );
                let shadowed = rows[..index]
                    .iter()
                    .flat_map(|(_, earlier)| earlier.iter())
                    .find(|earlier| needle.contains(*earlier));
                // classify() takes the first row that matches, so a needle already
                // covered by an earlier row can never be reported by this one.
                assert!(
                    shadowed.is_none(),
                    "needle {needle} of {kind:?} is covered by earlier row needle {shadowed:?}"
                );
            }
        }
    }

    #[test]
    fn classifier_row_order_decides_a_message_that_matches_two_rows() {
        let rows = CaptureErrorKind::CLASSIFIERS;
        for (index, (winner, needles)) in rows.iter().enumerate() {
            for (loser, loser_needles) in &rows[index + 1..] {
                let message = format!("{} {}", needles[0], loser_needles[0]);
                assert_eq!(
                    CaptureErrorKind::classify(&message),
                    *winner,
                    "{message} matches both {winner:?} and {loser:?}, so table order decides"
                );
            }
        }
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
