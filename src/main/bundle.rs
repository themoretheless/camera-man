use super::*;

pub(super) struct InstallableProfiles {
    app: provisioning::ValidatedProvisioningProfile,
    extension: provisioning::ValidatedProvisioningProfile,
    application_group: String,
}

pub(super) fn bundle_app() -> Result<(), Box<dyn std::error::Error>> {
    let identity = signing_identity();
    let app_profile_path = provisioning_profile();
    let extension_profile_path = extension_provisioning_profile();
    let application_group = configured_application_group();
    validate_bundle_signing_configuration(
        &identity,
        app_profile_path.is_some(),
        extension_profile_path.is_some(),
        application_group.is_some(),
    )?;
    let installable_profiles = load_installable_profiles(
        app_profile_path,
        extension_profile_path,
        application_group.as_deref(),
    )?;
    let extension_binary = prepare_extension_binary()?;

    let current_exe = env::current_exe()?;
    let bundle = PathBuf::from("target/CameraMan.app");
    remove_bundle_if_present(&bundle)?;
    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos)?;
    fs::create_dir_all(&resources)?;

    let app_exe = macos.join("CameraMan");
    fs::copy(&current_exe, &app_exe)?;
    copy_extension_into_app(&contents, &extension_binary, installable_profiles.as_ref())?;

    write_plist(
        contents.join("Info.plist"),
        &app_info_plist(
            installable_profiles
                .as_ref()
                .map(|profiles| profiles.application_group.as_str()),
        ),
    )?;

    let mut pkg_info = fs::File::create(contents.join("PkgInfo"))?;
    pkg_info.write_all(b"APPL????")?;

    sign_path(&app_exe)?;

    // com.apple.developer.system-extension.install is a restricted entitlement.
    // AMFI kills the whole process at launch when the entitlement is present
    // without a matching provisioning profile. Therefore a plain
    // CODESIGN_IDENTITY is not enough: only embed the entitlement when the
    // caller also provides CAMERAMAN_PROVISIONING_PROFILE / PROVISIONING_PROFILE.
    if identity == AD_HOC_IDENTITY {
        sign_path(&bundle)?;
        eprintln!(
            "note: signed ad-hoc (no CODESIGN_IDENTITY set), so the restricted \
             system-extension.install entitlement was left out; the app will launch, \
             but system-extension activation needs real signing and provisioning."
        );
    } else if let Some(profiles) = installable_profiles {
        let signing_team_identifier = codesign_team_identifier(&app_exe)?;
        validate_team_identifier_match(
            signing_team_identifier.as_deref(),
            profiles.app.team_identifier(),
        )?;
        validate_team_identifier_match(
            signing_team_identifier.as_deref(),
            profiles.extension.team_identifier(),
        )?;
        fs::copy(
            profiles.app.path(),
            contents.join("embedded.provisionprofile"),
        )?;
        eprintln!(
            "Embedded provisioning profile: {} (Team ID {}, App Group {})",
            profiles.app.path().display(),
            profiles.app.team_identifier().unwrap_or("<missing>"),
            profiles.application_group,
        );

        let app_entitlements = write_app_entitlements(&profiles.application_group)?;
        sign_path_with_entitlements(&bundle, &app_entitlements)?;
    } else {
        sign_path(&bundle)?;
        eprintln!(
            "note: CODESIGN_IDENTITY is set, but no CAMERAMAN_PROVISIONING_PROFILE \
             or PROVISIONING_PROFILE was provided. The app was signed without \
             system-extension.install so macOS will not kill it at launch, but \
             extension activation will fail until a matching profile is embedded."
        );
    }

    verify_codesign(&bundle, true)?;

    println!("Created {}", bundle.display());
    println!("Launch with: open {}", bundle.display());
    Ok(())
}

pub(crate) fn provisioning_profile() -> Option<PathBuf> {
    env::var_os("CAMERAMAN_PROVISIONING_PROFILE")
        .or_else(|| env::var_os("PROVISIONING_PROFILE"))
        .map(PathBuf::from)
}

pub(super) fn extension_provisioning_profile() -> Option<PathBuf> {
    env::var_os("CAMERAMAN_EXTENSION_PROVISIONING_PROFILE").map(PathBuf::from)
}

