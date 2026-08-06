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

/// How a Stage position becomes the `(u, v)` pair the shape ranks on.
///
/// Stored projections predate this field, so an absent `kind` is [`Self::Planar`] and old
/// shows keep their exact ranking.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionKind {
    /// Orthographic projection along `view_direction`; `u` and `v` span the viewing plane.
    #[default]
    Planar,
    /// `u` is the angular distance from the start angle around the anchored axis, `v` the
    /// distance along that axis.
    Cylindrical,
    /// `u` is the angular distance from the centre direction, `v` the distance from the anchor.
    Spherical,
}

impl ProjectionKind {
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde skip_serializing_if"
    )]
    fn is_planar(&self) -> bool {
        matches!(self, Self::Planar)
    }
}

impl Vector3 {
    fn is_zero(&self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.z == 0.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SpatialProjection {
    pub anchor: Position3d,
    pub view_direction: Vector3,
    pub rotation_degrees: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<ProjectionPreset>,
    /// Each of the four is omitted at its default, so a planar projection persists exactly the
    /// bytes it did before the other kinds existed and old Shows round-trip unchanged.
    #[serde(default, skip_serializing_if = "ProjectionKind::is_planar")]
    pub kind: ProjectionKind,
    /// Euler degrees about X, Y then Z turning world +Z into the cylinder axis. Cylindrical only.
    #[serde(default, skip_serializing_if = "Vector3::is_zero")]
    pub axis_rotation: Vector3,
    /// Cylindrical: where the spread starts around the axis. Spherical: the centre's azimuth.
    #[serde(default, skip_serializing_if = "is_zero_degrees")]
    pub start_angle_degrees: f64,
    /// Spherical only: the centre's elevation above the plane perpendicular to world +Z.
    #[serde(default, skip_serializing_if = "is_zero_degrees")]
    pub elevation_degrees: f64,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde skip_serializing_if"
)]
fn is_zero_degrees(value: &f64) -> bool {
    *value == 0.0
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
            #[serde(default)]
            kind: ProjectionKind,
            #[serde(default)]
            axis_rotation: Vector3,
            #[serde(default)]
            start_angle_degrees: f64,
            #[serde(default)]
            elevation_degrees: f64,
        }

