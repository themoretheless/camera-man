use super::*;

pub(super) struct LatestFrameReader {
    pub(super) transport: FrameTransportReader,
    pub(super) last_frame: Option<FrameSnapshot>,
    pub(super) last_pixel_buffer: Option<CFRetained<CVPixelBuffer>>,
    pub(super) last_error: Option<String>,
    pub(super) stale: bool,
    pub(super) integrity: FrameIntegrityState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FrameSnapshot {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixel_format: PixelFormat,
    pub(super) sequence: u64,
    pub(super) generation: u64,
    pub(super) monotonic_timestamp_nanos: u64,
    pub(super) fps: u32,
}

impl FrameSnapshot {
    fn from_transport(frame: &TransportFrameRef<'_>) -> Self {
        let view = frame.frame_view();
        Self {
            width: view.width(),
            height: view.height(),
            pixel_format: view.pixel_format(),
            sequence: frame.sequence(),
            generation: frame.generation(),
            monotonic_timestamp_nanos: frame.monotonic_timestamp_nanos(),
            fps: frame.fps(),
        }
    }

    fn observation(self) -> FrameObservation {
        FrameObservation {
            width: self.width,
            height: self.height,
            pixel_format: self.pixel_format,
            sequence: self.sequence,
            generation: self.generation,
            monotonic_timestamp_nanos: self.monotonic_timestamp_nanos,
        }
    }
}

pub(super) struct FramePoll {
    pub(super) pixel_buffer: Option<CFRetained<CVPixelBuffer>>,
    pub(super) fps: u32,
    pub(super) discontinuity: FrameDiscontinuity,
}

impl LatestFrameReader {
    pub(super) fn new() -> Self {
        Self {
            transport: FrameTransportReader::default(),
            last_frame: None,
            last_pixel_buffer: None,
            last_error: None,
            stale: false,
            integrity: FrameIntegrityState::new("virtual-camera-output"),
        }
    }

    /// Polls the selected transport and returns the latest known frame together
    /// with its producer-reported fps. Both come from the same read, in
    /// one `&mut self` call, so callers never observe a frame from one
    /// tick paired with an fps from a different tick (a two-method split
    /// here would also fight the borrow checker: the `&Frame` returned
    /// would keep `self` mutably borrowed for as long as it is held).
    pub(super) fn poll(&mut self, pixel_pool: &OutputPixelBufferPool, sequence: u64) -> FramePoll {
        let mut discontinuity = FrameDiscontinuity::None;
        match self.transport.poll_borrowed() {
            Ok(Some(frame)) => {
                self.last_error = None;
                let snapshot = FrameSnapshot::from_transport(&frame);
                discontinuity = self.integrity.observe(snapshot.observation());
                if let Some(pixel_buffer) =
                    create_pixel_buffer(sequence, Some(frame.frame_view()), pixel_pool)
                {
                    self.last_pixel_buffer = Some(pixel_buffer);
                    self.last_frame = Some(snapshot);
                    self.stale = false;
                } else {
                    self.integrity
                        .mark_drop(IntegrityDropReason::ConsumerBackpressure, 1);
                    discontinuity = FrameDiscontinuity::FramesDropped;
                }
            }
            Ok(None) => {}
            Err(error) => {
                let message = error.to_string();
                if self.last_error.as_ref() != Some(&message) {
                    eprintln!("CameraMan frame transport read failed: {message}");
                    self.last_error = Some(message);
                    self.integrity
                        .mark_drop(IntegrityDropReason::TransportReadFailed, 1);
                }
            }
        }

        if self.last_frame.as_ref().is_some_and(frame_is_stale) {
            self.last_frame = None;
            self.last_pixel_buffer = None;
            if !self.stale {
                discontinuity = self.integrity.mark_stale();
                self.stale = true;
                record_drop(DropReason::Stale, 1, Some(sequence));
            }
        }

        let fps = normalized_transport_fps(self.last_frame.as_ref().map(|transport| transport.fps));
        FramePoll {
            pixel_buffer: self.last_pixel_buffer.clone(),
            fps,
            discontinuity,
        }
    }
}

#[cfg(test)]
pub(super) fn classify_discontinuity(
    previous: Option<&FrameSnapshot>,
    current: &FrameSnapshot,
    was_stale: bool,
) -> FrameDiscontinuity {
    classify_frame_integrity(
        previous.map(|frame| frame.observation()).as_ref(),
        &current.observation(),
        was_stale,
    )
}

pub(super) fn frame_is_stale(frame: &FrameSnapshot) -> bool {
    let age = monotonic_time_nanos().saturating_sub(frame.monotonic_timestamp_nanos);
    age > stale_timeout_nanos(frame.fps)
}

pub(super) fn stale_timeout_nanos(fps: u32) -> u64 {
    const MINIMUM_STALE_NANOS: u64 = 500_000_000;
    let frame_interval = 1_000_000_000 / u64::from(fps.max(1));
    MINIMUM_STALE_NANOS.max(frame_interval.saturating_mul(5))
}

pub(super) fn cmio_discontinuity(
    frame: FrameDiscontinuity,
    skipped_deadlines: u64,
    clock_discontinuity: bool,
) -> CMIOExtensionStreamDiscontinuityFlags {
    let mut flags = match frame {
        FrameDiscontinuity::None => CMIOExtensionStreamDiscontinuityFlags::None,
        FrameDiscontinuity::FramesDropped => CMIOExtensionStreamDiscontinuityFlags::SampleDropped,
        FrameDiscontinuity::FormatChanged => CMIOExtensionStreamDiscontinuityFlags::Unknown,
        FrameDiscontinuity::WriterRestarted
        | FrameDiscontinuity::TimingReset
        | FrameDiscontinuity::BecameStale
        | FrameDiscontinuity::Resumed => {
            CMIOExtensionStreamDiscontinuityFlags::Time
                | CMIOExtensionStreamDiscontinuityFlags::SampleDropped
        }
    };
    if skipped_deadlines > 0 {
        flags |= CMIOExtensionStreamDiscontinuityFlags::SampleDropped;
    }
    if clock_discontinuity {
        flags |= CMIOExtensionStreamDiscontinuityFlags::Time
            | CMIOExtensionStreamDiscontinuityFlags::SampleDropped;
    }
    flags
}
