use std::path::PathBuf;
use std::time::{Duration, Instant};

use camera_man::FrameSource;
use camera_man::{
    CameraDevice, CameraDiscovery, CapturedFrame, CompositionLayout, Compositor, Frame,
    FrameSpoolSink, NokhwaCameraDiscovery, PixelFormat, SyntheticFrameSource,
    ThreadedNokhwaFrameSource, VideoFormat, VirtualCameraSink, write_ppm,
};
use eframe::egui;

const OUTPUT_WIDTH: u32 = 1920;
const OUTPUT_HEIGHT: u32 = 1080;
const SOURCE_WIDTH: u32 = 320;
const SOURCE_HEIGHT: u32 = 240;

/// 1/30 s in nanoseconds. Integer milliseconds (1000/30 = 33 ms) drift to ~30.3
/// fps and, combined with 16 ms repaints, the old code averaged ~21 fps.
const FRAME_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 30);
/// How long a non-error event (export confirmation etc.) stays in the status bar.
const EVENT_TTL: Duration = Duration::from_secs(4);
/// Errors stay longer so a 30 fps render loop cannot wipe them instantly.
const ERROR_TTL: Duration = Duration::from_secs(8);
/// Consecutive capture failures (~3 s at 30 fps) before the preview auto-stops
/// instead of hammering a broken camera and spamming the status bar.
const MAX_CAPTURE_ERROR_STREAK: u32 = 90;

// Palette: one place for every hardcoded color in the app.
const COLOR_WINDOW: egui::Color32 = egui::Color32::from_rgb(24, 27, 31);
const COLOR_PANEL: egui::Color32 = egui::Color32::from_rgb(30, 34, 38);
const COLOR_WIDGET: egui::Color32 = egui::Color32::from_rgb(48, 54, 60);
const COLOR_WIDGET_HOVER: egui::Color32 = egui::Color32::from_rgb(62, 72, 82);
const COLOR_WIDGET_ACTIVE: egui::Color32 = egui::Color32::from_rgb(67, 115, 132);
const COLOR_ACCENT: egui::Color32 = egui::Color32::from_rgb(64, 132, 158);
const COLOR_STOP: egui::Color32 = egui::Color32::from_rgb(160, 68, 68);
const COLOR_OK: egui::Color32 = egui::Color32::from_rgb(112, 176, 128);
const COLOR_ERROR: egui::Color32 = egui::Color32::from_rgb(224, 108, 108);
const COLOR_WARNING: egui::Color32 = egui::Color32::from_rgb(214, 172, 96);
const COLOR_DIM: egui::Color32 = egui::Color32::from_gray(150);

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CameraMan")
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([920.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "CameraMan",
        options,
        Box::new(|cc| Ok(Box::new(CameraManApp::new(cc)))),
    )
}

#[derive(Debug, Clone)]
struct SourceSlot {
    id: &'static str,
    name: &'static str,
    bgra: [u8; 4],
    selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Synthetic,
    Real,
}

/// A short-lived message for the status bar: export confirmations, errors.
/// Kept separate from the derived state line so a 30 fps status update cannot
/// overwrite feedback the user still needs to read.
struct StatusEvent {
    text: String,
    at: Instant,
    is_error: bool,
}

impl StatusEvent {
    fn is_visible(&self) -> bool {
        let ttl = if self.is_error { ERROR_TTL } else { EVENT_TTL };
        self.at.elapsed() < ttl
    }
}

pub struct CameraManApp {
    sources: Vec<SourceSlot>,
    input_mode: InputMode,
    real_devices: Vec<CameraDevice>,
    selected_real_id: Option<String>,
    real_source: Option<ThreadedNokhwaFrameSource>,
    layout: CompositionLayout,
    compositor: Compositor,
    running: bool,
    tick: u64,
    frames_rendered: u64,
    last_frame_at: Instant,
    event: Option<StatusEvent>,
    capture_error_streak: u32,
    waiting_for_camera: bool,
    fps_window_start: Instant,
    fps_window_frames: u32,
    measured_fps: f32,
    preview: Option<Frame>,
    preview_texture: Option<egui::TextureHandle>,
    virtual_output_enabled: bool,
    virtual_sink: FrameSpoolSink,
    virtual_connected: bool,
    virtual_frames_sent: u64,
    export_path: PathBuf,
}

