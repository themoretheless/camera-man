use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::app_preferences::{
    AppPreferences, CURRENT_SCHEMA_VERSION, FpsMode, UiLocale, default_preferences_path,
};
use crate::camera_discovery_worker::CameraDiscoveryWorker;
use crate::media_clock::MediaClock;
use crate::render_worker::{RenderJob, RenderWorker};
use crate::{
    APPLICATION_GROUPS_ENTITLEMENT, EXTENSION_APP_SANDBOX_ENTITLEMENT,
    SYSTEM_EXTENSION_INSTALL_ENTITLEMENT, provisioning,
};
use camera_man::FrameSource;
use camera_man::{
    CameraDevice, CameraRuntime, CaptureErrorKind, CapturedFrame, CompositionLayout,
    ConsumerProgress, EXTENSION_BUNDLE_ID, ErrorCode, ExtensionActivationStatus,
    ExtensionInstaller, Frame, FrameMetadata, FrameTransportSink, MissingSourcePolicy,
    PipelineStage, PixelFormat, ProducerProgress, Rotation, SCENE_PARSER_LIMITS, ScalingFilter,
    SceneChange, SceneDocument, SourceDescriptor, SourceFit, SourceHealthSummary,
    SourceHealthVector, SourceKind, SourceTransform, SyntheticFrameSource,
    ThreadedNokhwaFrameSource, VIRTUAL_CAMERA_DEFAULT_FPS, VIRTUAL_CAMERA_FPS_PRESETS,
    VIRTUAL_CAMERA_HEIGHT, VIRTUAL_CAMERA_MAX_FPS, VIRTUAL_CAMERA_WIDTH, VideoFormat,
    camera_open_timeout,
};
use eframe::egui;

mod accessibility;
mod capture_coordination;
mod controller;
mod diagnostics_report;
mod extension_readiness;
mod framerate;
mod io_worker;
mod localization;
mod render_coordination;
mod scene_commands;
mod scenes;
mod self_test;
mod status_line;
mod ui;
mod ui_contracts;
mod ui_output;
mod ui_scene;
mod ui_setup;
mod ui_tokens;

use accessibility::{progress_indicator, system_accessibility_settings};
use capture_coordination::{SourceSlot, camera_locators_match};
use controller::{AppCommand, AppEvent, WorkflowState, reduce};
use localization::{UiText, tr};
use render_coordination::RenderFingerprint;
use scene_commands::SceneEditCommand;
use self_test::VirtualCameraSelfTest;
use status_line::StatusEvent;
use ui_contracts::*;
use ui_tokens::*;

pub(crate) use extension_readiness::{
    app_bundle_application_group, app_bundle_has_activation_location, current_app_bundle_path,
    extension_activation_metadata,
};
#[cfg(test)]
use extension_readiness::{
    app_bundle_path_for_executable, extension_activation_metadata_from_value,
};
use extension_readiness::{extension_capability, provisioning_profile_status};
use io_worker::{DEFAULT_DIAGNOSTICS_PATH, IoOperation, IoWorker, RedactedDiagnostics};

// Synthetic sources are solid colors. A 4:3 logical tile preserves their
// composition geometry without allocating a 320x240 buffer every app tick.
const SOURCE_WIDTH: u32 = 4;
const SOURCE_HEIGHT: u32 = 3;
const PREVIEW_MAX_WIDTH: u32 = 960;
const PREVIEW_MAX_HEIGHT: u32 = 540;
const PREVIEW_INTERVAL: Duration =
    Duration::from_nanos(1_000_000_000 / VIRTUAL_CAMERA_DEFAULT_FPS as u64);
const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(8);
const CAMERA_DISCOVERY_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DEFAULT_SCENE_PATH: &str = "target/cameraman-scene.json";

