use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use camera_man::{
    CameraManError, CapturedFrame, CompositionLayout, Compositor, ConsumerProgress, CopyStage,
    DropReason, Frame, FrameTransportSink, PipelineStage, ProducerProgress, ScalingFilter,
    SourceTransform, VideoFormat, VirtualCameraSink, copy_ledger, record_drop, stage_span,
};
use eframe::egui;

pub(crate) trait RenderSink: VirtualCameraSink + Send {
    fn set_target_fps(&mut self, fps: u32);

    fn producer_progress(&self) -> Option<ProducerProgress> {
        None
    }

    fn consumer_progress(&self) -> Option<ConsumerProgress> {
        None
    }
}

impl RenderSink for FrameTransportSink {
    fn set_target_fps(&mut self, fps: u32) {
        FrameTransportSink::set_target_fps(self, fps);
    }

    fn producer_progress(&self) -> Option<ProducerProgress> {
        FrameTransportSink::producer_progress(self)
    }

    fn consumer_progress(&self) -> Option<ConsumerProgress> {
        FrameTransportSink::consumer_progress(self)
    }
}

#[cfg(test)]
impl RenderSink for camera_man::MemorySink {
    fn set_target_fps(&mut self, _fps: u32) {}
}

/// One complete composition request. `RenderWorker::submit` keeps only the
/// newest pending request, so a slow compositor cannot build an old-frame
/// backlog behind the UI.
pub(crate) struct RenderJob {
    frames: Vec<Option<CapturedFrame>>,
    layout: CompositionLayout,
    format: VideoFormat,
    epoch: u64,
    live: bool,
    publish_virtual: bool,
    force_preview: bool,
    scaling_filter: ScalingFilter,
    preview_size: Option<(u32, u32)>,
    source_transforms: Vec<SourceTransform>,
}

impl RenderJob {
    pub(crate) fn new(
        frames: Vec<Option<CapturedFrame>>,
        layout: CompositionLayout,
        format: VideoFormat,
        epoch: u64,
        live: bool,
        publish_virtual: bool,
        force_preview: bool,
    ) -> Self {
        Self {
            frames,
            layout,
            format,
            epoch,
            live,
            publish_virtual,
            force_preview,
            scaling_filter: ScalingFilter::Nearest,
            preview_size: None,
            source_transforms: Vec::new(),
        }
    }

    pub(crate) const fn with_scaling_filter(mut self, scaling_filter: ScalingFilter) -> Self {
        self.scaling_filter = scaling_filter;
        self
    }

    pub(crate) const fn with_preview_size(mut self, preview_size: Option<(u32, u32)>) -> Self {
        self.preview_size = preview_size;
        self
    }

    pub(crate) fn with_source_transforms(
        mut self,
        source_transforms: Vec<SourceTransform>,
    ) -> Self {
        self.source_transforms = source_transforms;
        self
    }
}

/// Worker-owned derived snapshot transferred back to the UI thread.
pub(crate) struct RenderResult {
    pub(crate) epoch: u64,
    pub(crate) frame: Result<Frame, CameraManError>,
    pub(crate) force_preview: bool,
    pub(crate) rendered_jobs: u64,
    pub(crate) live_rendered_jobs: u64,
    pub(crate) virtual_frames_sent: u64,
    pub(crate) output_allocations: u64,
    pub(crate) output_reuses: u64,
    pub(crate) preview_image: Option<Arc<egui::ColorImage>>,
    pub(crate) transport_error: Option<CameraManError>,
    pub(crate) producer_progress: Option<ProducerProgress>,
    pub(crate) consumer_progress: Option<ConsumerProgress>,
}

#[derive(Default)]
struct WorkerState {
    pending: Option<RenderJob>,
    result: Option<RenderResult>,
    output_enabled: bool,
    latest_epoch: u64,
    disconnect_requested: bool,
    processing: bool,
    stopping: bool,
    recycled_frames: Vec<Frame>,
}

struct SharedState {
    state: Mutex<WorkerState>,
    wake: Condvar,
}

