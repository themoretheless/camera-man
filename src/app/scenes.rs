use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use super::*;

const SCENE_HISTORY_LIMIT: usize = 64;
const MAX_SCENE_FILE_BYTES: usize = SCENE_PARSER_LIMITS.max_input_bytes;
const SOURCE_STALE_AFTER: Duration = Duration::from_secs(2);
const FREEZE_BRIEFLY_FOR: Duration = Duration::from_secs(1);

pub(super) struct PreparedSources {
    pub(super) frames: Vec<Option<CapturedFrame>>,
    pub(super) transforms: Vec<SourceTransform>,
    pub(super) suppress_output: bool,
}

impl CameraManApp {
    pub(super) fn selected_source_ids(&self) -> Vec<String> {
        match self.input_mode {
            InputMode::Synthetic => self
                .sources
                .iter()
                .filter(|source| source.selected)
                .map(|source| source.id.to_owned())
                .collect(),
            InputMode::Real => self.selected_real_ids.clone(),
        }
    }

    pub(super) fn scene_snapshot(&self) -> SceneSnapshot {
        SceneSnapshot {
            input_mode: self.input_mode,
            source_ids: self.selected_source_ids(),
            layout: self.layout,
            scaling_filter: self.scaling_filter,
            fps_mode: self.fps_mode,
            source_transforms: self.source_transforms.clone(),
            missing_source_policy: self.missing_source_policy,
            active_scene_name: self.active_scene_name.clone(),
        }
    }

    pub(super) fn commit_scene_history(&mut self, before: SceneSnapshot) {
        if std::mem::take(&mut self.skip_history_commit) {
            return;
        }
        let mut after = self.scene_snapshot();
        if before.same_scene_content(&after) {
            return;
        }
        if before.active_scene_name == after.active_scene_name {
            self.active_scene_name = None;
            after.active_scene_name = None;
        }
        push_bounded(&mut self.undo_stack, before);
        self.redo_stack.clear();
    }

    pub(super) fn finish_scene_history(&mut self, before: SceneSnapshot, pointer_down: bool) {
        let changed = !before.same_scene_content(&self.scene_snapshot());
        if let Some(baseline) = history_baseline_after_pointer(
            &mut self.pending_scene_edit,
            before,
            changed,
            pointer_down,
        ) {
            self.commit_scene_history(baseline);
        }
    }

    pub(super) fn undo_scene_edit(&mut self) -> bool {
        let Some(previous) = self.undo_stack.pop() else {
            return false;
        };
        let current = self.scene_snapshot();
        push_bounded(&mut self.redo_stack, current);
        self.restore_scene_snapshot(previous);
        self.skip_history_commit = true;
        true
    }

    pub(super) fn redo_scene_edit(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        let current = self.scene_snapshot();
        push_bounded(&mut self.undo_stack, current);
        self.restore_scene_snapshot(next);
        self.skip_history_commit = true;
        true
    }

    pub(super) fn move_selected_source(&mut self, from: usize, to: usize) -> bool {
        let mut ids = self.selected_source_ids();
        if !move_item(&mut ids, from, to) {
            return false;
        }
        self.apply_source_ids(ids);
        true
    }

    pub(super) fn save_current_scene(&mut self) -> Result<(), String> {
        let name = self.scene_name_input.trim().to_owned();
        let document = self.current_scene_document(name.clone());
        document
            .to_json_pretty()
            .map_err(|error| error.to_string())?;
        if let Some(existing) = self.scenes.iter_mut().find(|scene| scene.name == name) {
            *existing = document;
        } else {
            if self.scenes.len() == 32 {
                self.scenes.remove(0);
            }
            self.scenes.push(document);
        }
        self.scenes
            .sort_by(|left, right| left.name.cmp(&right.name));
        self.active_scene_name = Some(name);
        Ok(())
    }

    pub(super) fn apply_scene(&mut self, index: usize) -> Result<(), String> {
        let scene = self
            .scenes
            .get(index)
            .cloned()
            .ok_or_else(|| String::from("scene no longer exists"))?;
        scene.to_json_pretty().map_err(|error| error.to_string())?;
        self.input_mode = match scene.input_mode {
            SceneInputMode::Synthetic => InputMode::Synthetic,
            SceneInputMode::Real => InputMode::Real,
        };
        self.apply_source_ids(scene.source_ids.clone());
        self.layout = scene.layout;
        self.scaling_filter = scene.scaling_filter;
        self.source_transforms = scene.source_transforms.clone();
        self.missing_source_policy = scene.missing_source_policy;
        self.fps_mode = scene.output_fps.map_or(FpsMode::Auto, FpsMode::Fixed);
        self.active_scene_name = Some(scene.name.clone());
        self.scene_name_input = scene.name;
        self.selected_source_id = self.selected_source_ids().into_iter().next();
        Ok(())
    }

