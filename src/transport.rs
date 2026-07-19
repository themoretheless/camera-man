use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::{Duration, Instant};

use crate::error::CameraManError;
use crate::frame::{Frame, FrameView};
use crate::frame_transport::{
    FrameSpoolSink, TransportFrame, default_frame_spool_path, read_latest_frame,
};
use crate::shared_memory_transport::{
    BorrowedTransportFrame, SharedFrameEndpoint, SharedFrameReader, SharedFrameSink,
    default_shared_frame_endpoint,
};
use crate::virtual_camera::VirtualCameraSink;
use crate::{ConsumerProgress, ProducerProgress};

pub const FRAME_TRANSPORT_ENV: &str = "CAMERAMAN_FRAME_TRANSPORT";
const SHARED_REOPEN_INTERVAL: Duration = Duration::from_millis(250);
const WRITER_LIVENESS_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameTransportMode {
    SharedMemory,
    File,
}

impl FrameTransportMode {
    pub fn from_environment() -> Self {
        if crate::app_group::bundled_application_group().is_some() {
            return Self::SharedMemory;
        }
        match std::env::var(FRAME_TRANSPORT_ENV) {
            Ok(value) if value.eq_ignore_ascii_case("file") => Self::File,
            _ => Self::SharedMemory,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::SharedMemory => "shared memory",
            Self::File => "file fallback",
        }
    }
}

pub struct FrameTransportSink {
    inner: SinkInner,
}

enum SinkInner {
    SharedMemory(SharedFrameSink),
    File(FrameSpoolSink),
}

impl Default for FrameTransportSink {
    fn default() -> Self {
        Self::new(FrameTransportMode::from_environment())
    }
}

impl FrameTransportSink {
    pub fn new(mode: FrameTransportMode) -> Self {
        let inner = match mode {
            FrameTransportMode::SharedMemory => SinkInner::SharedMemory(SharedFrameSink::default()),
            FrameTransportMode::File => SinkInner::File(FrameSpoolSink::default()),
        };
        Self { inner }
    }

    pub const fn mode(&self) -> FrameTransportMode {
        match self.inner {
            SinkInner::SharedMemory(_) => FrameTransportMode::SharedMemory,
            SinkInner::File(_) => FrameTransportMode::File,
        }
    }

    pub fn description(&self) -> String {
        match &self.inner {
            SinkInner::SharedMemory(sink) => format!("RAM {}", sink.description()),
            SinkInner::File(sink) => format!("File {}", sink.path().display()),
        }
    }

    pub fn set_target_fps(&mut self, fps: u32) {
        match &mut self.inner {
            SinkInner::SharedMemory(sink) => sink.set_target_fps(fps),
            SinkInner::File(sink) => sink.set_target_fps(fps),
        }
    }

    pub fn producer_progress(&self) -> Option<ProducerProgress> {
        match &self.inner {
            SinkInner::SharedMemory(sink) => sink.producer_progress(),
            SinkInner::File(_) => None,
        }
    }

    pub fn consumer_progress(&self) -> Option<ConsumerProgress> {
        match &self.inner {
            SinkInner::SharedMemory(sink) => sink.consumer_progress(),
            SinkInner::File(_) => None,
        }
    }
}

impl VirtualCameraSink for FrameTransportSink {
    fn connect(&mut self) -> Result<(), CameraManError> {
        match &mut self.inner {
            SinkInner::SharedMemory(sink) => sink.connect(),
            SinkInner::File(sink) => sink.connect(),
        }
    }

    fn send(&mut self, frame: &Frame) -> Result<(), CameraManError> {
        match &mut self.inner {
            SinkInner::SharedMemory(sink) => sink.send(frame),
            SinkInner::File(sink) => sink.send(frame),
        }
    }

    fn disconnect(&mut self) {
        match &mut self.inner {
            SinkInner::SharedMemory(sink) => sink.disconnect(),
            SinkInner::File(sink) => sink.disconnect(),
        }
    }
}

pub struct FrameTransportReader {
    inner: ReaderInner,
}

enum ReaderInner {
    SharedMemory {
        endpoint: SharedFrameEndpoint,
        reader: Option<SharedFrameReader>,
        last_open_attempt: Option<Instant>,
        last_liveness_check: Option<Instant>,
    },
    File {
        path: PathBuf,
        last_identity: Option<(u64, u64)>,
        last_stamp: Option<FileStamp>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    inode: u64,
}

/// Latest transport payload without forcing an mmap-to-heap copy. File
/// fallback frames stay owned because reading the spool already allocates.
pub enum TransportFrameRef<'a> {
    Shared(BorrowedTransportFrame<'a>),
    Owned(TransportFrame),
}

impl TransportFrameRef<'_> {
    pub fn frame_view(&self) -> FrameView<'_> {
        match self {
            Self::Shared(frame) => frame.frame,
            Self::Owned(frame) => frame.frame.as_view(),
        }
    }

    pub const fn sequence(&self) -> u64 {
        match self {
            Self::Shared(frame) => frame.sequence,
            Self::Owned(frame) => frame.sequence,
        }
    }

    pub const fn generation(&self) -> u64 {
        match self {
            Self::Shared(frame) => frame.generation,
            Self::Owned(frame) => frame.generation,
        }
    }

    pub const fn monotonic_timestamp_nanos(&self) -> u64 {
        match self {
            Self::Shared(frame) => frame.monotonic_timestamp_nanos,
            Self::Owned(frame) => frame.monotonic_timestamp_nanos,
        }
    }

    pub const fn timestamp_nanos(&self) -> u128 {
        match self {
            Self::Shared(frame) => frame.timestamp_nanos,
            Self::Owned(frame) => frame.timestamp_nanos,
        }
    }

    pub const fn fps(&self) -> u32 {
        match self {
            Self::Shared(frame) => frame.fps,
            Self::Owned(frame) => frame.fps,
        }
    }

    pub fn into_owned(self) -> Result<TransportFrame, CameraManError> {
        match self {
            Self::Shared(frame) => frame.into_owned(),
            Self::Owned(frame) => Ok(frame),
        }
    }
}

