use super::*;

/// How long a non-error event (export confirmation etc.) stays in the status bar.
const EVENT_TTL: Duration = Duration::from_secs(4);

/// A short-lived confirmation kept separate from sticky errors and live state.
pub(super) struct StatusEvent {
    text: String,
    at: Instant,
}

impl StatusEvent {
    pub(super) fn is_visible(&self) -> bool {
        self.at.elapsed() < EVENT_TTL
    }
}

/// The status bar's message path end to end: `set_event` decides what happened
/// just now and whether it sticks, `state_line` turns the live state into the
/// colour and sentence that get shown. It lives here rather than in `ui.rs`
/// because it is the whole of that decision, and `ui.rs` only paints the
/// result.
impl CameraManApp {
    pub(super) fn set_event(&mut self, text: impl Into<String>, is_error: bool) {
        let text = text.into();
        if is_error {
            if self.sticky_error.as_deref() == Some(text.as_str()) {
                return;
            }
            self.sticky_error = Some(text);
        } else {
            if self
                .notice
                .as_ref()
                .is_some_and(|notice| notice.is_visible() && notice.text == text)
            {
                return;
            }
            self.notice = Some(StatusEvent {
                text,
                at: Instant::now(),
            });
        }
    }

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
