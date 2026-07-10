use plist::{Dictionary, Value};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

const APP_IDENTIFIER_KEYS: [&str; 2] =
    ["application-identifier", "com.apple.application-identifier"];

#[derive(Debug, Clone)]
pub(crate) struct ValidatedProvisioningProfile {
    path: PathBuf,
}

impl ValidatedProvisioningProfile {
    pub(crate) fn path(&self) -> &Path {
        &self.path
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

    validate_decoded_profile(&output.stdout, path, app_bundle_id, required_entitlement)?;

    Ok(ValidatedProvisioningProfile {
        path: path.to_path_buf(),
    })
}

fn validate_decoded_profile(
    decoded: &[u8],
    path: &Path,
    app_bundle_id: &str,
    required_entitlement: &str,
) -> Result<(), Box<dyn Error>> {
    let profile = Value::from_reader_xml(decoded).map_err(|error| -> Box<dyn Error> {
        format!(
            "decoded provisioning profile {} is not a valid plist: {error}",
            path.display()
        )
        .into()
    })?;
    let Some(root) = profile.as_dictionary() else {
        return Err(format!(
            "decoded provisioning profile {} has a non-dictionary root",
            path.display()
        )
        .into());
    };
    let entitlements = profile_entitlements(root, path)?;

    if !dictionary_grants(entitlements, required_entitlement) {
        return Err(format!(
            "provisioning profile {} does not grant {required_entitlement}",
            path.display()
        )
        .into());
    }

    let app_identifiers = APP_IDENTIFIER_KEYS
        .iter()
        .filter_map(|key| entitlements.get(key).and_then(Value::as_string))
        .collect::<Vec<_>>();
    let expected_suffix = format!(".{app_bundle_id}");
    if !app_identifiers
        .iter()
        .any(|identifier| *identifier == app_bundle_id || identifier.ends_with(&expected_suffix))
    {
        return Err(format!(
            "provisioning profile {} does not match app bundle id {app_bundle_id}; found application identifiers: {}",
            path.display(),
            if app_identifiers.is_empty() {
                String::from("<none>")
            } else {
                app_identifiers.join(", ")
            }
        )
        .into());
    }

    Ok(())
}

pub(crate) fn decoded_entitlements_grant(decoded: &[u8], entitlement: &str) -> bool {
    Value::from_reader_xml(decoded)
        .ok()
        .and_then(Value::into_dictionary)
        .is_some_and(|entitlements| dictionary_grants(&entitlements, entitlement))
}

fn dictionary_grants(dictionary: &Dictionary, entitlement: &str) -> bool {
    dictionary.get(entitlement).and_then(Value::as_boolean) == Some(true)
}

fn profile_entitlements<'a>(
    root: &'a Dictionary,
    path: &Path,
) -> Result<&'a Dictionary, Box<dyn Error>> {
    root.get("Entitlements")
        .and_then(Value::as_dictionary)
        .ok_or_else(|| {
            format!(
                "decoded provisioning profile {} has no Entitlements dictionary",
                path.display()
            )
            .into()
        })
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

    fn validate_xml(xml: &str) -> Result<(), Box<dyn Error>> {
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
