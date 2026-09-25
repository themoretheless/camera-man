use super::*;

/// One run of the virtual-camera self test: whether a frame the app submits
/// comes back out through the transport. The states carry the timing of the
/// in-flight run and the progress snapshot that proved it. Only the controller
/// below interprets them; `ui_setup.rs` renders the section and issues run and
/// cancel.
pub(super) enum VirtualCameraSelfTest {
    Idle,
    Running {
        started: Instant,
        last_submit: Instant,
        target: Option<ProducerProgress>,
    },
    Passed {
        consumer: ConsumerProgress,
    },
    Failed(String),
}

impl CameraManApp {
    pub(super) fn start_virtual_camera_self_test(&mut self, ctx: &egui::Context) {
        if self.running {
            self.set_event(
                "Stop preview before running the virtual-camera self-test",
                true,
            );
            return;
        }
        let now = Instant::now();
        self.virtual_camera_self_test = VirtualCameraSelfTest::Running {
            started: now,
            last_submit: now.checked_sub(Duration::from_secs(1)).unwrap_or(now),
            target: None,
        };
        self.sticky_error = None;
        self.drive_virtual_camera_self_test(ctx);
    }

    pub(super) fn cancel_virtual_camera_self_test(&mut self, ctx: &egui::Context) {
        if !matches!(
            self.virtual_camera_self_test,
            VirtualCameraSelfTest::Running { .. }
        ) {
            return;
        }
        self.virtual_camera_self_test = VirtualCameraSelfTest::Idle;
        self.invalidate_render_epoch();
        self.disconnect_virtual_output();
        self.request_render(ctx, false, true);
        self.set_event(tr(self.locale, UiText::SelfTestCancelled), false);
    }

    pub(super) fn drive_virtual_camera_self_test(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let (timed_out, submit_due) = match &self.virtual_camera_self_test {
            VirtualCameraSelfTest::Running {
                started,
                last_submit,
                ..
            } => (
                now.duration_since(*started) >= Duration::from_secs(5),
                now.duration_since(*last_submit) >= Duration::from_millis(120),
            ),
            _ => return,
        };
        if timed_out {
            self.virtual_camera_self_test = VirtualCameraSelfTest::Failed(String::from(
                "No extension acknowledgement arrived within 5 seconds",
            ));
            self.disconnect_virtual_output();
            self.set_event("Virtual camera self-test timed out", true);
            return;
        }
        if submit_due {
            if let VirtualCameraSelfTest::Running { last_submit, .. } =
                &mut self.virtual_camera_self_test
            {
                *last_submit = now;
            }
            self.submit_virtual_camera_test_pattern();
        }
        ctx.request_repaint_after(Duration::from_millis(40));
    }

    fn submit_virtual_camera_test_pattern(&mut self) {
        const WIDTH: u32 = 160;
        const HEIGHT: u32 = 90;
        const BARS: [[u8; 4]; 8] = [
            [255, 255, 255, 255],
            [0, 255, 255, 255],
            [255, 255, 0, 255],
            [0, 255, 0, 255],
            [255, 0, 255, 255],
            [0, 0, 255, 255],
            [255, 0, 0, 255],
            [0, 0, 0, 255],
        ];
        let mut data = Vec::with_capacity(WIDTH as usize * HEIGHT as usize * 4);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let mut pixel =
                    BARS[(x as usize * BARS.len() / WIDTH as usize).min(BARS.len() - 1)];
                if y >= HEIGHT - 12 && (x / 6 + y / 6).is_multiple_of(2) {
                    pixel = [24, 24, 24, 255];
                }
                data.extend_from_slice(&pixel);
            }
        }
        let frame = Frame::new_checked(WIDTH, HEIGHT, PixelFormat::Bgra8, data)
            .expect("fixed virtual-camera test pattern is valid");
        let captured = CapturedFrame::new(frame, FrameMetadata::new("virtual-camera-self-test", 1));
        let format = VideoFormat {
            width: VIRTUAL_CAMERA_WIDTH,
            height: VIRTUAL_CAMERA_HEIGHT,
            fps: VIRTUAL_CAMERA_DEFAULT_FPS,
            pixel_format: PixelFormat::Bgra8,
        };
        self.render_worker.submit(
            RenderJob::new(
                vec![Some(captured)],
                CompositionLayout::Grid,
                format,
                self.render_epoch,
                false,
                true,
                true,
            )
            .with_preview_size(Some((PREVIEW_MAX_WIDTH, PREVIEW_MAX_HEIGHT))),
        );
    }

    pub(super) fn update_virtual_camera_self_test(
        &mut self,
        producer: Option<ProducerProgress>,
        consumer: Option<ConsumerProgress>,
    ) {
        let VirtualCameraSelfTest::Running { target, .. } = &mut self.virtual_camera_self_test
        else {
            return;
        };
        let acknowledged = consumer.filter(|consumer| {
            target
                .as_ref()
                .copied()
                .into_iter()
                .chain(producer)
                .any(|published| {
                    consumer.generation == published.generation
                        && consumer.sequence >= published.sequence
                })
        });
        if let Some(consumer) = acknowledged {
            self.virtual_camera_self_test = VirtualCameraSelfTest::Passed { consumer };
            self.disconnect_virtual_output();
            self.set_event("Virtual camera self-test passed", false);
        } else if let Some(producer) = producer {
            *target = Some(producer);
        }
    }
}
