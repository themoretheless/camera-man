use eframe::egui;

pub(super) const COLOR_WINDOW: egui::Color32 = egui::Color32::from_rgb(24, 27, 31);
pub(super) const COLOR_PANEL: egui::Color32 = egui::Color32::from_rgb(30, 34, 38);
pub(super) const COLOR_WIDGET: egui::Color32 = egui::Color32::from_rgb(48, 54, 60);
pub(super) const COLOR_WIDGET_HOVER: egui::Color32 = egui::Color32::from_rgb(62, 72, 82);
pub(super) const COLOR_WIDGET_ACTIVE: egui::Color32 = egui::Color32::from_rgb(67, 115, 132);
pub(super) const COLOR_ACCENT: egui::Color32 = egui::Color32::from_rgb(64, 132, 158);
pub(super) const COLOR_FOCUS: egui::Color32 = egui::Color32::from_rgb(0, 142, 198);
pub(super) const COLOR_STOP: egui::Color32 = egui::Color32::from_rgb(160, 68, 68);
pub(super) const COLOR_OK: egui::Color32 = egui::Color32::from_rgb(112, 176, 128);
pub(super) const COLOR_ERROR: egui::Color32 = egui::Color32::from_rgb(224, 108, 108);
pub(super) const COLOR_WARNING: egui::Color32 = egui::Color32::from_rgb(214, 172, 96);
pub(super) const COLOR_DIM: egui::Color32 = egui::Color32::from_gray(150);
pub(super) const COLOR_PREVIEW_BACKGROUND: egui::Color32 = egui::Color32::BLACK;

pub(super) fn configure_style(ctx: &egui::Context, high_contrast: bool) {
    ctx.set_visuals(egui::Visuals::dark());
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.visuals.panel_fill = COLOR_PANEL;
        style.visuals.window_fill = COLOR_WINDOW;
        style.visuals.widgets.inactive.bg_fill = COLOR_WIDGET;
        style.visuals.widgets.hovered.bg_fill = COLOR_WIDGET_HOVER;
        style.visuals.widgets.active.bg_fill = COLOR_WIDGET_ACTIVE;
        if high_contrast {
            style.visuals.override_text_color = Some(egui::Color32::WHITE);
            style.visuals.selection.bg_fill = COLOR_FOCUS;
            style.visuals.selection.stroke = egui::Stroke::new(2.0, egui::Color32::WHITE);
            style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.5, egui::Color32::WHITE);
            style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(2.0, egui::Color32::WHITE);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_palette_is_distinct_and_not_one_hue() {
        assert!(COLOR_OK.g() > COLOR_OK.r() && COLOR_OK.g() > COLOR_OK.b());
        assert!(COLOR_ERROR.r() > COLOR_ERROR.g() && COLOR_ERROR.r() > COLOR_ERROR.b());
        assert!(COLOR_WARNING.r() > COLOR_WARNING.b());
        assert!(COLOR_ACCENT.b() > COLOR_ACCENT.r());

        for foreground in [COLOR_OK, COLOR_ERROR, COLOR_WARNING, COLOR_ACCENT] {
            assert!(contrast_ratio(foreground, COLOR_PANEL) >= 3.0);
        }
    }

    fn contrast_ratio(left: egui::Color32, right: egui::Color32) -> f32 {
        let left = relative_luminance(left);
        let right = relative_luminance(right);
        (left.max(right) + 0.05) / (left.min(right) + 0.05)
    }

    fn relative_luminance(color: egui::Color32) -> f32 {
        let channel = |value: u8| {
            let value = f32::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }
}
