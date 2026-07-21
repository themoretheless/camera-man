pub(super) const MIN_WINDOW_WIDTH: f32 = 920.0;
pub(super) const MIN_WINDOW_HEIGHT: f32 = 560.0;
pub(super) const MIN_POINTER_TARGET: f32 = 28.0;
pub(super) const STATUS_BAR_HEIGHT: f32 = 40.0;
pub(super) const SOURCES_PANEL_MIN_WIDTH: f32 = 270.0;
pub(super) const SOURCES_PANEL_DEFAULT_WIDTH: f32 = 294.0;
pub(super) const SOURCES_PANEL_MAX_WIDTH: f32 = 380.0;
pub(super) const OUTPUT_PANEL_MIN_WIDTH: f32 = 230.0;
pub(super) const OUTPUT_PANEL_DEFAULT_WIDTH: f32 = 252.0;
pub(super) const OUTPUT_PANEL_MAX_WIDTH: f32 = 330.0;
pub(super) const CENTRAL_HORIZONTAL_MARGIN: f32 = 12.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum UiFixtureState {
    #[default]
    Workspace,
    Setup,
    Empty,
    Mixed,
    Disconnected,
    InstallError,
    Running,
}

impl UiFixtureState {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "workspace" => Some(Self::Workspace),
            "setup" => Some(Self::Setup),
            "empty" => Some(Self::Empty),
            "mixed" => Some(Self::Mixed),
            "disconnected" => Some(Self::Disconnected),
            "install-error" => Some(Self::InstallError),
            "running" => Some(Self::Running),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_workspace_preserves_a_useful_preview_column() {
        let preview_width = MIN_WINDOW_WIDTH
            - SOURCES_PANEL_MIN_WIDTH
            - OUTPUT_PANEL_MIN_WIDTH
            - CENTRAL_HORIZONTAL_MARGIN * 2.0;
        let workspace_height = MIN_WINDOW_HEIGHT - STATUS_BAR_HEIGHT;
        assert!(preview_width >= 360.0);
        assert!(workspace_height >= 500.0);
    }

    #[test]
    fn frequent_pointer_targets_share_one_minimum() {
        let frequent_target_heights = [MIN_POINTER_TARGET, 30.0, 32.0, 36.0, STATUS_BAR_HEIGHT];
        assert!(
            frequent_target_heights
                .into_iter()
                .all(|height| height >= MIN_POINTER_TARGET)
        );
    }

    #[test]
    fn every_visual_fixture_state_has_a_stable_name() {
        for (name, state) in [
            ("workspace", UiFixtureState::Workspace),
            ("setup", UiFixtureState::Setup),
            ("empty", UiFixtureState::Empty),
            ("mixed", UiFixtureState::Mixed),
            ("disconnected", UiFixtureState::Disconnected),
            ("install-error", UiFixtureState::InstallError),
            ("running", UiFixtureState::Running),
        ] {
            assert_eq!(UiFixtureState::parse(name), Some(state));
        }
    }
}
