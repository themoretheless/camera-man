use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use camera_man::EXTENSION_BUNDLE_ID;
use camera_man::atomic_file::replace_file_atomically;
use serde::Serialize;

const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
struct ReleaseManifest {
    schema_version: u32,
    version: &'static str,
    commit: String,
    source_date_epoch: String,
    repository: String,
    git_ref: String,
    toolchain: Toolchain,
    platform: Platform,
    bundles: Bundles,
    artifacts: Vec<Artifact>,
    evidence: Evidence,
    stages: Vec<Stage>,
}

#[derive(Debug, Serialize)]
struct Toolchain {
    rustc: String,
    target: String,
    xcode: String,
    macos_sdk: String,
    cargo_auditable: String,
}

#[derive(Debug, Serialize)]
struct Platform {
    minimum_macos: String,
    build_macos: String,
    build_macos_build: String,
}

#[derive(Debug, Serialize)]
struct Bundles {
    application_id: String,
    extension_id: String,
    application_group: Option<String>,
    team_id: String,
}

#[derive(Debug, Serialize)]
struct Artifact {
    role: &'static str,
    path: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct Evidence {
    sbom_paths: Vec<String>,
    notary_result: String,
    checksum: String,
    workflow_run: Option<String>,
    provenance_index: Option<String>,
}

#[derive(Debug, Serialize)]
struct Stage {
    name: &'static str,
    evidence: &'static str,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let output_dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/release-artifacts"));
    let app = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/CameraMan.app"));
    if args.next().is_some() {
        return Err("usage: cameraman-release-manifest [output-dir] [app-bundle]".into());
    }

    let app_info = app.join("Contents/Info.plist");
    let extension = app.join(format!(
        "Contents/Library/SystemExtensions/{EXTENSION_BUNDLE_ID}.systemextension"
    ));
    let extension_info = extension.join("Contents/Info.plist");
    let app_binary = app.join("Contents/MacOS/CameraMan");
    let extension_binary = extension.join("Contents/MacOS/CameraManExtension");
    let archive = output_dir.join("CameraMan.zip");
    let checksum = output_dir.join("CameraMan.zip.sha256");
    let sbom_spdx2 = output_dir.join("camera-man.spdx.json");
    let sbom_spdx3 = output_dir.join("camera-man.spdx3.jsonld");
    let notary_result = output_dir.join("notary-result.json");
    let reproducibility_evidence = output_dir
        .parent()
        .unwrap_or_else(|| Path::new("target"))
        .join("reproducibility/unsigned-binaries.sha256");

    for required in [
        &app_info,
        &extension_info,
        &app_binary,
        &extension_binary,
        &archive,
        &checksum,
        &sbom_spdx2,
        &sbom_spdx3,
        &notary_result,
        &reproducibility_evidence,
    ] {
        if !required.exists() {
            return Err(format!("release evidence is missing: {}", required.display()).into());
        }
    }

    let app_team = codesign_team_identifier(&app)?;
    let extension_team = codesign_team_identifier(&extension)?;
    if app_team != extension_team {
        return Err(format!(
            "nested signature Team ID mismatch: app={app_team}, extension={extension_team}"
        )
        .into());
    }

    let rustc_verbose = command_text(Command::new("rustc").arg("--version").arg("--verbose"))?;
    let target = value_after_prefix(&rustc_verbose, "host: ")
        .ok_or("rustc --version --verbose did not report a host target")?;
    let repository =
        env::var("GITHUB_REPOSITORY").unwrap_or_else(|_| String::from("themoretheless/camera-man"));
    let run_url = env::var("GITHUB_RUN_ID")
        .ok()
        .map(|run_id| format!("https://github.com/{repository}/actions/runs/{run_id}"));
    let provenance_index = Some(format!("https://github.com/{repository}/attestations"));

