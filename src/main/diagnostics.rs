use super::*;

pub(super) fn run_check() {
    let config = VirtualCameraConfig::default();
    println!("config.device_name={}", config.device_name);
    println!("config.device_uid={}", config.device_uid);
    println!("config.width={}", config.format.width);
    println!("config.height={}", config.format.height);
    println!("config.fps={}", config.format.fps);
    println!("config.pixel_format={:?}", config.format.pixel_format);
    println!(
        "config.max_frame_bytes={}",
        default_frame_limits().max_frame_bytes()
    );
}

pub(super) fn is_help_flag(argument: &str) -> bool {
    matches!(argument, "help" | "--help" | "-h")
}

pub(super) fn print_version() {
    println!("CameraMan {}", env!("CARGO_PKG_VERSION"));
}

pub(super) fn diagnose_extension() {
    let app_bundle =
        app::current_app_bundle_path().unwrap_or_else(|| PathBuf::from("target/CameraMan.app"));
    let app_contents = app_bundle.join("Contents");
    let extension_bundle = embedded_extension_bundle_path(&app_contents);
    let identity = signing_identity();

    println!("CameraMan extension diagnostics");
    println!("version={}", env!("CARGO_PKG_VERSION"));
    println!("platform={}", env::consts::OS);
    println!(
        "build.profile={}",
        if is_release_build() {
            "release"
        } else {
            "debug"
        }
    );
    println!(
        "signing.identity={}",
        if identity == AD_HOC_IDENTITY {
            "ad-hoc (-)"
        } else {
            &identity
        }
    );
    println!("bundle.app.path={}", app_bundle.display());
    println!("bundle.app.exists={}", app_bundle.is_dir());
    println!(
        "bundle.app.app_group={}",
        app::app_bundle_application_group(&app_bundle)
            .as_deref()
            .unwrap_or("unavailable")
    );
    println!(
        "bundle.app.activation_location={}",
        app::app_bundle_has_activation_location(&app_bundle)
    );
    println!("bundle.extension.path={}", extension_bundle.display());
    println!("bundle.extension.exists={}", extension_bundle.is_dir());
    let extension_metadata = app::extension_activation_metadata(&extension_bundle);
    match &extension_metadata {
        Ok(metadata) => {
            println!(
                "bundle.extension.mach_service={}",
                metadata.mach_service_name
            );
            println!(
                "bundle.extension.usage_description={}",
                metadata.usage_description.replace('\n', " ")
            );
            println!(
                "bundle.extension.app_group={}",
                metadata
                    .application_group
                    .as_deref()
                    .unwrap_or("unavailable")
            );
        }
        Err(error) => println!("bundle.extension.metadata=unavailable ({error})"),
    }
    println!(
        "expected.app.entitlements={SYSTEM_EXTENSION_INSTALL_ENTITLEMENT}=true (requires provisioning)"
    );
    println!("expected.extension.entitlements=com.apple.security.app-sandbox=true");
    println!("expected.shared.entitlements={APPLICATION_GROUPS_ENTITLEMENT}=<same exact group>");

    let embedded_app_profile = app_contents.join("embedded.provisionprofile");
    let diagnostic_app_profile = embedded_app_profile
        .is_file()
        .then_some(embedded_app_profile)
        .or_else(provisioning_profile);
    let validated_app_profile = match diagnostic_app_profile {
        None => {
            println!(
                "provisioning.profile=missing (set CAMERAMAN_PROVISIONING_PROFILE or PROVISIONING_PROFILE)"
            );
            println!("provisioning.profile.team_id=unavailable");
            println!("provisioning.profile.app_groups=unavailable");
            None
        }
        Some(profile) => match provisioning::validate(
            &profile,
            APP_BUNDLE_ID,
            SYSTEM_EXTENSION_INSTALL_ENTITLEMENT,
        ) {
            Ok(validated) => {
                println!("provisioning.profile=valid ({})", profile.display());
                println!(
                    "provisioning.profile.team_id={}",
                    validated.team_identifier().unwrap_or("unavailable")
                );
                println!(
                    "provisioning.profile.app_groups={}",
                    display_list(validated.application_groups())
                );
                Some(validated)
            }
            Err(error) => {
                println!(
                    "provisioning.profile=invalid ({}): {error}",
                    profile.display()
                );
                println!("provisioning.profile.team_id=unavailable");
                println!("provisioning.profile.app_groups=unavailable");
                None
            }
        },
    };
    let embedded_extension_profile = extension_bundle
        .join("Contents")
        .join("embedded.provisionprofile");
    let diagnostic_extension_profile = embedded_extension_profile
        .is_file()
        .then_some(embedded_extension_profile)
        .or_else(extension_provisioning_profile);
    let validated_extension_profile = match diagnostic_extension_profile {
        None => {
            println!(
                "provisioning.extension_profile=missing (set CAMERAMAN_EXTENSION_PROVISIONING_PROFILE)"
            );
            println!("provisioning.extension_profile.team_id=unavailable");
            println!("provisioning.extension_profile.app_groups=unavailable");
            None
        }
        Some(profile) => match provisioning::validate(
            &profile,
            EXTENSION_BUNDLE_ID,
            EXTENSION_APP_SANDBOX_ENTITLEMENT,
        ) {
            Ok(validated) => {
                println!(
                    "provisioning.extension_profile=valid ({})",
                    profile.display()
                );
                println!(
                    "provisioning.extension_profile.team_id={}",
                    validated.team_identifier().unwrap_or("unavailable")
                );
                println!(
                    "provisioning.extension_profile.app_groups={}",
                    display_list(validated.application_groups())
                );
                Some(validated)
            }
            Err(error) => {
                println!(
                    "provisioning.extension_profile=invalid ({}): {error}",
                    profile.display()
                );
                println!("provisioning.extension_profile.team_id=unavailable");
                println!("provisioning.extension_profile.app_groups=unavailable");
                None
            }
        },
    };
    let profile_team_identifier = validated_app_profile
        .as_ref()
        .and_then(|profile| profile.team_identifier().map(str::to_owned));
    let extension_profile_team_identifier = validated_extension_profile
        .as_ref()
        .and_then(|profile| profile.team_identifier().map(str::to_owned));
    match (
        validated_app_profile.as_ref(),
        validated_extension_profile.as_ref(),
    ) {
        (Some(app_profile), Some(extension_profile)) => {
            let configured_application_group = configured_application_group();
            let requested_application_group = extension_metadata
                .as_ref()
                .ok()
                .and_then(|metadata| metadata.application_group.as_deref())
                .or(configured_application_group.as_deref());
            match select_shared_application_group(
                app_profile,
                extension_profile,
                requested_application_group,
            ) {
                Ok(application_group) => {
                    println!("provisioning.shared_app_group={application_group}");
                    let prefix_status = extension_metadata.as_ref().is_ok_and(|metadata| {
                        mach_service_has_application_group_prefix(
                            &metadata.mach_service_name,
                            &application_group,
                        )
                    });
                    println!("bundle.extension.mach_service_group_match={prefix_status}");
                }
                Err(error) => println!("provisioning.shared_app_group=invalid ({error})"),
            }
        }
        _ => println!("provisioning.shared_app_group=unavailable"),
    }

    print_codesign_entitlements("app", &app_bundle);
    print_codesign_entitlements("extension", &extension_bundle);
    let app_team_identifier = print_codesign_team_identifier("app", &app_bundle);
    let extension_team_identifier = print_codesign_team_identifier("extension", &extension_bundle);
    println!(
        "team_id.app_profile_match={}",
        team_identifier_match_status(
            app_team_identifier.as_deref(),
            profile_team_identifier.as_deref()
        )
    );
    println!(
        "team_id.app_extension_match={}",
        team_identifier_match_status(
            app_team_identifier.as_deref(),
            extension_team_identifier.as_deref()
        )
    );
    println!(
        "team_id.extension_profile_match={}",
        team_identifier_match_status(
            extension_team_identifier.as_deref(),
            extension_profile_team_identifier.as_deref()
        )
    );
    print_system_extensions_status();
}

