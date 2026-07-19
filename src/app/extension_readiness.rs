use super::*;

/// Whether this run can plausibly activate the system extension. This checks
/// the installed location, nested bundle, signatures, restricted entitlement,
/// both embedded profiles, and all Team ID relationships. Computed once at
/// startup because an already-running bundle cannot be safely re-signed.
pub(super) fn extension_capability() -> Result<(), String> {
    let Some(bundle) = current_app_bundle_path() else {
        return Err(String::from(
            "Unavailable: launch CameraMan from an installed .app bundle",
        ));
    };
    if !app_bundle_has_activation_location(&bundle) {
        return Err(String::from(
            "Unavailable: move CameraMan.app to an Applications folder",
        ));
    }
    let extension = bundle
        .join("Contents/Library/SystemExtensions")
        .join(format!("{EXTENSION_BUNDLE_ID}.systemextension"));
    if !extension.is_dir() {
        return Err(String::from(
            "Unavailable: bundled system extension is missing",
        ));
    }

    crate::verify_codesign(&bundle, true)
        .map_err(|error| format!("Unavailable: app signature is invalid: {error}"))?;
    let app_entitlements = signed_entitlements(&bundle, "app")?;
    if !provisioning::decoded_entitlements_grant(
        &app_entitlements,
        SYSTEM_EXTENSION_INSTALL_ENTITLEMENT,
    ) {
        return Err(String::from(
            "Unavailable: signed system-extension.install entitlement missing",
        ));
    }

    let app_profile_path = bundle.join("Contents/embedded.provisionprofile");
    let app_profile = provisioning::validate(
        &app_profile_path,
        crate::APP_BUNDLE_ID,
        SYSTEM_EXTENSION_INSTALL_ENTITLEMENT,
    )
    .map_err(|error| format!("Unavailable: app provisioning profile is invalid: {error}"))?;
    let extension_profile_path = extension.join("Contents/embedded.provisionprofile");
    let extension_profile = provisioning::validate(
        &extension_profile_path,
        EXTENSION_BUNDLE_ID,
        EXTENSION_APP_SANDBOX_ENTITLEMENT,
    )
    .map_err(|error| format!("Unavailable: extension provisioning profile is invalid: {error}"))?;
    let metadata = extension_activation_metadata(&extension)?;
    let extension_application_group = metadata
        .application_group
        .as_deref()
        .ok_or_else(|| String::from("Unavailable: extension App Group metadata is missing"))?;
    let application_group = crate::select_shared_application_group(
        &app_profile,
        &extension_profile,
        Some(extension_application_group),
    )
    .map_err(|error| format!("Unavailable: App Group mismatch: {error}"))?;
    let host_application_group = app_bundle_application_group(&bundle)
        .ok_or_else(|| String::from("Unavailable: host App Group metadata is missing"))?;
    if host_application_group != application_group {
        return Err(format!(
            "Unavailable: host App Group {host_application_group} does not match extension group {application_group}"
        ));
    }
    let extension_entitlements = signed_entitlements(&extension, "extension")?;
    for (label, entitlements) in [
        ("app", app_entitlements.as_slice()),
        ("extension", extension_entitlements.as_slice()),
    ] {
        let groups =
            provisioning::decoded_entitlement_strings(entitlements, APPLICATION_GROUPS_ENTITLEMENT);
        if !groups.contains(&application_group) {
            return Err(format!(
                "Unavailable: signed {label} App Group {application_group} is missing"
            ));
        }
    }
    if metadata.usage_description.trim().is_empty() {
        return Err(String::from(
            "Unavailable: extension usage description is empty",
        ));
    }
    if !crate::mach_service_has_application_group_prefix(
        &metadata.mach_service_name,
        &application_group,
    ) {
        return Err(format!(
            "Unavailable: Mach service {} is not prefixed by App Group {application_group}",
            metadata.mach_service_name
        ));
    }

    let app_team_identifier = crate::codesign_team_identifier(&bundle)
        .map_err(|error| format!("Unavailable: cannot inspect app Team ID: {error}"))?;
    let extension_team_identifier = crate::codesign_team_identifier(&extension)
        .map_err(|error| format!("Unavailable: cannot inspect extension Team ID: {error}"))?;
    crate::validate_team_identifier_match(
        app_team_identifier.as_deref(),
        app_profile.team_identifier(),
    )
    .map_err(|error| format!("Unavailable: app signing mismatch: {error}"))?;
    crate::validate_team_identifier_match(
        app_team_identifier.as_deref(),
        extension_team_identifier.as_deref(),
    )
    .map_err(|error| format!("Unavailable: app/extension signing mismatch: {error}"))?;
    crate::validate_team_identifier_match(
        extension_team_identifier.as_deref(),
        extension_profile.team_identifier(),
    )
    .map_err(|error| format!("Unavailable: extension signing mismatch: {error}"))?;

    Ok(())
}

