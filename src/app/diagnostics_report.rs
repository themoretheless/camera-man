use super::*;

/// What this run reports about itself: the activation status every panel shows,
/// the full text block the setup panel copies to the clipboard, and the redacted
/// record written to the diagnostics file. All three read the same startup facts
/// and the same accessibility settings, so the three renderings of them live
/// together instead of being kept in sync across `app.rs`.
impl CameraManApp {
    pub(super) fn extension_status(&self) -> ExtensionActivationStatus {
        self.extension_status_override
            .clone()
            .unwrap_or_else(|| self.extension_installer.status())
    }

    pub(super) fn diagnostics_summary(&self) -> String {
        let (profile_status, profile_team_identifier) = match &self.provisioning_profile_status {
            ProvisioningProfileStatus::Missing => ("missing", "unavailable"),
            ProvisioningProfileStatus::Valid {
                team_identifier, ..
            } => ("valid", team_identifier.as_deref().unwrap_or("unavailable")),
            ProvisioningProfileStatus::Invalid { .. } => ("invalid", "unavailable"),
        };
        format!(
            "CameraMan {}\nlaunch={}\nsigning_team_id={}\nprovisioning_profile={}\nprofile_team_id={}\nextension_profile_present={}\nactivation_capable={}\nactivation_capability_reason={}\nactivation_status={}\ntransport={}\ncamera_open_timeout_ms={}\nreduce_motion={}\ndifferentiate_without_color={}\nvoice_over_enabled={}\n",
            env!("CARGO_PKG_VERSION"),
            self.launch_context.label(),
            self.signing_team_identifier
                .as_deref()
                .unwrap_or("unavailable"),
            profile_status,
            profile_team_identifier,
            self.extension_profile_present,
            self.extension_capable,
            self.extension_capability_reason
                .as_deref()
                .unwrap_or("ready"),
            self.extension_status(),
            self.virtual_endpoint,
            camera_open_timeout().as_millis(),
            self.reduce_motion,
            self.differentiate_without_color,
            self.voice_over_enabled,
        )
    }

    fn redacted_diagnostics(&self) -> RedactedDiagnostics {
        let profile_status = match self.provisioning_profile_status {
            ProvisioningProfileStatus::Missing => "missing",
            ProvisioningProfileStatus::Valid { .. } => "valid",
            ProvisioningProfileStatus::Invalid { .. } => "invalid",
        };
        RedactedDiagnostics::new(
            self.launch_context.label(),
            profile_status,
            self.extension_profile_present,
            self.extension_capable,
            self.extension_status().to_string(),
            self.selected_count(),
            self.scenes.len(),
            self.running,
            self.reduce_motion,
            self.high_contrast || self.system_increase_contrast,
            self.differentiate_without_color,
            self.voice_over_enabled,
        )
    }

    pub(super) fn export_diagnostics(&mut self) {
        let path = self.diagnostics_path.clone();
        let diagnostics = self.redacted_diagnostics();
        match self
            .io_worker
            .start_diagnostics_export(diagnostics, path.clone())
        {
            Ok(true) => self.set_event(
                format!("Exporting redacted diagnostics to {}", path.display()),
                false,
            ),
            Ok(false) => self.set_event("Another file operation is already running", true),
            Err(error) => self.set_event(
                format!("Diagnostics export failed. {error} Check the destination and retry."),
                true,
            ),
        }
    }
}