impl Default for FrameTransportReader {
    fn default() -> Self {
        Self::new(FrameTransportMode::from_environment())
    }
}

impl FrameTransportReader {
    pub fn new(mode: FrameTransportMode) -> Self {
        let inner = match mode {
            FrameTransportMode::SharedMemory => ReaderInner::SharedMemory {
                endpoint: default_shared_frame_endpoint(),
                reader: None,
                last_open_attempt: None,
                last_liveness_check: None,
            },
            FrameTransportMode::File => ReaderInner::File {
                path: default_frame_spool_path(),
                last_identity: None,
                last_stamp: None,
            },
        };
        Self { inner }
    }

    pub fn poll(&mut self) -> Result<Option<TransportFrame>, CameraManError> {
        self.poll_borrowed()?
            .map(TransportFrameRef::into_owned)
            .transpose()
    }

    pub fn poll_borrowed(&mut self) -> Result<Option<TransportFrameRef<'_>>, CameraManError> {
        match &mut self.inner {
            ReaderInner::SharedMemory {
                endpoint,
                reader,
                last_open_attempt,
                last_liveness_check,
            } => {
                let now = Instant::now();
                if reader.is_some()
                    && interval_due(last_liveness_check, now, WRITER_LIVENESS_INTERVAL)
                    && reader
                        .as_ref()
                        .is_some_and(|reader| !reader.writer_is_alive())
                {
                    *reader = None;
                    *last_open_attempt = None;
                }
                if reader.is_none() && interval_due(last_open_attempt, now, SHARED_REOPEN_INTERVAL)
                {
                    *reader = SharedFrameReader::try_open_endpoint(endpoint)?;
                    if reader.is_some() {
                        *last_liveness_check = Some(now);
                    }
                }
                match reader {
                    Some(reader) => reader
                        .read_latest_borrowed()
                        .map(|frame| frame.map(TransportFrameRef::Shared)),
                    None => Ok(None),
                }
            }
            ReaderInner::File {
                path,
                last_identity,
                last_stamp,
            } => {
                let stamp = file_stamp(path)?;
                if *last_stamp == stamp {
                    return Ok(None);
                }
                *last_stamp = stamp;
                let Some(frame) = read_latest_frame(path)? else {
                    return Ok(None);
                };
                let identity = (frame.generation, frame.sequence);
                if *last_identity == Some(identity) {
                    return Ok(None);
                }
                *last_identity = Some(identity);
                Ok(Some(TransportFrameRef::Owned(frame)))
            }
        }
    }

    pub const fn mode(&self) -> FrameTransportMode {
        match self.inner {
            ReaderInner::SharedMemory { .. } => FrameTransportMode::SharedMemory,
            ReaderInner::File { .. } => FrameTransportMode::File,
        }
    }
}

fn file_stamp(path: &PathBuf) -> Result<Option<FileStamp>, CameraManError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt as _;
    Ok(Some(FileStamp {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        #[cfg(unix)]
        inode: metadata.ino(),
    }))
}

fn interval_due(last: &mut Option<Instant>, now: Instant, interval: Duration) -> bool {
    if last.is_some_and(|last| now.saturating_duration_since(last) < interval) {
        return false;
    }
    *last = Some(now);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_memory_is_the_default_mode_name() {
        assert_eq!(FrameTransportMode::SharedMemory.name(), "shared memory");
        assert_eq!(FrameTransportMode::File.name(), "file fallback");
    }

    #[test]
    fn sink_describes_its_transport() {
        assert!(
            FrameTransportSink::new(FrameTransportMode::SharedMemory)
                .description()
                .starts_with("RAM POSIX /")
        );
        assert!(
            FrameTransportSink::new(FrameTransportMode::File)
                .description()
                .starts_with("File ")
        );
    }

    #[test]
    fn retry_interval_is_immediate_then_bounded() {
        let started = Instant::now();
        let mut last = None;

        assert!(interval_due(&mut last, started, SHARED_REOPEN_INTERVAL));
        assert!(!interval_due(
            &mut last,
            started + SHARED_REOPEN_INTERVAL / 2,
            SHARED_REOPEN_INTERVAL
        ));
        assert!(interval_due(
            &mut last,
            started + SHARED_REOPEN_INTERVAL,
            SHARED_REOPEN_INTERVAL
        ));
    }

    #[test]
    fn file_stamp_skips_unchanged_spool_and_detects_replacement() {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "camera-man-file-stamp-{}-{nonce}",
            std::process::id()
        ));
        let path = directory.join("frame.bgra");
        fs::create_dir_all(&directory).unwrap();
        assert_eq!(file_stamp(&path).unwrap(), None);
        fs::write(&path, b"first").unwrap();
        let first = file_stamp(&path).unwrap();
        assert_eq!(file_stamp(&path).unwrap(), first);

        crate::replace_file_atomically(&path, b"second").unwrap();

        assert_ne!(file_stamp(&path).unwrap(), first);
        fs::remove_dir_all(directory).unwrap();
    }
}