    pub(super) fn validate_scene_import(&mut self) -> Result<(), String> {
        let bytes = read_scene_file_bounded(&self.scene_path)?;
        self.import_preview = Some(
            SceneDocument::from_json(&bytes)
                .map_err(|error| format!("scene validation failed: {error}"))?,
        );
        Ok(())
    }

    pub(super) fn accept_scene_import(&mut self) -> Result<(), String> {
        let scene = self
            .import_preview
            .take()
            .ok_or_else(|| String::from("validate a scene before importing it"))?;
        if let Some(existing) = self
            .scenes
            .iter_mut()
            .find(|existing| existing.name == scene.name)
        {
            *existing = scene;
        } else {
            if self.scenes.len() == 32 {
                self.scenes.remove(0);
            }
            self.scenes.push(scene);
        }
        self.scenes
            .sort_by(|left, right| left.name.cmp(&right.name));
        Ok(())
    }

    pub(super) fn export_current_scene(&self) -> Result<(), String> {
        let name = self.scene_name_input.trim().to_owned();
        let bytes = self
            .current_scene_document(name)
            .to_json_pretty()
            .map_err(|error| error.to_string())?;
        write_scene_file_atomic(&self.scene_path, &bytes)
    }

    pub(super) fn prepare_sources(
        &mut self,
        source_ids: Vec<String>,
        frames: Vec<Option<CapturedFrame>>,
    ) -> PreparedSources {
        let now = Instant::now();
        let mut prepared_frames = Vec::with_capacity(frames.len());
        let mut transforms = Vec::with_capacity(frames.len());
        let mut missing = 0_usize;

        for (source_id, frame) in source_ids.into_iter().zip(frames) {
            let fresh = frame.filter(|frame| frame.metadata().age() <= SOURCE_STALE_AFTER);
            if let Some(frame) = &fresh {
                self.last_good_frames
                    .insert(source_id.clone(), (frame.clone(), now));
            } else {
                missing += 1;
            }

            let resolved = match self.missing_source_policy {
                MissingSourcePolicy::FreezeBriefly if fresh.is_none() => self
                    .last_good_frames
                    .get(&source_id)
                    .filter(|(_, captured_at)| {
                        now.duration_since(*captured_at) <= FREEZE_BRIEFLY_FOR
                    })
                    .map(|(frame, _)| frame.clone()),
                _ => fresh,
            };

            if self.missing_source_policy == MissingSourcePolicy::HideCell && resolved.is_none() {
                continue;
            }
            prepared_frames.push(resolved);
            transforms.push(
                self.source_transforms
                    .get(&source_id)
                    .copied()
                    .unwrap_or_default(),
            );
        }

        self.stale_source_count = missing;
        self.last_good_frames
            .retain(|_, (_, captured_at)| now.duration_since(*captured_at) <= SOURCE_STALE_AFTER);
        PreparedSources {
            frames: prepared_frames,
            transforms,
            suppress_output: missing > 0
                && self.missing_source_policy == MissingSourcePolicy::StopOutput,
        }
    }

    fn current_scene_document(&self, name: String) -> SceneDocument {
        let source_ids = self.selected_source_ids();
        let source_transforms = source_ids
            .iter()
            .filter_map(|id| {
                self.source_transforms
                    .get(id)
                    .copied()
                    .filter(|transform| !transform.is_identity())
                    .map(|transform| (id.clone(), transform))
            })
            .collect::<BTreeMap<_, _>>();
        SceneDocument {
            schema_version: SCENE_SCHEMA_VERSION,
            name,
            input_mode: match self.input_mode {
                InputMode::Synthetic => SceneInputMode::Synthetic,
                InputMode::Real => SceneInputMode::Real,
            },
            source_ids,
            layout: self.layout,
            scaling_filter: self.scaling_filter,
            source_transforms,
            missing_source_policy: self.missing_source_policy,
            output_fps: match self.fps_mode {
                FpsMode::Auto => None,
                FpsMode::Fixed(fps) => Some(fps),
            },
        }
    }

    fn restore_scene_snapshot(&mut self, snapshot: SceneSnapshot) {
        self.input_mode = snapshot.input_mode;
        self.apply_source_ids(snapshot.source_ids);
        self.layout = snapshot.layout;
        self.scaling_filter = snapshot.scaling_filter;
        self.fps_mode = snapshot.fps_mode;
        self.source_transforms = snapshot.source_transforms;
        self.missing_source_policy = snapshot.missing_source_policy;
        self.active_scene_name = snapshot.active_scene_name;
        self.selected_source_id = self.selected_source_ids().into_iter().next();
    }

