use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use camera_man::{
    CompositionLayout, MAX_SOURCE_KEY_BYTES, MissingSourcePolicy, PREFERENCES_PARSER_LIMITS,
    ScalingFilter, SceneDocument, SourceDescriptor, SourceKind, SourceTransform,
    VIRTUAL_CAMERA_FPS_PRESETS, read_bounded, replace_file_atomically, validate_json_envelope,
};
use serde::{Deserialize, Serialize};

const STORAGE_KEY: &str = "camera-man-preferences-v1";
pub(crate) const CURRENT_SCHEMA_VERSION: u32 = 4;
const MAX_PREFERENCES_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const DEFAULT_EXPORT_PATH: &str = "target/camera-man-app-preview.ppm";
// Keep the canonical path stable; the embedded schema version performs migration.
const PREFERENCES_FILE_NAME: &str = "preferences-v3.json";

pub(crate) fn default_preferences_path() -> Option<PathBuf> {
    eframe::storage_dir("CameraMan").map(|directory| directory.join(PREFERENCES_FILE_NAME))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyInputMode {
    #[default]
    Synthetic,
    Real,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiLocale {
    #[default]
    English,
    Russian,
    /// Visual-QA-only expansion mode selected through an environment fixture.
    PseudoLong,
}

/// How to pick the render/output frame rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FpsMode {
    /// Follow the highest negotiated rate across selected real cameras.
    Auto,
    Fixed(u32),
}

/// Persisted source of truth. Runtime-only handles, frames and worker state are
/// intentionally absent and are rebuilt after validation/migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct AppPreferences {
    #[serde(default = "legacy_schema_version")]
    pub(crate) schema_version: u32,
    #[serde(default)]
    pub(crate) selected_sources: Vec<SourceDescriptor>,
    #[serde(default, rename = "input_mode", skip_serializing)]
    pub(crate) legacy_input_mode: LegacyInputMode,
    #[serde(default, rename = "selected_synthetic_ids", skip_serializing)]
    pub(crate) legacy_selected_synthetic_ids: Vec<String>,
    #[serde(default, rename = "selected_real_ids", skip_serializing)]
    pub(crate) legacy_selected_real_ids: Vec<String>,
    pub(crate) layout: CompositionLayout,
    pub(crate) scaling_filter: ScalingFilter,
    pub(crate) fps_mode: FpsMode,
    pub(crate) virtual_output_enabled: bool,
    pub(crate) export_path: PathBuf,
    pub(crate) locale: UiLocale,
    pub(crate) high_contrast: bool,
    pub(crate) source_transforms: BTreeMap<String, SourceTransform>,
    pub(crate) missing_source_policy: MissingSourcePolicy,
    pub(crate) scenes: Vec<SceneDocument>,
    pub(crate) active_scene_name: Option<String>,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            selected_sources: vec![
                SourceDescriptor::synthetic("desk"),
                SourceDescriptor::synthetic("side"),
                SourceDescriptor::synthetic("wide"),
            ],
            legacy_input_mode: LegacyInputMode::Synthetic,
            legacy_selected_synthetic_ids: Vec::new(),
            legacy_selected_real_ids: Vec::new(),
            layout: CompositionLayout::Grid,
            scaling_filter: ScalingFilter::Nearest,
            fps_mode: FpsMode::Auto,
            virtual_output_enabled: true,
            export_path: PathBuf::from(DEFAULT_EXPORT_PATH),
            locale: UiLocale::English,
            high_contrast: false,
            source_transforms: BTreeMap::new(),
            missing_source_policy: MissingSourcePolicy::Placeholder,
            scenes: Vec::new(),
            active_scene_name: None,
        }
    }
}

impl AppPreferences {
    pub(crate) fn load(
        storage: Option<&dyn eframe::Storage>,
        canonical_path: Option<&Path>,
    ) -> Self {
        canonical_path
            .and_then(|path| Self::load_from_atomic_path(path).ok())
            .or_else(|| {
                storage
                    .filter(|storage| {
                        storage
                            .get_string(STORAGE_KEY)
                            .is_some_and(|encoded| encoded.len() <= MAX_PREFERENCES_BYTES)
                    })
                    .and_then(|storage| eframe::get_value::<Self>(storage, STORAGE_KEY))
            })
            .unwrap_or_default()
            .migrated()
            .sanitized()
    }

    pub(crate) fn load_from_atomic_path(path: &Path) -> Result<Self, String> {
        let bytes = read_bounded(path, PREFERENCES_PARSER_LIMITS)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        validate_json_envelope(&bytes, PREFERENCES_PARSER_LIMITS)
            .map_err(|error| format!("invalid preferences envelope: {error}"))?;
        serde_json::from_slice::<Self>(&bytes)
            .map(Self::migrated)
            .map(Self::sanitized)
            .map_err(|error| format!("invalid preferences JSON: {error}"))
    }

