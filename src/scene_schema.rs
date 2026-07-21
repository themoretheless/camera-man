use std::collections::{BTreeMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::parser_limits::{SCENE_PARSER_LIMITS, validate_json_envelope};
use crate::{
    CompositionLayout, ScalingFilter, SourceDescriptor, SourceKind, SourceTransform,
    VIRTUAL_CAMERA_FPS_PRESETS,
};

pub const SCENE_SCHEMA_VERSION: u32 = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingSourcePolicy {
    #[default]
    Placeholder,
    FreezeBriefly,
    HideCell,
    StopOutput,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacySceneInputMode {
    #[default]
    Synthetic,
    Real,
}

/// Validated, versioned persisted source of truth for one named scene.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneDocument {
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub sources: Vec<SourceDescriptor>,
    #[doc(hidden)]
    #[serde(default, rename = "input_mode", skip_serializing)]
    pub legacy_input_mode: LegacySceneInputMode,
    #[doc(hidden)]
    #[serde(default, rename = "source_ids", skip_serializing)]
    pub legacy_source_ids: Vec<String>,
    pub layout: CompositionLayout,
    pub scaling_filter: ScalingFilter,
    #[serde(default)]
    pub source_transforms: BTreeMap<String, SourceTransform>,
    #[serde(default)]
    pub missing_source_policy: MissingSourcePolicy,
    #[serde(default)]
    pub output_fps: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneSchemaError(String);

impl fmt::Display for SceneSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SceneSchemaError {}

impl SceneDocument {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        sources: Vec<SourceDescriptor>,
        layout: CompositionLayout,
        scaling_filter: ScalingFilter,
        source_transforms: BTreeMap<String, SourceTransform>,
        missing_source_policy: MissingSourcePolicy,
        output_fps: Option<u32>,
    ) -> Self {
        Self {
            schema_version: SCENE_SCHEMA_VERSION,
            name,
            sources,
            legacy_input_mode: LegacySceneInputMode::Synthetic,
            legacy_source_ids: Vec::new(),
            layout,
            scaling_filter,
            source_transforms,
            missing_source_policy,
            output_fps,
        }
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, SceneSchemaError> {
        validate_json_envelope(bytes, SCENE_PARSER_LIMITS)
            .map_err(|error| SceneSchemaError(format!("scene parser limit: {error}")))?;
        let mut value = serde_json::from_slice::<Value>(bytes)
            .map_err(|error| SceneSchemaError(format!("invalid scene JSON: {error}")))?;
        migrate_scene_value(&mut value)?;
        let document = serde_json::from_value::<Self>(value)
            .map_err(|error| SceneSchemaError(format!("invalid scene schema: {error}")))?;
        document.validate()?;
        Ok(document)
    }

    pub fn to_json_pretty(&self) -> Result<Vec<u8>, SceneSchemaError> {
        self.validate()?;
        serde_json::to_vec_pretty(self)
            .map_err(|error| SceneSchemaError(format!("could not encode scene: {error}")))
    }

    fn validate(&self) -> Result<(), SceneSchemaError> {
        if self.schema_version != SCENE_SCHEMA_VERSION {
            return Err(SceneSchemaError(format!(
                "unsupported scene schema {}",
                self.schema_version
            )));
        }
        if self.name.trim().is_empty()
            || self.name.len() > SCENE_PARSER_LIMITS.max_string_bytes.min(128)
        {
            return Err(SceneSchemaError(String::from("scene name is invalid")));
        }
        let source_ids = self
            .sources
            .iter()
            .map(SourceDescriptor::stable_key)
            .collect::<Vec<_>>();
        let unique_source_ids = source_ids.iter().collect::<HashSet<_>>();
        if self.sources.len() > SCENE_PARSER_LIMITS.max_elements
            || unique_source_ids.len() != self.sources.len()
            || self.sources.iter().any(|source| source.validate().is_err())
        {
            return Err(SceneSchemaError(String::from(
                "scene source list is invalid",
            )));
        }
        if self.source_transforms.len() > self.sources.len()
            || self.source_transforms.iter().any(|(source_id, transform)| {
                !source_ids.contains(source_id) || transform.validate().is_err()
            })
        {
            return Err(SceneSchemaError(String::from(
                "scene source transforms are invalid",
            )));
        }
        if self
            .output_fps
            .is_some_and(|fps| !VIRTUAL_CAMERA_FPS_PRESETS.contains(&fps))
        {
            return Err(SceneSchemaError(String::from(
                "scene output fps is unsupported",
            )));
        }
        Ok(())
    }

    /// Migrates scenes deserialized as part of the preferences document, where
    /// the raw JSON value migration used by `from_json` is not available.
    pub fn into_current(mut self) -> Result<Self, SceneSchemaError> {
        if self.schema_version > SCENE_SCHEMA_VERSION {
            return Err(SceneSchemaError(format!(
                "scene schema {} is newer than supported",
                self.schema_version
            )));
        }
        if self.schema_version < 4 {
            if self.sources.is_empty() {
                let kind = match self.legacy_input_mode {
                    LegacySceneInputMode::Synthetic => SourceKind::Synthetic,
                    LegacySceneInputMode::Real => SourceKind::Camera,
                };
                self.sources = self
                    .legacy_source_ids
                    .iter()
                    .cloned()
                    .map(|locator| SourceDescriptor { kind, locator })
                    .collect();
            }
            remap_legacy_transform_keys(&self.sources, &mut self.source_transforms);
            self.schema_version = SCENE_SCHEMA_VERSION;
        }
        self.legacy_source_ids.clear();
        self.legacy_input_mode = LegacySceneInputMode::Synthetic;
        self.validate()?;
        Ok(self)
    }
}

fn migrate_scene_value(value: &mut Value) -> Result<(), SceneSchemaError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| SceneSchemaError(String::from("scene root must be an object")))?;
    let mut version = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .unwrap_or(1) as u32;
    if version > SCENE_SCHEMA_VERSION {
        return Err(SceneSchemaError(format!(
            "scene schema {version} is newer than supported"
        )));
    }
    while version < SCENE_SCHEMA_VERSION {
        match version {
            1 => migrate_v1_to_v2(object),
            2 => migrate_v2_to_v3(object),
            3 => migrate_v3_to_v4(object)?,
            _ => {
                return Err(SceneSchemaError(format!(
                    "no migration from scene schema {version}"
                )));
            }
        }
        version += 1;
        object.insert(String::from("schema_version"), Value::from(version));
    }
    Ok(())
}

