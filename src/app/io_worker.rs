use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use camera_man::Frame;
use serde::Serialize;

pub(super) const DEFAULT_DIAGNOSTICS_PATH: &str = "target/cameraman-diagnostics.json";
const DIAGNOSTICS_SCHEMA_VERSION: u32 = 1;
const IO_ACTIVE: u8 = 0;
const IO_CANCELLED: u8 = 1;
const IO_COMMITTING: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IoOperation {
    FrameExport,
    DiagnosticsExport,
}

impl fmt::Display for IoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FrameExport => "frame export",
            Self::DiagnosticsExport => "diagnostics export",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum IoOutcome {
    Completed,
    Cancelled,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct IoResult {
    pub(super) operation: IoOperation,
    pub(super) path: PathBuf,
    pub(super) outcome: IoOutcome,
}

/// Versioned support snapshot with identifiers, paths and device names omitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct RedactedDiagnostics {
    schema_version: u32,
    generated_unix_seconds: u64,
    app_version: String,
    launch_context: String,
    provisioning_profile_status: String,
    extension_profile_present: bool,
    activation_capable: bool,
    activation_status: String,
    transport: &'static str,
    selected_source_count: usize,
    scene_count: usize,
    running: bool,
    reduce_motion: bool,
    increase_contrast: bool,
    differentiate_without_color: bool,
    voice_over_enabled: bool,
    redacted_fields: [&'static str; 5],
}

impl RedactedDiagnostics {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        launch_context: impl Into<String>,
        provisioning_profile_status: impl Into<String>,
        extension_profile_present: bool,
        activation_capable: bool,
        activation_status: impl Into<String>,
        selected_source_count: usize,
        scene_count: usize,
        running: bool,
        reduce_motion: bool,
        increase_contrast: bool,
        differentiate_without_color: bool,
        voice_over_enabled: bool,
    ) -> Self {
        let generated_unix_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            schema_version: DIAGNOSTICS_SCHEMA_VERSION,
            generated_unix_seconds,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            launch_context: launch_context.into(),
            provisioning_profile_status: provisioning_profile_status.into(),
            extension_profile_present,
            activation_capable,
            activation_status: activation_status.into(),
            transport: "local_shared_memory",
            selected_source_count,
            scene_count,
            running,
            reduce_motion,
            increase_contrast,
            differentiate_without_color,
            voice_over_enabled,
            redacted_fields: [
                "camera device names and identifiers",
                "filesystem paths",
                "signing team identifiers",
                "transport endpoint paths",
                "frame pixels",
            ],
        }
    }
}

enum IoJob {
    ExportFrame {
        frame: Frame,
        path: PathBuf,
    },
    ExportDiagnostics {
        diagnostics: RedactedDiagnostics,
        path: PathBuf,
    },
}

impl IoJob {
    fn operation(&self) -> IoOperation {
        match self {
            Self::ExportFrame { .. } => IoOperation::FrameExport,
            Self::ExportDiagnostics { .. } => IoOperation::DiagnosticsExport,
        }
    }

    fn path(&self) -> &Path {
        match self {
            Self::ExportFrame { path, .. } | Self::ExportDiagnostics { path, .. } => path,
        }
    }
}

struct ActiveIo {
    receiver: Receiver<IoResult>,
    state: Arc<AtomicU8>,
    operation: IoOperation,
    path: PathBuf,
}

/// One bounded background file operation. A cancelled job never performs the
/// final atomic replace, so an existing readable destination remains intact.
pub(super) struct IoWorker {
    active: Option<ActiveIo>,
}

impl IoWorker {
    pub(super) const fn new() -> Self {
        Self { active: None }
    }

    pub(super) fn is_running(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn is_cancelling(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.state.load(Ordering::Acquire) == IO_CANCELLED)
    }