pub(super) fn print_codesign_entitlements(label: &str, path: &Path) {
    if !path.exists() {
        println!(
            "codesign.{label}.entitlements=unavailable (path does not exist: {})",
            path.display()
        );
        return;
    }

    match Command::new("/usr/bin/codesign")
        .args(["--display", "--entitlements", "-", "--xml"])
        .arg(path)
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("codesign.{label}.status={}", output.status);
            println!(
                "codesign.{label}.entitlements={}",
                if stdout.trim().is_empty() {
                    "<none>"
                } else {
                    stdout.trim()
                }
            );
            if !stderr.trim().is_empty() {
                println!("codesign.{label}.details={}", stderr.trim());
            }
        }
        Err(error) => println!("codesign.{label}.entitlements=unavailable ({error})"),
    }
}

pub(super) fn print_codesign_team_identifier(label: &str, path: &Path) -> Option<String> {
    if !path.exists() {
        println!("codesign.{label}.team_id=unavailable (path does not exist)");
        return None;
    }
    match codesign_team_identifier(path) {
        Ok(Some(team_identifier)) => {
            println!("codesign.{label}.team_id={team_identifier}");
            Some(team_identifier)
        }
        Ok(None) => {
            println!("codesign.{label}.team_id=unavailable (ad-hoc or unsigned)");
            None
        }
        Err(error) => {
            println!("codesign.{label}.team_id=unavailable ({error})");
            None
        }
    }
}

