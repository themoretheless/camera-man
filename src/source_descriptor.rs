use std::fmt;

use serde::{Deserialize, Serialize};

pub const MAX_SOURCE_LOCATOR_BYTES: usize = 1_024;
pub const MAX_SOURCE_KEY_BYTES: usize = MAX_SOURCE_LOCATOR_BYTES + "synthetic:".len();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Synthetic,
    Camera,
}

impl SourceKind {
    pub const fn key_prefix(self) -> &'static str {
        match self {
            Self::Synthetic => "synthetic",
            Self::Camera => "camera",
        }
    }
}

/// Persisted identity of one scene source.
///
/// `kind` is intentionally attached to each source instead of being a global
/// scene mode, so cameras and generated/video-like sources can share one
/// ordered composition graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceDescriptor {
    pub kind: SourceKind,
    pub locator: String,
}

impl SourceDescriptor {
    pub fn synthetic(locator: impl Into<String>) -> Self {
        Self {
            kind: SourceKind::Synthetic,
            locator: locator.into(),
        }
    }

    pub fn camera(locator: impl Into<String>) -> Self {
        Self {
            kind: SourceKind::Camera,
            locator: locator.into(),
        }
    }

    pub fn stable_key(&self) -> String {
        format!("{}:{}", self.kind.key_prefix(), self.locator)
    }

    pub fn validate(&self) -> Result<(), SourceDescriptorError> {
        if self.locator.trim().is_empty() {
            return Err(SourceDescriptorError("source locator is empty"));
        }
        if self.locator.len() > MAX_SOURCE_LOCATOR_BYTES {
            return Err(SourceDescriptorError("source locator is too long"));
        }
        if self.locator.chars().any(char::is_control) {
            return Err(SourceDescriptorError(
                "source locator contains control characters",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceDescriptorError(&'static str);

impl fmt::Display for SourceDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for SourceDescriptorError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_is_part_of_the_stable_identity() {
        assert_ne!(
            SourceDescriptor::synthetic("0").stable_key(),
            SourceDescriptor::camera("0").stable_key()
        );
    }

    #[test]
    fn rejects_empty_oversized_and_control_character_locators() {
        assert!(SourceDescriptor::camera("").validate().is_err());
        assert!(
            SourceDescriptor::camera("x".repeat(MAX_SOURCE_LOCATOR_BYTES + 1))
                .validate()
                .is_err()
        );
        assert!(SourceDescriptor::camera("camera\n1").validate().is_err());
    }
}
