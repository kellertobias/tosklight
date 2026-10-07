use super::{IntentError, ScalarDomain, ScalarIntent, require};
use crate::{OpeningConvention, Point};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetReference {
    #[default]
    Origin,
    Point {
        point_id: Uuid,
    },
}

/// Exactly one Position representation is active. Mounting is an independent fixture transform,
/// never a second reference hidden inside the recorded target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PositionIntent {
    Angles {
        pan_degrees: ScalarIntent,
        tilt_degrees: ScalarIntent,
    },
    Target {
        reference: TargetReference,
        offset_metres: [ScalarIntent; 3],
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointAngles {
    pub pan_degrees: f32,
    pub tilt_degrees: f32,
}

impl PositionIntent {
    pub fn angles(pan_degrees: f32, tilt_degrees: f32) -> Self {
        Self::Angles {
            pan_degrees: ScalarIntent::Value(pan_degrees),
            tilt_degrees: ScalarIntent::Value(tilt_degrees),
        }
    }
    pub fn target(reference: TargetReference, offset_metres: Point) -> Self {
        Self::Target {
            reference,
            offset_metres: offset_metres.map(ScalarIntent::Value),
        }
    }
    pub fn validate(&self) -> Result<(), IntentError> {
        match self {
            Self::Angles {
                pan_degrees,
                tilt_degrees,
            } => {
                pan_degrees.validate(ScalarDomain::Finite)?;
                tilt_degrees.validate(ScalarDomain::Finite)
            }
            Self::Target {
                reference,
                offset_metres,
            } => {
                if let TargetReference::Point { point_id } = reference {
                    require(!point_id.is_nil(), "target point must have a stable UUID")?;
                }
                for value in offset_metres {
                    value.validate(ScalarDomain::Finite)?;
                }
                Ok(())
            }
        }
    }
    pub fn referenced_point(&self) -> Option<Uuid> {
        match self {
            Self::Target {
                reference: TargetReference::Point { point_id },
                ..
            } => Some(*point_id),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZoomIntent {
    pub opening_degrees: ScalarIntent,
    pub convention: OpeningConvention,
}
impl ZoomIntent {
    pub fn validate(&self) -> Result<(), IntentError> {
        self.opening_degrees.validate(ScalarDomain::Bounded {
            bounds: crate::AttributeBounds {
                min: 0.0,
                max: 180.0,
            },
        })
    }
}
