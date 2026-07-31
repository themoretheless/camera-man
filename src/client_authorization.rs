//! Signing-identity policy for CoreMediaIO extension clients.
//!
//! CameraMan is a general-purpose virtual camera: arbitrary legitimate
//! consumers (QuickTime, Zoom, browsers, Control Center preview) must keep
//! working, so an app allowlist is the wrong shape. macOS TCC already gates
//! camera access per consumer app; this layer is the extension's own defense
//! in depth, not the primary consent mechanism. The policy is therefore:
//! allow any client whose code-signing identity CMIO can establish, deny a
//! client whose signing identity cannot be established, and treat pid as
//! audit data only, never a policy input. Every consumer TCC could identify
//! well enough to grant camera access carries a signing identifier, so the
//! deny branch rejects only processes that hide who they are; the local
//! analogue of "network reachability is never identity" from
//! `docs/threat-model-remote-source.md` is that mach reachability is not
//! identity either.

/// Identity facts CMIO established for one connecting client process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIdentity {
    /// CMIO-minted per-connection UUID; opaque, stable only for the connection.
    pub client_id: String,
    /// Code-signing identifier reported by CMIO, when one exists.
    pub signing_id: Option<String>,
    /// Client process id, when CMIO reported a positive one.
    pub pid: Option<i32>,
}

impl ClientIdentity {
    /// Signing id usable as identity: present and not blank.
    pub fn established_signing_id(&self) -> Option<&str> {
        self.signing_id
            .as_deref()
            .map(str::trim)
            .filter(|signing_id| !signing_id.is_empty())
    }

    /// One-line audit form; absent fields print an explicit "unknown"
    /// instead of disappearing from the log.
    pub fn audit_line(&self) -> String {
        let signing_id = self.established_signing_id().unwrap_or("unknown");
        let pid = self
            .pid
            .map_or_else(|| String::from("unknown"), |pid| pid.to_string());
        format!(
            "client {}, signing id {}, pid {}",
            self.client_id, signing_id, pid
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientDenialReason {
    /// CMIO reported no usable code-signing identity for the process.
    UnidentifiedSigning,
}

impl ClientDenialReason {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::UnidentifiedSigning => "no code-signing identity could be established",
        }
    }
}

impl std::fmt::Display for ClientDenialReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.reason())
    }
}

impl std::error::Error for ClientDenialReason {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientAuthorization {
    Allow,
    Deny(ClientDenialReason),
}

/// Single policy consulted by both `connectClient:error:` and
/// `authorizedToStartStreamForClient:`. See the module documentation for why
/// identified clients pass without an allowlist and unidentifiable ones are
/// denied rather than allowed with an audit trail.
pub fn authorize_client(identity: &ClientIdentity) -> ClientAuthorization {
    match identity.established_signing_id() {
        Some(_) => ClientAuthorization::Allow,
        None => ClientAuthorization::Deny(ClientDenialReason::UnidentifiedSigning),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(signing_id: Option<&str>, pid: Option<i32>) -> ClientIdentity {
        ClientIdentity {
            client_id: String::from("6F9619FF-8B86-D011-B42D-00CF4FC964FF"),
            signing_id: signing_id.map(String::from),
            pid,
        }
    }

    #[test]
    fn signed_consumer_is_allowed() {
        let identity = identity(Some("com.apple.quicktimeplayerx"), Some(4242));
        assert_eq!(authorize_client(&identity), ClientAuthorization::Allow);
    }

    #[test]
    fn missing_signing_identity_is_denied() {
        let identity = identity(None, Some(4242));
        assert_eq!(
            authorize_client(&identity),
            ClientAuthorization::Deny(ClientDenialReason::UnidentifiedSigning)
        );
    }

    #[test]
    fn blank_signing_identity_is_denied() {
        let identity = identity(Some("  "), Some(4242));
        assert_eq!(
            authorize_client(&identity),
            ClientAuthorization::Deny(ClientDenialReason::UnidentifiedSigning)
        );
    }

    #[test]
    fn pid_is_audit_data_not_policy_input() {
        let signed_without_pid = identity(Some("com.example.consumer"), None);
        assert_eq!(
            authorize_client(&signed_without_pid),
            ClientAuthorization::Allow
        );

        let unsigned_with_pid = identity(None, Some(1));
        assert_eq!(
            authorize_client(&unsigned_with_pid),
            ClientAuthorization::Deny(ClientDenialReason::UnidentifiedSigning)
        );
    }

    #[test]
    fn audit_line_reports_unknown_fields_explicitly() {
        let unidentified = identity(None, None);
        let line = unidentified.audit_line();
        assert!(line.contains("6F9619FF-8B86-D011-B42D-00CF4FC964FF"));
        assert!(line.contains("signing id unknown"));
        assert!(line.contains("pid unknown"));

        let identified = identity(Some("com.example.consumer"), Some(4242));
        let line = identified.audit_line();
        assert!(line.contains("signing id com.example.consumer"));
        assert!(line.contains("pid 4242"));
    }

    #[test]
    fn denial_reason_has_stable_wording() {
        let reason = ClientDenialReason::UnidentifiedSigning;
        assert!(reason.reason().contains("signing"));
        assert_eq!(reason.to_string(), reason.reason());
    }
}