fn signed_entitlements(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    let output = std::process::Command::new("/usr/bin/codesign")
        .args(["--display", "--entitlements", "-", "--xml"])
        .arg(path)
        .output()
        .map_err(|error| format!("Unavailable: cannot inspect {label} entitlements: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "Unavailable: cannot inspect {label} entitlements: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub(crate) struct ExtensionActivationMetadata {
    pub(crate) mach_service_name: String,
    pub(crate) usage_description: String,
    pub(crate) application_group: Option<String>,
}

pub(crate) fn extension_activation_metadata(
    extension: &Path,
) -> Result<ExtensionActivationMetadata, String> {
    let path = extension.join("Contents/Info.plist");
    let info = plist::Value::from_file(&path)
        .map_err(|error| format!("Unavailable: invalid extension Info.plist: {error}"))?;
    extension_activation_metadata_from_value(&info)
}

pub(crate) fn extension_activation_metadata_from_value(
    info: &plist::Value,
) -> Result<ExtensionActivationMetadata, String> {
    let dictionary = info
        .as_dictionary()
        .ok_or_else(|| String::from("Unavailable: extension Info.plist is not a dictionary"))?;
    let mach_service_name = dictionary
        .get("CMIOExtension")
        .and_then(plist::Value::as_dictionary)
        .and_then(|cmio| cmio.get("CMIOExtensionMachServiceName"))
        .and_then(plist::Value::as_string)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| String::from("Unavailable: extension Mach service is missing"))?;
    let usage_description = dictionary
        .get("NSSystemExtensionUsageDescription")
        .and_then(plist::Value::as_string)
        .ok_or_else(|| String::from("Unavailable: extension usage description is missing"))?;
    let application_group = dictionary
        .get(camera_man::APP_GROUP_INFO_KEY)
        .and_then(plist::Value::as_string)
        .filter(|group| !group.is_empty())
        .map(str::to_owned);
    Ok(ExtensionActivationMetadata {
        mach_service_name: mach_service_name.to_owned(),
        usage_description: usage_description.to_owned(),
        application_group,
    })
}

pub(crate) fn app_bundle_has_activation_location(bundle: &Path) -> bool {
    bundle.starts_with("/Applications")
}

pub(crate) fn app_bundle_application_group(bundle: &Path) -> Option<String> {
    let info = plist::Value::from_file(bundle.join("Contents/Info.plist")).ok()?;
    info.as_dictionary()?
        .get(camera_man::APP_GROUP_INFO_KEY)?
        .as_string()
        .filter(|group| !group.is_empty())
        .map(str::to_owned)
}

pub(super) fn provisioning_profile_status() -> ProvisioningProfileStatus {
    let embedded_profile = current_app_bundle_path()
        .map(|bundle| bundle.join("Contents/embedded.provisionprofile"))
        .filter(|path| path.is_file());
    let Some(path) = embedded_profile.or_else(crate::provisioning_profile) else {
        return ProvisioningProfileStatus::Missing;
    };

    match provisioning::validate(
        &path,
        crate::APP_BUNDLE_ID,
        SYSTEM_EXTENSION_INSTALL_ENTITLEMENT,
    ) {
        Ok(validated) => ProvisioningProfileStatus::Valid {
            path,
            team_identifier: validated.team_identifier().map(str::to_owned),
        },
        Err(error) => ProvisioningProfileStatus::Invalid {
            path,
            error: error.to_string(),
        },
    }
}

/// The `.app` bundle containing the running executable (e.g.
/// `/Applications/CameraMan.app`), or `None` for a bare binary such as
/// `cargo run`'s `target/debug/camera-man`.
pub(crate) fn current_app_bundle_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    app_bundle_path_for_executable(&exe)
}

pub(crate) fn app_bundle_path_for_executable(exe: &Path) -> Option<PathBuf> {
    let macos_dir = exe.parent()?;
    (macos_dir.file_name()? == "MacOS").then_some(())?;
    let contents_dir = macos_dir.parent()?;
    (contents_dir.file_name()? == "Contents").then_some(())?;
    let bundle = contents_dir.parent()?;
    (bundle.extension()? == "app").then(|| bundle.to_path_buf())
}
