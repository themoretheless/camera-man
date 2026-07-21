use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use camera_man::{
    CameraDevice, CameraDiscovery, CameraManError, CaptureErrorKind, NokhwaCameraDiscovery,
};

type DiscoveryResult = Result<Vec<CameraDevice>, CameraManError>;

/// Runs camera enumeration away from egui and permits only one request at a time.
pub(crate) struct CameraDiscoveryWorker {
    receiver: Option<Receiver<DiscoveryResult>>,
}

impl CameraDiscoveryWorker {
    pub(crate) const fn new() -> Self {
        Self { receiver: None }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.receiver.is_some()
    }

    /// Stops waiting for the result. Platform enumeration may finish in its
    /// detached thread, but its sender is disconnected and cannot mutate UI.
    pub(crate) fn cancel(&mut self) -> bool {
        self.receiver.take().is_some()
    }

    /// Starts discovery unless an earlier request is still running.
    pub(crate) fn start(&mut self) -> Result<bool, CameraManError> {
        self.start_with(|| NokhwaCameraDiscovery.list_devices())
    }

    pub(crate) fn poll(&mut self) -> Option<DiscoveryResult> {
        let result = match self.receiver.as_ref()?.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(CameraManError::capture_with_kind(
                CaptureErrorKind::Other,
                "camera discovery worker disconnected",
            ))),
        };
        if result.is_some() {
            self.receiver = None;
        }
        result
    }

    fn start_with<F>(&mut self, discover: F) -> Result<bool, CameraManError>
    where
        F: FnOnce() -> DiscoveryResult + Send + 'static,
    {
        if self.is_running() {
            return Ok(false);
        }

        let (sender, receiver) =
            mpsc::sync_channel(camera_man::backpressure::CAMERA_DISCOVERY_RESULTS.capacity);
        thread::Builder::new()
            .name(String::from("camera-discovery"))
            .spawn(move || {
                let _ = sender.send(discover());
            })
            .map_err(|error| {
                CameraManError::capture_with_kind(
                    CaptureErrorKind::Other,
                    format!("failed to start camera discovery worker: {error}"),
                )
            })?;
        self.receiver = Some(receiver);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn returns_discovered_devices_and_reopens_the_slot() {
        let mut worker = CameraDiscoveryWorker::new();
        assert!(
            worker
                .start_with(|| { Ok(vec![CameraDevice::new("camera-1", "Camera One")]) })
                .unwrap()
        );

        let result = wait_for_result(&mut worker).unwrap();
        assert_eq!(result.unwrap()[0].id, "camera-1");
        assert!(!worker.is_running());
        assert!(worker.start_with(|| Ok(Vec::new())).unwrap());
    }

    #[test]
    fn rejects_a_second_request_while_discovery_is_running() {
        let (release_sender, release_receiver) = mpsc::channel();
        let mut worker = CameraDiscoveryWorker::new();
        assert!(
            worker
                .start_with(move || {
                    release_receiver.recv().unwrap();
                    Ok(Vec::new())
                })
                .unwrap()
        );

        assert!(!worker.start_with(|| Ok(Vec::new())).unwrap());
        release_sender.send(()).unwrap();
        assert!(wait_for_result(&mut worker).unwrap().is_ok());
    }

    #[test]
    fn cancelled_discovery_result_is_ignored_and_slot_reopens() {
        let (release_sender, release_receiver) = mpsc::channel();
        let mut worker = CameraDiscoveryWorker::new();
        worker
            .start_with(move || {
                release_receiver.recv().unwrap();
                Ok(vec![CameraDevice::new("late-camera", "Late Camera")])
            })
            .unwrap();

        assert!(worker.cancel());
        assert!(!worker.is_running());
        assert!(worker.poll().is_none());
        assert!(worker.start_with(|| Ok(Vec::new())).unwrap());
        release_sender.send(()).unwrap();
        assert!(wait_for_result(&mut worker).unwrap().is_ok());
    }

    fn wait_for_result(worker: &mut CameraDiscoveryWorker) -> Option<DiscoveryResult> {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if let Some(result) = worker.poll() {
                return Some(result);
            }
            thread::sleep(Duration::from_millis(1));
        }
        None
    }
}
