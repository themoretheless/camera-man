use super::*;

pub(super) fn advertised_frame_duration_range() -> (CMTime, CMTime) {
    // With validFrameDurations=nil, CMIO accepts every duration between
    // these bounds, covering the app's 15/24/30/60 fps presets.
    // SAFETY: both shared fps bounds are compile-time positive values that fit
    // the positive i32 timescale required by CMTime.
    let (max_frame_duration, min_frame_duration) = unsafe {
        (
            CMTime::new(1, VIRTUAL_CAMERA_MIN_FPS as i32),
            CMTime::new(1, VIRTUAL_CAMERA_MAX_FPS as i32),
        )
    };
    (max_frame_duration, min_frame_duration)
}

pub(super) fn normalized_transport_fps(fps: Option<u32>) -> u32 {
    // Before the first frame, the shared default keeps the placeholder
    // moving; malformed/external producers are clamped to the CMIO range.
    fps.unwrap_or(VIRTUAL_CAMERA_DEFAULT_FPS)
        .clamp(VIRTUAL_CAMERA_MIN_FPS, VIRTUAL_CAMERA_MAX_FPS)
}

pub(super) fn host_time_nanoseconds() -> u64 {
    let ticks = CVGetCurrentHostTime();
    let frequency = CVGetHostClockFrequency();
    if frequency <= 0.0 {
        return 0;
    }
    ((ticks as f64 / frequency) * 1_000_000_000.0) as u64
}
