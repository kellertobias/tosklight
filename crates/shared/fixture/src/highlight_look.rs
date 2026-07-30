use light_core::Xyz;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Installation-wide semantic look used by the transient Highlight contribution.
///
/// Fixture profiles remain responsible for translating these portable semantic values into
/// channel functions and exact DMX. This type deliberately contains no show or patch identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HighlightLook {
    pub intensity: f32,
    #[serde(default)]
    pub shutter: HighlightShutterPolicy,
    #[serde(default)]
    pub color: Option<HighlightColor>,
    #[serde(default)]
    pub iris: Option<f32>,
    #[serde(default)]
    pub zoom: Option<f32>,
    #[serde(default)]
    pub focus: Option<f32>,
    #[serde(default)]
    pub frost: Option<f32>,
    #[serde(default)]
    pub compatibility: HighlightLookCompatibility,
}

impl Default for HighlightLook {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            shutter: HighlightShutterPolicy::Open,
            color: None,
            iris: None,
            zoom: None,
            focus: None,
            frost: None,
            compatibility: HighlightLookCompatibility::Semantic,
        }
    }
}

impl HighlightLook {
    /// Compatibility default for installation configuration written before Highlight Look became
    /// desk-owned. The persisted show remains authoritative until an operator reviews migration.
    pub fn needs_review() -> Self {
        Self {
            compatibility: HighlightLookCompatibility::NeedsReview,
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), HighlightLookValidationError> {
        validate_normalized("intensity", self.intensity)?;
        for (field, value) in [
            ("iris", self.iris),
            ("zoom", self.zoom),
            ("focus", self.focus),
            ("frost", self.frost),
        ] {
            if let Some(value) = value {
                validate_normalized(field, value)?;
            }
        }
        Ok(())
    }
}

/// Highlight always requests the authored semantic Open function. There is intentionally no raw
/// or alternate shutter policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightShutterPolicy {
    #[default]
    Open,
}

/// Explicit compatibility state for legacy per-fixture raw Highlight maps.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightLookCompatibility {
    Semantic,
    LegacyRaw,
    #[default]
    NeedsReview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightColor {
    White,
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Amber,
}

impl HighlightColor {
    pub fn to_xyz(self) -> Xyz {
        let (red, green, blue) = match self {
            Self::White => (1.0, 1.0, 1.0),
            Self::Red => (1.0, 0.0, 0.0),
            Self::Green => (0.0, 1.0, 0.0),
            Self::Blue => (0.0, 0.0, 1.0),
            Self::Cyan => (0.0, 1.0, 1.0),
            Self::Magenta => (1.0, 0.0, 1.0),
            // Match the operator-facing amber swatch (#ff9f2f).
            Self::Amber => (1.0, 159.0 / 255.0, 47.0 / 255.0),
        };
        crate::srgb_to_xyz(red, green, blue)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Highlight Look {field} must contain a finite normalized value from 0 to 1")]
pub struct HighlightLookValidationError {
    pub field: &'static str,
}

fn validate_normalized(
    field: &'static str,
    value: f32,
) -> Result<(), HighlightLookValidationError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(HighlightLookValidationError { field })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_look_is_semantic_and_optional_parts_are_ignored() {
        let look = HighlightLook::default();
        assert_eq!(look.compatibility, HighlightLookCompatibility::Semantic);
        assert_eq!(look.shutter, HighlightShutterPolicy::Open);
        assert_eq!(look.intensity, 1.0);
        assert_eq!(
            (look.color, look.iris, look.zoom, look.focus, look.frost),
            (None, None, None, None, None)
        );
        assert_eq!(look.validate(), Ok(()));
    }

    #[test]
    fn normalized_values_are_validated() {
        for (field, look) in [
            (
                "intensity",
                HighlightLook {
                    intensity: f32::NAN,
                    ..HighlightLook::default()
                },
            ),
            (
                "zoom",
                HighlightLook {
                    zoom: Some(1.1),
                    ..HighlightLook::default()
                },
            ),
        ] {
            assert_eq!(look.validate(), Err(HighlightLookValidationError { field }));
        }
    }
}