fn migrate_v3_to_v4(object: &mut Map<String, Value>) -> Result<(), SceneSchemaError> {
    let kind = match object
        .get("input_mode")
        .and_then(Value::as_str)
        .unwrap_or("synthetic")
    {
        "synthetic" => SourceKind::Synthetic,
        "real" => SourceKind::Camera,
        value => {
            return Err(SceneSchemaError(format!(
                "unknown legacy scene input mode {value}"
            )));
        }
    };
    let source_ids = object
        .get("source_ids")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut sources = Vec::with_capacity(source_ids.len());
    let mut key_migrations = Vec::with_capacity(source_ids.len());
    for source_id in source_ids {
        let locator = source_id.as_str().ok_or_else(|| {
            SceneSchemaError(String::from("legacy scene source id must be a string"))
        })?;
        let descriptor = SourceDescriptor {
            kind,
            locator: locator.to_owned(),
        };
        key_migrations.push((locator.to_owned(), descriptor.stable_key()));
        sources.push(
            serde_json::to_value(descriptor)
                .map_err(|error| SceneSchemaError(format!("source migration failed: {error}")))?,
        );
    }
    if let Some(transforms) = object
        .get_mut("source_transforms")
        .and_then(Value::as_object_mut)
    {
        for (legacy, current) in key_migrations {
            if let Some(transform) = transforms.remove(&legacy) {
                transforms.entry(current).or_insert(transform);
            }
        }
    }
    object.insert(String::from("sources"), Value::Array(sources));
    object.remove("input_mode");
    object.remove("source_ids");
    Ok(())
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

fn migrate_v2_to_v3(object: &mut Map<String, Value>) {
    object
        .entry(String::from("input_mode"))
        .or_insert_with(|| Value::String(String::from("synthetic")));
    object
        .entry(String::from("source_transforms"))
        .or_insert_with(|| Value::Object(Map::new()));
    object
        .entry(String::from("missing_source_policy"))
        .or_insert_with(|| Value::String(String::from("placeholder")));
    object
        .entry(String::from("output_fps"))
        .or_insert(Value::Null);
}

fn migrate_v1_to_v2(object: &mut Map<String, Value>) {
    object
        .entry(String::from("scaling_filter"))
        .or_insert_with(|| Value::String(String::from("nearest")));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_v1_scene_before_domain_decode() {
        let old = br#"{
            "schema_version": 1,
            "name": "Desk",
            "source_ids": ["desk"],
            "layout": "grid"
        }"#;
        let scene = SceneDocument::from_json(old).unwrap();
        assert_eq!(scene.schema_version, SCENE_SCHEMA_VERSION);
        assert_eq!(scene.sources, [SourceDescriptor::synthetic("desk")]);
        assert_eq!(scene.scaling_filter, ScalingFilter::Nearest);
        assert_eq!(
            scene.missing_source_policy,
            MissingSourcePolicy::Placeholder
        );
    }

    #[test]
    fn rejects_unknown_future_schema() {
        let future = br#"{"schema_version":999}"#;
        assert!(SceneDocument::from_json(future).is_err());
    }

    #[test]
    fn rejects_duplicate_source_ids() {
        let scene = SceneDocument::new(
            String::from("Duplicate"),
            vec![
                SourceDescriptor::synthetic("desk"),
                SourceDescriptor::synthetic("desk"),
            ],
            CompositionLayout::Grid,
            ScalingFilter::Nearest,
            BTreeMap::new(),
            MissingSourcePolicy::Placeholder,
            None,
        );

        assert!(scene.to_json_pretty().is_err());
    }

    #[test]
    fn v4_scene_can_mix_camera_and_synthetic_sources() {
        let scene = SceneDocument::new(
            String::from("Mixed"),
            vec![
                SourceDescriptor::synthetic("desk"),
                SourceDescriptor::camera("device-1"),
            ],
            CompositionLayout::PictureInPicture,
            ScalingFilter::Bilinear,
            BTreeMap::new(),
            MissingSourcePolicy::Placeholder,
            Some(30),
        );

        let decoded = SceneDocument::from_json(&scene.to_json_pretty().unwrap()).unwrap();
        assert_eq!(decoded, scene);
    }

    #[test]
    fn maximum_source_locator_round_trips_with_its_transform_key() {
        let source = SourceDescriptor::synthetic("x".repeat(crate::MAX_SOURCE_LOCATOR_BYTES));
        let transform = SourceTransform {
            mirror_horizontal: true,
            ..SourceTransform::default()
        };
        let scene = SceneDocument::new(
            String::from("Long locator"),
            vec![source.clone()],
            CompositionLayout::Grid,
            ScalingFilter::Nearest,
            BTreeMap::from([(source.stable_key(), transform)]),
            MissingSourcePolicy::Placeholder,
            None,
        );

        let decoded = SceneDocument::from_json(&scene.to_json_pretty().unwrap()).unwrap();

        assert_eq!(decoded, scene);
    }

    #[test]
    fn v3_camera_scene_remaps_transform_keys() {
        let old = br#"{
            "schema_version":3,
            "name":"Camera",
            "input_mode":"real",
            "source_ids":["device-1"],
            "layout":"grid",
            "scaling_filter":"nearest",
            "source_transforms":{"device-1":{"mirror_horizontal":true}},
            "missing_source_policy":"placeholder",
            "output_fps":30
        }"#;

        let scene = SceneDocument::from_json(old).unwrap();

        assert_eq!(scene.sources, [SourceDescriptor::camera("device-1")]);
        assert!(scene.source_transforms["camera:device-1"].mirror_horizontal);
        let encoded = String::from_utf8(scene.to_json_pretty().unwrap()).unwrap();
        assert!(!encoded.contains("input_mode"));
        assert!(!encoded.contains("source_ids"));
    }

    #[test]
    fn rejects_deep_json_before_domain_deserialization() {
        let mut deep = vec![b'['; SCENE_PARSER_LIMITS.max_nesting_depth + 1];
        deep.extend(std::iter::repeat_n(
            b']',
            SCENE_PARSER_LIMITS.max_nesting_depth + 1,
        ));
        assert!(
            SceneDocument::from_json(&deep)
                .unwrap_err()
                .to_string()
                .contains("nesting")
        );
    }
}