pub fn run() -> eframe::Result<()> {
    let fixture_capture = std::env::var_os("CAMERAMAN_UI_SCREENSHOT_TO").map(PathBuf::from);
    let preferences_path = fixture_capture
        .as_ref()
        .map(|path| path.with_extension("preferences.json"))
        .or_else(default_preferences_path);
    let logical_size = ui_fixture_logical_size();
    let initial_size = [logical_size[0] as f32, logical_size[1] as f32];
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CameraMan")
            .with_inner_size(initial_size)
            .with_min_inner_size([MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT]),
        persist_window: fixture_capture.is_none(),
        persistence_path: fixture_capture
            .as_ref()
            .map(|path| path.with_extension("state.ron")),
        ..Default::default()
    };

    eframe::run_native(
        "CameraMan",
        options,
        Box::new(move |cc| Ok(Box::new(CameraManApp::new(cc, preferences_path.clone())))),
    )
}

fn ui_fixture_logical_size() -> [u32; 2] {
    std::env::var("CAMERAMAN_UI_FIXTURE_SIZE")
        .ok()
        .and_then(|value| {
            value
                .split_once('x')
                .map(|(width, height)| (width.to_owned(), height.to_owned()))
        })
        .and_then(|(width, height)| Some([width.parse().ok()?, height.parse().ok()?]))
        .unwrap_or([1120, 720])
}

fn ui_fixture_target_size() -> [u32; 2] {
    let logical = ui_fixture_logical_size();
    let scale = std::env::var("CAMERAMAN_UI_FIXTURE_SCALE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|scale| matches!(scale, 1 | 2))
        .unwrap_or(1);
    [logical[0] * scale, logical[1] * scale]
}

/// Derived undo/redo value. It can be rebuilt from runtime scene state and is
/// never an independent persisted source of truth.
#[derive(Clone, PartialEq)]
struct SceneSnapshot {
    sources: Vec<SourceDescriptor>,
    layout: CompositionLayout,
    scaling_filter: ScalingFilter,
    fps_mode: FpsMode,
    source_transforms: BTreeMap<String, SourceTransform>,
    missing_source_policy: MissingSourcePolicy,
    active_scene_name: Option<String>,
}

struct UiFixtureCapture {
    path: PathBuf,
    target_size: [u32; 2],
    settle_frames: u8,
    requested: bool,
}

enum ProvisioningProfileStatus {
    Missing,
    Valid {
        path: PathBuf,
        team_identifier: Option<String>,
    },
    Invalid {
        path: PathBuf,
        error: String,
    },
}

#[derive(Clone, Copy)]
enum AppLaunchContext {
    BareExecutable,
    DevelopmentBundle,
    ApplicationsBundle,
}

impl AppLaunchContext {
    const fn label(self) -> &'static str {
        match self {
            Self::BareExecutable => "bare executable",
            Self::DevelopmentBundle => "development .app",
            Self::ApplicationsBundle => "Applications .app",
        }
    }
}

/// How the composited output is built: which grid, which resampler, which
/// target rate, and what a missing source means. The scene snapshot, the output
/// panel and every render job read all four together, so they travel together.
#[derive(Debug, Clone, Copy)]
struct OutputSettings {
    fps_mode: FpsMode,
    layout: CompositionLayout,
    scaling_filter: ScalingFilter,
    missing_source_policy: MissingSourcePolicy,
}

/// Scene editing state: the name field, the undo/redo history and the baseline
/// held across one pointer gesture, and the file the scene panel reads and
/// writes. It sits here rather than in `scenes.rs` so that `ui_scene.rs` can
/// reach its fields.
#[derive(Default)]
struct SceneEditing {
    scene_name_input: String,
    undo_stack: Vec<SceneSnapshot>,
    redo_stack: Vec<SceneSnapshot>,
    pending_scene_edit: Option<SceneSnapshot>,
    skip_history_commit: bool,
    scene_path: PathBuf,
    import_preview: Option<SceneDocument>,
}

