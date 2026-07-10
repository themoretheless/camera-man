use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

mod app;
mod provisioning;

const APP_BUNDLE_ID: &str = "com.cameraman.rust";
const SYSTEM_EXTENSION_INSTALL_ENTITLEMENT: &str = "com.apple.developer.system-extension.install";

use camera_man::FrameSource;
use camera_man::{
    CameraDiscovery, CompositionLayout, Compositor, EXTENSION_BUNDLE_ID, NokhwaCameraDiscovery,
    PipelineEngine, PixelFormat, PpmSequenceSink, SyntheticFrameSource, VideoFormat,
    VirtualCameraConfig, capture_one_with_timeout, write_ppm,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        None | Some("app") => app::run()?,
        Some("status") => print_status(),
        Some("demo") | Some("--demo") => run_demo(args.get(1).map(PathBuf::from))?,
        Some("pipeline-demo") => run_pipeline_demo(args.get(1).map(PathBuf::from))?,
        Some("list-cameras") => list_cameras()?,
        Some("capture-demo") => capture_demo(args.get(1).map(PathBuf::from))?,
        Some("bundle") => bundle_app()?,
        Some("bundle-extension") => bundle_extension()?,
        Some("check") => run_check(),
        Some("help") | Some("--help") | Some("-h") => print_help(),
        Some(command) => {
            eprintln!("Unknown command: {command}");
            print_help();
            std::process::exit(2);
        }
    }

    Ok(())
}

fn print_status() {
    let config = VirtualCameraConfig::default();
    println!("CameraMan (Rust)");
    println!("Virtual camera backend: Rust CoreMediaIO system-extension provider");
    println!(
        "Virtual camera: {} ({})",
        config.device_name, config.device_uid
    );
    println!("Extension stream: 1920x1080 30fps BGRA app-frame spool with placeholder fallback");
    println!(
        "Install note: macOS still requires system-extension approval and proper release signing/notarization."
    );
    println!(
        "Output format: {}x{} {}fps {:?}",
        config.format.width, config.format.height, config.format.fps, config.format.pixel_format
    );
    println!("Run `cargo test` to verify the core.");
    println!("Run `cargo run` to launch the Rust desktop app.");
    println!("Run `cargo run -- demo` to render a synthetic composed frame.");
    println!("Run `cargo run -- pipeline-demo` to render through the pipeline.");
    println!("Run `cargo run -- list-cameras` to query real cameras.");
    println!("Run `cargo run -- bundle` to create target/CameraMan.app.");
}

fn run_demo(output_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let format = VideoFormat {
        width: 960,
        height: 540,
        fps: 30,
        pixel_format: PixelFormat::Bgra8,
    };
    let compositor = Compositor::new(format);
    let mut blue = SyntheticFrameSource::with_id("blue", 320, 240, [255, 40, 40, 255]);
    let mut green = SyntheticFrameSource::with_id("green", 320, 240, [40, 255, 40, 255]);
    let mut red = SyntheticFrameSource::with_id("red", 320, 240, [40, 40, 255, 255]);
    let frames = vec![
        blue.latest_frame()?,
        green.latest_frame()?,
        red.latest_frame()?,
    ];

    let output = compositor.compose_captured(&frames, CompositionLayout::Grid)?;
    let path = output_path.unwrap_or_else(|| PathBuf::from("target/camera-man-demo.ppm"));
    write_ppm(&path, &output)?;
    println!("Rendered demo frame: {}", path.display());
    Ok(())
}

fn run_pipeline_demo(output_dir: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = output_dir.unwrap_or_else(|| PathBuf::from("target/camera-man-pipeline-demo"));
    let compositor = Compositor::new(VideoFormat {
        width: 960,
        height: 540,
        fps: 30,
        pixel_format: PixelFormat::Bgra8,
    });
    let sources: Vec<Box<dyn FrameSource>> = vec![
        Box::new(SyntheticFrameSource::with_id(
            "blue",
            320,
            240,
            [255, 40, 40, 255],
        )),
        Box::new(SyntheticFrameSource::with_id(
            "green",
            320,
            240,
            [40, 255, 40, 255],
        )),
        Box::new(SyntheticFrameSource::with_id(
            "red",
            320,
            240,
            [40, 40, 255, 255],
        )),
    ];
    let sink = PpmSequenceSink::new(&output_dir, "frame");
    let mut pipeline = PipelineEngine::new(compositor, sources, sink, CompositionLayout::Grid);

    pipeline.start()?;
    let summary = pipeline.render_n(3)?;
    pipeline.stop();

    println!(
        "Rendered {} pipeline frames into {}",
        summary.frames_sent,
        output_dir.display()
    );
    Ok(())
}

fn list_cameras() -> Result<(), Box<dyn std::error::Error>> {
    let discovery = NokhwaCameraDiscovery;
    let devices = discovery.list_devices()?;
    if devices.is_empty() {
        println!("No cameras found.");
    } else {
        for device in devices {
            println!("{} {}", device.id, device.name);
        }
    }
    Ok(())
}