    pub(crate) fn store(
        &self,
        storage: &mut dyn eframe::Storage,
        canonical_path: Option<&Path>,
    ) -> Result<(), String> {
        eframe::set_value(storage, STORAGE_KEY, self);
        canonical_path.map_or(Ok(()), |path| self.store_atomically(path))
    }

    pub(crate) fn store_atomically(&self, path: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("could not encode preferences: {error}"))?;
        validate_json_envelope(&bytes, PREFERENCES_PARSER_LIMITS)
            .map_err(|error| format!("preferences exceed parser limits: {error}"))?;
        let checked = serde_json::from_slice::<Self>(&bytes)
            .map_err(|error| format!("could not verify encoded preferences: {error}"))?
            .migrated()
            .sanitized();
        if checked != *self {
            return Err(String::from(
                "preferences failed validation; the previous file was preserved",
            ));
        }
        replace_file_atomically(path, &bytes)
            .map_err(|error| format!("could not atomically replace {}: {error}", path.display()))
    }

    fn sanitized(mut self) -> Self {
        self.selected_sources = sanitized_sources(self.selected_sources);
        if matches!(self.fps_mode, FpsMode::Fixed(fps) if !VIRTUAL_CAMERA_FPS_PRESETS.contains(&fps))
        {
            self.fps_mode = FpsMode::Auto;
        }
        if self.export_path.as_os_str().is_empty() {
            self.export_path = PathBuf::from(DEFAULT_EXPORT_PATH);
        }
        self.source_transforms.retain(|id, transform| {
            !id.is_empty() && id.len() <= MAX_SOURCE_KEY_BYTES && transform.validate().is_ok()
        });
        let mut scene_names = HashSet::new();
        self.scenes = self
            .scenes
            .into_iter()
            .filter_map(|scene| scene.into_current().ok())
            .filter(|scene| {
                scene.to_json_pretty().is_ok() && scene_names.insert(scene.name.clone())
            })
            .collect();
        self.scenes.truncate(32);
        if self
            .active_scene_name
            .as_ref()
            .is_some_and(|name| !self.scenes.iter().any(|scene| &scene.name == name))
        {
            self.active_scene_name = None;
        }
        self
    }

    fn migrated(mut self) -> Self {
        if self.schema_version > CURRENT_SCHEMA_VERSION {
            return Self::default();
        }
        while self.schema_version < CURRENT_SCHEMA_VERSION {
            match self.schema_version {
                // v2 formalized explicit scaling and FPS policies. Serde's
                // defaults already materialize those fields for old data.
                0 | 1 => self.schema_version = 2,
                2 => self.schema_version = 3,
                3 => {
                    if self.selected_sources.is_empty() {
                        let (kind, ids) = match self.legacy_input_mode {
                            LegacyInputMode::Synthetic => (
                                SourceKind::Synthetic,
                                std::mem::take(&mut self.legacy_selected_synthetic_ids),
                            ),
                            LegacyInputMode::Real => (
                                SourceKind::Camera,
                                std::mem::take(&mut self.legacy_selected_real_ids),
                            ),
                        };
                        self.selected_sources = ids
                            .into_iter()
                            .map(|locator| SourceDescriptor { kind, locator })
                            .collect();
                    }
                    remap_legacy_transform_keys(
                        &self.selected_sources,
                        &mut self.source_transforms,
                    );
                    self.scenes = self
                        .scenes
                        .into_iter()
                        .filter_map(|scene| scene.into_current().ok())
                        .collect();
                    self.schema_version = 4;
                }
                _ => return Self::default(),
            }
        }
        self.legacy_input_mode = LegacyInputMode::Synthetic;
        self.legacy_selected_synthetic_ids.clear();
        self.legacy_selected_real_ids.clear();
        self
    }
}

const fn legacy_schema_version() -> u32 {
    1
}

fn sanitized_sources(sources: Vec<SourceDescriptor>) -> Vec<SourceDescriptor> {
    let mut seen = HashSet::new();
    sources
        .into_iter()
        .filter(|source| source.validate().is_ok())
        .filter(|source| seen.insert(source.stable_key()))
        .take(64)
        .collect()
}