/// UI-thread-owned runtime coordinator. Workers receive typed jobs and return
/// typed snapshots; they never borrow this aggregate.
pub struct CameraManApp {
    sources: Vec<SourceSlot>,
    /// Ordered heterogeneous scene graph. Composition order is this vector's order.
    selected_sources: Vec<SourceDescriptor>,
    real_devices: Vec<CameraDevice>,
    camera_discovery: CameraDiscoveryWorker,
    announce_discovery_result: bool,
    selected_source_id: Option<String>,
    source_transforms: BTreeMap<String, SourceTransform>,
    last_good_frames: HashMap<String, (CapturedFrame, Instant)>,
    scenes: Vec<SceneDocument>,
    active_scene_name: Option<String>,
    scene_editing: SceneEditing,
    locale: UiLocale,
    high_contrast: bool,
    system_increase_contrast: bool,
    reduce_motion: bool,
    differentiate_without_color: bool,
    voice_over_enabled: bool,
    show_setup: bool,
    force_setup_fixture: bool,
    fixture_state: Option<UiFixtureState>,
    fixture_capture: Option<UiFixtureCapture>,
    missing_source_count: usize,
    virtual_camera_self_test: VirtualCameraSelfTest,
    /// Only camera descriptors currently selected have an entry here;
    /// unchecking a device drops its entry immediately, releasing the camera.
    /// The trailing `u32` is that camera's own consecutive-failure streak,
    /// tracked per source so one broken camera in a multi-camera composite
    /// reports itself once, flips to the RETRY state and waits for the user,
    /// instead of spamming the status bar forever or depending on the global
    /// `capture_error_streak` (which only ever sees the all-cameras-failing
    /// case). Nothing removes a failing camera automatically; the source keeps
    /// its slot in the scene until the user retries or unchecks it.
    real_sources: Vec<(String, Box<dyn CameraRuntime>, u32)>,
    extension_installer: ExtensionInstaller,
    extension_status_override: Option<ExtensionActivationStatus>,
    /// Computed once at startup; see `extension_capability()`.
    extension_capable: bool,
    extension_capability_reason: Option<String>,
    launch_context: AppLaunchContext,
    signing_team_identifier: Option<String>,
    extension_profile_present: bool,
    provisioning_profile_status: ProvisioningProfileStatus,
    output: OutputSettings,
    /// The fps actually in effect right now: either the fixed choice, or (in
    /// Auto) the capped max negotiated across open real cameras / the shared
    /// virtual-camera default.
    /// Recomputed at the top of every `request_render` call.
    active_fps: u32,
    media_clock: MediaClock,
    render_worker: RenderWorker,
    /// Invalidates a completed frame when source/layout state changed while
    /// that frame was still being composed on the worker.
    render_epoch: u64,
    last_render_fingerprint: Option<RenderFingerprint>,
    running: bool,
    tick: u64,
    frames_rendered: u64,
    notice: Option<StatusEvent>,
    sticky_error: Option<String>,
    capture_error_streak: u32,
    waiting_for_camera: bool,
    /// A selected camera's capture watchdog fired: its open, its first frame or
    /// a mid-stream read stopped returning. Every surface that speaks while no
    /// frame is on screen (status line, preview overlay, the preview's
    /// accessible name) must then say the camera is not responding instead of
    /// claiming it is still warming up.
    camera_not_responding: bool,
    fps_window_start: Instant,
    fps_window_frames: u32,
    measured_fps: f32,
    preview: Option<Frame>,
    preview_texture: Option<egui::TextureHandle>,
    last_preview_upload: Instant,
    virtual_output_enabled: bool,
    virtual_endpoint: String,
    virtual_frames_sent: u64,
    export_path: PathBuf,
    preferences_path: Option<PathBuf>,
    io_worker: IoWorker,
    diagnostics_path: PathBuf,
}

