use std::path::{Path, PathBuf};

use plist::Value;

use crate::error::CameraManError;

pub const APP_GROUP_INFO_KEY: &str = "CameraManAppGroup";
const SHARED_FRAME_DIRECTORY: &str = "CameraMan";
const SHARED_FRAME_FILENAME: &str = "frames-v5.mmap";

/// App Group embedded into the current app or system-extension Info.plist.
/// Bare development binaries deliberately return `None` and keep using POSIX
/// shared memory, while installable bundles use their shared group container.
pub fn bundled_application_group() -> Option<String> {
    let executable = std::env::current_exe().ok()?;
    let info_path = bundle_info_path_for_executable(&executable)?;
    let info = Value::from_file(info_path).ok()?;
    application_group_from_info(&info)
}

pub fn shared_frame_file_path() -> Result<Option<PathBuf>, CameraManError> {
    let Some(application_group) = bundled_application_group() else {
        return Ok(None);
    };
    shared_container_path(&application_group).map(|container| {
        Some(
            container
                .join(SHARED_FRAME_DIRECTORY)
                .join(SHARED_FRAME_FILENAME),
        )
    })
}

fn bundle_info_path_for_executable(executable: &Path) -> Option<PathBuf> {
    let macos = executable.parent()?;
    (macos.file_name()? == "MacOS").then_some(())?;
    let contents = macos.parent()?;
    (contents.file_name()? == "Contents").then_some(())?;
    Some(contents.join("Info.plist"))
}

fn application_group_from_info(info: &Value) -> Option<String> {
    info.as_dictionary()?
        .get(APP_GROUP_INFO_KEY)?
        .as_string()
        .filter(|group| !group.is_empty())
        .map(str::to_owned)
}

#[cfg(target_os = "macos")]
fn shared_container_path(application_group: &str) -> Result<PathBuf, CameraManError> {
    use objc2_foundation::{NSFileManager, NSString};

    let manager = NSFileManager::defaultManager();
    let identifier = NSString::from_str(application_group);
    let container = manager
        .containerURLForSecurityApplicationGroupIdentifier(&identifier)
        .ok_or(CameraManError::VirtualCameraUnavailable(
            "the signed App Group container is unavailable",
        ))?;
    let path = container
        .path()
        .ok_or(CameraManError::VirtualCameraUnavailable(
            "the App Group container has no filesystem path",
        ))?;
    Ok(PathBuf::from(path.to_string()))
}

#[cfg(not(target_os = "macos"))]
fn shared_container_path(_application_group: &str) -> Result<PathBuf, CameraManError> {
    Err(CameraManError::VirtualCameraUnavailable(
        "App Group containers are available only on macOS",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_a_nonempty_string_group() {
        let valid = Value::from_reader_xml(
            br#"<plist version="1.0"><dict><key>CameraManAppGroup</key><string>TEAM.group</string></dict></plist>"#
                .as_slice(),
        )
        .unwrap();
        let empty = Value::from_reader_xml(
            br#"<plist version="1.0"><dict><key>CameraManAppGroup</key><string></string></dict></plist>"#
                .as_slice(),
        )
        .unwrap();

        assert_eq!(
            application_group_from_info(&valid).as_deref(),
            Some("TEAM.group")
        );
        assert_eq!(application_group_from_info(&empty), None);
    }

    #[test]
    fn derives_info_path_only_for_bundle_executables() {
        assert_eq!(
            bundle_info_path_for_executable(Path::new(
                "/Applications/CameraMan.app/Contents/MacOS/CameraMan"
            )),
            Some(PathBuf::from(
                "/Applications/CameraMan.app/Contents/Info.plist"
            ))
        );
        assert_eq!(
            bundle_info_path_for_executable(Path::new("target/debug/camera-man")),
            None
        );
    }
}
