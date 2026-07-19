use std::collections::{BTreeMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::parser_limits::{SCENE_PARSER_LIMITS, validate_json_envelope};
use crate::{CompositionLayout, ScalingFilter, SourceTransform, VIRTUAL_CAMERA_FPS_PRESETS};

pub const SCENE_SCHEMA_VERSION: u32 = 3;

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
pub enum SceneInputMode {
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
    pub input_mode: SceneInputMode,
    pub source_ids: Vec<String>,
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
        let unique_source_ids = self.source_ids.iter().collect::<HashSet<_>>();
        if self.source_ids.len() > SCENE_PARSER_LIMITS.max_elements
            || unique_source_ids.len() != self.source_ids.len()
            || self.source_ids.iter().any(|source| {
                source.is_empty() || source.len() > SCENE_PARSER_LIMITS.max_string_bytes
            })
        {
            return Err(SceneSchemaError(String::from(
                "scene source list is invalid",
            )));
        }
        if self.source_transforms.len() > self.source_ids.len()
            || self.source_transforms.iter().any(|(source_id, transform)| {
                !self.source_ids.contains(source_id) || transform.validate().is_err()
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
        let scene = SceneDocument {
            schema_version: SCENE_SCHEMA_VERSION,
            name: String::from("Duplicate"),
            input_mode: SceneInputMode::Synthetic,
            source_ids: vec![String::from("desk"), String::from("desk")],
            layout: CompositionLayout::Grid,
            scaling_filter: ScalingFilter::Nearest,
            source_transforms: BTreeMap::new(),
            missing_source_policy: MissingSourcePolicy::Placeholder,
            output_fps: None,
        };

        assert!(scene.to_json_pretty().is_err());
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