fn remap_legacy_transform_keys(
    sources: &[SourceDescriptor],
    transforms: &mut BTreeMap<String, SourceTransform>,
) {
    for source in sources {
        if let Some(transform) = transforms.remove(&source.locator) {
            transforms.entry(source.stable_key()).or_insert(transform);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MemoryStorage(HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn preferences_round_trip_without_runtime_state() {
        let preferences = AppPreferences {
            selected_sources: vec![SourceDescriptor::camera("camera-2")],
            layout: CompositionLayout::Row,
            scaling_filter: ScalingFilter::Bilinear,
            fps_mode: FpsMode::Fixed(60),
            virtual_output_enabled: false,
            ..AppPreferences::default()
        };

        let json = serde_json::to_string(&preferences).unwrap();
        let decoded: AppPreferences = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, preferences);
    }

    #[test]
    fn corrupted_preferences_are_bounded_and_repaired() {
        let preferences = AppPreferences {
            selected_sources: vec![
                SourceDescriptor::camera("camera-1"),
                SourceDescriptor::camera("camera-1"),
                SourceDescriptor::camera(""),
            ],
            fps_mode: FpsMode::Fixed(0),
            export_path: PathBuf::new(),
            ..AppPreferences::default()
        }
        .sanitized();

        assert_eq!(
            preferences.selected_sources,
            [SourceDescriptor::camera("camera-1")]
        );
        assert_eq!(preferences.fps_mode, FpsMode::Auto);
        assert_eq!(preferences.export_path, PathBuf::from(DEFAULT_EXPORT_PATH));
    }

    #[test]
    fn unsupported_fixed_fps_does_not_leave_the_segmented_control_unselected() {
        let preferences = AppPreferences {
            fps_mode: FpsMode::Fixed(45),
            ..AppPreferences::default()
        }
        .sanitized();

        assert_eq!(preferences.fps_mode, FpsMode::Auto);
    }

    #[test]
    fn eframe_storage_round_trips_preferences() {
        let preferences = AppPreferences {
            layout: CompositionLayout::Column,
            fps_mode: FpsMode::Fixed(24),
            export_path: PathBuf::from("target/custom.ppm"),
            ..AppPreferences::default()
        };
        let mut storage = MemoryStorage::default();

        preferences.store(&mut storage, None).unwrap();

        assert_eq!(AppPreferences::load(Some(&storage), None), preferences);
    }

    #[test]
    fn legacy_preferences_migrate_sequentially() {
        let json = r#"{
            "input_mode":"synthetic",
            "selected_synthetic_ids":["desk"],
            "selected_real_ids":[],
            "layout":"grid",
            "scaling_filter":"nearest",
            "fps_mode":"auto",
            "virtual_output_enabled":true,
            "export_path":"target/legacy.ppm"
        }"#;
        let migrated = serde_json::from_str::<AppPreferences>(json)
            .unwrap()
            .migrated();
        assert_eq!(migrated.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(
            migrated.selected_sources,
            [SourceDescriptor::synthetic("desk")]
        );
    }

    #[test]
    fn v3_real_preferences_preserve_camera_and_transform() {
        let json = r#"{
            "schema_version":3,
            "input_mode":"real",
            "selected_synthetic_ids":["desk"],
            "selected_real_ids":["camera-7"],
            "layout":"grid",
            "scaling_filter":"nearest",
            "fps_mode":"auto",
            "virtual_output_enabled":true,
            "export_path":"target/legacy.ppm",
            "source_transforms":{"camera-7":{"mirror_horizontal":true}}
        }"#;

        let migrated = serde_json::from_str::<AppPreferences>(json)
            .unwrap()
            .migrated()
            .sanitized();

        assert_eq!(
            migrated.selected_sources,
            [SourceDescriptor::camera("camera-7")]
        );
        assert!(migrated.source_transforms["camera:camera-7"].mirror_horizontal);
        let encoded = serde_json::to_string(&migrated).unwrap();
        assert!(!encoded.contains("input_mode"));
        assert!(!encoded.contains("selected_real_ids"));
    }

    #[test]
    fn canonical_preferences_round_trip_atomically() {
        let directory = temporary_directory("round-trip");
        let path = directory.join("preferences.json");
        let preferences = AppPreferences {
            selected_sources: vec![SourceDescriptor::synthetic("wide")],
            layout: CompositionLayout::PictureInPicture,
            ..AppPreferences::default()
        };

        preferences.store_atomically(&path).unwrap();

        assert_eq!(
            AppPreferences::load_from_atomic_path(&path).unwrap(),
            preferences
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn corrupt_canonical_file_falls_back_to_legacy_storage() {
        let directory = temporary_directory("fallback");
        let path = directory.join("preferences.json");
        std::fs::write(&path, b"{not-json").unwrap();
        let expected = AppPreferences {
            layout: CompositionLayout::Column,
            ..AppPreferences::default()
        };
        let mut storage = MemoryStorage::default();
        eframe::set_value(&mut storage, STORAGE_KEY, &expected);

        assert_eq!(AppPreferences::load(Some(&storage), Some(&path)), expected);
        assert_eq!(std::fs::read(&path).unwrap(), b"{not-json");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn failed_validation_preserves_previous_preferences() {
        let directory = temporary_directory("preserve");
        let path = directory.join("preferences.json");
        let previous = AppPreferences::default();
        previous.store_atomically(&path).unwrap();
        let previous_bytes = std::fs::read(&path).unwrap();
        let invalid = AppPreferences {
            schema_version: CURRENT_SCHEMA_VERSION,
            selected_sources: vec![SourceDescriptor::camera("")],
            ..AppPreferences::default()
        };

        assert!(invalid.store_atomically(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), previous_bytes);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn temporary_directory(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "camera-man-preferences-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }
}
