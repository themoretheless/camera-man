//! The seam tests in `src/camera.rs` can see crate internals. This file is
//! compiled as a separate crate against the published API only, so what fails
//! here is a third party's compile error rather than a passing internal check.

use std::thread;
use std::time::Duration;

use camera_man::{
    CameraManError, CameraRuntime, CaptureErrorKind, CapturedFrame, ErrorCode, Frame,
    FrameMetadata, FrameSource, PixelFormat, SyntheticFrameSource, VideoFormat,
};

const LOOPBACK_FORMAT: VideoFormat = VideoFormat {
    width: 2,
    height: 2,
    fps: 7,
    pixel_format: PixelFormat::Bgra8,
};

/// A camera backend with no device, no worker thread and no capture library:
/// the shape a third party would write.
struct LoopbackCameraSource {
    format: Option<VideoFormat>,
    sequence: u64,
    stall: Duration,
    gap_exceeded: bool,
    failure: Option<CameraManError>,
}

impl LoopbackCameraSource {
    fn warming_up() -> Self {
        Self {
            format: None,
            sequence: 0,
            stall: Duration::ZERO,
            gap_exceeded: false,
            failure: None,
        }
    }

    fn open(format: VideoFormat) -> Self {
        Self {
            format: Some(format),
            ..Self::warming_up()
        }
    }

    fn with_stall(mut self, stall: Duration) -> Self {
        self.stall = stall;
        self
    }

    fn failing(mut self, failure: CameraManError) -> Self {
        self.failure = Some(failure);
        self
    }
}

impl FrameSource for LoopbackCameraSource {
    fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
        if let Some(failure) = self.failure.take() {
            return Err(failure);
        }
        let Some(format) = self.format else {
            return Ok(None);
        };
        self.sequence += 1;
        let frame = Frame::solid_bgra(format.width, format.height, [0, 0, 0, 255])?;
        Ok(Some(CapturedFrame::new(
            frame,
            FrameMetadata::new("loopback", self.sequence),
        )))
    }
}

impl CameraRuntime for LoopbackCameraSource {
    fn negotiated_format(&self) -> Option<VideoFormat> {
        self.format
    }

    fn stall_age(&self) -> Duration {
        self.stall
    }

    fn frame_gap_exceeded(&self) -> bool {
        self.gap_exceeded
    }

    fn as_frame_source(&mut self) -> &mut dyn FrameSource {
        self
    }
}

/// The slot shape `CameraManApp::real_sources` stores: a runtime locator, the
/// boxed backend, and the consecutive-failure streak the retry badge shows.
type AppSourceSlot = (String, Box<dyn CameraRuntime>, u32);

fn app_slots(source: LoopbackCameraSource) -> Vec<AppSourceSlot> {
    vec![("loopback-0".to_owned(), Box::new(source), 0)]
}

#[test]
fn a_backend_fits_the_handle_shape_the_app_stores() {
    let mut sources = app_slots(LoopbackCameraSource::warming_up());

    {
        let (locator, warming, streak) = sources.first_mut().unwrap();
        assert_eq!(locator.as_str(), "loopback-0");
        assert_eq!(warming.negotiated_fps(), None);
        assert!(warming.as_frame_source().latest_frame().unwrap().is_none());
        assert!(!warming.frame_gap_exceeded());
        // A source that has not delivered yet is not a failure to read.
        assert_eq!(*streak, 0);
    }

    sources[0].1 = Box::new(LoopbackCameraSource::open(LOOPBACK_FORMAT));

    {
        let (_, source, streak) = sources.first_mut().unwrap();
        assert_eq!(source.negotiated_format(), Some(LOOPBACK_FORMAT));
        // The app takes its fps preset from the negotiated format alone, so the
        // trait's default derivation has to hold for a backend it has never seen.
        assert_eq!(source.negotiated_fps(), Some(LOOPBACK_FORMAT.fps));
        assert_eq!(source.stall_age(), Duration::ZERO);

        *streak = streak.saturating_add(1);
        let frame = source
            .as_frame_source()
            .latest_frame()
            .unwrap()
            .expect("an opened backend delivers a frame");
        assert_eq!(frame.metadata().sequence, 1);
        assert_eq!(frame.frame().bgra_at(0, 0), Some([0, 0, 0, 255]));
        *streak = 0;
    }
    assert_eq!(sources[0].2, 0);
}

#[test]
fn a_replacement_carries_the_stall_of_the_source_it_replaces() {
    let mut sources =
        app_slots(LoopbackCameraSource::open(LOOPBACK_FORMAT).with_stall(Duration::from_secs(41)));
    sources[0].2 = 3;

    // Retry reads the age off the wedged source before dropping it, so a
    // replacement cannot restart the deadline and look like a fresh warm-up.
    let inherited = sources[0].1.stall_age();
    sources.retain(|(id, _, _)| id != "loopback-0");
    assert!(sources.is_empty());

    sources.push((
        "loopback-0".to_owned(),
        Box::new(LoopbackCameraSource::open(LOOPBACK_FORMAT).with_stall(inherited)),
        0,
    ));
    assert_eq!(sources[0].1.stall_age(), inherited);
}

#[test]
fn a_backend_reports_the_timeout_the_app_acts_on() {
    let mut source: Box<dyn CameraRuntime> = Box::new(
        LoopbackCameraSource::open(LOOPBACK_FORMAT)
            .failing(CameraManError::capture("camera read timed out")),
    );

    let error = source
        .as_frame_source()
        .latest_frame()
        .expect_err("the scripted backend fails once");
    // This is the comparison the read loop raises the not-responding overlay on,
    // and it has to be reachable from a message a backend writes itself.
    assert_eq!(error.code(), ErrorCode::Capture(CaptureErrorKind::Timeout));
    assert!(
        error
            .actionable_message("Read camera Loopback")
            .starts_with("Read camera Loopback failed.")
    );
}

#[test]
fn a_backend_needs_no_camera_state_to_be_a_source() {
    let mut sources: Vec<Box<dyn FrameSource>> = vec![
        Box::new(LoopbackCameraSource::open(LOOPBACK_FORMAT)),
        Box::new(SyntheticFrameSource::new(2, 2, [9, 9, 9, 255])),
    ];

    let mut delivered = 0;
    for source in &mut sources {
        if source.latest_frame().unwrap().is_some() {
            delivered += 1;
        }
    }
    assert_eq!(delivered, 2);
}

#[test]
fn the_handle_moves_to_another_thread_and_back() {
    let handle: Box<dyn CameraRuntime> = Box::new(LoopbackCameraSource::open(LOOPBACK_FORMAT));

    let (frames, fps) = thread::spawn(move || {
        let mut handle = handle;
        let mut frames = 0;
        for _ in 0..3 {
            if handle.as_frame_source().latest_frame().unwrap().is_some() {
                frames += 1;
            }
        }
        (frames, handle.negotiated_fps())
    })
    .join()
    .expect("the backend thread must not panic");

    assert_eq!((frames, fps), (3, Some(LOOPBACK_FORMAT.fps)));
}
