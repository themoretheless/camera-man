use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SceneEditCommand {
    SetSourceSelected {
        source: SourceDescriptor,
        selected: bool,
    },
    MoveSource {
        from: usize,
        to: usize,
    },
    SetTransform {
        source_id: String,
        transform: SourceTransform,
    },
    SetLayout(CompositionLayout),
    SetScalingFilter(ScalingFilter),
    SetFrameRate(FpsMode),
    SetMissingSourcePolicy(MissingSourcePolicy),
}

impl SceneEditCommand {
    const fn invalidation(&self) -> SceneChange {
        match self {
            Self::SetSourceSelected { .. } => SceneChange::SourceTopology,
            Self::MoveSource { .. } => SceneChange::SourceOrder,
            Self::SetTransform { .. } => SceneChange::Transform,
            Self::SetLayout(_) => SceneChange::Layout,
            Self::SetScalingFilter(_) => SceneChange::Scaling,
            Self::SetFrameRate(_) => SceneChange::FrameRate,
            Self::SetMissingSourcePolicy(_) => SceneChange::MissingSourcePolicy,
        }
    }
}

impl CameraManApp {
    pub(super) fn apply_scene_command(&mut self, command: SceneEditCommand) -> Option<SceneChange> {
        let invalidation = command.invalidation();
        let changed = match command {
            SceneEditCommand::SetSourceSelected { source, selected } => {
                let stable_key = source.stable_key();
                let present = self.selected_sources.contains(&source);
                let changed = if selected && !present {
                    self.selected_sources.push(source);
                    true
                } else if !selected && present {
                    self.selected_sources
                        .retain(|candidate| candidate != &source);
                    true
                } else {
                    false
                };
                if changed {
                    if selected {
                        self.selected_source_id = Some(stable_key.clone());
                    } else if self.selected_source_id.as_deref() == Some(stable_key.as_str()) {
                        self.selected_source_id = self.selected_source_ids().first().cloned();
                    }
                }
                changed
            }
            SceneEditCommand::MoveSource { from, to } => self.move_selected_source(from, to),
            SceneEditCommand::SetTransform {
                source_id,
                transform,
            } => {
                let transform = transform.sanitized();
                let previous = self
                    .source_transforms
                    .get(&source_id)
                    .copied()
                    .unwrap_or_default();
                if previous == transform {
                    false
                } else {
                    if transform.is_identity() {
                        self.source_transforms.remove(&source_id);
                    } else {
                        self.source_transforms.insert(source_id, transform);
                    }
                    true
                }
            }
            SceneEditCommand::SetLayout(layout) => {
                replace_if_changed(&mut self.output.layout, layout)
            }
            SceneEditCommand::SetScalingFilter(filter) => {
                replace_if_changed(&mut self.output.scaling_filter, filter)
            }
            SceneEditCommand::SetFrameRate(mode) => {
                replace_if_changed(&mut self.output.fps_mode, mode)
            }
            SceneEditCommand::SetMissingSourcePolicy(policy) => {
                replace_if_changed(&mut self.output.missing_source_policy, policy)
            }
        };
        changed.then_some(invalidation)
    }
}

fn replace_if_changed<T: PartialEq>(target: &mut T, value: T) -> bool {
    if *target == value {
        false
    } else {
        *target = value;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_typed_scene_command_declares_its_invalidation() {
        assert_eq!(
            SceneEditCommand::MoveSource { from: 0, to: 1 }.invalidation(),
            SceneChange::SourceOrder
        );
        assert_eq!(
            SceneEditCommand::SetTransform {
                source_id: String::from("desk"),
                transform: SourceTransform::default(),
            }
            .invalidation(),
            SceneChange::Transform
        );
        assert_eq!(
            SceneEditCommand::SetLayout(CompositionLayout::Grid).invalidation(),
            SceneChange::Layout
        );
    }
}