fn capture_demo(output_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let frame = capture_one_with_timeout("0", Duration::from_secs(5))?.into_frame();
    let path = output_path.unwrap_or_else(|| PathBuf::from("target/camera-man-capture.ppm"));
    write_ppm(&path, &frame)?;
    println!("Captured real camera frame: {}", path.display());
    Ok(())
}

fn bundle_app() -> Result<(), Box<dyn std::error::Error>> {
    let current_exe = env::current_exe()?;
    let bundle = PathBuf::from("target/CameraMan.app");
    fs::remove_dir_all(&bundle).ok();
    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos)?;
    fs::create_dir_all(&resources)?;

    let app_exe = macos.join("CameraMan");
    fs::copy(&current_exe, &app_exe)?;
    copy_extension_into_app(&contents)?;

    let mut info = fs::File::create(contents.join("Info.plist"))?;
    info.write_all(app_info_plist().as_bytes())?;

    let mut pkg_info = fs::File::create(contents.join("PkgInfo"))?;
    pkg_info.write_all(b"APPL????")?;

    sign_path(&app_exe);

    // com.apple.developer.system-extension.install is a restricted entitlement.
    // AMFI kills the whole process at launch when the entitlement is present
    // without a matching provisioning profile. Therefore a plain
    // CODESIGN_IDENTITY is not enough: only embed the entitlement when the
    // caller also provides CAMERAMAN_PROVISIONING_PROFILE / PROVISIONING_PROFILE.
    if signing_identity() == AD_HOC_IDENTITY {
        sign_path(&bundle);
        eprintln!(
            "note: signed ad-hoc (no CODESIGN_IDENTITY set), so the restricted \
             system-extension.install entitlement was left out; the app will launch, \
             but system-extension activation needs real signing and provisioning."
        );
    } else if let Some(profile) = provisioning_profile() {
        let profile = provisioning::validate(
            &profile,
            APP_BUNDLE_ID,
            SYSTEM_EXTENSION_INSTALL_ENTITLEMENT,
        )?;
        fs::copy(profile.path(), contents.join("embedded.provisionprofile"))?;
        eprintln!(
            "Embedded provisioning profile: {}",
            profile.path().display()
        );

        let app_entitlements = write_app_entitlements()?;
        sign_path_with_entitlements(&bundle, &app_entitlements);
    } else {
        sign_path(&bundle);
        eprintln!(
            "note: CODESIGN_IDENTITY is set, but no CAMERAMAN_PROVISIONING_PROFILE \
             or PROVISIONING_PROFILE was provided. The app was signed without \
             system-extension.install so macOS will not kill it at launch, but \
             extension activation will fail until a matching profile is embedded."
        );
    }

    println!("Created {}", bundle.display());
    println!("Launch with: open {}", bundle.display());
    Ok(())
}

fn provisioning_profile() -> Option<PathBuf> {
    env::var_os("CAMERAMAN_PROVISIONING_PROFILE")
        .or_else(|| env::var_os("PROVISIONING_PROFILE"))
        .map(PathBuf::from)
}

fn write_app_entitlements() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let app_entitlements = PathBuf::from("target/signing/CameraMan.entitlements");
    if let Some(parent) = app_entitlements.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut entitlements = fs::File::create(&app_entitlements)?;
    entitlements.write_all(app_entitlements_plist().as_bytes())?;
    Ok(app_entitlements)
}

fn bundle_extension() -> Result<(), Box<dyn std::error::Error>> {
    build_extension_binary()?;
    let extension_bundle = extension_bundle_path();
    assemble_extension_bundle(&extension_bundle)?;
    println!("Created {}", extension_bundle.display());
    Ok(())
}

fn copy_extension_into_app(contents: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    build_extension_binary()?;
    let system_extensions_dir = contents.join("Library").join("SystemExtensions");
    let embedded = system_extensions_dir.join(format!("{EXTENSION_BUNDLE_ID}.systemextension"));
    assemble_extension_bundle(&embedded)?;
    Ok(())
}

/// True when the currently running `camera-man` binary was itself built in
/// release mode (baked in at compile time). Used so `cargo run --release --
/// bundle` does not silently embed a debug (unoptimized, ~10x slower per the
/// fps investigation) `cameraman-extension` inside an otherwise-release app.
fn is_release_build() -> bool {
    !cfg!(debug_assertions)
}

fn build_extension_binary() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["build", "--bin", "cameraman-extension"];
    if is_release_build() {
        args.push("--release");
    }
    let status = Command::new("cargo").args(args).status()?;
    if !status.success() {
        return Err("failed to build cameraman-extension".into());
    }
    Ok(())
}

fn assemble_extension_bundle(bundle: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    fs::remove_dir_all(bundle).ok();
    fs::create_dir_all(&macos)?;

    let extension_profile_dir = if is_release_build() {
        "release"
    } else {
        "debug"
    };
    fs::copy(
        format!("target/{extension_profile_dir}/cameraman-extension"),
        macos.join("CameraManExtension"),
    )?;

    let mut info = fs::File::create(contents.join("Info.plist"))?;
    info.write_all(extension_info_plist().as_bytes())?;

    let extension_entitlements = PathBuf::from("target/signing/CameraManExtension.entitlements");
    if let Some(parent) = extension_entitlements.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut entitlements = fs::File::create(&extension_entitlements)?;
    entitlements.write_all(EXTENSION_ENTITLEMENTS.as_bytes())?;

    sign_path(macos.join("CameraManExtension"));
    sign_path_with_entitlements(bundle, extension_entitlements);
    Ok(())
}

