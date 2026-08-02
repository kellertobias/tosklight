use light_core::FixtureId;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

const MIN_DIRECTION_LENGTH: f64 = 1.0e-12;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Position3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpatialSelectionMapping {
    pub projection: SpatialProjection,
    pub shape: SpatialSelectionShape,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SpatialProjection {
    pub anchor: Position3d,
    pub view_direction: Vector3,
    pub rotation_degrees: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<ProjectionPreset>,
}

impl<'de> Deserialize<'de> for SpatialProjection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct StoredProjection {
            anchor: Position3d,
            view_direction: Vector3,
            rotation_degrees: f64,
            #[serde(default)]
            preset: Option<ProjectionPreset>,
        }

        let stored = StoredProjection::deserialize(deserializer)?;
        let mut projection = Self {
            anchor: stored.anchor,
            view_direction: stored.view_direction,
            rotation_degrees: stored.rotation_degrees,
            preset: stored.preset,
        };
        if projection
            .preset
            .is_some_and(|preset| !projection.matches_preset(preset))
        {
            projection.preset = None;
        }
        Ok(projection)
    }
}

impl SpatialProjection {
    pub fn from_preset(preset: ProjectionPreset, anchor: Position3d) -> Self {
        Self {
            anchor,
            view_direction: preset.view_direction(),
            rotation_degrees: 0.0,
            preset: Some(preset),
        }
    }