impl CameraManApp {
    fn new(cc: &eframe::CreationContext<'_>, preferences_path: Option<PathBuf>) -> Self {
        let mut preferences = AppPreferences::load(cc.storage, preferences_path.as_deref());
        let accessibility = system_accessibility_settings();
        if let Ok(locale) = std::env::var("CAMERAMAN_UI_LOCALE") {
            preferences.locale = match locale.to_ascii_lowercase().as_str() {
                "ru" => UiLocale::Russian,
                "pseudo" | "pseudo-long" => UiLocale::PseudoLong,
                _ => UiLocale::English,
            };
        }
        if std::env::var_os("CAMERAMAN_HIGH_CONTRAST").is_some() {
            preferences.high_contrast = true;
        }
        configure_style(
            &cc.egui_ctx,
            preferences.high_contrast || accessibility.increase_contrast,
        );
        let sources = vec![
            SourceSlot {
                id: "desk",
                name: "Desk Camera",
                bgra: [235, 94, 40, 255],
            },
            SourceSlot {
                id: "side",
                name: "Side Camera",
                bgra: [62, 181, 137, 255],
            },
            SourceSlot {
                id: "wide",
                name: "Wide Camera",
                bgra: [72, 126, 220, 255],
            },
            SourceSlot {
                id: "overhead",
                name: "Overhead",
                bgra: [196, 166, 76, 255],
            },
        ];

        let virtual_sink = FrameTransportSink::default();
        let virtual_endpoint = virtual_sink.description();
        let render_worker = RenderWorker::new(virtual_sink);
        let media_clock = MediaClock::new(cc.egui_ctx.clone(), VIRTUAL_CAMERA_DEFAULT_FPS);
        let current_bundle = current_app_bundle_path();
        let launch_context = match current_bundle.as_deref() {
            Some(bundle) if app_bundle_has_activation_location(bundle) => {
                AppLaunchContext::ApplicationsBundle
            }
            Some(_) => AppLaunchContext::DevelopmentBundle,
            None => AppLaunchContext::BareExecutable,
        };
        let signing_team_identifier = current_bundle
            .as_deref()
            .and_then(|bundle| crate::codesign_team_identifier(bundle).ok().flatten());
        let extension_profile_present = current_bundle.as_deref().is_some_and(|bundle| {
            bundle
                .join("Contents/Library/SystemExtensions")
                .join(format!("{EXTENSION_BUNDLE_ID}.systemextension"))
                .join("Contents/embedded.provisionprofile")
                .is_file()
        });
        let extension_capability = extension_capability();
        let selected_source_id = preferences
            .selected_sources
            .first()
            .map(SourceDescriptor::stable_key);
        let scene_name_input = preferences
            .active_scene_name
            .clone()
            .unwrap_or_else(|| String::from("Scene 1"));
        let fixture_state = std::env::var("CAMERAMAN_UI_FIXTURE_STATE")
            .ok()
            .and_then(|value| UiFixtureState::parse(&value));
        let force_setup_fixture = std::env::var_os("CAMERAMAN_UI_SHOW_SETUP").is_some()
            || matches!(
                fixture_state,
                Some(UiFixtureState::Setup | UiFixtureState::InstallError)
            );
        let fixture_capture =
            std::env::var_os("CAMERAMAN_UI_SCREENSHOT_TO").map(|path| UiFixtureCapture {
                path: PathBuf::from(path),
                target_size: ui_fixture_target_size(),
                settle_frames: 8,
                requested: false,
            });
        let mut app = Self {
            sources,
            selected_sources: preferences.selected_sources,
            real_devices: Vec::new(),
            camera_discovery: CameraDiscoveryWorker::new(),
            announce_discovery_result: false,
            selected_source_id,
            source_transforms: preferences.source_transforms,
            last_good_frames: HashMap::new(),
            scenes: preferences.scenes,
            active_scene_name: preferences.active_scene_name,
            scene_editing: SceneEditing {
                scene_name_input,
                scene_path: PathBuf::from(DEFAULT_SCENE_PATH),
                ..Default::default()
            },
            locale: preferences.locale,
            high_contrast: preferences.high_contrast,
            system_increase_contrast: accessibility.increase_contrast,
            reduce_motion: accessibility.reduce_motion,
            differentiate_without_color: accessibility.differentiate_without_color,
            voice_over_enabled: accessibility.voice_over_enabled,
            show_setup: force_setup_fixture,
            force_setup_fixture,
            fixture_state,
            fixture_capture,
            missing_source_count: 0,
            virtual_camera_self_test: VirtualCameraSelfTest::Idle,
            real_sources: Vec::new(),
            extension_installer: ExtensionInstaller::new(),
            extension_status_override: None,
            extension_capable: extension_capability.is_ok(),
            extension_capability_reason: extension_capability.err(),
            launch_context,
            signing_team_identifier,
            extension_profile_present,
            provisioning_profile_status: provisioning_profile_status(),
            output: OutputSettings {
                fps_mode: preferences.fps_mode,
                layout: preferences.layout,
                scaling_filter: preferences.scaling_filter,
                missing_source_policy: preferences.missing_source_policy,
            },
            active_fps: VIRTUAL_CAMERA_DEFAULT_FPS,
            media_clock,
            render_worker,
            render_epoch: 0,
            last_render_fingerprint: None,
            running: false,
            tick: 0,
            frames_rendered: 0,
            notice: None,
            sticky_error: None,
            capture_error_streak: 0,
            waiting_for_camera: false,
            camera_not_responding: false,
            fps_window_start: Instant::now(),
            fps_window_frames: 0,
            measured_fps: 0.0,
            preview: None,
            preview_texture: None,
            last_preview_upload: Instant::now(),
            virtual_output_enabled: preferences.virtual_output_enabled,
            virtual_endpoint,
            virtual_frames_sent: 0,
            export_path: preferences.export_path,
            preferences_path,
            io_worker: IoWorker::new(),
            diagnostics_path: PathBuf::from(DEFAULT_DIAGNOSTICS_PATH),
        };
        if let Some(state) = fixture_state {
            app.configure_ui_fixture_state(state);
        } else {
            app.start_camera_discovery(false);
        }
        if !matches!(
            fixture_state,
            Some(UiFixtureState::Empty | UiFixtureState::Disconnected)
        ) {
            let fixture_running = app.running;
            if fixture_state.is_some() {
                app.running = false;
            }
            app.request_render(&cc.egui_ctx, false, true);
            app.running = fixture_running;
        }
        app
    }