pub(super) fn configured_application_group() -> Option<String> {
    env::var("CAMERAMAN_APP_GROUP")
        .ok()
        .filter(|value| !value.is_empty())
}

pub(super) fn load_installable_profiles(
    app_profile_path: Option<PathBuf>,
    extension_profile_path: Option<PathBuf>,
    requested_application_group: Option<&str>,
) -> Result<Option<InstallableProfiles>, Box<dyn std::error::Error>> {
    let (Some(app_profile_path), Some(extension_profile_path)) =
        (app_profile_path, extension_profile_path)
    else {
        return Ok(None);
    };
    let app = provisioning::validate(
        &app_profile_path,
        APP_BUNDLE_ID,
        SYSTEM_EXTENSION_INSTALL_ENTITLEMENT,
    )?;
    let extension = provisioning::validate(
        &extension_profile_path,
        EXTENSION_BUNDLE_ID,
        EXTENSION_APP_SANDBOX_ENTITLEMENT,
    )?;
    validate_team_identifier_match(app.team_identifier(), extension.team_identifier())?;
    let application_group =
        select_shared_application_group(&app, &extension, requested_application_group)?;
    Ok(Some(InstallableProfiles {
        app,
        extension,
        application_group,
    }))
}

pub(crate) fn select_shared_application_group(
    app: &provisioning::ValidatedProvisioningProfile,
    extension: &provisioning::ValidatedProvisioningProfile,
    requested: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    select_shared_application_group_values(
        app.application_groups(),
        extension.application_groups(),
        requested,
    )
}

pub(super) fn select_shared_application_group_values(
    app_groups: &[String],
    extension_groups: &[String],
    requested: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut common = app_groups
        .iter()
        .filter(|group| extension_groups.contains(group))
        .filter(|group| !group.contains('*'))
        .cloned()
        .collect::<Vec<_>>();
    common.sort();
    common.dedup();
    if let Some(requested) = requested {
        if common.iter().any(|group| group == requested) {
            return Ok(requested.to_owned());
        }
        return Err(format!(
            "CAMERAMAN_APP_GROUP {requested:?} is not granted by both provisioning profiles; common groups: {}",
            display_list(&common)
        )
        .into());
    }
    common.into_iter().next().ok_or_else(|| {
        format!(
            "app and extension provisioning profiles have no common non-wildcard {APPLICATION_GROUPS_ENTITLEMENT} value"
        )
        .into()
    })
}

pub(super) fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        String::from("<none>")
    } else {
        values.join(", ")
    }
}

pub(super) fn write_app_entitlements(
    application_group: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let app_entitlements = PathBuf::from("target/signing/CameraMan.entitlements");
    if let Some(parent) = app_entitlements.parent() {
        fs::create_dir_all(parent)?;
    }
    write_plist(
        &app_entitlements,
        &app_entitlements_plist(application_group),
    )?;
    Ok(app_entitlements)
}

pub(super) fn bundle_extension() -> Result<(), Box<dyn std::error::Error>> {
    let extension_binary = prepare_extension_binary()?;
    let extension_bundle = extension_bundle_path();
    let profile = match extension_provisioning_profile() {
        Some(path) => {
            if signing_identity() == AD_HOC_IDENTITY {
                return Err(
                    "CAMERAMAN_EXTENSION_PROVISIONING_PROFILE requires CODESIGN_IDENTITY".into(),
                );
            }
            Some(provisioning::validate(
                &path,
                EXTENSION_BUNDLE_ID,
                EXTENSION_APP_SANDBOX_ENTITLEMENT,
            )?)
        }
        None => None,
    };
    let application_group = profile
        .as_ref()
        .map(|profile| {
            select_shared_application_group(
                profile,
                profile,
                configured_application_group().as_deref(),
            )
        })
        .transpose()?;
    assemble_extension_bundle(
        &extension_bundle,
        &extension_binary,
        profile.as_ref(),
        application_group.as_deref(),
    )?;
    println!("Created {}", extension_bundle.display());
    Ok(())
}

pub(super) fn copy_extension_into_app(
    contents: &std::path::Path,
    extension_binary: &std::path::Path,
    profiles: Option<&InstallableProfiles>,
) -> Result<(), Box<dyn std::error::Error>> {
    let embedded = embedded_extension_bundle_path(contents);
    assemble_extension_bundle(
        &embedded,
        extension_binary,
        profiles.map(|profiles| &profiles.extension),
        profiles.map(|profiles| profiles.application_group.as_str()),
    )
}

