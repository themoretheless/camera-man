use std::fmt;

use plist::{Dictionary, Value};

use crate::parser_limits::{PROFILE_PARSER_LIMITS, validate_input_size};

const APP_IDENTIFIER_KEYS: [&str; 2] =
    ["application-identifier", "com.apple.application-identifier"];
const TEAM_IDENTIFIER_ENTITLEMENT: &str = "com.apple.developer.team-identifier";
const APPLICATION_GROUPS_ENTITLEMENT: &str = "com.apple.security.application-groups";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileMetadata {
    pub team_identifier: Option<String>,
    pub application_groups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisioningProfileError(String);

impl fmt::Display for ProvisioningProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProvisioningProfileError {}

pub fn validate_decoded_profile(
    decoded: &[u8],
    app_bundle_id: &str,
    required_entitlement: &str,
) -> Result<ProfileMetadata, ProvisioningProfileError> {
    validate_input_size(decoded, PROFILE_PARSER_LIMITS)
        .map_err(|error| ProvisioningProfileError(format!("profile parser limit: {error}")))?;
    let profile = Value::from_reader_xml(decoded)
        .map_err(|error| ProvisioningProfileError(format!("invalid profile plist: {error}")))?;
    let root = profile.as_dictionary().ok_or_else(|| {
        ProvisioningProfileError(String::from("profile has a non-dictionary root"))
    })?;
    let entitlements = root
        .get("Entitlements")
        .and_then(Value::as_dictionary)
        .ok_or_else(|| ProvisioningProfileError(String::from("profile has no Entitlements")))?;

    if !dictionary_grants(entitlements, required_entitlement) {
        return Err(ProvisioningProfileError(format!(
            "profile does not grant {required_entitlement}"
        )));
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
        return Err(ProvisioningProfileError(format!(
            "profile does not match app bundle id {app_bundle_id}; found: {}",
            if app_identifiers.is_empty() {
                String::from("<none>")
            } else {
                app_identifiers.join(", ")
            }
        )));
    }

    Ok(ProfileMetadata {
        team_identifier: profile_team_identifier(root, entitlements)?,
        application_groups: entitlement_strings(entitlements, APPLICATION_GROUPS_ENTITLEMENT)
            .map_err(|error| {
                ProvisioningProfileError(format!(
                    "invalid {APPLICATION_GROUPS_ENTITLEMENT}: {error}"
                ))
            })?,
    })
}

pub fn decoded_entitlements_grant(decoded: &[u8], entitlement: &str) -> bool {
    if validate_input_size(decoded, PROFILE_PARSER_LIMITS).is_err() {
        return false;
    }
    Value::from_reader_xml(decoded)
        .ok()
        .and_then(Value::into_dictionary)
        .is_some_and(|entitlements| dictionary_grants(&entitlements, entitlement))
}

pub fn decoded_entitlement_strings(decoded: &[u8], entitlement: &str) -> Vec<String> {
    if validate_input_size(decoded, PROFILE_PARSER_LIMITS).is_err() {
        return Vec::new();
    }
    Value::from_reader_xml(decoded)
        .ok()
        .and_then(Value::into_dictionary)
        .and_then(|entitlements| entitlement_strings(&entitlements, entitlement).ok())
        .unwrap_or_default()
}

fn dictionary_grants(dictionary: &Dictionary, entitlement: &str) -> bool {
    dictionary.get(entitlement).and_then(Value::as_boolean) == Some(true)
}

fn profile_team_identifier(
    root: &Dictionary,
    entitlements: &Dictionary,
) -> Result<Option<String>, ProvisioningProfileError> {
    let mut candidates = Vec::new();
    if let Some(values) = root.get("TeamIdentifier").and_then(Value::as_array) {
        candidates.extend(values.iter().filter_map(Value::as_string));
    }
    if let Some(value) = entitlements
        .get(TEAM_IDENTIFIER_ENTITLEMENT)
        .and_then(Value::as_string)
    {
        candidates.push(value);
    }
    let mut unique = Vec::new();
    for candidate in candidates {
        if !candidate.is_empty() && !unique.contains(&candidate) {
            unique.push(candidate);
        }
    }
    if unique.len() > 1 {
        return Err(ProvisioningProfileError(format!(
            "profile contains conflicting Team IDs: {}",
            unique.join(", ")
        )));
    }
    Ok(unique.first().map(|team_id| (*team_id).to_owned()))
}

fn entitlement_strings(
    entitlements: &Dictionary,
    entitlement: &str,
) -> Result<Vec<String>, &'static str> {
    let Some(value) = entitlements.get(entitlement) else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_array() else {
        return Err("expected an array");
    };
    if values.len() > PROFILE_PARSER_LIMITS.max_elements {
        return Err("array exceeds the element limit");
    }
    let mut strings = Vec::with_capacity(values.len());
    for value in values {
        let Some(value) = value.as_string() else {
            return Err("array contains a non-string value");
        };
        if value.len() > PROFILE_PARSER_LIMITS.max_string_bytes {
            return Err("array string exceeds the byte limit");
        }
        if !value.is_empty() && !strings.iter().any(|existing| existing == value) {
            strings.push(value.to_owned());
        }
    }
    Ok(strings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_inputs_never_receive_entitlements() {
        assert!(!decoded_entitlements_grant(
            b"not a plist",
            "com.apple.security.app-sandbox"
        ));
        assert!(decoded_entitlement_strings(b"<plist/>", "group").is_empty());
    }

    #[test]
    fn oversized_profile_is_rejected_before_plist_deserialization() {
        let oversized = vec![b' '; PROFILE_PARSER_LIMITS.max_input_bytes + 1];
        let error = validate_decoded_profile(&oversized, "com.example.app", "required")
            .unwrap_err()
            .to_string();
        assert!(error.contains("parser limit"));
        assert!(!decoded_entitlements_grant(&oversized, "required"));
    }
}
