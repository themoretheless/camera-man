use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneChange {
    SourceTopology,
    SourceOrder,
    Transform,
    Layout,
    Scaling,
    FrameRate,
    MissingSourcePolicy,
    SceneRestore,
    RuntimeFrame,
    InspectorSelection,
    SceneName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidationTargets {
    pub preview: bool,
    pub output: bool,
    pub persistence: bool,
    pub undo: bool,
}

impl SceneChange {
    pub const fn invalidates(self) -> InvalidationTargets {
        match self {
            Self::RuntimeFrame => InvalidationTargets {
                preview: true,
                output: true,
                persistence: false,
                undo: false,
            },
            Self::InspectorSelection => InvalidationTargets {
                preview: false,
                output: false,
                persistence: false,
                undo: false,
            },
            Self::SceneName => InvalidationTargets {
                preview: false,
                output: false,
                persistence: true,
                undo: false,
            },
            Self::SourceTopology
            | Self::SourceOrder
            | Self::Transform
            | Self::Layout
            | Self::Scaling
            | Self::FrameRate
            | Self::MissingSourcePolicy
            | Self::SceneRestore => InvalidationTargets {
                preview: true,
                output: true,
                persistence: true,
                undo: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspector_selection_does_not_recompose_pixels() {
        let targets = SceneChange::InspectorSelection.invalidates();
        assert!(!targets.preview);
        assert!(!targets.output);
    }

    #[test]
    fn runtime_frames_do_not_pollute_persistence_or_undo() {
        let targets = SceneChange::RuntimeFrame.invalidates();
        assert!(targets.preview && targets.output);
        assert!(!targets.persistence && !targets.undo);
    }
}
