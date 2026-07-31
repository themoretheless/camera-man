use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use plist::{Dictionary, Value};

mod app;
mod app_preferences;
#[path = "main/bundle.rs"]
mod bundle;
mod camera_discovery_worker;
#[path = "main/cli.rs"]
mod cli;
#[path = "main/diagnostics.rs"]
mod diagnostics;
mod media_clock;
mod provisioning;
mod render_worker;
#[path = "main/signing.rs"]
mod signing;
#[path = "main/typed_plist.rs"]
mod typed_plist;

use bundle::*;
use cli::*;
use diagnostics::*;
use signing::*;
use typed_plist::*;

const APP_BUNDLE_ID: &str = "com.cameraman.rust";
const SYSTEM_EXTENSION_INSTALL_ENTITLEMENT: &str = "com.apple.developer.system-extension.install";
const EXTENSION_APP_SANDBOX_ENTITLEMENT: &str = "com.apple.security.app-sandbox";
const APPLICATION_GROUPS_ENTITLEMENT: &str = "com.apple.security.application-groups";

use camera_man::FrameSource;
use camera_man::{
    APP_GROUP_INFO_KEY, CameraDiscovery, CompositionLayout, Compositor, EXTENSION_BUNDLE_ID,
    FrameTransportMode, FrameTransportSink, NokhwaCameraDiscovery, PixelFormat,
    SyntheticFrameSource, VideoFormat, VirtualCameraConfig, camera_open_timeout,
    capture_one_with_timeout, default_frame_limits, write_ppm,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        None | Some("app") => app::run()?,
        Some("status") => print_status(),
        Some("demo") | Some("--demo") => run_demo(args.get(1).map(PathBuf::from))?,
        Some("list-cameras") => list_cameras()?,
        Some("capture-demo") => capture_demo(args.get(1).map(PathBuf::from))?,
        Some("bundle") if args.get(1).is_some_and(|arg| is_help_flag(arg)) => print_bundle_help(),
        Some("bundle") => bundle_app()?,
        Some("bundle-extension") => bundle_extension()?,
        Some("check") => run_check(),
        Some("version") => print_version(),
        Some("diagnose-extension") => diagnose_extension(),
        Some("help") | Some("--help") | Some("-h") => print_help(),
        Some(command) => {
            eprintln!("Unknown command: {command}");
            print_help();
            std::process::exit(2);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_APP_GROUP: &str = "TEAM123456.com.cameraman.shared";
    const TEST_MACH_SERVICE: &str = "TEAM123456.com.cameraman.shared.cmio";

    #[test]
    fn generated_app_plists_use_shared_constants() {
        let info = app_info_plist(Some(TEST_APP_GROUP));
        let bundle_id = info
            .as_dictionary()
            .and_then(|dictionary| dictionary.get("CFBundleIdentifier"))
            .and_then(plist::Value::as_string);
        assert_eq!(bundle_id, Some(APP_BUNDLE_ID));
        assert_eq!(
            info.as_dictionary()
                .and_then(|dictionary| dictionary.get(APP_GROUP_INFO_KEY))
                .and_then(Value::as_string),
            Some(TEST_APP_GROUP)
        );

        assert!(provisioning::decoded_entitlements_grant(
            &plist_xml_bytes(&app_entitlements_plist(TEST_APP_GROUP)),
            SYSTEM_EXTENSION_INSTALL_ENTITLEMENT,
        ));
        assert_eq!(
            provisioning::decoded_entitlement_strings(
                &plist_xml_bytes(&app_entitlements_plist(TEST_APP_GROUP)),
                APPLICATION_GROUPS_ENTITLEMENT,
            ),
            [TEST_APP_GROUP]
        );
        assert!(
            app_info_plist(None)
                .as_dictionary()
                .is_some_and(|dictionary| !dictionary.contains_key(APP_GROUP_INFO_KEY))
        );
    }

    #[test]
    fn generated_extension_plist_uses_shared_identity_and_version() {
        let info = extension_info_plist(TEST_MACH_SERVICE);
        let dictionary = info.as_dictionary().unwrap();
        assert_eq!(
            dictionary
                .get("CFBundleIdentifier")
                .and_then(Value::as_string),
            Some(EXTENSION_BUNDLE_ID)
        );
        assert_eq!(
            dictionary
                .get("CFBundleShortVersionString")
                .and_then(Value::as_string),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(
            dictionary
                .get("CMIOExtension")
                .and_then(Value::as_dictionary)
                .and_then(|cmio| cmio.get("CMIOExtensionMachServiceName"))
                .and_then(Value::as_string),
            Some(TEST_MACH_SERVICE)
        );
        assert_eq!(
            dictionary
                .get(APP_GROUP_INFO_KEY)
                .and_then(Value::as_string),
            Some(TEST_APP_GROUP)
        );
        assert!(
            dictionary
                .get("NSSystemExtensionUsageDescription")
                .and_then(Value::as_string)
                .is_some_and(|description| !description.is_empty())
        );
        assert!(provisioning::decoded_entitlements_grant(
            &plist_xml_bytes(&extension_entitlements_plist(Some(TEST_APP_GROUP))),
            EXTENSION_APP_SANDBOX_ENTITLEMENT,
        ));
        assert_eq!(
            provisioning::decoded_entitlement_strings(
                &plist_xml_bytes(&extension_entitlements_plist(Some(TEST_APP_GROUP))),
                APPLICATION_GROUPS_ENTITLEMENT,
            ),
            [TEST_APP_GROUP]
        );
        assert!(
            extension_info_plist(EXTENSION_BUNDLE_ID)
                .as_dictionary()
                .is_some_and(|dictionary| !dictionary.contains_key(APP_GROUP_INFO_KEY))
        );
    }

    #[test]
    fn generated_plists_round_trip_as_xml() {
        for value in [
            app_info_plist(Some(TEST_APP_GROUP)),
            app_entitlements_plist(TEST_APP_GROUP),
            extension_info_plist(TEST_MACH_SERVICE),
            extension_entitlements_plist(Some(TEST_APP_GROUP)),
        ] {
            let bytes = plist_xml_bytes(&value);
            let decoded = Value::from_reader_xml(bytes.as_slice()).unwrap();
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn extension_paths_select_the_requested_profile_and_bundle_location() {
        assert_eq!(
            extension_binary_path(false),
            PathBuf::from("target/debug/cameraman-extension")
        );
        assert_eq!(
            extension_binary_path(true),
            PathBuf::from("target/release/cameraman-extension")
        );
        assert_eq!(
            embedded_extension_bundle_path(Path::new("target/CameraMan.app/Contents")),
            PathBuf::from(format!(
                "target/CameraMan.app/Contents/Library/SystemExtensions/{EXTENSION_BUNDLE_ID}.systemextension"
            ))
        );
    }

    #[test]
    fn signing_failures_are_strict_for_release_or_real_identity() {
        assert!(!signing_failure_is_fatal(false, AD_HOC_IDENTITY));
        assert!(signing_failure_is_fatal(true, AD_HOC_IDENTITY));
        assert!(signing_failure_is_fatal(
            false,
            "Apple Development: Example"
        ));
    }

    #[test]
    fn installable_bundle_requires_identity_and_both_profiles() {
        assert!(
            validate_bundle_signing_configuration(AD_HOC_IDENTITY, false, false, false).is_ok()
        );
        assert!(
            validate_bundle_signing_configuration(
                "Apple Development: Example",
                false,
                false,
                false
            )
            .is_ok()
        );
        assert!(
            validate_bundle_signing_configuration("Apple Development: Example", true, true, false)
                .is_ok()
        );
        assert!(validate_bundle_signing_configuration(AD_HOC_IDENTITY, true, true, false).is_err());
        assert!(
            validate_bundle_signing_configuration("Apple Development: Example", true, false, false)
                .is_err()
        );
        assert!(
            validate_bundle_signing_configuration("Apple Development: Example", false, true, false)
                .is_err()
        );
        assert!(
            validate_bundle_signing_configuration("Apple Development: Example", false, false, true)
                .is_err()
        );
    }

    #[test]
    fn app_group_selection_requires_a_common_exact_value() {
        let app = vec![
            String::from("TEAM.group.shared"),
            String::from("TEAM.group.other"),
            String::from("TEAM.*"),
        ];
        let extension = vec![String::from("TEAM.group.shared"), String::from("TEAM.*")];

        assert_eq!(
            select_shared_application_group_values(&app, &extension, None).unwrap(),
            "TEAM.group.shared"
        );
        assert!(
            select_shared_application_group_values(&app, &extension, Some("TEAM.group.other"))
                .is_err()
        );
        assert!(select_shared_application_group_values(&app, &[], None).is_err());
    }

    #[test]
    fn mach_service_uses_a_nonempty_app_group_prefix() {
        assert_eq!(
            extension_mach_service_name(Some(TEST_APP_GROUP)),
            TEST_MACH_SERVICE
        );
        assert!(mach_service_has_application_group_prefix(
            TEST_MACH_SERVICE,
            TEST_APP_GROUP
        ));
        assert!(!mach_service_has_application_group_prefix(
            TEST_APP_GROUP,
            TEST_APP_GROUP
        ));
        assert!(!mach_service_has_application_group_prefix(
            "OTHER.group.cmio",
            TEST_APP_GROUP
        ));
    }

    #[test]
    fn codesign_context_contains_identity_entitlements_and_target() {
        let context = codesign_command_context(
            "Apple Development: Example",
            Path::new("target/CameraMan.app"),
            Some(Path::new("target/signing/CameraMan.entitlements")),
        );

        assert!(context.contains("Apple Development: Example"));
        assert!(context.contains("--timestamp --options runtime"));
        assert!(context.contains("CameraMan.entitlements"));
        assert!(context.contains("target/CameraMan.app"));

        let ad_hoc =
            codesign_command_context(AD_HOC_IDENTITY, Path::new("target/CameraMan.app"), None);
        assert!(ad_hoc.contains("--timestamp=none"));
        assert!(!ad_hoc.contains("--options runtime"));
    }

    #[test]
    fn codesign_team_identifier_parser_handles_real_and_ad_hoc_signatures() {
        assert_eq!(
            parse_codesign_team_identifier(b"Identifier=com.example\nTeamIdentifier=TEAM123456\n"),
            Some(String::from("TEAM123456"))
        );
        assert_eq!(
            parse_codesign_team_identifier(b"Signature=adhoc\nTeamIdentifier=not set\n"),
            None
        );
    }

    #[test]
    fn team_identifier_match_requires_equal_nonempty_values() {
        assert!(validate_team_identifier_match(Some("TEAM1"), Some("TEAM1")).is_ok());
        assert!(validate_team_identifier_match(Some("TEAM1"), Some("TEAM2")).is_err());
        assert!(validate_team_identifier_match(None, Some("TEAM1")).is_err());
        assert_eq!(
            team_identifier_match_status(Some("TEAM1"), Some("TEAM2")),
            "false"
        );
        assert_eq!(
            team_identifier_match_status(None, Some("TEAM2")),
            "unavailable"
        );
    }
}
