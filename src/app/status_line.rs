use super::*;

/// The one line that says what the app is doing right now: a colour and a
/// sentence, derived from the live capture state. It lives here rather than in
/// `ui.rs` because it is the whole of that decision, and `ui.rs` only paints
/// the result.
impl CameraManApp {
    /// The always-derivable state line for the status bar (left side).
    pub(super) fn state_line(&self) -> (egui::Color32, String) {
        if let Some(error) = &self.sticky_error {
            return (COLOR_ERROR, error.clone());
        }
        if let Some(notice) = self.notice.as_ref().filter(|notice| notice.is_visible()) {
            return (COLOR_OK, notice.text.clone());
        }
        if self.running {
            if self.waiting_for_camera {
                // The status line, the preview overlay and the preview's
                // accessible name all derive their wording from one decision,
                // so a not-responding camera cannot be announced as a warm-up
                // on any of them.
                let (headline, _) = ui::waiting_overlay_text(
                    self.camera_not_responding,
                    self.fixture_state == Some(UiFixtureState::Disconnected),
                );
                (COLOR_WARNING, tr(self.locale, headline).to_owned())
            } else {
                (COLOR_OK, tr(self.locale, UiText::PreviewRunning).to_owned())
            }
        } else if self.preview.is_some() {
            (COLOR_DIM, tr(self.locale, UiText::Paused).to_owned())
        } else {
            (COLOR_DIM, tr(self.locale, UiText::Ready).to_owned())
        }
    }
}
