use camera_man::provisioning_profile::{self, ProfileMetadata};
pub(crate) use camera_man::provisioning_profile::{
    decoded_entitlement_strings, decoded_entitlements_grant,
};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
const APPLICATION_GROUPS_ENTITLEMENT: &str = "com.apple.security.application-groups";

#[derive(Debug, Clone)]
pub(crate) struct ValidatedProvisioningProfile {
    path: PathBuf,
    team_identifier: Option<String>,
    application_groups: Vec<String>,
}

impl ValidatedProvisioningProfile {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn team_identifier(&self) -> Option<&str> {
        self.team_identifier.as_deref()
    }

    pub(crate) fn application_groups(&self) -> &[String] {
        &self.application_groups
    }
}

pub(crate) fn validate(
    path: &Path,
    app_bundle_id: &str,
    required_entitlement: &str,
) -> Result<ValidatedProvisioningProfile, Box<dyn Error>> {
    if !path.is_file() {
        return Err(format!("provisioning profile not found: {}", path.display()).into());
    }

    let output = Command::new("/usr/bin/security")
        .args(["cms", "-D", "-i"])
        .arg(path)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "failed to decode provisioning profile {} with security cms: {}",
            path.display(),
            stderr.trim()
        )
        .into());
    }

    let metadata =
        validate_decoded_profile(&output.stdout, path, app_bundle_id, required_entitlement)?;

    Ok(ValidatedProvisioningProfile {
        path: path.to_path_buf(),
        team_identifier: metadata.team_identifier,
        application_groups: metadata.application_groups,
    })
}