pub(super) fn embedded_extension_bundle_path(contents: &Path) -> PathBuf {
    contents
        .join("Library")
        .join("SystemExtensions")
        .join(format!("{EXTENSION_BUNDLE_ID}.systemextension"))
}

/// True when the currently running `camera-man` binary was itself built in
/// release mode (baked in at compile time). Used so `cargo run --release --
/// bundle` does not silently embed a debug (unoptimized, ~10x slower per the
/// fps investigation) `cameraman-extension` inside an otherwise-release app.
pub(super) fn is_release_build() -> bool {
    !cfg!(debug_assertions)
}

pub(super) fn build_extension_binary() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["build", "--locked", "--bin", "cameraman-extension"];
    if is_release_build() {
        args.push("--release");
    }
    let status = Command::new("cargo").args(args).status()?;
    if !status.success() {
        return Err("failed to build cameraman-extension".into());
    }
    Ok(())
}

pub(super) fn prepare_extension_binary() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = env::var_os("CAMERAMAN_PREBUILT_EXTENSION").map(PathBuf::from) {
        if !path.is_file() {
            return Err(format!(
                "CAMERAMAN_PREBUILT_EXTENSION is not a regular file: {}",
                path.display()
            )
            .into());
        }
        return Ok(path);
    }
    build_extension_binary()?;
    Ok(extension_binary_path(is_release_build()))
}

pub(super) fn assemble_extension_bundle(
    bundle: &std::path::Path,
    extension_binary: &std::path::Path,
    profile: Option<&provisioning::ValidatedProvisioningProfile>,
    application_group: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if profile.is_some() != application_group.is_some() {
        return Err(
            "extension profile and shared application group must be supplied together".into(),
        );
    }
    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    remove_bundle_if_present(bundle)?;
    fs::create_dir_all(&macos)?;

    fs::copy(extension_binary, macos.join("CameraManExtension"))?;

    let mach_service_name = extension_mach_service_name(application_group);
    write_plist(
        contents.join("Info.plist"),
        &extension_info_plist(&mach_service_name),
    )?;

    let extension_entitlements = PathBuf::from("target/signing/CameraManExtension.entitlements");
    if let Some(parent) = extension_entitlements.parent() {
        fs::create_dir_all(parent)?;
    }
    write_plist(
        &extension_entitlements,
        &extension_entitlements_plist(application_group),
    )?;

    if let Some(profile) = profile {
        fs::copy(profile.path(), contents.join("embedded.provisionprofile"))?;
        eprintln!(
            "Embedded extension provisioning profile: {} (Team ID {}, App Group {}, Mach service {})",
            profile.path().display(),
            profile.team_identifier().unwrap_or("<missing>"),
            application_group.unwrap_or("<missing>"),
            mach_service_name,
        );
    }

    sign_path(macos.join("CameraManExtension"))?;
    sign_path_with_entitlements(bundle, extension_entitlements)?;
    verify_codesign(bundle, false)?;
    if let Some(profile) = profile {
        let signing_team_identifier = codesign_team_identifier(bundle)?;
        validate_team_identifier_match(
            signing_team_identifier.as_deref(),
            profile.team_identifier(),
        )?;
    }
    Ok(())
}

fn remove_bundle_if_present(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(format!("failed to remove stale bundle {}: {error}", path.display()).into())
        }
    }
}

pub(super) fn extension_mach_service_name(application_group: Option<&str>) -> String {
    application_group.map_or_else(
        || EXTENSION_BUNDLE_ID.to_owned(),
        |application_group| format!("{application_group}.cmio"),
    )
}

pub(crate) fn mach_service_has_application_group_prefix(
    mach_service: &str,
    application_group: &str,
) -> bool {
    mach_service
        .strip_prefix(application_group)
        .is_some_and(|suffix| suffix.starts_with('.') && suffix.len() > 1)
}

pub(super) fn extension_binary_path(release: bool) -> PathBuf {
    PathBuf::from("target")
        .join(if release { "release" } else { "debug" })
        .join("cameraman-extension")
}

pub(super) fn extension_bundle_path() -> PathBuf {
    PathBuf::from(format!("target/{EXTENSION_BUNDLE_ID}.systemextension"))
}
