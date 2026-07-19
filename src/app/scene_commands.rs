use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SceneEditCommand {
    SetInputMode(InputMode),
    SetSourceSelected {
        source_id: String,
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
            Self::SetInputMode(_) | Self::SetSourceSelected { .. } => SceneChange::SourceTopology,
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
            SceneEditCommand::SetInputMode(mode) => {
                if self.input_mode == mode {
                    false
                } else {
                    self.input_mode = mode;
                    self.selected_source_id = self.selected_source_ids().first().cloned();
                    true
                }
            }
            SceneEditCommand::SetSourceSelected {
                source_id,
                selected,
            } => {
                let changed = match self.input_mode {
                    InputMode::Synthetic => self
                        .sources
                        .iter_mut()
                        .find(|source| source.id == source_id)
                        .is_some_and(|source| {
                            let changed = source.selected != selected;
                            source.selected = selected;
                            changed
                        }),
                    InputMode::Real => {
                        let present = self.selected_real_ids.contains(&source_id);
                        if selected && !present {
                            self.selected_real_ids.push(source_id.clone());
                            true
                        } else if !selected && present {
                            self.selected_real_ids.retain(|id| id != &source_id);
                            true
                        } else {
                            false
                        }
                    }
                };
                if changed {
                    if selected {
                        self.selected_source_id = Some(source_id);
                    } else if self.selected_source_id.as_deref() == Some(source_id.as_str()) {
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
            SceneEditCommand::SetLayout(layout) => replace_if_changed(&mut self.layout, layout),
            SceneEditCommand::SetScalingFilter(filter) => {
                replace_if_changed(&mut self.scaling_filter, filter)
            }
            SceneEditCommand::SetFrameRate(mode) => replace_if_changed(&mut self.fps_mode, mode),
            SceneEditCommand::SetMissingSourcePolicy(policy) => {
                replace_if_changed(&mut self.missing_source_policy, policy)
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