enum WorkerAction {
    Render(RenderJob),
    Disconnect,
    Stop,
}

pub(crate) struct RenderWorker {
    shared: Arc<SharedState>,
    worker: Option<JoinHandle<()>>,
}

impl RenderWorker {
    pub(crate) fn new(sink: impl RenderSink + 'static) -> Self {
        debug_assert_eq!(camera_man::backpressure::RENDER_REQUESTS.capacity, 1);
        let shared = Arc::new(SharedState {
            state: Mutex::new(WorkerState::default()),
            wake: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name(String::from("camera-man-render"))
            .spawn(move || run_worker(worker_shared, Box::new(sink)))
            .expect("failed to spawn render worker");

        Self {
            shared,
            worker: Some(worker),
        }
    }

    pub(crate) fn submit(&self, job: RenderJob) {
        let epoch = job.epoch;
        let mut state = self.shared.state.lock().expect("render mutex poisoned");
        if job.publish_virtual {
            state.disconnect_requested = false;
        } else if state.output_enabled {
            state.disconnect_requested = true;
        }
        state.output_enabled = job.publish_virtual;
        state.latest_epoch = job.epoch;
        if state.pending.replace(job).is_some() {
            record_drop(DropReason::QueueReplacement, 1, Some(epoch));
        }
        self.shared.wake.notify_one();
    }

    pub(crate) fn disconnect_output(&self) {
        let mut state = self.shared.state.lock().expect("render mutex poisoned");
        state.output_enabled = false;
        state.disconnect_requested = true;
        self.shared.wake.notify_one();
    }

    pub(crate) fn take_result(&self) -> Option<RenderResult> {
        self.shared
            .state
            .lock()
            .expect("render mutex poisoned")
            .result
            .take()
    }

    pub(crate) fn recycle_frame(&self, frame: Frame) {
        let mut state = self.shared.state.lock().expect("render mutex poisoned");
        if state.recycled_frames.len() < 3 {
            state.recycled_frames.push(frame);
        }
    }

    /// Includes an unread result to close the race where the worker finishes
    /// between the UI's result poll and its repaint scheduling decision.
    pub(crate) fn has_work(&self) -> bool {
        let state = self.shared.state.lock().expect("render mutex poisoned");
        state.processing
            || state.pending.is_some()
            || state.disconnect_requested
            || state.result.is_some()
    }
}

impl Drop for RenderWorker {
    fn drop(&mut self) {
        {
            let mut state = self.shared.state.lock().expect("render mutex poisoned");
            state.stopping = true;
            state.pending = None;
            self.shared.wake.notify_one();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_worker(shared: Arc<SharedState>, mut sink: Box<dyn RenderSink>) {
    let mut compositor = None;
    let mut connected = false;
    let mut preview_pool = PreviewBufferPool::default();

    loop {
        let action = next_action(&shared);
        match action {
            WorkerAction::Stop => {
                if connected {
                    sink.disconnect();
                }
                return;
            }
            WorkerAction::Disconnect => {
                if connected {
                    sink.disconnect();
                    connected = false;
                }
            }
            WorkerAction::Render(job) => {
                let missing_sources = job.frames.iter().filter(|frame| frame.is_none()).count();
                record_drop(
                    DropReason::SourceMissing,
                    missing_sources as u64,
                    Some(job.epoch),
                );
                if compositor
                    .as_ref()
                    .map(Compositor::format)
                    .is_none_or(|format| format != job.format)
                {
                    compositor = Some(Compositor::new(job.format));
                }
                compositor
                    .as_mut()
                    .expect("compositor initialized above")
                    .set_scaling_filter(job.scaling_filter);

                let compositor = compositor.as_ref().expect("compositor initialized above");
                let recycled = take_recycled_frame(&shared, job.format);
                let reused_output = recycled.is_some();
                let frame = {
                    let _compose_span = stage_span(
                        PipelineStage::Compose,
                        job.epoch,
                        job.format.width,
                        job.format.height,
                    );
                    match recycled {
                        Some(mut output) => compositor
                            .compose_captured_into_with_transforms(
                                &job.frames,
                                &job.source_transforms,
                                job.layout,
                                &mut output,
                            )
                            .map(|()| output),
                        None => compositor.compose_captured_with_transforms(
                            &job.frames,
                            &job.source_transforms,
                            job.layout,
                        ),
                    }
                };
                let rendered = frame.is_ok();
                let mut transport_error = None;
                let mut virtual_frame_sent = false;
                let (output_enabled, latest_epoch) = {
                    let state = shared.state.lock().expect("render mutex poisoned");
                    (state.output_enabled, state.latest_epoch)
                };
                let publish_current = output_enabled && job.epoch == latest_epoch;

                if publish_current {
                    sink.set_target_fps(job.format.fps);
                    if !connected {
                        match sink.connect() {
                            Ok(()) => connected = true,
                            Err(error) => transport_error = Some(error),
                        }
                    }
                    if connected && let Ok(frame) = &frame {
                        let still_current = {
                            let state = shared.state.lock().expect("render mutex poisoned");
                            state.output_enabled && state.latest_epoch == job.epoch
                        };
                        if still_current {
                            match sink.send(frame) {
                                Ok(()) => virtual_frame_sent = true,
                                Err(error) => {
                                    sink.disconnect();
                                    connected = false;
                                    transport_error = Some(error);
                                }
                            }
                        }
                    }
                } else if !output_enabled && connected {
                    sink.disconnect();
                    connected = false;
                }

                let preview_image = job.preview_size.and_then(|(max_width, max_height)| {
                    frame
                        .as_ref()
                        .ok()
                        .map(|frame| preview_pool.render(frame, max_width, max_height))
                });
                let producer_progress = sink.producer_progress();
                let consumer_progress = sink.consumer_progress();

                let mut result = RenderResult {
                    epoch: job.epoch,
                    frame,
                    force_preview: job.force_preview,
                    rendered_jobs: u64::from(rendered),
                    live_rendered_jobs: u64::from(rendered && job.live),
                    virtual_frames_sent: u64::from(virtual_frame_sent),
                    output_allocations: u64::from(rendered && !reused_output),
                    output_reuses: u64::from(rendered && reused_output),
                    preview_image,
                    transport_error,
                    producer_progress,
                    consumer_progress,
                };
                let mut state = shared.state.lock().expect("render mutex poisoned");
                state.processing = false;
                if let Some(previous) = state.result.take() {
                    let RenderResult {
                        epoch,
                        frame,
                        force_preview,
                        rendered_jobs,
                        live_rendered_jobs,
                        virtual_frames_sent,
                        output_allocations,
                        output_reuses,
                        preview_image,
                        transport_error,
                        producer_progress,
                        consumer_progress,
                    } = previous;
                    result.rendered_jobs += rendered_jobs;
                    result.live_rendered_jobs += live_rendered_jobs;
                    result.virtual_frames_sent += virtual_frames_sent;
                    result.output_allocations += output_allocations;
                    result.output_reuses += output_reuses;
                    if epoch == result.epoch {
                        if result.transport_error.is_none() {
                            result.transport_error = transport_error;
                        }
                        result.force_preview |= force_preview;
                        if result.preview_image.is_none() {
                            result.preview_image = preview_image;
                        }
                        if result.producer_progress.is_none() {
                            result.producer_progress = producer_progress;
                        }
                        if result.consumer_progress.is_none() {
                            result.consumer_progress = consumer_progress;
                        }
                    }
                    if let Ok(frame) = frame
                        && state.recycled_frames.len() < 3
                    {
                        state.recycled_frames.push(frame);
                    }
                }
                state.result = Some(result);
            }
        }
    }
}

#[derive(Default)]
struct PreviewBufferPool {
    buffers: Vec<Arc<egui::ColorImage>>,
    x_offsets: PreviewCoordinateMap,
}

#[derive(Default)]
struct PreviewCoordinateMap {
    key: Option<(u32, u32)>,
    offsets: Vec<usize>,
}

impl PreviewCoordinateMap {
    fn offsets(&mut self, source_width: u32, destination_width: u32) -> &[usize] {
        let key = (source_width, destination_width);
        if self.key != Some(key) {
            self.offsets.clear();
            self.offsets.extend((0..destination_width).map(|x| {
                (u64::from(x) * u64::from(source_width) / u64::from(destination_width)) as usize * 4
            }));
            self.key = Some(key);
        }
        &self.offsets
    }
}

impl PreviewBufferPool {
    fn render(&mut self, frame: &Frame, max_width: u32, max_height: u32) -> Arc<egui::ColorImage> {
        let (width, height) = preview_dimensions(frame, max_width, max_height);
        let size = [width as usize, height as usize];
        let pixel_count = size[0].saturating_mul(size[1]);
        let reusable = self
            .buffers
            .iter()
            .position(|buffer| Arc::strong_count(buffer) == 1);

        let index = if let Some(index) = reusable {
            let image =
                Arc::get_mut(&mut self.buffers[index]).expect("preview buffer is uniquely owned");
            if image.size != size {
                *image = egui::ColorImage::new(size, vec![egui::Color32::TRANSPARENT; pixel_count]);
            }
            index
        } else if self.buffers.len() < 3 {
            self.buffers.push(Arc::new(egui::ColorImage::new(
                size,
                vec![egui::Color32::TRANSPARENT; pixel_count],
            )));
            self.buffers.len() - 1
        } else {
            let mut image =
                egui::ColorImage::new(size, vec![egui::Color32::TRANSPARENT; pixel_count]);
            fill_preview_pixels(frame, &mut image, &mut self.x_offsets);
            copy_ledger().record_allocation(CopyStage::PreviewConversion, pixel_count * 4);
            copy_ledger().record_copy(CopyStage::PreviewConversion, pixel_count * 4);
            return Arc::new(image);
        };

        let image = Arc::get_mut(&mut self.buffers[index])
            .expect("selected preview buffer is uniquely owned");
        fill_preview_pixels(frame, image, &mut self.x_offsets);
        if reusable.is_none() {
            copy_ledger().record_allocation(CopyStage::PreviewConversion, pixel_count * 4);
        }
        copy_ledger().record_copy(CopyStage::PreviewConversion, pixel_count * 4);
        Arc::clone(&self.buffers[index])
    }
}

fn preview_dimensions(frame: &Frame, max_width: u32, max_height: u32) -> (u32, u32) {
    let max_width = max_width.max(1);
    let max_height = max_height.max(1);
    if frame.width() <= max_width && frame.height() <= max_height {
        return (frame.width(), frame.height());
    }
    if u64::from(max_width) * u64::from(frame.height())
        <= u64::from(max_height) * u64::from(frame.width())
    {
        (
            max_width,
            (u64::from(frame.height()) * u64::from(max_width) / u64::from(frame.width())).max(1)
                as u32,
        )
    } else {
        (
            (u64::from(frame.width()) * u64::from(max_height) / u64::from(frame.height())).max(1)
                as u32,
            max_height,
        )
    }
}

fn fill_preview_pixels(
    frame: &Frame,
    image: &mut egui::ColorImage,
    x_offsets: &mut PreviewCoordinateMap,
) {
    let width = image.size[0] as u32;
    let height = image.size[1] as u32;
    let offsets = x_offsets.offsets(frame.width(), width);
    for y in 0..height {
        let source_y = (u64::from(y) * u64::from(frame.height()) / u64::from(height)) as usize;
        let source_row = source_y * frame.width() as usize * 4;
        for (x, source_x) in offsets.iter().enumerate() {
            let offset = source_row + source_x;
            let pixel = &frame.data()[offset..offset + 4];
            image.pixels[y as usize * width as usize + x] =
                egui::Color32::from_rgba_unmultiplied(pixel[2], pixel[1], pixel[0], pixel[3]);
        }
    }
}

fn take_recycled_frame(shared: &SharedState, format: VideoFormat) -> Option<Frame> {
    let mut state = shared.state.lock().expect("render mutex poisoned");
    let index = state.recycled_frames.iter().rposition(|frame| {
        frame.width() == format.width
            && frame.height() == format.height
            && frame.pixel_format() == format.pixel_format
    })?;
    Some(state.recycled_frames.swap_remove(index))
}

fn next_action(shared: &SharedState) -> WorkerAction {
    let mut state = shared.state.lock().expect("render mutex poisoned");
    while !state.stopping && state.pending.is_none() && !state.disconnect_requested {
        state = shared.wake.wait(state).expect("render mutex poisoned");
    }
    if state.stopping {
        WorkerAction::Stop
    } else if state.disconnect_requested {
        state.disconnect_requested = false;
        WorkerAction::Disconnect
    } else {
        state.processing = true;
        WorkerAction::Render(state.pending.take().expect("pending job checked above"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::{Duration, Instant};

    use camera_man::{FrameMetadata, MemorySink, PixelFormat};

    use super::*;

    #[derive(Clone, Default)]
    struct ConnectGate {
        entered: Arc<(Mutex<bool>, Condvar)>,
        released: Arc<(Mutex<bool>, Condvar)>,
    }

    impl ConnectGate {
        fn wait_until_entered(&self, timeout: Duration) -> bool {
            let (lock, wake) = &*self.entered;
            let entered = lock.lock().unwrap();
            let (entered, _) = wake
                .wait_timeout_while(entered, timeout, |entered| !*entered)
                .unwrap();
            *entered
        }

        fn release(&self) {
            let (lock, wake) = &*self.released;
            *lock.lock().unwrap() = true;
            wake.notify_all();
        }
    }

    struct BlockingConnectSink {
        gate: ConnectGate,
        connected: bool,
        frames_sent: Arc<AtomicUsize>,
    }

    impl VirtualCameraSink for BlockingConnectSink {
        fn connect(&mut self) -> Result<(), CameraManError> {
            let (entered, wake) = &*self.gate.entered;
            *entered.lock().unwrap() = true;
            wake.notify_all();

            let (released, wake) = &*self.gate.released;
            let mut released = released.lock().unwrap();
            while !*released {
                released = wake.wait(released).unwrap();
            }
            self.connected = true;
            Ok(())
        }

        fn send(&mut self, _frame: &Frame) -> Result<(), CameraManError> {
            assert!(self.connected);
            self.frames_sent.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn disconnect(&mut self) {
            self.connected = false;
        }
    }

    impl RenderSink for BlockingConnectSink {
        fn set_target_fps(&mut self, _fps: u32) {}
    }

    fn test_job(epoch: u64, publish_virtual: bool) -> RenderJob {
        let frame = Frame::solid_bgra(2, 2, [epoch as u8, 2, 3, 255]).unwrap();
        let captured = CapturedFrame::new(frame, FrameMetadata::new("test", epoch));
        RenderJob::new(
            vec![Some(captured)],
            CompositionLayout::Grid,
            VideoFormat {
                width: 4,
                height: 2,
                fps: 30,
                pixel_format: PixelFormat::Bgra8,
            },
            epoch,
            true,
            publish_virtual,
            false,
        )
    }

    #[test]
    fn composes_a_job_off_the_calling_thread() {
        let worker = RenderWorker::new(MemorySink::default());
        let frame = Frame::solid_bgra(2, 2, [1, 2, 3, 255]).unwrap();
        let captured = CapturedFrame::new(frame, FrameMetadata::new("test", 1));
        worker.submit(RenderJob::new(
            vec![Some(captured)],
            CompositionLayout::Grid,
            VideoFormat {
                width: 4,
                height: 2,
                fps: 30,
                pixel_format: PixelFormat::Bgra8,
            },
            7,
            false,
            false,
            true,
        ));

        let deadline = Instant::now() + Duration::from_secs(1);
        let result = loop {
            if let Some(result) = worker.take_result() {
                break result;
            }
            assert!(Instant::now() < deadline, "render worker timed out");
            thread::sleep(Duration::from_millis(1));
        };

        assert_eq!(result.epoch, 7);
        assert_eq!(result.rendered_jobs, 1);
        assert_eq!(result.output_allocations, 1);
        assert_eq!(result.output_reuses, 0);
        assert_eq!(result.frame.unwrap().bgra_at(1, 0), Some([1, 2, 3, 255]));
    }

    #[test]
    fn reuses_a_frame_returned_by_the_ui() {
        let worker = RenderWorker::new(MemorySink::default());
        worker.submit(test_job(1, false));
        let deadline = Instant::now() + Duration::from_secs(1);
        let first = loop {
            if let Some(result) = worker.take_result() {
                break result;
            }
            assert!(Instant::now() < deadline, "render worker timed out");
            thread::sleep(Duration::from_millis(1));
        };
        worker.recycle_frame(first.frame.unwrap());

        worker.submit(test_job(2, false));
        let deadline = Instant::now() + Duration::from_secs(1);
        let second = loop {
            if let Some(result) = worker.take_result() {
                break result;
            }
            assert!(Instant::now() < deadline, "render worker timed out");
            thread::sleep(Duration::from_millis(1));
        };

        assert_eq!(second.output_allocations, 0);
        assert_eq!(second.output_reuses, 1);
    }

    #[test]
    fn preview_pool_downscales_off_thread_and_reuses_unique_storage() {
        let frame = Frame::solid_bgra(1920, 1080, [10, 20, 30, 255]).unwrap();
        let mut pool = PreviewBufferPool::default();

        let first = pool.render(&frame, 960, 540);
        let storage = Arc::as_ptr(&first);
        assert_eq!(first.size, [960, 540]);
        assert_eq!(first.pixels[0], egui::Color32::from_rgb(30, 20, 10));
        drop(first);

        let second = pool.render(&frame, 960, 540);
        assert_eq!(storage, Arc::as_ptr(&second));
    }

    #[test]
    fn preview_pool_never_upscales_small_frames() {
        let frame = Frame::solid_bgra(320, 240, [1, 2, 3, 255]).unwrap();
        let image = PreviewBufferPool::default().render(&frame, 960, 540);
        assert_eq!(image.size, [320, 240]);
    }

    #[test]
    fn preview_coordinate_map_keeps_only_the_current_width() {
        let mut map = PreviewCoordinateMap::default();

        assert_eq!(map.offsets(1_920, 960).len(), 960);
        assert_eq!(map.key, Some((1_920, 960)));
        assert_eq!(map.offsets(1_920, 640).len(), 640);
        assert_eq!(map.key, Some((1_920, 640)));
        assert_eq!(map.offsets.capacity(), 960);
    }

    #[test]
    fn keeps_only_the_newest_pending_job() {
        let gate = ConnectGate::default();
        let frames_sent = Arc::new(AtomicUsize::new(0));
        let worker = RenderWorker::new(BlockingConnectSink {
            gate: gate.clone(),
            connected: false,
            frames_sent: Arc::clone(&frames_sent),
        });
        worker.submit(test_job(1, true));
        if !gate.wait_until_entered(Duration::from_secs(1)) {
            gate.release();
            panic!("render worker never entered the blocking sink");
        }

        worker.submit(test_job(2, true));
        worker.submit(test_job(3, true));
        gate.release();

        let deadline = Instant::now() + Duration::from_secs(1);
        let mut epochs = Vec::new();
        let mut rendered_jobs = 0;
        loop {
            if let Some(result) = worker.take_result() {
                epochs.push(result.epoch);
                rendered_jobs += result.rendered_jobs;
            }
            if !worker.has_work() {
                break;
            }
            assert!(Instant::now() < deadline, "render worker timed out");
            thread::sleep(Duration::from_millis(1));
        }

        assert_eq!(rendered_jobs, 2);
        assert_eq!(frames_sent.load(Ordering::Relaxed), 1);
        assert!(epochs.contains(&3));
        assert!(!epochs.contains(&2));
    }
}