impl CameraManApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);

        let format = VideoFormat {
            width: OUTPUT_WIDTH,
            height: OUTPUT_HEIGHT,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        };
        let mut app = Self {
            sources: vec![
                SourceSlot {
                    id: "desk",
                    name: "Desk Camera",
                    bgra: [235, 94, 40, 255],
                    selected: true,
                },
                SourceSlot {
                    id: "side",
                    name: "Side Camera",
                    bgra: [62, 181, 137, 255],
                    selected: true,
                },
                SourceSlot {
                    id: "wide",
                    name: "Wide Camera",
                    bgra: [72, 126, 220, 255],
                    selected: true,
                },
                SourceSlot {
                    id: "overhead",
                    name: "Overhead",
                    bgra: [196, 166, 76, 255],
                    selected: false,
                },
            ],
            input_mode: InputMode::Synthetic,
            real_devices: Vec::new(),
            selected_real_id: None,
            real_source: None,
            layout: CompositionLayout::Grid,
            compositor: Compositor::new(format),
            running: false,
            tick: 0,
            frames_rendered: 0,
            last_frame_at: Instant::now(),
            event: None,
            capture_error_streak: 0,
            waiting_for_camera: false,
            fps_window_start: Instant::now(),
            fps_window_frames: 0,
            measured_fps: 0.0,
            preview: None,
            preview_texture: None,
            virtual_output_enabled: true,
            virtual_sink: FrameSpoolSink::default(),
            virtual_connected: false,
            virtual_frames_sent: 0,
            export_path: PathBuf::from("target/camera-man-app-preview.ppm"),
        };
        app.refresh_real_devices(false);
        app.render_preview(&cc.egui_ctx, false);
        app
    }

    fn selected_count(&self) -> usize {
        match self.input_mode {
            InputMode::Synthetic => self.sources.iter().filter(|source| source.selected).count(),
            InputMode::Real => usize::from(self.selected_real_id.is_some()),
        }
    }

    fn set_event(&mut self, text: impl Into<String>, is_error: bool) {
        self.event = Some(StatusEvent {
            text: text.into(),
            at: Instant::now(),
            is_error,
        });
    }

    fn toggle_running(&mut self, ctx: &egui::Context) {
        if self.running {
            self.stop_streaming();
        } else {
            if self.selected_count() == 0 {
                self.set_event("Select at least one source first", true);
                return;
            }
            self.running = true;
            self.capture_error_streak = 0;
            self.last_frame_at = Instant::now();
            self.fps_window_start = Instant::now();
            self.fps_window_frames = 0;
            self.render_preview(ctx, true);
        }
    }

    /// Stops the render loop AND releases the camera. Keeping the camera open
    /// after Stop leaves the LED on and looks like spying.
    fn stop_streaming(&mut self) {
        self.running = false;
        self.real_source = None;
        self.waiting_for_camera = false;
        self.measured_fps = 0.0;
        self.disconnect_virtual_output();
    }

    fn render_preview(&mut self, ctx: &egui::Context, allow_open_camera: bool) {
        self.waiting_for_camera = false;
        let frames = match self.input_mode {
            InputMode::Synthetic => self
                .sources
                .iter()
                .filter(|source| source.selected)
                .map(|source| self.synthetic_frame(source))
                .collect::<Result<Vec<_>, _>>(),
            InputMode::Real => self.real_frame(allow_open_camera).map(|frame| vec![frame]),
        };

        let frames = match frames {
            Ok(frames) if !frames.is_empty() => frames,
            Ok(_) => {
                self.preview = None;
                self.preview_texture = None;
                return;
            }
            Err(error) => {
                self.on_capture_error(error.to_string());
                return;
            }
        };

        if self.input_mode == InputMode::Real && frames.iter().all(Option::is_none) {
            // Camera thread is still warming up (or closed): keep the previous
            // texture on screen and report the waiting state.
            self.waiting_for_camera = true;
            return;
        }

        match self.compositor.compose_captured(&frames, self.layout) {
            Ok(frame) => {
                self.tick += 1;
                self.frames_rendered += 1;
                self.capture_error_streak = 0;
                self.update_texture(ctx, &frame);
                self.send_virtual_frame(&frame);
                self.preview = Some(frame);
            }
            Err(error) => {
                self.set_event(error.to_string(), true);
            }
        }
    }

    fn on_capture_error(&mut self, message: String) {
        if self.capture_error_streak == 0 {
            self.set_event(message, true);
        }
        self.capture_error_streak += 1;
        if self.running && self.capture_error_streak >= MAX_CAPTURE_ERROR_STREAK {
            self.stop_streaming();
            self.set_event(
                "Camera keeps failing, preview stopped. Check the camera and press Start to retry.",
                true,
            );
        }
    }

    fn synthetic_frame(
        &self,
        source: &SourceSlot,
    ) -> Result<Option<CapturedFrame>, camera_man::CameraManError> {
        let bgra = animated_color(source.bgra, self.tick);
        let mut frame_source =
            SyntheticFrameSource::with_id(source.id, SOURCE_WIDTH, SOURCE_HEIGHT, bgra);
        frame_source.latest_frame()
    }

    /// Returns the newest real camera frame. The camera is opened only when
    /// `allow_open` is true (Start pressed / already streaming), never as a
    /// side effect of switching modes or clicking around the UI.
    fn real_frame(
        &mut self,
        allow_open: bool,
    ) -> Result<Option<CapturedFrame>, camera_man::CameraManError> {
        if self.real_source.is_none() {
            if !allow_open {
                return Ok(None);
            }
            let Some(id) = self.selected_real_id.clone() else {
                return Ok(None);
            };
            self.real_source = Some(ThreadedNokhwaFrameSource::open_id(&id));
        }
        match self.real_source.as_mut() {
            Some(source) => source.latest_frame(),
            None => Ok(None),
        }
    }

    fn refresh_real_devices(&mut self, announce: bool) {
        let discovery = NokhwaCameraDiscovery;
        match discovery.list_devices() {
            Ok(devices) => {
                self.real_devices = devices;
                if self
                    .selected_real_id
                    .as_ref()
                    .is_none_or(|id| !self.real_devices.iter().any(|device| &device.id == id))
                {
                    self.selected_real_id =
                        self.real_devices.first().map(|device| device.id.clone());
                }
                self.real_source = None;
                if announce {
                    if self.real_devices.is_empty() {
                        self.set_event("No cameras found", true);
                    } else {
                        self.set_event(
                            format!("Found {} camera(s)", self.real_devices.len()),
                            false,
                        );
                    }
                }
            }
            Err(error) => {
                self.real_devices.clear();
                self.selected_real_id = None;
                self.real_source = None;
                self.set_event(error.to_string(), true);
            }
        }
    }

    fn update_texture(&mut self, ctx: &egui::Context, frame: &Frame) {
        let image = frame_to_color_image(frame);
        match self.preview_texture.as_mut() {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                self.preview_texture = Some(ctx.load_texture(
                    "camera-man-preview",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
    }

    fn export_preview(&mut self) {
        match self.preview.as_ref() {
            Some(frame) => match write_ppm(&self.export_path, frame) {
                Ok(()) => {
                    self.set_event(format!("Exported {}", self.export_path.display()), false);
                }
                Err(error) => {
                    self.set_event(error.to_string(), true);
                }
            },
            None => {
                self.set_event("Nothing to export yet", true);
            }
        }
    }

    fn disconnect_virtual_output(&mut self) {
        if self.virtual_connected {
            self.virtual_sink.disconnect();
            self.virtual_connected = false;
        }
    }

    fn send_virtual_frame(&mut self, frame: &Frame) {
        if !self.running || !self.virtual_output_enabled {
            return;
        }

        if !self.virtual_connected {
            match self.virtual_sink.connect() {
                Ok(()) => {
                    self.virtual_connected = true;
                }
                Err(error) => {
                    self.set_event(error.to_string(), true);
                    return;
                }
            }
        }

        match self.virtual_sink.send(frame) {
            Ok(()) => {
                self.virtual_frames_sent += 1;
            }
            Err(error) => {
                self.virtual_connected = false;
                self.set_event(error.to_string(), true);
            }
        }
    }

    fn update_fps(&mut self) {
        self.fps_window_frames += 1;
        let elapsed = self.fps_window_start.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.measured_fps = self.fps_window_frames as f32 / elapsed.as_secs_f32();
            self.fps_window_start = Instant::now();
            self.fps_window_frames = 0;
        }
    }

    /// The always-derivable state line for the status bar (left side).
    fn state_line(&self) -> (egui::Color32, String) {
        if let Some(event) = self.event.as_ref().filter(|event| event.is_visible()) {
            let color = if event.is_error {
                COLOR_ERROR
            } else {
                COLOR_OK
            };
            return (color, event.text.clone());
        }
        if self.running {
            if self.waiting_for_camera {
                (COLOR_WARNING, String::from("Waiting for camera frame"))
            } else {
                (COLOR_OK, String::from("Preview running"))
            }
        } else if self.preview.is_some() {
            (COLOR_DIM, String::from("Paused"))
        } else {
            (COLOR_DIM, String::from("Ready"))
        }
    }
}

impl eframe::App for CameraManApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.running {
            let now = Instant::now();
            if now.duration_since(self.last_frame_at) >= FRAME_INTERVAL {
                // Advance by whole intervals to hold 30 fps cadence; resync when
                // the app fell far behind (window hidden, heavy load).
                self.last_frame_at += FRAME_INTERVAL;
                if now.duration_since(self.last_frame_at) > FRAME_INTERVAL * 3 {
                    self.last_frame_at = now;
                }
                self.render_preview(ctx, true);
                self.update_fps();
            }
            let until_next =
                FRAME_INTERVAL.saturating_sub(Instant::now().duration_since(self.last_frame_at));
            ctx.request_repaint_after(until_next.min(FRAME_INTERVAL));
        } else if self.event.as_ref().is_some_and(StatusEvent::is_visible) {
            // Keep repainting until the transient event expires from the status bar.
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        // Space toggles Start/Stop when no widget wants the keyboard.
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.toggle_running(ctx);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let full_rect = ui.max_rect();
        ui.painter().rect_filled(full_rect, 0.0, COLOR_WINDOW);

        let status_height = 36.0;
        let main_height = (ui.available_height() - status_height).max(1.0);

        ui.vertical(|ui| {
            ui.set_width(full_rect.width());
            ui.horizontal(|ui| {
                ui.set_height(main_height);
                ui.vertical(|ui| {
                    ui.set_width(304.0);
                    ui.set_height(main_height);
                    self.controls_ui(ui, &ctx);
                });

                ui.separator();

                ui.vertical_centered(|ui| {
                    ui.set_width((full_rect.width() - 320.0).max(1.0));
                    ui.set_height(main_height);
                    self.preview_ui(ui);
                });
            });

            ui.separator();
            self.status_ui(ui);
        });
    }
}