    pub fn matches_preset(&self, preset: ProjectionPreset) -> bool {
        self.view_direction == preset.view_direction()
            && normalize_degrees(self.rotation_degrees) == 0.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionPreset {
    Top,
    Front,
    Back,
    Left,
    Right,
}

impl ProjectionPreset {
    const fn view_direction(self) -> Vector3 {
        match self {
            Self::Top => Vector3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
            Self::Front => Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            Self::Back => Vector3 {
                x: 0.0,
                y: -1.0,
                z: 0.0,
            },
            Self::Left => Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            Self::Right => Vector3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpatialSelectionShape {
    Grid {
        angle_degrees: f64,
        direction: RankDirection,
    },
    Radial {
        center_u: f64,
        center_v: f64,
        direction: RadialDirection,
    },
    Radar {
        center_u: f64,
        center_v: f64,
        start_angle_degrees: f64,
        sweep: RadarSweep,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicSelectionShape {
    Grid {
        angle_degrees: f64,
        direction: RankDirection,
    },
    Radial {
        center_u: f64,
        center_v: f64,
        direction: RadialDirection,
    },
    Radar {
        center_u: f64,
        center_v: f64,
        start_angle_degrees: f64,
        sweep: RadarSweep,
    },
    Random {
        seed: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicSpatialMappingOverride {
    #[serde(default)]
    pub projection: OverrideStage<SpatialProjection>,
    #[serde(default)]
    pub shape: OverrideStage<DynamicSelectionShape>,
}

impl Default for DynamicSpatialMappingOverride {
    fn default() -> Self {
        Self {
            projection: OverrideStage::Inherit,
            shape: OverrideStage::Inherit,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum OverrideStage<T> {
    #[default]
    Inherit,
    Replace(T),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadialDirection {
    Outward,
    Inward,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadarSweep {
    Clockwise,
    CounterClockwise,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialTarget {
    pub fixture_id: FixtureId,
    pub position: Option<Position3d>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedSelection {
    pub ordered_fixture_ids: Vec<FixtureId>,
    pub rank_by_fixture: HashMap<FixtureId, usize>,
    pub rank_count: usize,
    pub warnings: Vec<SpatialMappingWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpatialMappingWarning {
    MissingPosition { fixture_id: FixtureId },
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SpatialMappingError {
    #[error("spatial mapping field {field} must be finite")]
    NonFinite { field: &'static str },
    #[error("spatial projection view direction must be non-zero")]
    InvalidViewDirection,
}

#[derive(Clone, Copy)]
struct ProjectedTarget {
    source_index: usize,
    fixture_id: FixtureId,
    key: f64,
}

pub fn evaluate_spatial_mapping(
    mapping: &SpatialSelectionMapping,
    targets: &[SpatialTarget],
) -> Result<RankedSelection, SpatialMappingError> {
    validate_mapping(mapping)?;
    let (screen_right, screen_up) = projection_basis(&mapping.projection)?;
    let mut seen = HashSet::new();
    let mut positioned = Vec::new();
    let mut missing = Vec::new();
    let mut warnings = Vec::new();

    for (source_index, target) in targets.iter().copied().enumerate() {
        if !seen.insert(target.fixture_id) {
            continue;
        }
        let Some(position) = target.position else {
            missing.push((source_index, target.fixture_id));
            warnings.push(SpatialMappingWarning::MissingPosition {
                fixture_id: target.fixture_id,
            });
            continue;
        };
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            missing.push((source_index, target.fixture_id));
            warnings.push(SpatialMappingWarning::MissingPosition {
                fixture_id: target.fixture_id,
            });
            continue;
        }
        let relative = Vector3 {
            x: position.x - mapping.projection.anchor.x,
            y: position.y - mapping.projection.anchor.y,
            z: position.z - mapping.projection.anchor.z,
        };
        let u = canonical_zero(dot(relative, screen_right));
        let v = canonical_zero(dot(relative, screen_up));
        positioned.push(ProjectedTarget {
            source_index,
            fixture_id: target.fixture_id,
            key: canonical_zero(shape_key(&mapping.shape, u, v)),
        });
    }

    positioned.sort_by(|left, right| {
        shape_ordering(&mapping.shape, left.key, right.key)
            .then_with(|| left.source_index.cmp(&right.source_index))
    });
    missing.sort_by_key(|(source_index, _)| *source_index);

    let mut ordered_fixture_ids = Vec::with_capacity(positioned.len() + missing.len());
    let mut rank_by_fixture = HashMap::with_capacity(positioned.len() + missing.len());
    let mut rank_count = 0usize;
    let mut previous_key = None;
    for target in positioned {
        if previous_key.is_none_or(|key| key != target.key) {
            rank_count += 1;
        }
        previous_key = Some(target.key);
        ordered_fixture_ids.push(target.fixture_id);
        rank_by_fixture.insert(target.fixture_id, rank_count - 1);
    }
    for (_, fixture_id) in missing {
        ordered_fixture_ids.push(fixture_id);
        rank_by_fixture.insert(fixture_id, rank_count);
        rank_count += 1;
    }

    Ok(RankedSelection {
        ordered_fixture_ids,
        rank_by_fixture,
        rank_count,
        warnings,
    })
}

fn validate_mapping(mapping: &SpatialSelectionMapping) -> Result<(), SpatialMappingError> {
    for (field, value) in [
        ("projection.anchor.x", mapping.projection.anchor.x),
        ("projection.anchor.y", mapping.projection.anchor.y),
        ("projection.anchor.z", mapping.projection.anchor.z),
        (
            "projection.view_direction.x",
            mapping.projection.view_direction.x,
        ),
        (
            "projection.view_direction.y",
            mapping.projection.view_direction.y,
        ),
        (
            "projection.view_direction.z",
            mapping.projection.view_direction.z,
        ),
        (
            "projection.rotation_degrees",
            mapping.projection.rotation_degrees,
        ),
    ] {
        require_finite(field, value)?;
    }
    match mapping.shape {
        SpatialSelectionShape::Grid { angle_degrees, .. } => {
            require_finite("shape.angle_degrees", angle_degrees)?;
        }
        SpatialSelectionShape::Radial {
            center_u, center_v, ..
        } => {
            require_finite("shape.center_u", center_u)?;
            require_finite("shape.center_v", center_v)?;
        }
        SpatialSelectionShape::Radar {
            center_u,
            center_v,
            start_angle_degrees,
            ..
        } => {
            require_finite("shape.center_u", center_u)?;
            require_finite("shape.center_v", center_v)?;
            require_finite("shape.start_angle_degrees", start_angle_degrees)?;
        }
    }
    Ok(())
}

fn require_finite(field: &'static str, value: f64) -> Result<(), SpatialMappingError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(SpatialMappingError::NonFinite { field })
    }
}

fn projection_basis(
    projection: &SpatialProjection,
) -> Result<(Vector3, Vector3), SpatialMappingError> {
    let direction = normalize(projection.view_direction)?;
    let preferred_up = Vector3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };
    let fallback_up = Vector3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    let reference_up = if dot(direction, preferred_up).abs() > 1.0 - MIN_DIRECTION_LENGTH {
        fallback_up
    } else {
        preferred_up
    };
    let right = normalize(cross(direction, reference_up))?;
    let up = normalize(cross(right, direction))?;
    let radians = normalize_degrees(projection.rotation_degrees).to_radians();
    let (sin, cos) = radians.sin_cos();
    Ok((
        Vector3 {
            x: right.x * cos + up.x * sin,
            y: right.y * cos + up.y * sin,
            z: right.z * cos + up.z * sin,
        },
        Vector3 {
            x: up.x * cos - right.x * sin,
            y: up.y * cos - right.y * sin,
            z: up.z * cos - right.z * sin,
        },
    ))
}

fn shape_key(shape: &SpatialSelectionShape, u: f64, v: f64) -> f64 {
    match *shape {
        SpatialSelectionShape::Grid { angle_degrees, .. } => {
            let radians = normalize_degrees(angle_degrees).to_radians();
            u * radians.cos() + v * radians.sin()
        }
        SpatialSelectionShape::Radial {
            center_u, center_v, ..
        } => (u - center_u).hypot(v - center_v),
        SpatialSelectionShape::Radar {
            center_u,
            center_v,
            start_angle_degrees,
            sweep,
        } => {
            let angle = (v - center_v).atan2(u - center_u).to_degrees();
            match sweep {
                RadarSweep::CounterClockwise => normalize_degrees(angle - start_angle_degrees),
                RadarSweep::Clockwise => normalize_degrees(start_angle_degrees - angle),
            }
        }
    }
}

fn shape_ordering(shape: &SpatialSelectionShape, left: f64, right: f64) -> std::cmp::Ordering {
    let ordering = left.total_cmp(&right);
    match shape {
        SpatialSelectionShape::Grid {
            direction: RankDirection::Descending,
            ..
        }
        | SpatialSelectionShape::Radial {
            direction: RadialDirection::Inward,
            ..
        } => ordering.reverse(),
        _ => ordering,
    }
}

fn normalize(vector: Vector3) -> Result<Vector3, SpatialMappingError> {
    let length = (vector.x * vector.x + vector.y * vector.y + vector.z * vector.z).sqrt();
    if !length.is_finite() || length <= MIN_DIRECTION_LENGTH {
        return Err(SpatialMappingError::InvalidViewDirection);
    }
    Ok(Vector3 {
        x: vector.x / length,
        y: vector.y / length,
        z: vector.z / length,
    })
}

const fn dot(left: Vector3, right: Vector3) -> f64 {
    left.x * right.x + left.y * right.y + left.z * right.z
}

const fn cross(left: Vector3, right: Vector3) -> Vector3 {
    Vector3 {
        x: left.y * right.z - left.z * right.y,
        y: left.z * right.x - left.x * right.z,
        z: left.x * right.y - left.y * right.x,
    }
}

fn normalize_degrees(value: f64) -> f64 {
    canonical_zero(value.rem_euclid(360.0))
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn fixture(value: u128) -> FixtureId {
        FixtureId(Uuid::from_u128(value))
    }

    fn target(value: u128, x: f64, y: f64, z: f64) -> SpatialTarget {
        SpatialTarget {
            fixture_id: fixture(value),
            position: Some(Position3d { x, y, z }),
        }
    }

    fn mapping(preset: ProjectionPreset, shape: SpatialSelectionShape) -> SpatialSelectionMapping {
        SpatialSelectionMapping {
            projection: SpatialProjection::from_preset(preset, Position3d::default()),
            shape,
        }
    }

    #[test]
    fn named_projection_presets_follow_the_stage_xyz_convention() {
        let targets = [
            target(1, -2.0, 0.0, 0.0),
            target(2, 2.0, 0.0, 0.0),
            target(3, 0.0, 2.0, 0.0),
            target(4, 0.0, -2.0, 0.0),
            target(5, 0.0, 0.0, 2.0),
            target(6, 0.0, 0.0, -2.0),
        ];
        let ascending_x = SpatialSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Ascending,
        };
        let ascending_up = SpatialSelectionShape::Grid {
            angle_degrees: 90.0,
            direction: RankDirection::Ascending,
        };

        let top = evaluate_spatial_mapping(
            &mapping(ProjectionPreset::Top, ascending_up.clone()),
            &targets,
        )
        .unwrap();
        assert!(top.rank_by_fixture[&fixture(4)] < top.rank_by_fixture[&fixture(3)]);

        let front = evaluate_spatial_mapping(
            &mapping(ProjectionPreset::Front, ascending_up.clone()),
            &targets,
        )
        .unwrap();
        assert!(front.rank_by_fixture[&fixture(6)] < front.rank_by_fixture[&fixture(5)]);

        let back = evaluate_spatial_mapping(
            &mapping(ProjectionPreset::Back, ascending_x.clone()),
            &targets,
        )
        .unwrap();
        assert!(back.rank_by_fixture[&fixture(2)] < back.rank_by_fixture[&fixture(1)]);

        for preset in [ProjectionPreset::Left, ProjectionPreset::Right] {
            let side =
                evaluate_spatial_mapping(&mapping(preset, ascending_up.clone()), &targets).unwrap();
            assert!(side.rank_by_fixture[&fixture(6)] < side.rank_by_fixture[&fixture(5)]);
        }
    }

    #[test]
    fn anchor_rotation_and_negative_zero_are_deterministic() {
        let mut projection = SpatialProjection::from_preset(
            ProjectionPreset::Top,
            Position3d {
                x: 10.0,
                y: 20.0,
                z: 5.0,
            },
        );
        projection.rotation_degrees = 90.0;
        projection.preset = None;
        let ranked = evaluate_spatial_mapping(
            &SpatialSelectionMapping {
                projection,
                shape: SpatialSelectionShape::Grid {
                    angle_degrees: -0.0,
                    direction: RankDirection::Ascending,
                },
            },
            &[target(1, 10.0, 19.0, 5.0), target(2, 10.0, 21.0, 5.0)],
        )
        .unwrap();
        assert_eq!(ranked.ordered_fixture_ids, [fixture(1), fixture(2)]);
    }

    #[test]
    fn equal_spatial_keys_share_ranks_and_missing_positions_remain_individual() {
        let targets = [
            target(1, 0.0, 0.0, 0.0),
            target(2, 0.0, 2.0, 0.0),
            target(3, 2.0, 0.0, 0.0),
            SpatialTarget {
                fixture_id: fixture(4),
                position: None,
            },
            SpatialTarget {
                fixture_id: fixture(5),
                position: None,
            },
            target(1, 99.0, 99.0, 99.0),
        ];
        let ranked = evaluate_spatial_mapping(
            &mapping(
                ProjectionPreset::Top,
                SpatialSelectionShape::Grid {
                    angle_degrees: 0.0,
                    direction: RankDirection::Ascending,
                },
            ),
            &targets,
        )
        .unwrap();

        assert_eq!(
            ranked.ordered_fixture_ids,
            [fixture(1), fixture(2), fixture(3), fixture(4), fixture(5)]
        );
        assert_eq!(ranked.rank_by_fixture[&fixture(1)], 0);
        assert_eq!(ranked.rank_by_fixture[&fixture(2)], 0);
        assert_eq!(ranked.rank_by_fixture[&fixture(3)], 1);
        assert_eq!(ranked.rank_by_fixture[&fixture(4)], 2);
        assert_eq!(ranked.rank_by_fixture[&fixture(5)], 3);
        assert_eq!(ranked.rank_count, 4);
        assert_eq!(ranked.warnings.len(), 2);
    }

    #[test]
    fn radial_and_radar_directions_produce_literal_rank_order() {
        let targets = [
            target(1, 0.0, 0.0, 0.0),
            target(2, 1.0, 0.0, 0.0),
            target(3, 0.0, 1.0, 0.0),
            target(4, -1.0, 0.0, 0.0),
        ];
        let outward = evaluate_spatial_mapping(
            &mapping(
                ProjectionPreset::Top,
                SpatialSelectionShape::Radial {
                    center_u: 0.0,
                    center_v: 0.0,
                    direction: RadialDirection::Outward,
                },
            ),
            &targets,
        )
        .unwrap();
        assert_eq!(outward.rank_by_fixture[&fixture(1)], 0);
        assert_eq!(outward.rank_by_fixture[&fixture(2)], 1);
        assert_eq!(outward.rank_by_fixture[&fixture(3)], 1);

        let inward = evaluate_spatial_mapping(
            &mapping(
                ProjectionPreset::Top,
                SpatialSelectionShape::Radial {
                    center_u: 0.0,
                    center_v: 0.0,
                    direction: RadialDirection::Inward,
                },
            ),
            &targets,
        )
        .unwrap();
        assert!(inward.rank_by_fixture[&fixture(2)] < inward.rank_by_fixture[&fixture(1)]);

        let clockwise = evaluate_spatial_mapping(
            &mapping(
                ProjectionPreset::Top,
                SpatialSelectionShape::Radar {
                    center_u: 0.0,
                    center_v: 0.0,
                    start_angle_degrees: 0.0,
                    sweep: RadarSweep::Clockwise,
                },
            ),
            &targets[1..],
        )
        .unwrap();
        let counter_clockwise = evaluate_spatial_mapping(
            &mapping(
                ProjectionPreset::Top,
                SpatialSelectionShape::Radar {
                    center_u: 0.0,
                    center_v: 0.0,
                    start_angle_degrees: 0.0,
                    sweep: RadarSweep::CounterClockwise,
                },
            ),
            &targets[1..],
        )
        .unwrap();
        assert_ne!(
            clockwise.ordered_fixture_ids,
            counter_clockwise.ordered_fixture_ids
        );
    }

    #[test]
    fn invalid_vectors_and_non_finite_configuration_are_rejected() {
        let mut invalid = mapping(
            ProjectionPreset::Top,
            SpatialSelectionShape::Grid {
                angle_degrees: 0.0,
                direction: RankDirection::Ascending,
            },
        );
        invalid.projection.view_direction = Vector3::default();
        assert_eq!(
            evaluate_spatial_mapping(&invalid, &[]),
            Err(SpatialMappingError::InvalidViewDirection)
        );
        invalid.projection.view_direction = ProjectionPreset::Top.view_direction();
        invalid.projection.anchor.x = f64::NAN;
        assert_eq!(
            evaluate_spatial_mapping(&invalid, &[]),
            Err(SpatialMappingError::NonFinite {
                field: "projection.anchor.x"
            })
        );
    }

    #[test]
    fn decoding_clears_a_stale_preset_hint_and_defaults_dynamic_override_to_inherit() {
        let projection: SpatialProjection = serde_json::from_value(serde_json::json!({
            "anchor": {"x": 0.0, "y": 0.0, "z": 0.0},
            "view_direction": {"x": 0.0, "y": 1.0, "z": 0.0},
            "rotation_degrees": 15.0,
            "preset": "front"
        }))
        .unwrap();
        assert_eq!(projection.preset, None);

        let override_state: DynamicSpatialMappingOverride =
            serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(override_state, DynamicSpatialMappingOverride::default());
    }
}
