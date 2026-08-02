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

impl DynamicSpatialMappingOverride {
    pub(crate) fn is_inherit(&self) -> bool {
        matches!(self.projection, OverrideStage::Inherit)
            && matches!(self.shape, OverrideStage::Inherit)
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectedSpatialPosition {
    pub fixture_id: FixtureId,
    pub u: Option<f64>,
    pub v: Option<f64>,
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
    #[error(
        "dynamic spatial override requires both a projection and shape when no inherited mapping is available"
    )]
    IncompleteDynamicOverride,
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
        let (u, v) = projected_coordinates(&mapping.projection, position, screen_right, screen_up);
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

/// Projects each unique target into the same authoritative `(u, v)` plane used for ranking.
/// Missing and non-finite Stage positions remain visible with absent coordinates.
pub fn project_spatial_positions(
    projection: &SpatialProjection,
    targets: &[SpatialTarget],
) -> Result<Vec<ProjectedSpatialPosition>, SpatialMappingError> {
    validate_projection(projection)?;
    let (screen_right, screen_up) = projection_basis(projection)?;
    Ok(deduplicated_targets(targets)
        .map(|(_, target)| {
            let coordinates = target.position.filter(|position| {
                position.x.is_finite() && position.y.is_finite() && position.z.is_finite()
            });
            let (u, v) = coordinates
                .map(|position| {
                    projected_coordinates(projection, position, screen_right, screen_up)
                })
                .map_or((None, None), |(u, v)| (Some(u), Some(v)));
            ProjectedSpatialPosition {
                fixture_id: target.fixture_id,
                u,
                v,
            }
        })
        .collect())
}

/// Resolves one Dynamic's mapping override against its optional live Group mapping.
///
/// `loop_index` perturbs only [`DynamicSelectionShape::Random`]. Source-order and spatial shapes
/// ignore it. Random is position-independent, so it neither validates nor evaluates a projection.
pub fn evaluate_dynamic_spatial_mapping(
    inherited: Option<&SpatialSelectionMapping>,
    mapping_override: &DynamicSpatialMappingOverride,
    targets: &[SpatialTarget],
    loop_index: Option<u64>,
) -> Result<RankedSelection, SpatialMappingError> {
    if let OverrideStage::Replace(DynamicSelectionShape::Random { seed }) = &mapping_override.shape
    {
        return Ok(evaluate_random_mapping(
            *seed,
            loop_index.unwrap_or(0),
            targets,
        ));
    }

    let projection = match &mapping_override.projection {
        OverrideStage::Inherit => inherited.map(|mapping| mapping.projection.clone()),
        OverrideStage::Replace(projection) => Some(projection.clone()),
    };
    let shape = match &mapping_override.shape {
        OverrideStage::Inherit => inherited.map(|mapping| mapping.shape.clone()),
        OverrideStage::Replace(shape) => dynamic_spatial_shape(shape),
    };

    match (projection, shape) {
        (None, None) => Ok(evaluate_source_order(targets)),
        (Some(projection), Some(shape)) => {
            evaluate_spatial_mapping(&SpatialSelectionMapping { projection, shape }, targets)
        }
        (None, Some(_)) | (Some(_), None) => Err(SpatialMappingError::IncompleteDynamicOverride),
    }
}

fn dynamic_spatial_shape(shape: &DynamicSelectionShape) -> Option<SpatialSelectionShape> {
    match *shape {
        DynamicSelectionShape::Grid {
            angle_degrees,
            direction,
        } => Some(SpatialSelectionShape::Grid {
            angle_degrees,
            direction,
        }),
        DynamicSelectionShape::Radial {
            center_u,
            center_v,
            direction,
        } => Some(SpatialSelectionShape::Radial {
            center_u,
            center_v,
            direction,
        }),
        DynamicSelectionShape::Radar {
            center_u,
            center_v,
            start_angle_degrees,
            sweep,
        } => Some(SpatialSelectionShape::Radar {
            center_u,
            center_v,
            start_angle_degrees,
            sweep,
        }),
        DynamicSelectionShape::Random { .. } => None,
    }
}

fn evaluate_source_order(targets: &[SpatialTarget]) -> RankedSelection {
    let ordered_fixture_ids = deduplicated_targets(targets)
        .map(|(_, target)| target.fixture_id)
        .collect::<Vec<_>>();
    let rank_by_fixture = ordered_fixture_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(rank, fixture_id)| (fixture_id, rank))
        .collect();
    RankedSelection {
        rank_count: ordered_fixture_ids.len(),
        ordered_fixture_ids,
        rank_by_fixture,
        warnings: Vec::new(),
    }
}