    fn apply_source_ids(&mut self, ids: Vec<String>) {
        match self.input_mode {
            InputMode::Synthetic => {
                for source in &mut self.sources {
                    source.selected = ids.iter().any(|id| id == source.id);
                }
                self.sources.sort_by_key(|source| {
                    ids.iter()
                        .position(|id| id == source.id)
                        .unwrap_or(usize::MAX)
                });
            }
            InputMode::Real => self.selected_real_ids = ids,
        }
    }
}

impl SceneSnapshot {
    fn same_scene_content(&self, other: &Self) -> bool {
        self.input_mode == other.input_mode
            && self.source_ids == other.source_ids
            && self.layout == other.layout
            && self.scaling_filter == other.scaling_filter
            && self.fps_mode == other.fps_mode
            && self.source_transforms == other.source_transforms
            && self.missing_source_policy == other.missing_source_policy
    }
}

fn push_bounded(stack: &mut Vec<SceneSnapshot>, snapshot: SceneSnapshot) {
    if stack.len() == SCENE_HISTORY_LIMIT {
        stack.remove(0);
    }
    stack.push(snapshot);
}

fn read_scene_file_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("could not open {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    if metadata.len() > MAX_SCENE_FILE_BYTES as u64 {
        return Err(format!(
            "scene file exceeds the {MAX_SCENE_FILE_BYTES}-byte limit: {}",
            path.display()
        ));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_SCENE_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    if bytes.len() > MAX_SCENE_FILE_BYTES {
        return Err(format!(
            "scene file grew beyond the {MAX_SCENE_FILE_BYTES}-byte limit while reading: {}",
            path.display()
        ));
    }
    Ok(bytes)
}

fn write_scene_file_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    camera_man::replace_file_atomically(path, bytes)
        .map_err(|error| format!("could not atomically replace {}: {error}", path.display()))
}

fn history_baseline_after_pointer(
    pending: &mut Option<SceneSnapshot>,
    before: SceneSnapshot,
    changed: bool,
    pointer_down: bool,
) -> Option<SceneSnapshot> {
    if pointer_down {
        if changed && pending.is_none() {
            *pending = Some(before);
        }
        None
    } else {
        Some(pending.take().unwrap_or(before))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn scene_content_comparison_ignores_only_active_name() {
        let base = SceneSnapshot {
            input_mode: InputMode::Synthetic,
            source_ids: vec![String::from("desk")],
            layout: CompositionLayout::Grid,
            scaling_filter: ScalingFilter::Nearest,
            fps_mode: FpsMode::Auto,
            source_transforms: BTreeMap::new(),
            missing_source_policy: MissingSourcePolicy::Placeholder,
            active_scene_name: Some(String::from("Desk")),
        };
        let mut renamed = base.clone();
        renamed.active_scene_name = None;
        assert!(base.same_scene_content(&renamed));

        let mut changed = base.clone();
        changed.layout = CompositionLayout::Row;
        assert!(!base.same_scene_content(&changed));
    }

    #[test]
    fn scene_import_rejects_files_larger_than_the_limit() {
        let path = unique_test_path("oversized.json");
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_SCENE_FILE_BYTES as u64 + 1).unwrap();

        let error = read_scene_file_bounded(&path).unwrap_err();

        fs::remove_file(path).unwrap();
        assert!(error.contains("exceeds"));
    }

    #[test]
    fn scene_export_replaces_the_target_without_leaving_a_temporary_file() {
        let directory = unique_test_path("atomic-export");
        let path = directory.join("scene.json");
        fs::create_dir_all(&directory).unwrap();
        fs::write(&path, b"old").unwrap();

        write_scene_file_atomic(&path, b"new scene").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new scene");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn pointer_drag_keeps_one_history_baseline_until_release() {
        let original = scene_snapshot(CompositionLayout::Grid);
        let intermediate = scene_snapshot(CompositionLayout::Row);
        let mut pending = None;

        assert!(
            history_baseline_after_pointer(&mut pending, original.clone(), true, true).is_none()
        );
        assert!(
            history_baseline_after_pointer(&mut pending, intermediate.clone(), true, true)
                .is_none()
        );
        let baseline = history_baseline_after_pointer(&mut pending, intermediate, false, false)
            .expect("release should finish the gesture");

        assert!(baseline == original);
        assert!(pending.is_none());
    }

    fn scene_snapshot(layout: CompositionLayout) -> SceneSnapshot {
        SceneSnapshot {
            input_mode: InputMode::Synthetic,
            source_ids: vec![String::from("desk")],
            layout,
            scaling_filter: ScalingFilter::Nearest,
            fps_mode: FpsMode::Auto,
            source_transforms: BTreeMap::new(),
            missing_source_policy: MissingSourcePolicy::Placeholder,
            active_scene_name: Some(String::from("Desk")),
        }
    }

    fn unique_test_path(suffix: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "camera-man-scenes-{}-{nonce}-{suffix}",
            std::process::id()
        ))
    }
}