impl CameraManApp {
    fn controls_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(8.0);
        ui.heading("CameraMan");
        ui.label(egui::RichText::new("Multi-camera preview").color(COLOR_DIM));
        ui.add_space(16.0);

        ui.label("Sources");
        let mut mode_changed = false;
        ui.horizontal(|ui| {
            mode_changed |= ui
                .selectable_value(&mut self.input_mode, InputMode::Synthetic, "Synthetic")
                .changed();
            mode_changed |= ui
                .selectable_value(&mut self.input_mode, InputMode::Real, "Real")
                .changed();
        });
        if mode_changed {
            // Leaving Real mode must release the camera; entering it must not
            // grab the camera until the user explicitly starts the preview.
            self.stop_streaming();
            self.render_preview(ctx, false);
        }

        ui.add_space(8.0);
        match self.input_mode {
            InputMode::Synthetic => {
                let mut source_changed = false;
                for source in &mut self.sources {
                    ui.horizontal(|ui| {
                        let (swatch, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(
                            swatch,
                            3.0,
                            egui::Color32::from_rgb(source.bgra[2], source.bgra[1], source.bgra[0]),
                        );
                        source_changed |= ui.checkbox(&mut source.selected, source.name).changed();
                    });
                }
                if source_changed {
                    self.render_preview(ctx, false);
                }
            }
            InputMode::Real => {
                if ui
                    .add_sized([190.0, 30.0], egui::Button::new("Refresh cameras"))
                    .clicked()
                {
                    self.refresh_real_devices(true);
                }

                if self.real_devices.is_empty() {
                    ui.label(
                        egui::RichText::new("No cameras found. Connect one and refresh.")
                            .color(COLOR_DIM),
                    );
                }
                let mut selected_changed = false;
                for device in &self.real_devices {
                    selected_changed |= ui
                        .radio_value(
                            &mut self.selected_real_id,
                            Some(device.id.clone()),
                            &device.name,
                        )
                        .changed();
                }
                if selected_changed {
                    let was_running = self.running;
                    self.stop_streaming();
                    self.running = was_running;
                    self.render_preview(ctx, self.running);
                }
            }
        }

        ui.add_space(16.0);
        ui.label("Layout");
        let previous_layout = self.layout;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.layout, CompositionLayout::Grid, "Grid");
            ui.selectable_value(&mut self.layout, CompositionLayout::Row, "Row");
            ui.selectable_value(&mut self.layout, CompositionLayout::Column, "Column");
        });
        if previous_layout != self.layout {
            self.render_preview(ctx, false);
        }

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            let (label, fill) = if self.running {
                ("Stop", COLOR_STOP)
            } else {
                ("Start", COLOR_ACCENT)
            };
            let start_button = egui::Button::new(
                egui::RichText::new(label)
                    .strong()
                    .color(egui::Color32::WHITE),
            )
            .fill(fill);
            let can_start = self.running || self.selected_count() > 0;
            if ui
                .add_enabled_ui(can_start, |ui| ui.add_sized([92.0, 34.0], start_button))
                .inner
                .on_hover_text("Space")
                .on_disabled_hover_text("Select at least one source")
                .clicked()
            {
                self.toggle_running(ctx);
            }

            let render_enabled = !self.running && self.input_mode == InputMode::Synthetic;
            if ui
                .add_enabled_ui(render_enabled, |ui| {
                    ui.add_sized([92.0, 34.0], egui::Button::new("Render"))
                })
                .inner
                .on_hover_text("Render a single frame")
                .on_disabled_hover_text(if self.running {
                    "Already rendering continuously"
                } else {
                    "Use Start for the live camera preview"
                })
                .clicked()
            {
                self.render_preview(ctx, false);
            }
        });

        ui.add_space(8.0);
        let export_button = egui::Button::new("Export PPM");
        if ui
            .add_enabled_ui(self.preview.is_some(), |ui| {
                ui.add_sized([190.0, 34.0], export_button)
            })
            .inner
            .on_disabled_hover_text("Nothing to export yet")
            .clicked()
        {
            self.export_preview();
        }

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);
        ui.label("Output");
        ui.monospace(format!("{}x{} BGRA @ 30 fps", OUTPUT_WIDTH, OUTPUT_HEIGHT));
        ui.monospace(self.export_path.display().to_string());
        ui.add_space(4.0);
        if ui
            .checkbox(&mut self.virtual_output_enabled, "Virtual camera output")
            .changed()
            && !self.virtual_output_enabled
        {
            self.disconnect_virtual_output();
        }
        ui.monospace(self.virtual_sink.path().display().to_string());
        ui.add_space(4.0);
        let virtual_label = if self.virtual_output_enabled {
            "Virtual camera output: enabled"
        } else {
            "Virtual camera output: off"
        };
        ui.label(egui::RichText::new(virtual_label).color(COLOR_DIM).small());
    }

    fn preview_ui(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let target_ratio = OUTPUT_WIDTH as f32 / OUTPUT_HEIGHT as f32;
        let mut preview_size = egui::vec2(available.x, available.x / target_ratio);
        if preview_size.y > available.y {
            preview_size.y = available.y;
            preview_size.x = available.y * target_ratio;
        }
        preview_size.x = preview_size.x.max(1.0);
        preview_size.y = preview_size.y.max(1.0);

        ui.add_space(((available.y - preview_size.y) / 2.0).max(0.0));
        let (rect, _) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
        ui.painter().rect_filled(rect, 4.0, egui::Color32::BLACK);
        if let Some(texture) = &self.preview_texture {
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            self.paint_preview_badges(ui, rect);
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No preview yet.\nSelect sources and press Start.",
                egui::FontId::proportional(15.0),
                COLOR_DIM,
            );
        }
    }

    /// Small overlay in the preview corner: live indicator plus source mode,
    /// so a screenshot of the window can never be mistaken for a real stream.
    fn paint_preview_badges(&self, ui: &egui::Ui, rect: egui::Rect) {
        let painter = ui.painter();
        let mode = match self.input_mode {
            InputMode::Synthetic => "SYNTHETIC",
            InputMode::Real => "REAL",
        };
        let state = if self.running { "LIVE" } else { "PAUSED" };
        let text = format!("{state} · {mode}");
        let font = egui::FontId::proportional(12.0);

        let galley = painter.layout_no_wrap(text, font, egui::Color32::WHITE);
        let padding = egui::vec2(8.0, 4.0);
        let dot_space = 12.0;
        let badge = egui::Rect::from_min_size(
            rect.min + egui::vec2(10.0, 10.0),
            galley.size() + padding * 2.0 + egui::vec2(dot_space, 0.0),
        );
        painter.rect_filled(badge, 4.0, egui::Color32::from_black_alpha(160));
        let dot_center = egui::pos2(badge.min.x + padding.x + 3.0, badge.center().y);
        let dot_color = if self.running { COLOR_ERROR } else { COLOR_DIM };
        painter.circle_filled(dot_center, 3.0, dot_color);
        painter.galley(
            egui::pos2(badge.min.x + padding.x + dot_space, badge.min.y + padding.y),
            galley,
            egui::Color32::WHITE,
        );
    }

    fn status_ui(&self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            let (color, text) = self.state_line();
            ui.painter().circle_filled(
                ui.cursor().min + egui::vec2(4.0, ui.available_height() / 2.0),
                4.0,
                color,
            );
            ui.add_space(14.0);
            ui.label(egui::RichText::new(text).color(color));
            ui.separator();
            if self.running && self.measured_fps > 0.0 {
                ui.label(format!("{:.0} fps", self.measured_fps));
                ui.separator();
            }
            ui.label(format!("sources {}", self.selected_count()));
            ui.separator();
            ui.label(format!("frames {}", self.frames_rendered));
            ui.separator();
            ui.label(format!("virtual {}", self.virtual_frames_sent));
            ui.separator();
            ui.label(format!("layout {}", self.layout.name()));
        });
    }
}

fn configure_style(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.visuals.panel_fill = COLOR_PANEL;
        style.visuals.window_fill = COLOR_WINDOW;
        style.visuals.widgets.inactive.bg_fill = COLOR_WIDGET;
        style.visuals.widgets.hovered.bg_fill = COLOR_WIDGET_HOVER;
        style.visuals.widgets.active.bg_fill = COLOR_WIDGET_ACTIVE;
    });
}

fn frame_to_color_image(frame: &Frame) -> egui::ColorImage {
    let mut rgba = Vec::with_capacity(frame.data().len());
    for pixel in frame.data().chunks_exact(4) {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    egui::ColorImage::from_rgba_unmultiplied(
        [frame.width() as usize, frame.height() as usize],
        &rgba,
    )
}

fn animated_color(base: [u8; 4], tick: u64) -> [u8; 4] {
    let wave = ((tick % 60) as i16 - 30).unsigned_abs() as i16;
    [
        base[0].saturating_add((wave / 4) as u8),
        base[1].saturating_add((wave / 5) as u8),
        base[2].saturating_add((wave / 6) as u8),
        base[3],
    ]
}