    let manifest = ReleaseManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        version: env!("CARGO_PKG_VERSION"),
        commit: required_env_or_command("CAMERAMAN_RELEASE_COMMIT", "git", &["rev-parse", "HEAD"])?,
        source_date_epoch: required_env_or_command(
            "SOURCE_DATE_EPOCH",
            "git",
            &["show", "-s", "--format=%ct", "HEAD"],
        )?,
        repository,
        git_ref: env::var("GITHUB_REF").unwrap_or_else(|_| String::from("local")),
        toolchain: Toolchain {
            rustc: rustc_verbose.lines().next().unwrap_or("unknown").to_owned(),
            target,
            xcode: command_text(Command::new("xcodebuild").arg("-version"))?
                .lines()
                .collect::<Vec<_>>()
                .join("; "),
            macos_sdk: command_text(
                Command::new("xcrun")
                    .arg("--sdk")
                    .arg("macosx")
                    .arg("--show-sdk-version"),
            )?,
            cargo_auditable: command_text(Command::new("cargo").arg("auditable").arg("--version"))?,
        },
        platform: Platform {
            minimum_macos: plist_string(&app_info, "LSMinimumSystemVersion")?,
            build_macos: command_text(Command::new("/usr/bin/sw_vers").arg("-productVersion"))?,
            build_macos_build: command_text(Command::new("/usr/bin/sw_vers").arg("-buildVersion"))?,
        },
        bundles: Bundles {
            application_id: plist_string(&app_info, "CFBundleIdentifier")?,
            extension_id: plist_string(&extension_info, "CFBundleIdentifier")?,
            application_group: plist_optional_string(&app_info, "CameraManAppGroup")?,
            team_id: app_team,
        },
        artifacts: vec![
            artifact("signed-host-binary", &app_binary)?,
            artifact("signed-extension-binary", &extension_binary)?,
            artifact("notarized-archive", &archive)?,
            artifact("archive-checksum", &checksum)?,
            artifact("spdx-2.3-sbom", &sbom_spdx2)?,
            artifact("spdx-3.0.1-sbom", &sbom_spdx3)?,
            artifact("apple-notary-result", &notary_result)?,
            artifact(
                "unsigned-reproducibility-digests",
                &reproducibility_evidence,
            )?,
        ],
        evidence: Evidence {
            sbom_paths: vec![path_text(&sbom_spdx2)?, path_text(&sbom_spdx3)?],
            notary_result: path_text(&notary_result)?,
            checksum: path_text(&checksum)?,
            workflow_run: run_url,
            provenance_index,
        },
        stages: vec![
            Stage {
                name: "checkout",
                evidence: "manifest commit and immutable workflow checkout action",
            },
            Stage {
                name: "auditable-build",
                evidence: "embedded dependency metadata extracted from both final Mach-O binaries",
            },
            Stage {
                name: "reproducibility-compare",
                evidence: "two clean unsigned builds compared byte-for-byte",
            },
            Stage {
                name: "nested-signing",
                evidence: "strict codesign verification and matching Team IDs",
            },
            Stage {
                name: "notarization",
                evidence: "accepted notary result plus stapler and Gatekeeper checks",
            },
            Stage {
                name: "packaging",
                evidence: "archive digest and final archive artifact scan",
            },
            Stage {
                name: "publication",
                evidence: "GitHub artifact upload and hosted artifact attestations",
            },
        ],
    };

    let output = output_dir.join("release-manifest.json");
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    replace_file_atomically(&output, &bytes)?;
    println!("Generated {}", output.display());
    Ok(())
}

fn artifact(role: &'static str, path: &Path) -> Result<Artifact, Box<dyn Error>> {
    Ok(Artifact {
        role,
        path: path_text(path)?,
        sha256: sha256(path)?,
    })
}

fn sha256(path: &Path) -> Result<String, Box<dyn Error>> {
    let output = command_output(
        Command::new("/usr/bin/shasum")
            .arg("-a")
            .arg("256")
            .arg(path),
    )?;
    output
        .split_whitespace()
        .next()
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_owned)
        .ok_or_else(|| format!("invalid SHA-256 output for {}", path.display()).into())
}

fn plist_string(path: &Path, key: &str) -> Result<String, Box<dyn Error>> {
    plist_optional_string(path, key)?
        .ok_or_else(|| format!("{} does not contain string key {key}", path.display()).into())
}

fn plist_optional_string(path: &Path, key: &str) -> Result<Option<String>, Box<dyn Error>> {
    let value = plist::Value::from_file(path)?;
    let dictionary = value
        .as_dictionary()
        .ok_or_else(|| format!("{} is not a plist dictionary", path.display()))?;
    match dictionary.get(key) {
        Some(plist::Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("{} key {key} is not a string", path.display()).into()),
        None => Ok(None),
    }
}

fn codesign_team_identifier(path: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new("/usr/bin/codesign")
        .arg("--display")
        .arg("--verbose=4")
        .arg(path)
        .output()?;
    if !output.status.success() {
        return Err(command_failure("codesign", &output).into());
    }
    let details = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    value_after_prefix(&details, "TeamIdentifier=")
        .filter(|value| value != "not set")
        .ok_or_else(|| format!("{} has no signing Team ID", path.display()).into())
}

fn required_env_or_command(
    variable: &str,
    program: &str,
    args: &[&str],
) -> Result<String, Box<dyn Error>> {
    if let Ok(value) = env::var(variable)
        && !value.trim().is_empty()
    {
        return Ok(value);
    }
    command_text(Command::new(program).args(args))
}

fn command_text(command: &mut Command) -> Result<String, Box<dyn Error>> {
    Ok(command_output(command)?.trim().to_owned())
}

fn command_output(command: &mut Command) -> Result<String, Box<dyn Error>> {
    let label = format!("{command:?}");
    let output = command.output()?;
    if !output.status.success() {
        return Err(command_failure(&label, &output).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn command_failure(label: &str, output: &Output) -> String {
    format!(
        "{label} failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

fn value_after_prefix(text: &str, prefix: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn path_text(path: &Path) -> Result<String, Box<dyn Error>> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("release path is not UTF-8: {}", path.display()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_line_metadata_without_accepting_empty_values() {
        assert_eq!(
            value_after_prefix(
                "Executable=/tmp/app\nTeamIdentifier=TEAM123\n",
                "TeamIdentifier="
            ),
            Some(String::from("TEAM123"))
        );
        assert_eq!(value_after_prefix("host: \n", "host: "), None);
    }
}