fn evaluate_random_mapping(
    seed: u64,
    loop_index: u64,
    targets: &[SpatialTarget],
) -> RankedSelection {
    let mut ranked = deduplicated_targets(targets)
        .map(|(source_index, target)| {
            (
                dynamic_random_key(seed, loop_index, target.fixture_id),
                source_index,
                target.fixture_id,
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(key, source_index, _)| (*key, *source_index));
    let ordered_fixture_ids = ranked
        .into_iter()
        .map(|(_, _, fixture_id)| fixture_id)
        .collect::<Vec<_>>();
    let rank_by_fixture = ordered_fixture_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(rank, fixture_id)| (fixture_id, rank))
        .collect();
    RankedSelection {
        rank_count: ordered_fixture_ids.len(),
        ordered_fixture_ids,
        rank_by_fixture,
        warnings: Vec::new(),
    }
}

fn deduplicated_targets(
    targets: &[SpatialTarget],
) -> impl Iterator<Item = (usize, SpatialTarget)> + '_ {
    let mut seen = HashSet::new();
    targets
        .iter()
        .copied()
        .enumerate()
        .filter(move |(_, target)| seen.insert(target.fixture_id))
}

// SplitMix64 finalization over the stable fixture UUID, authored seed, and optional loop index.
// Keep this explicitly specified: persisted Random mappings must not depend on std hash behavior.
fn dynamic_random_key(seed: u64, loop_index: u64, fixture_id: FixtureId) -> u64 {
    let fixture_id = fixture_id.0.as_u128();
    let folded_fixture = fixture_id as u64 ^ (fixture_id >> 64) as u64;
    let mut value = seed ^ folded_fixture ^ loop_index.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn validate_mapping(mapping: &SpatialSelectionMapping) -> Result<(), SpatialMappingError> {
    validate_projection(&mapping.projection)?;
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

fn validate_projection(projection: &SpatialProjection) -> Result<(), SpatialMappingError> {
    for (field, value) in [
        ("projection.anchor.x", projection.anchor.x),
        ("projection.anchor.y", projection.anchor.y),
        ("projection.anchor.z", projection.anchor.z),
        ("projection.view_direction.x", projection.view_direction.x),
        ("projection.view_direction.y", projection.view_direction.y),
        ("projection.view_direction.z", projection.view_direction.z),
        ("projection.rotation_degrees", projection.rotation_degrees),
    ] {
        require_finite(field, value)?;
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

fn projected_coordinates(
    projection: &SpatialProjection,
    position: Position3d,
    screen_right: Vector3,
    screen_up: Vector3,
) -> (f64, f64) {
    let relative = Vector3 {
        x: position.x - projection.anchor.x,
        y: position.y - projection.anchor.y,
        z: position.z - projection.anchor.z,
    };
    (
        canonical_zero(dot(relative, screen_right)),
        canonical_zero(dot(relative, screen_up)),
    )
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
    fn projected_position_preview_uses_ranking_plane_and_retains_missing_targets() {
        let projection = SpatialProjection::from_preset(
            ProjectionPreset::Top,
            Position3d {
                x: 10.0,
                y: 20.0,
                z: 5.0,
            },
        );
        let targets = [
            target(1, 12.0, 17.0, 9.0),
            SpatialTarget {
                fixture_id: fixture(2),
                position: None,
            },
            target(1, 99.0, 99.0, 99.0),
        ];

        assert_eq!(
            project_spatial_positions(&projection, &targets).unwrap(),
            [
                ProjectedSpatialPosition {
                    fixture_id: fixture(1),
                    u: Some(2.0),
                    v: Some(-3.0),
                },
                ProjectedSpatialPosition {
                    fixture_id: fixture(2),
                    u: None,
                    v: None,
                },
            ]
        );
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
    fn dynamic_mapping_inherits_and_replaces_each_spatial_stage_independently() {
        let targets = [target(1, -2.0, 0.0, 3.0), target(2, 2.0, 0.0, -3.0)];
        let inherited = mapping(
            ProjectionPreset::Top,
            SpatialSelectionShape::Grid {
                angle_degrees: 0.0,
                direction: RankDirection::Ascending,
            },
        );

        let inherited_result = evaluate_dynamic_spatial_mapping(
            Some(&inherited),
            &DynamicSpatialMappingOverride::default(),
            &targets,
            Some(99),
        )
        .unwrap();
        assert_eq!(
            inherited_result,
            evaluate_spatial_mapping(&inherited, &targets).unwrap(),
            "non-Random mappings ignore the loop index"
        );

        let front = SpatialProjection::from_preset(ProjectionPreset::Front, Position3d::default());
        let projection_override = DynamicSpatialMappingOverride {
            projection: OverrideStage::Replace(front.clone()),
            shape: OverrideStage::Inherit,
        };
        assert_eq!(
            evaluate_dynamic_spatial_mapping(
                Some(&inherited),
                &projection_override,
                &targets,
                None,
            )
            .unwrap(),
            evaluate_spatial_mapping(
                &SpatialSelectionMapping {
                    projection: front,
                    shape: inherited.shape.clone(),
                },
                &targets,
            )
            .unwrap()
        );

        let local_shape = DynamicSelectionShape::Radial {
            center_u: 0.0,
            center_v: 0.0,
            direction: RadialDirection::Inward,
        };
        let shape_override = DynamicSpatialMappingOverride {
            projection: OverrideStage::Inherit,
            shape: OverrideStage::Replace(local_shape),
        };
        assert_eq!(
            evaluate_dynamic_spatial_mapping(Some(&inherited), &shape_override, &targets, None,)
                .unwrap(),
            evaluate_spatial_mapping(
                &SpatialSelectionMapping {
                    projection: inherited.projection.clone(),
                    shape: SpatialSelectionShape::Radial {
                        center_u: 0.0,
                        center_v: 0.0,
                        direction: RadialDirection::Inward,
                    },
                },
                &targets,
            )
            .unwrap()
        );
    }

    #[test]
    fn dynamic_mapping_without_a_group_uses_source_order_or_rejects_incomplete_spatial_stages() {
        let targets = [
            target(3, 3.0, 0.0, 0.0),
            target(1, 1.0, 0.0, 0.0),
            target(3, 99.0, 99.0, 99.0),
            target(2, 2.0, 0.0, 0.0),
        ];
        let source_order = evaluate_dynamic_spatial_mapping(
            None,
            &DynamicSpatialMappingOverride::default(),
            &targets,
            Some(7),
        )
        .unwrap();
        assert_eq!(
            source_order.ordered_fixture_ids,
            [fixture(3), fixture(1), fixture(2)]
        );
        assert_eq!(source_order.rank_count, 3);
        assert!(source_order.warnings.is_empty());

        let projection_only = DynamicSpatialMappingOverride {
            projection: OverrideStage::Replace(SpatialProjection::from_preset(
                ProjectionPreset::Top,
                Position3d::default(),
            )),
            shape: OverrideStage::Inherit,
        };
        assert_eq!(
            evaluate_dynamic_spatial_mapping(None, &projection_only, &targets, None),
            Err(SpatialMappingError::IncompleteDynamicOverride)
        );

        let shape_only = DynamicSpatialMappingOverride {
            projection: OverrideStage::Inherit,
            shape: OverrideStage::Replace(DynamicSelectionShape::Grid {
                angle_degrees: 0.0,
                direction: RankDirection::Ascending,
            }),
        };
        assert_eq!(
            evaluate_dynamic_spatial_mapping(None, &shape_only, &targets, None),
            Err(SpatialMappingError::IncompleteDynamicOverride)
        );

        let complete = DynamicSpatialMappingOverride {
            projection: projection_only.projection,
            shape: shape_only.shape,
        };
        assert!(
            evaluate_dynamic_spatial_mapping(None, &complete, &targets, None).is_ok(),
            "both local spatial stages make an override complete without a Group mapping"
        );
    }

    #[test]
    fn dynamic_random_is_position_independent_repeatable_and_loop_perturbed() {
        let targets = [
            SpatialTarget {
                fixture_id: fixture(1),
                position: None,
            },
            target(2, f64::NAN, 0.0, 0.0),
            target(3, 30.0, 20.0, 10.0),
            target(4, -4.0, -3.0, -2.0),
            target(5, 5.0, 4.0, 3.0),
            target(2, 999.0, 999.0, 999.0),
        ];
        let random = DynamicSpatialMappingOverride {
            projection: OverrideStage::Inherit,
            shape: OverrideStage::Replace(DynamicSelectionShape::Random { seed: 0x5eed }),
        };
        let mut invalid_projection_random = random.clone();
        invalid_projection_random.projection = OverrideStage::Replace(SpatialProjection {
            anchor: Position3d {
                x: f64::NAN,
                y: f64::NAN,
                z: f64::NAN,
            },
            view_direction: Vector3::default(),
            rotation_degrees: f64::NAN,
            preset: None,
        });

        let base = evaluate_dynamic_spatial_mapping(None, &random, &targets, None).unwrap();
        assert_eq!(
            base.ordered_fixture_ids,
            [fixture(1), fixture(3), fixture(4), fixture(5), fixture(2)]
        );
        assert_eq!(base.rank_count, 5);
        assert!(base.warnings.is_empty());
        assert_eq!(
            evaluate_dynamic_spatial_mapping(None, &random, &targets, None).unwrap(),
            base
        );
        assert_eq!(
            evaluate_dynamic_spatial_mapping(None, &invalid_projection_random, &targets, None,)
                .unwrap(),
            base,
            "Random ignores even an invalid local projection"
        );

        let loop_seven =
            evaluate_dynamic_spatial_mapping(None, &random, &targets, Some(7)).unwrap();
        assert_eq!(
            loop_seven.ordered_fixture_ids,
            [fixture(4), fixture(2), fixture(1), fixture(3), fixture(5)]
        );
        assert_ne!(loop_seven.ordered_fixture_ids, base.ordered_fixture_ids);
    }

    #[test]
    fn dynamic_spatial_mapping_preserves_position_warnings() {
        let inherited = mapping(
            ProjectionPreset::Top,
            SpatialSelectionShape::Grid {
                angle_degrees: 0.0,
                direction: RankDirection::Ascending,
            },
        );
        let missing = SpatialTarget {
            fixture_id: fixture(7),
            position: None,
        };

        let ranked = evaluate_dynamic_spatial_mapping(
            Some(&inherited),
            &DynamicSpatialMappingOverride::default(),
            &[target(6, 0.0, 0.0, 0.0), missing],
            None,
        )
        .unwrap();

        assert_eq!(
            ranked.warnings,
            [SpatialMappingWarning::MissingPosition {
                fixture_id: fixture(7)
            }]
        );
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
