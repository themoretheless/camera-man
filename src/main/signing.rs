use super::*;

/// Sentinel returned by `signing_identity()` when no real identity is configured.
pub(super) const AD_HOC_IDENTITY: &str = "-";

/// Reads CODESIGN_IDENTITY (e.g. "Apple Development: Jane Doe (TEAMID1234)").
/// Defaults to ad-hoc ("-"), which is fine for plain code but cannot carry
/// restricted entitlements: see the comment in `bundle_app`.
pub(super) fn signing_identity() -> String {
    env::var("CODESIGN_IDENTITY").unwrap_or_else(|_| AD_HOC_IDENTITY.to_string())
}

pub(super) fn sign_path(path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    run_codesign(path.as_ref(), None)
}

pub(super) fn sign_path_with_entitlements(
    path: impl AsRef<Path>,
    entitlements: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    run_codesign(path.as_ref(), Some(entitlements.as_ref()))
}

pub(super) fn run_codesign(
    path: &Path,
    entitlements: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let identity = signing_identity();
    let mut command = Command::new("/usr/bin/codesign");
    command.args(["--force", "--sign", &identity]);
    if identity == AD_HOC_IDENTITY {
        command.arg("--timestamp=none");
    } else {
        command.args(["--timestamp", "--options", "runtime"]);
    }
    if let Some(entitlements) = entitlements {
        command.arg("--entitlements").arg(entitlements);
    }
    command.arg(path);

    let command_context = codesign_command_context(&identity, path, entitlements);
    let failure = match command.output() {
        Ok(output) if output.status.success() => return Ok(()),
        Ok(output) => format!(
            "{command_context} exited with {}; stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => format!("could not run {command_context}: {error}"),
    };

    if signing_failure_is_fatal(is_release_build(), &identity) {
        return Err(failure.into());
    }

    eprintln!("warning: {failure}; debug ad-hoc bundle may remain unsigned");
    Ok(())
}

pub(super) fn codesign_command_context(
    identity: &str,
    path: &Path,
    entitlements: Option<&Path>,
) -> String {
    let signing_options = if identity == AD_HOC_IDENTITY {
        "--timestamp=none"
    } else {
        "--timestamp --options runtime"
    };
    let mut context = format!("/usr/bin/codesign --force --sign {identity:?} {signing_options}");
    if let Some(entitlements) = entitlements {
        context.push_str(&format!(" --entitlements {entitlements:?}"));
    }
    context.push_str(&format!(" {path:?}"));
    context
}

pub(super) fn signing_failure_is_fatal(release: bool, identity: &str) -> bool {
    release || identity != AD_HOC_IDENTITY
}

pub(super) fn validate_bundle_signing_configuration(
    identity: &str,
    app_profile_configured: bool,
    extension_profile_configured: bool,
    application_group_configured: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if identity == AD_HOC_IDENTITY && (app_profile_configured || extension_profile_configured) {
        return Err(
            "provisioning profiles require CODESIGN_IDENTITY; refusing to silently ignore them"
                .into(),
        );
    }
    if app_profile_configured != extension_profile_configured {
        return Err(
            "an installable app bundle requires both CAMERAMAN_PROVISIONING_PROFILE and CAMERAMAN_EXTENSION_PROVISIONING_PROFILE"
                .into(),
        );
    }
    if application_group_configured && !app_profile_configured {
        return Err("CAMERAMAN_APP_GROUP is only valid with both provisioning profiles".into());
    }
    Ok(())
}

pub(crate) fn verify_codesign(path: &Path, deep: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new("/usr/bin/codesign");
    command.args(["--verify", "--strict", "--verbose=2"]);
    if deep {
        command.arg("--deep");
    }
    command.arg(path);
    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "codesign verification failed for {} with {}; stderr: {}",
        path.display(),
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    )
    .into())
}

pub(crate) fn codesign_team_identifier(
    path: &Path,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let output = Command::new("/usr/bin/codesign")
        .args(["--display", "--verbose=4"])
        .arg(path)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "codesign Team ID inspection failed for {} with {}; stderr: {}",
            path.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }

    Ok(parse_codesign_team_identifier(&output.stdout)
        .or_else(|| parse_codesign_team_identifier(&output.stderr)))
}

pub(super) fn parse_codesign_team_identifier(output: &[u8]) -> Option<String> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| line.trim().strip_prefix("TeamIdentifier="))
        .find(|team_identifier| !team_identifier.is_empty() && *team_identifier != "not set")
        .map(str::to_owned)
}

pub(super) fn validate_team_identifier_match(
    signing_team_identifier: Option<&str>,
    profile_team_identifier: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let signing_team_identifier = signing_team_identifier.ok_or(
        "the left signing/profile value has no TeamIdentifier; use an explicit Apple team identity",
    )?;
    let profile_team_identifier = profile_team_identifier
        .ok_or("the right signing/profile value has no Team ID and cannot be matched")?;
    if signing_team_identifier != profile_team_identifier {
        return Err(format!(
            "signing Team ID {signing_team_identifier} does not match provisioning profile Team ID {profile_team_identifier}"
        )
        .into());
    }
    Ok(())
}