    fn selected_count(&self) -> usize {
        self.selected_sources.len()
    }

    fn preferences(&self) -> AppPreferences {
        AppPreferences {
            schema_version: CURRENT_SCHEMA_VERSION,
            selected_sources: self.selected_sources.clone(),
            legacy_input_mode: Default::default(),
            legacy_selected_synthetic_ids: Vec::new(),
            legacy_selected_real_ids: Vec::new(),
            layout: self.output.layout,
            scaling_filter: self.output.scaling_filter,
            fps_mode: self.output.fps_mode,
            virtual_output_enabled: self.virtual_output_enabled,
            export_path: self.export_path.clone(),
            locale: self.locale,
            high_contrast: self.high_contrast,
            source_transforms: self.source_transforms.clone(),
            missing_source_policy: self.output.missing_source_policy,
            scenes: self.scenes.clone(),
            active_scene_name: self.active_scene_name.clone(),
        }
    }

    fn toggle_running(&mut self, ctx: &egui::Context) {
        let mut workflow = WorkflowState {
            running: self.running,
            selected_sources: self.selected_count(),
            virtual_output_enabled: self.virtual_output_enabled,
        };
        match reduce(&mut workflow, AppCommand::ToggleStreaming) {
            AppEvent::StreamingStopped => self.stop_streaming(),
            AppEvent::StartRejectedNoSources => {
                self.set_event("Select at least one source first", true);
            }
            AppEvent::StreamingStarted => {
                self.sticky_error = None;
                self.running = workflow.running;
                self.capture_error_streak = 0;
                self.fps_window_start = Instant::now();
                self.fps_window_frames = 0;
                self.recompute_active_fps();
                self.media_clock.start(self.active_fps);
                self.request_render(ctx, true, true);
            }
            AppEvent::VirtualOutputChanged { .. } | AppEvent::NoChange => {}
        }
    }