    pub(super) fn can_cancel(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.state.load(Ordering::Acquire) == IO_ACTIVE)
    }

    pub(super) fn operation(&self) -> Option<IoOperation> {
        self.active.as_ref().map(|active| active.operation)
    }

    pub(super) fn start_frame_export(
        &mut self,
        frame: Frame,
        path: PathBuf,
    ) -> Result<bool, String> {
        self.start(IoJob::ExportFrame { frame, path })
    }

    pub(super) fn start_diagnostics_export(
        &mut self,
        diagnostics: RedactedDiagnostics,
        path: PathBuf,
    ) -> Result<bool, String> {
        self.start(IoJob::ExportDiagnostics { diagnostics, path })
    }

    pub(super) fn cancel(&mut self) -> bool {
        let Some(active) = self.active.as_ref() else {
            return false;
        };
        active
            .state
            .compare_exchange(IO_ACTIVE, IO_CANCELLED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(super) fn poll(&mut self) -> Option<IoResult> {
        let result = match self.active.as_ref()?.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                let active = self.active.as_ref().expect("active I/O disappeared");
                Some(IoResult {
                    operation: active.operation,
                    path: active.path.clone(),
                    outcome: IoOutcome::Failed(String::from(
                        "background file worker disconnected before reporting a result",
                    )),
                })
            }
        };
        if result.is_some() {
            self.active = None;
        }
        result
    }

    fn start(&mut self, job: IoJob) -> Result<bool, String> {
        if self.is_running() {
            return Ok(false);
        }
        debug_assert_eq!(camera_man::backpressure::IO_RESULTS.capacity, 1);
        let operation = job.operation();
        let path = job.path().to_owned();
        let result_path = path.clone();
        let state = Arc::new(AtomicU8::new(IO_ACTIVE));
        let worker_state = Arc::clone(&state);
        let (sender, receiver) = mpsc::sync_channel(camera_man::backpressure::IO_RESULTS.capacity);
        thread::Builder::new()
            .name(String::from("camera-man-file-io"))
            .spawn(move || {
                let outcome = run_job(job, &worker_state);
                let _ = sender.send(IoResult {
                    operation,
                    path,
                    outcome,
                });
            })
            .map_err(|error| format!("could not start background file worker: {error}"))?;
        self.active = Some(ActiveIo {
            receiver,
            state,
            operation,
            path: result_path,
        });
        Ok(true)
    }
}

fn run_job(job: IoJob, state: &AtomicU8) -> IoOutcome {
    let result = match job {
        IoJob::ExportFrame { frame, path } => {
            encode_ppm(&frame, state).and_then(|bytes| commit_if_active(&path, &bytes, state))
        }
        IoJob::ExportDiagnostics { diagnostics, path } => serde_json::to_vec_pretty(&diagnostics)
            .map_err(|error| error.to_string())
            .and_then(|bytes| commit_if_active(&path, &bytes, state)),
    };
    match result {
        Ok(()) => IoOutcome::Completed,
        Err(error) if error == "cancelled" => IoOutcome::Cancelled,
        Err(error) => IoOutcome::Failed(error),
    }
}

fn encode_ppm(frame: &Frame, state: &AtomicU8) -> Result<Vec<u8>, String> {
    let pixel_count = frame.data().len() / 4;
    let mut bytes = format!("P6\n{} {}\n255\n", frame.width(), frame.height()).into_bytes();
    bytes.reserve(pixel_count.saturating_mul(3));
    for (index, pixels) in frame.data().chunks(4 * 4096).enumerate() {
        if index.is_multiple_of(4) && state.load(Ordering::Acquire) == IO_CANCELLED {
            return Err(String::from("cancelled"));
        }
        for pixel in pixels.chunks_exact(4) {
            bytes.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
    }
    Ok(bytes)
}

fn commit_if_active(path: &Path, bytes: &[u8], state: &AtomicU8) -> Result<(), String> {
    camera_man::atomic_file::replace_file_atomically_if(path, bytes, || {
        state
            .compare_exchange(
                IO_ACTIVE,
                IO_COMMITTING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    })
    .map_err(|error| format!("could not atomically replace {}: {error}", path.display()))?
    .then_some(())
    .ok_or_else(|| String::from("cancelled"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use camera_man::PixelFormat;

    #[test]
    fn cancelled_export_preserves_previous_destination() {
        let directory = temporary_directory("cancel");
        let path = directory.join("frame.ppm");
        std::fs::write(&path, b"old frame").unwrap();
        let frame = Frame::new_checked(2, 1, PixelFormat::Bgra8, vec![0; 8]).unwrap();
        let cancelled = AtomicU8::new(IO_CANCELLED);

        assert_eq!(
            run_job(
                IoJob::ExportFrame {
                    frame,
                    path: path.clone(),
                },
                &cancelled,
            ),
            IoOutcome::Cancelled
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"old frame");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn diagnostics_are_versioned_and_omit_sensitive_values() {
        let diagnostics = RedactedDiagnostics::new(
            "development",
            "valid",
            true,
            true,
            "activated",
            2,
            1,
            false,
            true,
            false,
            true,
            false,
        );
        let json = serde_json::to_string(&diagnostics).unwrap();

        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"redacted_fields\""));
        assert!(!json.contains("TEAM123456"));
        assert!(!json.contains("/Users/example"));
    }

    fn temporary_directory(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "camera-man-io-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }
}
