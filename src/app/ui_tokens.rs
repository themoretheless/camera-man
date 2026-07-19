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