        let stored = StoredProjection::deserialize(deserializer)?;
        let mut projection = Self {
            anchor: stored.anchor,
            view_direction: stored.view_direction,
            rotation_degrees: stored.rotation_degrees,
            preset: stored.preset,
            kind: stored.kind,
            axis_rotation: stored.axis_rotation,
            start_angle_degrees: stored.start_angle_degrees,
            elevation_degrees: stored.elevation_degrees,
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
            kind: ProjectionKind::Planar,
            axis_rotation: Vector3::default(),
            start_angle_degrees: 0.0,
            elevation_degrees: 0.0,
        }
    }

    /// Presets name a viewing direction, so only a planar projection can carry one.
    pub fn matches_preset(&self, preset: ProjectionPreset) -> bool {
        self.kind == ProjectionKind::Planar
            && self.view_direction == preset.view_direction()
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
        ("projection.rotation_degrees", projection.rotation_degrees),
    ] {
        require_finite(field, value)?;
    }
    match projection.kind {
        // A planar projection is nothing without the direction it looks along.
        ProjectionKind::Planar => {
            for (field, value) in [
                ("projection.view_direction.x", projection.view_direction.x),
                ("projection.view_direction.y", projection.view_direction.y),
                ("projection.view_direction.z", projection.view_direction.z),
            ] {
                require_finite(field, value)?;
            }
        }
        ProjectionKind::Cylindrical => {
            for (field, value) in [
                ("projection.axis_rotation.x", projection.axis_rotation.x),
                ("projection.axis_rotation.y", projection.axis_rotation.y),
                ("projection.axis_rotation.z", projection.axis_rotation.z),
                (
                    "projection.start_angle_degrees",
                    projection.start_angle_degrees,
                ),
            ] {
                require_finite(field, value)?;
            }
        }
        ProjectionKind::Spherical => {
            for (field, value) in [
                (
                    "projection.start_angle_degrees",
                    projection.start_angle_degrees,
                ),
                ("projection.elevation_degrees", projection.elevation_degrees),
            ] {
                require_finite(field, value)?;
            }
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

/// The viewing-plane basis a planar projection ranks in.
///
/// Cylindrical and spherical projections derive their own frame from the anchor and their
/// angles, so they neither need nor validate a view direction.
fn projection_basis(
    projection: &SpatialProjection,
) -> Result<(Vector3, Vector3), SpatialMappingError> {
    if projection.kind != ProjectionKind::Planar {
        // Unused by `projected_coordinates` for these kinds.
        return Ok((Vector3::default(), Vector3::default()));
    }
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

/// Applies Euler degrees about X, then Y, then Z.
fn rotate_euler(vector: Vector3, degrees: Vector3) -> Vector3 {
    let (sx, cx) = degrees.x.to_radians().sin_cos();
    let (sy, cy) = degrees.y.to_radians().sin_cos();
    let (sz, cz) = degrees.z.to_radians().sin_cos();
    let after_x = Vector3 {
        x: vector.x,
        y: vector.y * cx - vector.z * sx,
        z: vector.y * sx + vector.z * cx,
    };
    let after_y = Vector3 {
        x: after_x.x * cy + after_x.z * sy,
        y: after_x.y,
        z: -after_x.x * sy + after_x.z * cy,
    };
    Vector3 {
        x: after_y.x * cz - after_y.y * sz,
        y: after_y.x * sz + after_y.y * cz,
        z: after_y.z,
    }
}

/// The cylinder axis and the two perpendicular directions that place the start angle.
///
/// With every rotation at zero the axis is world +Z and the start angle is measured from
/// world +X.
fn cylinder_frame(
    projection: &SpatialProjection,
) -> Result<(Vector3, Vector3, Vector3), SpatialMappingError> {
    let axis = normalize(rotate_euler(
        Vector3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        projection.axis_rotation,
    ))?;
    let seed = rotate_euler(
        Vector3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        projection.axis_rotation,
    );
    let start = normalize(reject(seed, axis))?;
    let radians = normalize_degrees(projection.start_angle_degrees).to_radians();
    let (sin, cos) = radians.sin_cos();
    let side = cross(axis, start);
    Ok((
        axis,
        Vector3 {
            x: start.x * cos + side.x * sin,
            y: start.y * cos + side.y * sin,
            z: start.z * cos + side.z * sin,
        },
        Vector3 {
            x: side.x * cos - start.x * sin,
            y: side.y * cos - start.y * sin,
            z: side.z * cos - start.z * sin,
        },
    ))
}

/// The unit direction the spherical projection is centred on.
fn spherical_center(projection: &SpatialProjection) -> Vector3 {
    let azimuth = normalize_degrees(projection.start_angle_degrees).to_radians();
    let elevation = normalize_degrees(projection.elevation_degrees).to_radians();
    Vector3 {
        x: elevation.cos() * azimuth.cos(),
        y: elevation.cos() * azimuth.sin(),
        z: elevation.sin(),
    }
}

/// The angle in degrees between two directions, `0` through `180`.
fn angle_between(a: Vector3, b: Vector3) -> f64 {
    dot(a, b).clamp(-1.0, 1.0).acos().to_degrees()
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
    let (u, v) = match projection.kind {
        ProjectionKind::Planar => (dot(relative, screen_right), dot(relative, screen_up)),
        // The spread leaves the start angle in both directions and meets itself 180 degrees
        // away, so `u` is the unsigned angle. `v` keeps the position along the axis.
        ProjectionKind::Cylindrical => match cylinder_frame(projection) {
            Ok((axis, start, side)) => {
                let height = dot(relative, axis);
                let around = reject(relative, axis);
                let angle = if length(around) <= MIN_DIRECTION_LENGTH {
                    0.0
                } else {
                    dot(around, side)
                        .atan2(dot(around, start))
                        .to_degrees()
                        .abs()
                };
                (angle, height)
            }
            Err(_) => (0.0, 0.0),
        },
        // No axis: `u` is the great-circle distance from the centre direction, reaching 180
        // at the antipode, and `v` is the distance from the centre point.
        ProjectionKind::Spherical => {
            let radius = length(relative);
            let angle = match normalize(relative) {
                Ok(direction) => angle_between(direction, spherical_center(projection)),
                Err(_) => 0.0,
            };
            (angle, radius)
        }
    };
    (canonical_zero(u), canonical_zero(v))
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

fn length(vector: Vector3) -> f64 {
    (vector.x * vector.x + vector.y * vector.y + vector.z * vector.z).sqrt()
}

/// The part of `vector` perpendicular to the unit direction `axis`.
fn reject(vector: Vector3, axis: Vector3) -> Vector3 {
    let along = dot(vector, axis);
    Vector3 {
        x: vector.x - axis.x * along,
        y: vector.y - axis.y * along,
        z: vector.z - axis.z * along,
    }
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

    fn centered(kind: ProjectionKind, anchor: Position3d) -> SpatialProjection {
        SpatialProjection {
            anchor,
            view_direction: Vector3::default(),
            rotation_degrees: 0.0,
            preset: None,
            kind,
            axis_rotation: Vector3::default(),
            start_angle_degrees: 0.0,
            elevation_degrees: 0.0,
        }
    }

    fn coordinates(projection: &SpatialProjection, x: f64, y: f64, z: f64) -> (f64, f64) {
        let projected = project_spatial_positions(projection, &[target(1, x, y, z)]).unwrap();
        (projected[0].u.unwrap(), projected[0].v.unwrap())
    }

    #[test]
    fn a_stored_projection_without_a_kind_stays_planar() {
        // Shows saved before the cylindrical and spherical kinds existed carry no `kind`.
        let stored = serde_json::json!({
            "anchor": { "x": 0.0, "y": 0.0, "z": 0.0 },
            "view_direction": { "x": 0.0, "y": 0.0, "z": -1.0 },
            "rotation_degrees": 0.0,
            "preset": "top",
        });
        let projection: SpatialProjection = serde_json::from_value(stored).unwrap();
        assert_eq!(projection.kind, ProjectionKind::Planar);
        assert_eq!(projection.preset, Some(ProjectionPreset::Top));
        assert_eq!(projection.start_angle_degrees, 0.0);
        assert_eq!(projection.axis_rotation, Vector3::default());
    }

    #[test]
    fn a_planar_projection_serializes_exactly_what_it_did_before_the_new_kinds() {
        // Persisted Shows must not gain fields, so a stored planar projection has to round-trip
        // byte for byte.
        let stored = serde_json::json!({
            "anchor": { "x": 0.0, "y": 0.0, "z": 0.0 },
            "view_direction": { "x": 0.0, "y": 0.0, "z": -1.0 },
            "rotation_degrees": 0.0,
            "preset": "top",
        });
        let projection: SpatialProjection = serde_json::from_value(stored.clone()).unwrap();
        assert_eq!(serde_json::to_value(&projection).unwrap(), stored);
    }

    #[test]
    fn a_non_planar_projection_persists_its_own_fields() {
        let mut projection = centered(ProjectionKind::Cylindrical, Position3d::default());
        projection.start_angle_degrees = 45.0;
        let encoded = serde_json::to_value(&projection).unwrap();
        assert_eq!(encoded["kind"], "cylindrical");
        assert_eq!(encoded["start_angle_degrees"], 45.0);
        // Still omitted at their defaults, and restored as such.
        assert!(encoded.get("axis_rotation").is_none());
        assert!(encoded.get("elevation_degrees").is_none());
        assert_eq!(
            serde_json::from_value::<SpatialProjection>(encoded).unwrap(),
            projection
        );
    }

    #[test]
    fn a_cylindrical_projection_spreads_outward_from_the_start_angle() {
        let projection = centered(ProjectionKind::Cylindrical, Position3d::default());

        // With every rotation at zero the axis is world +Z and the start angle sits on +X.
        assert_eq!(coordinates(&projection, 3.0, 0.0, 0.0).0, 0.0);
        // The two sides of the start angle are equidistant, which is what "outward both
        // ways" means: +Y and -Y both read 90 degrees rather than 90 and 270.
        assert!((coordinates(&projection, 0.0, 3.0, 0.0).0 - 90.0).abs() < 1.0e-9);
        assert!((coordinates(&projection, 0.0, -3.0, 0.0).0 - 90.0).abs() < 1.0e-9);
        // They meet 180 degrees away, on the far side of the cylinder.
        assert!((coordinates(&projection, -3.0, 0.0, 0.0).0 - 180.0).abs() < 1.0e-9);
        // Distance from the axis does not change the angle; `v` carries the axial position.
        assert!((coordinates(&projection, 0.0, 40.0, 7.0).0 - 90.0).abs() < 1.0e-9);
        assert!((coordinates(&projection, 0.0, 40.0, 7.0).1 - 7.0).abs() < 1.0e-9);
    }

    #[test]
    fn the_cylindrical_start_angle_moves_the_center_of_the_spread() {
        let mut projection = centered(ProjectionKind::Cylindrical, Position3d::default());
        projection.start_angle_degrees = 90.0;

        // The centre has moved onto +Y, so +X and -X are now the equidistant pair.
        assert!(coordinates(&projection, 0.0, 3.0, 0.0).0.abs() < 1.0e-9);
        assert!((coordinates(&projection, 3.0, 0.0, 0.0).0 - 90.0).abs() < 1.0e-9);
        assert!((coordinates(&projection, -3.0, 0.0, 0.0).0 - 90.0).abs() < 1.0e-9);
        assert!((coordinates(&projection, 0.0, -3.0, 0.0).0 - 180.0).abs() < 1.0e-9);
    }

    #[test]
    fn rotating_the_cylinder_axis_reorients_the_spread() {
        let mut projection = centered(ProjectionKind::Cylindrical, Position3d::default());
        // A quarter turn about X lays the axis down onto world -Y.
        projection.axis_rotation = Vector3 {
            x: 90.0,
            y: 0.0,
            z: 0.0,
        };

        // The axis now runs along Y, so a Y offset is axial and reads on `v`, not on `u`.
        let (u, v) = coordinates(&projection, 0.0, 5.0, 0.0);
        assert!(u.abs() < 1.0e-9);
        assert!((v.abs() - 5.0).abs() < 1.0e-9);
        // The start direction stays on +X, so +X is still the centre of the spread.
        assert!(coordinates(&projection, 4.0, 0.0, 0.0).0.abs() < 1.0e-9);
        assert!((coordinates(&projection, -4.0, 0.0, 0.0).0 - 180.0).abs() < 1.0e-9);
    }

    #[test]
    fn the_cylinder_anchor_is_freely_positionable() {
        let projection = centered(
            ProjectionKind::Cylindrical,
            Position3d {
                x: 10.0,
                y: -4.0,
                z: 2.0,
            },
        );

        // Measured from the anchor, not from the world origin.
        assert_eq!(coordinates(&projection, 13.0, -4.0, 2.0).0, 0.0);
        assert!((coordinates(&projection, 7.0, -4.0, 2.0).0 - 180.0).abs() < 1.0e-9);
        assert!((coordinates(&projection, 10.0, -4.0, 9.0).1 - 7.0).abs() < 1.0e-9);
    }

    #[test]
    fn a_spherical_projection_spreads_outward_to_the_antipode() {
        let projection = centered(ProjectionKind::Spherical, Position3d::default());

        // Both angles at zero put the centre on +X.
        assert_eq!(coordinates(&projection, 3.0, 0.0, 0.0).0, 0.0);
        // Every direction 90 degrees off the centre reads the same, in both axes.
        for (x, y, z) in [
            (0.0, 3.0, 0.0),
            (0.0, -3.0, 0.0),
            (0.0, 0.0, 3.0),
            (0.0, 0.0, -3.0),
        ] {
            assert!((coordinates(&projection, x, y, z).0 - 90.0).abs() < 1.0e-9);
        }
        // 180 degrees at the antipode.
        assert!((coordinates(&projection, -3.0, 0.0, 0.0).0 - 180.0).abs() < 1.0e-9);
        // `v` is the distance from the anchor, so the angle ignores it.
        assert!((coordinates(&projection, 50.0, 0.0, 0.0).1 - 50.0).abs() < 1.0e-9);
    }

    #[test]
    fn the_two_spherical_angles_move_the_center_independently() {
        let mut projection = centered(ProjectionKind::Spherical, Position3d::default());
        projection.start_angle_degrees = 90.0;
        assert!(coordinates(&projection, 0.0, 3.0, 0.0).0.abs() < 1.0e-9);
        assert!((coordinates(&projection, 0.0, -3.0, 0.0).0 - 180.0).abs() < 1.0e-9);

        projection.start_angle_degrees = 0.0;
        projection.elevation_degrees = 90.0;
        assert!(coordinates(&projection, 0.0, 0.0, 3.0).0.abs() < 1.0e-9);
        assert!((coordinates(&projection, 0.0, 0.0, -3.0).0 - 180.0).abs() < 1.0e-9);
        assert!((coordinates(&projection, 3.0, 0.0, 0.0).0 - 90.0).abs() < 1.0e-9);
    }

    #[test]
    fn the_new_kinds_do_not_require_a_view_direction() {
        // A cylindrical or spherical projection derives its own frame, so the zero view
        // direction that would reject a planar projection is fine here.
        for kind in [ProjectionKind::Cylindrical, ProjectionKind::Spherical] {
            let projection = centered(kind, Position3d::default());
            assert!(project_spatial_positions(&projection, &[target(1, 1.0, 1.0, 1.0)]).is_ok());
        }
        let planar = centered(ProjectionKind::Planar, Position3d::default());
        assert!(matches!(
            project_spatial_positions(&planar, &[target(1, 1.0, 1.0, 1.0)]),
            Err(SpatialMappingError::InvalidViewDirection)
        ));
    }

    #[test]
    fn a_fixture_on_the_anchor_itself_stays_rankable() {
        for kind in [ProjectionKind::Cylindrical, ProjectionKind::Spherical] {
            let projection = centered(kind, Position3d::default());
            assert_eq!(coordinates(&projection, 0.0, 0.0, 0.0), (0.0, 0.0));
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
            kind: ProjectionKind::Planar,
            axis_rotation: Vector3::default(),
            start_angle_degrees: 0.0,
            elevation_degrees: 0.0,
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
