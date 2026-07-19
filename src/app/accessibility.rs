#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct AccessibilitySettings {
    pub(super) reduce_motion: bool,
    pub(super) increase_contrast: bool,
    pub(super) differentiate_without_color: bool,
    pub(super) voice_over_enabled: bool,
}

pub(super) fn progress_indicator(
    ui: &mut eframe::egui::Ui,
    reduce_motion: bool,
    accessible_label: &str,
) -> eframe::egui::Response {
    let response = if reduce_motion {
        ui.add_sized(
            [18.0, 18.0],
            eframe::egui::Label::new(eframe::egui::RichText::new("...").strong()),
        )
    } else {
        ui.add(eframe::egui::Spinner::new().size(14.0))
    };
    response.widget_info(|| {
        eframe::egui::WidgetInfo::labeled(
            eframe::egui::WidgetType::ProgressIndicator,
            true,
            accessible_label,
        )
    });
    response
}

#[cfg(target_os = "macos")]
pub(super) fn system_accessibility_settings() -> AccessibilitySettings {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    AccessibilitySettings {
        reduce_motion: workspace.accessibilityDisplayShouldReduceMotion(),
        increase_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
        differentiate_without_color: workspace
            .accessibilityDisplayShouldDifferentiateWithoutColor(),
        voice_over_enabled: workspace.isVoiceOverEnabled(),
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) const fn system_accessibility_settings() -> AccessibilitySettings {
    AccessibilitySettings {
        reduce_motion: false,
        increase_contrast: false,
        differentiate_without_color: false,
        voice_over_enabled: false,
    }
}
