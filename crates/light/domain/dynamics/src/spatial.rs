use light_core::FixtureId;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

const MIN_DIRECTION_LENGTH: f64 = 1.0e-12;

const UNIT_X: Vector3 = Vector3 {
    x: 1.0,
    y: 0.0,
    z: 0.0,
};
const UNIT_Y: Vector3 = Vector3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};
const UNIT_Z: Vector3 = Vector3 {
    x: 0.0,
    y: 0.0,
    z: 1.0,
};

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
    /// `u` is the angular distance from the centre direction, `v` the turn around it from the
    /// start angle.
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

/// Every kind is placed by one position and one direction, so the three of them share a shape.
///
/// `view_direction` is what the projection is oriented along: the viewing direction for planar,
/// the central axis for cylindrical, the direction to the centre of the spread for spherical.
/// `rotation_degrees` is the one remaining degree of freedom, the roll about that direction: the
/// turn of the viewing plane for planar, the start angle around the axis for cylindrical, and the
/// prime meridian a spherical spread measures its turn from.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SpatialProjection {
    pub anchor: Position3d,
    pub view_direction: Vector3,
    pub rotation_degrees: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<ProjectionPreset>,
    /// Omitted at its default, so a planar projection persists exactly the bytes it always did.
    #[serde(default, skip_serializing_if = "ProjectionKind::is_planar")]
    pub kind: ProjectionKind,
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
            // Shows written while the angular kinds carried their own angles. Their presence is
            // what marks a projection as one of those, so they are read but never written back.
            axis_rotation: Option<Vector3>,
            start_angle_degrees: Option<f64>,
            elevation_degrees: Option<f64>,
        }

        let stored = StoredProjection::deserialize(deserializer)?;
        let legacy_angles = stored.axis_rotation.is_some()
            || stored.start_angle_degrees.is_some()
            || stored.elevation_degrees.is_some();
        let mut projection = Self {
            anchor: stored.anchor,
            view_direction: stored.view_direction,
            rotation_degrees: stored.rotation_degrees,
            preset: stored.preset,
            kind: stored.kind,
        };
        if legacy_angles {
            projection.migrate_legacy_angles(
                stored.axis_rotation.unwrap_or_default(),
                stored.start_angle_degrees.unwrap_or_default(),
                stored.elevation_degrees.unwrap_or_default(),
            );
        }
        // An angular kind used to ignore the direction entirely, so one could be stored with
        // none at all. Its old frame is what it gets: a cylinder stood up, a sphere centred
        // where both of its angles at zero put it.
        if projection.kind != ProjectionKind::Planar
            && normalize(projection.view_direction).is_err()
        {
            projection.view_direction = match projection.kind {
                ProjectionKind::Spherical => UNIT_X,
                _ => UNIT_Z,
            };
        }
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
        }
    }

    /// Rewrites a Show that stored an angular kind as Euler angles into the direction and roll
    /// that replaced them, so it keeps ranking its fixtures in exactly the same order.
    ///
    /// A planar projection never used these, so it is left alone.
    fn migrate_legacy_angles(
        &mut self,
        axis_rotation: Vector3,
        start_angle_degrees: f64,
        elevation_degrees: f64,
    ) {
        match self.kind {
            ProjectionKind::Planar => {}
            ProjectionKind::Cylindrical => {
                let axis = rotate_euler(UNIT_Z, axis_rotation);
                let Ok(axis) = normalize(axis) else { return };
                // The old frame carried its own reference around the axis. The new frame derives
                // one from the axis alone, so the turn between them folds into the roll.
                let stored_start = normalize(reject(rotate_euler(UNIT_X, axis_rotation), axis));
                let Ok(stored_start) = stored_start else {
                    return;
                };
                let Ok((derived_start, derived_side)) = axis_reference(axis) else {
                    return;
                };
                let offset = dot(stored_start, derived_side)
                    .atan2(dot(stored_start, derived_start))
                    .to_degrees();
                self.view_direction = axis;
                self.rotation_degrees = start_angle_degrees + offset;
            }
            ProjectionKind::Spherical => {
                let azimuth = normalize_degrees(start_angle_degrees).to_radians();
                let elevation = normalize_degrees(elevation_degrees).to_radians();
                self.view_direction = Vector3 {
                    x: elevation.cos() * azimuth.cos(),
                    y: elevation.cos() * azimuth.sin(),
                    z: elevation.sin(),
                };
                self.rotation_degrees = 0.0;
            }
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
    Bottom,
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
            Self::Bottom => Vector3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
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

    let shape = match &mapping_override.shape {
        OverrideStage::Inherit => inherited.map(|mapping| mapping.shape.clone()),
        OverrideStage::Replace(shape) => dynamic_spatial_shape(shape),
    };
    let Some(shape) = shape else {
        return Ok(evaluate_source_order(targets));
    };
    let projection = match &mapping_override.projection {
        OverrideStage::Inherit => inherited
            .map(|mapping| mapping.projection.clone())
            .unwrap_or_else(|| {
                SpatialProjection::from_preset(ProjectionPreset::Top, Position3d::default())
            }),
        OverrideStage::Replace(projection) => projection.clone(),
    };
    evaluate_spatial_mapping(&SpatialSelectionMapping { projection, shape }, targets)
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
    // Every kind is oriented by the same direction, so every kind needs it to be usable.
    for (field, value) in [
        ("projection.anchor.x", projection.anchor.x),
        ("projection.anchor.y", projection.anchor.y),
        ("projection.anchor.z", projection.anchor.z),
        ("projection.rotation_degrees", projection.rotation_degrees),
        ("projection.view_direction.x", projection.view_direction.x),
        ("projection.view_direction.y", projection.view_direction.y),
        ("projection.view_direction.z", projection.view_direction.z),
    ] {
        require_finite(field, value)?;
    }
    normalize(projection.view_direction)?;
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
    let reference_up = if dot(direction, UNIT_Z).abs() > 1.0 - MIN_DIRECTION_LENGTH {
        UNIT_Y
    } else {
        UNIT_Z
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

/// Applies Euler degrees about X, then Y, then Z. Only the legacy angle migration still uses it.
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

/// A perpendicular pair spanning the plane at right angles to a unit `axis`, derived from the
/// axis alone so that the direction is the only thing that places it.
///
/// World +X is the reference, which keeps an axis of world +Z measuring its angles from world +X.
fn axis_reference(axis: Vector3) -> Result<(Vector3, Vector3), SpatialMappingError> {
    let reference = if dot(axis, UNIT_X).abs() > 1.0 - MIN_DIRECTION_LENGTH {
        UNIT_Z
    } else {
        UNIT_X
    };
    let start = normalize(reject(reference, axis))?;
    Ok((start, cross(axis, start)))
}

/// The axis and the two perpendicular directions that place the start angle around it.
///
/// The axis is the projection's direction and the start angle is its roll about that axis, so an
/// axis of world +Z with no roll starts at world +X. A cylinder rolls its seam this way and a
/// sphere its prime meridian, which is the same construction on the same two numbers.
fn cylinder_frame(
    projection: &SpatialProjection,
) -> Result<(Vector3, Vector3, Vector3), SpatialMappingError> {
    let axis = normalize(projection.view_direction)?;
    let (start, side) = axis_reference(axis)?;
    let radians = normalize_degrees(projection.rotation_degrees).to_radians();
    let (sin, cos) = radians.sin_cos();
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
        // `u` is the great-circle distance from the centre direction, reaching 180 at the
        // antipode. `v` is the turn around that centre, measured from the rotation angle, so
        // the roll is what decides which side of the spread a fixture falls on.
        ProjectionKind::Spherical => match cylinder_frame(projection) {
            Ok((center, start, side)) => {
                let angle = match normalize(relative) {
                    Ok(direction) => angle_between(direction, center),
                    Err(_) => 0.0,
                };
                let around = reject(relative, center);
                let meridian = if length(around) <= MIN_DIRECTION_LENGTH {
                    0.0
                } else {
                    dot(around, side).atan2(dot(around, start)).to_degrees()
                };
                (angle, meridian)
            }
            Err(_) => (0.0, 0.0),
        },
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
mod tests;
