use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameraManError {
    EmptyInput,
    EmptyFrame,
    Capture(String),
    InvalidDimensions { width: u32, height: u32 },
    InvalidBufferLength { expected: usize, actual: usize },
    BufferTooLarge { bytes: u64 },
    Io(String),
    UnsupportedPixelFormat,
    VirtualCameraUnavailable(&'static str),
}

impl fmt::Display for CameraManError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "no input frames were provided"),
            Self::EmptyFrame => write!(f, "frame has no pixels"),
            Self::Capture(message) => write!(f, "capture error: {message}"),
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
            Self::Io(message) => write!(f, "io error: {message}"),
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
        Self::Io(error.to_string())
    }
}