pub(super) fn team_identifier_match_status(
    left: Option<&str>,
    right: Option<&str>,
) -> &'static str {
    match (left, right) {
        (Some(left), Some(right)) if left == right => "true",
        (Some(_), Some(_)) => "false",
        _ => "unavailable",
    }
}

pub(super) fn print_system_extensions_status() {
    match Command::new("/usr/bin/systemextensionsctl")
        .arg("list")
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("systemextensionsctl.status={}", output.status);
            println!(
                "systemextensionsctl.list={}",
                if stdout.trim().is_empty() {
                    "<empty>"
                } else {
                    stdout.trim()
                }
            );
            if !stderr.trim().is_empty() {
                println!("systemextensionsctl.details={}", stderr.trim());
            }
        }
        Err(error) => println!("systemextensionsctl.status=unavailable ({error})"),
    }
}

pub(super) fn print_bundle_help() {
    println!("Usage: camera-man bundle");
    println!();
    println!("Builds target/CameraMan.app and embeds the Rust CoreMediaIO extension.");
    println!();
    println!("Signing environment:");
    println!("  CODESIGN_IDENTITY                 Apple signing identity; default is ad-hoc (-)");
    println!("  CAMERAMAN_PROVISIONING_PROFILE    Matching app provisioning profile");
    println!("  CAMERAMAN_EXTENSION_PROVISIONING_PROFILE  Matching extension provisioning profile");
    println!("  CAMERAMAN_APP_GROUP              Optional common App Group override");
    println!("  CAMERAMAN_PREBUILT_EXTENSION     Verified prebuilt extension for release tooling");
    println!("  PROVISIONING_PROFILE              Fallback provisioning profile variable");
    println!();
    println!("Release install checklist:");
    println!("  cargo run --release -- bundle");
    println!("  plutil -lint target/CameraMan.app/Contents/Info.plist");
    println!("  codesign --verify --deep --strict --verbose=2 target/CameraMan.app");
    println!("  cargo run -- diagnose-extension");
}

pub(super) fn print_help() {
    println!("CameraMan Rust");
    println!();
    println!("Commands:");
    println!("  app                    Launch the Rust desktop app");
    println!("  status                 Print project and output status");
    println!("  demo [path]            Render one synthetic composed PPM frame");
    println!("  pipeline-demo [dir]    Render three frames through PipelineEngine");
    println!("  list-cameras           Query real cameras through nokhwa");
    println!("  capture-demo [path]    Capture one real camera frame into PPM");
    println!("  bundle                 Create target/CameraMan.app (prefer a release build)");
    println!("  bundle-extension       Create target/*.systemextension (prefer release)");
    println!("  check                  Print normalized config values");
    println!("  version                Print the CameraMan version");
    println!("  diagnose-extension     Inspect bundles, signing, profile, and system state");
    println!("  help                   Print this help");
    println!();
    println!("Run `camera-man bundle --help` for signing variables and release checks.");
}