fn validate_decoded_profile(
    decoded: &[u8],
    path: &Path,
    app_bundle_id: &str,
    required_entitlement: &str,
) -> Result<ProfileMetadata, Box<dyn Error>> {
    provisioning_profile::validate_decoded_profile(decoded, app_bundle_id, required_entitlement)
        .map_err(|error| format!("provisioning profile {}: {error}", path.display()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLE_ID: &str = "com.cameraman.rust";
    const INSTALL_ENTITLEMENT: &str = "com.apple.developer.system-extension.install";
    const TEST_PATH: &str = "test.mobileprovision";

    #[test]
    fn accepts_legacy_application_identifier() {
        validate_xml(&profile_xml(
            "application-identifier",
            "TEAM123456.com.cameraman.rust",
            "<true/>",
        ))
        .unwrap();
    }

    #[test]
    fn accepts_modern_application_identifier() {
        validate_xml(&profile_xml(
            "com.apple.application-identifier",
            "TEAM123456.com.cameraman.rust",
            "<true/>",
        ))
        .unwrap();
    }

    #[test]
    fn rejects_missing_install_entitlement() {
        let xml = r#"
<plist version="1.0">
<dict>
  <key>Entitlements</key>
  <dict>
    <key>application-identifier</key>
    <string>TEAM123456.com.cameraman.rust</string>
  </dict>
</dict>
</plist>
"#;

        let error = validate_xml(xml).unwrap_err().to_string();
        assert!(error.contains(INSTALL_ENTITLEMENT));
    }

    #[test]
    fn rejects_string_instead_of_boolean_entitlement() {
        let error = validate_xml(&profile_xml(
            "application-identifier",
            "TEAM123456.com.cameraman.rust",
            "<string>true</string>",
        ))
        .unwrap_err()
        .to_string();

        assert!(error.contains("does not grant"));
    }

    #[test]
    fn ignores_matching_values_outside_entitlements() {
        let xml = r#"
<plist version="1.0">
<dict>
  <key>application-identifier</key>
  <string>TEAM123456.com.cameraman.rust</string>
  <key>com.apple.developer.system-extension.install</key>
  <true/>
  <key>Entitlements</key>
  <dict>
    <key>com.apple.security.app-sandbox</key>
    <true/>
  </dict>
</dict>
</plist>
"#;

        let error = validate_xml(xml).unwrap_err().to_string();
        assert!(error.contains("does not grant"));
    }

    #[test]
    fn rejects_mismatched_bundle_id() {
        let error = validate_xml(&profile_xml(
            "application-identifier",
            "TEAM123456.com.example.other",
            "<true/>",
        ))
        .unwrap_err()
        .to_string();

        assert!(error.contains(BUNDLE_ID));
        assert!(error.contains("TEAM123456.com.example.other"));
    }

    #[test]
    fn decoded_entitlements_require_boolean_true() {
        let granted = format!(
            r#"<plist version="1.0"><dict><key>{INSTALL_ENTITLEMENT}</key><true/></dict></plist>"#
        );
        let string_value = format!(
            r#"<plist version="1.0"><dict><key>{INSTALL_ENTITLEMENT}</key><string>true</string></dict></plist>"#
        );

        assert!(decoded_entitlements_grant(
            granted.as_bytes(),
            INSTALL_ENTITLEMENT
        ));
        assert!(!decoded_entitlements_grant(
            string_value.as_bytes(),
            INSTALL_ENTITLEMENT
        ));
        assert!(!decoded_entitlements_grant(
            b"not a plist",
            INSTALL_ENTITLEMENT
        ));
    }

    #[test]
    fn extracts_explicit_profile_team_identifier() {
        let metadata = validate_xml_metadata(&profile_xml(
            "application-identifier",
            "TEAM123456.com.cameraman.rust",
            "<true/>",
        ))
        .unwrap();

        assert_eq!(metadata.team_identifier.as_deref(), Some("TEAM123456"));
    }

    #[test]
    fn rejects_conflicting_team_identifiers() {
        let xml = format!(
            r#"
<plist version="1.0">
<dict>
  <key>TeamIdentifier</key>
  <array><string>OTHERTEAM</string></array>
  <key>Entitlements</key>
  <dict>
    <key>com.apple.developer.team-identifier</key>
    <string>TEAM123456</string>
    <key>application-identifier</key>
    <string>TEAM123456.com.cameraman.rust</string>
    <key>{INSTALL_ENTITLEMENT}</key>
    <true/>
  </dict>
</dict>
</plist>
"#
        );

        let error = validate_xml_metadata(&xml).unwrap_err().to_string();
        assert!(error.contains("conflicting Team IDs"));
        assert!(error.contains("OTHERTEAM"));
        assert!(error.contains("TEAM123456"));
    }

    #[test]
    fn extracts_and_deduplicates_application_groups() {
        let xml = format!(
            r#"
<plist version="1.0">
<dict>
  <key>TeamIdentifier</key>
  <array><string>TEAM123456</string></array>
  <key>Entitlements</key>
  <dict>
    <key>application-identifier</key>
    <string>TEAM123456.com.cameraman.rust</string>
    <key>{INSTALL_ENTITLEMENT}</key>
    <true/>
    <key>{APPLICATION_GROUPS_ENTITLEMENT}</key>
    <array>
      <string>TEAM123456.com.cameraman.shared</string>
      <string>TEAM123456.com.cameraman.shared</string>
    </array>
  </dict>
</dict>
</plist>
"#
        );

        let metadata = validate_xml_metadata(&xml).unwrap();
        assert_eq!(
            metadata.application_groups,
            ["TEAM123456.com.cameraman.shared"]
        );
    }

    #[test]
    fn rejects_non_array_application_groups() {
        let xml = format!(
            r#"
<plist version="1.0">
<dict>
  <key>Entitlements</key>
  <dict>
    <key>application-identifier</key>
    <string>TEAM123456.com.cameraman.rust</string>
    <key>{INSTALL_ENTITLEMENT}</key>
    <true/>
    <key>{APPLICATION_GROUPS_ENTITLEMENT}</key>
    <string>TEAM123456.com.cameraman.shared</string>
  </dict>
</dict>
</plist>
"#
        );

        let error = validate_xml_metadata(&xml).unwrap_err().to_string();
        assert!(error.contains(APPLICATION_GROUPS_ENTITLEMENT));
        assert!(error.contains("expected an array"));
    }

    fn validate_xml(xml: &str) -> Result<(), Box<dyn Error>> {
        validate_xml_metadata(xml).map(|_| ())
    }

    fn validate_xml_metadata(xml: &str) -> Result<ProfileMetadata, Box<dyn Error>> {
        validate_decoded_profile(
            xml.as_bytes(),
            Path::new(TEST_PATH),
            BUNDLE_ID,
            INSTALL_ENTITLEMENT,
        )
    }

    fn profile_xml(identifier_key: &str, identifier: &str, entitlement_value: &str) -> String {
        format!(
            r#"
<plist version="1.0">
<dict>
  <key>TeamIdentifier</key>
  <array><string>TEAM123456</string></array>
  <key>Entitlements</key>
  <dict>
    <key>{identifier_key}</key>
    <string>{identifier}</string>
    <key>{INSTALL_ENTITLEMENT}</key>
    {entitlement_value}
  </dict>
</dict>
</plist>
"#
        )
    }
}