fn extension_bundle_path() -> PathBuf {
    PathBuf::from(format!("target/{EXTENSION_BUNDLE_ID}.systemextension"))
}

/// Sentinel returned by `signing_identity()` when no real identity is configured.
const AD_HOC_IDENTITY: &str = "-";

/// Reads CODESIGN_IDENTITY (e.g. "Apple Development: Jane Doe (TEAMID1234)").
/// Defaults to ad-hoc ("-"), which is fine for plain code but cannot carry
/// restricted entitlements: see the comment in `bundle_app`.
fn signing_identity() -> String {
    env::var("CODESIGN_IDENTITY").unwrap_or_else(|_| AD_HOC_IDENTITY.to_string())
}

fn sign_path(path: impl AsRef<OsStr>) {
    let status = Command::new("codesign")
        .args(["--force", "--sign", &signing_identity(), "--timestamp=none"])
        .arg(path)
        .status();
    if let Ok(status) = status
        && status.success()
    {
        return;
    }
    eprintln!("warning: codesign failed or is unavailable; bundle left unsigned");
}

fn sign_path_with_entitlements(path: impl AsRef<OsStr>, entitlements: impl AsRef<OsStr>) {
    let status = Command::new("codesign")
        .args([
            "--force",
            "--sign",
            &signing_identity(),
            "--timestamp=none",
            "--entitlements",
        ])
        .arg(entitlements)
        .arg(path)
        .status();
    if let Ok(status) = status
        && status.success()
    {
        return;
    }
    eprintln!("warning: codesign with entitlements failed or is unavailable; bundle left unsigned");
}

fn run_check() {
    let config = VirtualCameraConfig::default();
    println!("config.device_name={}", config.device_name);
    println!("config.device_uid={}", config.device_uid);
    println!("config.width={}", config.format.width);
    println!("config.height={}", config.format.height);
    println!("config.fps={}", config.format.fps);
    println!("config.pixel_format={:?}", config.format.pixel_format);
}

fn print_help() {
    println!("CameraMan Rust");
    println!();
    println!("Commands:");
    println!("  app                    Launch the Rust desktop app");
    println!("  status                 Print project and output status");
    println!("  demo [path]            Render one synthetic composed PPM frame");
    println!("  pipeline-demo [dir]    Render three frames through PipelineEngine");
    println!("  list-cameras           Query real cameras through nokhwa");
    println!("  capture-demo [path]    Capture one real camera frame into PPM");
    println!("  bundle                 Create target/CameraMan.app");
    println!("  bundle-extension       Create target/*.systemextension");
    println!("  check                  Print normalized config values");
    println!("  help                   Print this help");
}

fn app_info_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>CameraMan</string>
  <key>CFBundleExecutable</key>
  <string>CameraMan</string>
  <key>CFBundleIdentifier</key>
  <string>{APP_BUNDLE_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>CameraMan</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.video</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.0</string>
  <key>NSCameraUsageDescription</key>
  <string>CameraMan captures camera frames locally so it can compose a multi-camera preview.</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
"#
    )
}

fn app_entitlements_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>{SYSTEM_EXTENSION_INSTALL_ENTITLEMENT}</key>
  <true/>
</dict>
</plist>
"#
    )
}

/// Built at bundle time (not a `const`) so `CFBundleIdentifier` and the CMIO
/// mach-service name are both derived from `EXTENSION_BUNDLE_ID`, the same
/// constant `system_extension.rs` uses to request activation, instead of
/// duplicating the literal a third time.
fn extension_info_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>CameraManExtension</string>
  <key>CFBundleIdentifier</key>
  <string>{EXTENSION_BUNDLE_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>CameraManExtension</string>
  <key>CFBundlePackageType</key>
  <string>SYSX</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>CMIOExtension</key>
  <dict>
    <key>CMIOExtensionMachServiceName</key>
    <string>{EXTENSION_BUNDLE_ID}</string>
  </dict>
</dict>
</plist>
"#
    )
}

const EXTENSION_ENTITLEMENTS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.app-sandbox</key>
  <true/>
</dict>
</plist>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_app_plists_use_shared_constants() {
        let info = plist::Value::from_reader_xml(app_info_plist().as_bytes()).unwrap();
        let bundle_id = info
            .as_dictionary()
            .and_then(|dictionary| dictionary.get("CFBundleIdentifier"))
            .and_then(plist::Value::as_string);
        assert_eq!(bundle_id, Some(APP_BUNDLE_ID));

        assert!(provisioning::decoded_entitlements_grant(
            app_entitlements_plist().as_bytes(),
            SYSTEM_EXTENSION_INSTALL_ENTITLEMENT,
        ));
    }
}