    /// Stops the render loop AND releases every open camera. Keeping a
    /// camera open after Stop leaves its LED on and looks like spying.
    fn stop_streaming(&mut self) {
        self.running = false;
        self.media_clock.stop();
        self.last_render_fingerprint = None;
        self.invalidate_render_epoch();
        self.real_sources.clear();
        self.waiting_for_camera = false;
        self.camera_not_responding = false;
        self.measured_fps = 0.0;
        self.disconnect_virtual_output();
        // No real sources left open: in Auto mode this drops back to the shared
        // virtual-camera default instead of showing a stale negotiated rate.
        self.recompute_active_fps();
    }

    fn on_capture_error(&mut self, message: String) {
        if self.capture_error_streak == 0 {
            self.set_event(message, true);
        }
        self.capture_error_streak = self.capture_error_streak.saturating_add(1);
    }

    fn disconnect_virtual_output(&mut self) {
        self.render_worker.disconnect_output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_bundle_path_is_derived_only_from_a_contents_macos_executable() {
        assert_eq!(
            app_bundle_path_for_executable(Path::new(
                "/Applications/CameraMan.app/Contents/MacOS/CameraMan"
            )),
            Some(PathBuf::from("/Applications/CameraMan.app"))
        );
        assert_eq!(
            app_bundle_path_for_executable(Path::new("target/debug/camera-man")),
            None
        );
        assert_eq!(
            app_bundle_path_for_executable(Path::new(
                "/Applications/CameraMan/Contents/MacOS/CameraMan"
            )),
            None
        );
    }

    #[test]
    fn extension_activation_location_requires_an_applications_directory() {
        assert!(app_bundle_has_activation_location(Path::new(
            "/Applications/CameraMan.app"
        )));
        assert!(!app_bundle_has_activation_location(Path::new(
            "/System/Applications/CameraMan.app"
        )));
        assert!(!app_bundle_has_activation_location(Path::new(
            "/Users/example/project/target/CameraMan.app"
        )));
        assert!(!app_bundle_has_activation_location(Path::new(
            "/tmp/Applications/CameraMan.app"
        )));
    }

    #[test]
    fn activation_metadata_requires_mach_service_and_usage_description() {
        let valid = plist::Value::from_reader_xml(
            br#"<plist version="1.0"><dict>
                <key>CMIOExtension</key><dict>
                    <key>CMIOExtensionMachServiceName</key>
                    <string>TEAM.group.cmio</string>
                </dict>
                <key>NSSystemExtensionUsageDescription</key>
                <string>Virtual camera</string>
                <key>CameraManAppGroup</key>
                <string>TEAM.group</string>
            </dict></plist>"#
                .as_slice(),
        )
        .unwrap();
        let missing_usage = plist::Value::from_reader_xml(
            br#"<plist version="1.0"><dict>
                <key>CMIOExtension</key><dict>
                    <key>CMIOExtensionMachServiceName</key>
                    <string>TEAM.group.cmio</string>
                </dict>
            </dict></plist>"#
                .as_slice(),
        )
        .unwrap();

        let metadata = extension_activation_metadata_from_value(&valid).unwrap();
        assert_eq!(metadata.mach_service_name, "TEAM.group.cmio");
        assert_eq!(metadata.usage_description, "Virtual camera");
        assert_eq!(metadata.application_group.as_deref(), Some("TEAM.group"));
        assert!(extension_activation_metadata_from_value(&missing_usage).is_err());
    }
}
