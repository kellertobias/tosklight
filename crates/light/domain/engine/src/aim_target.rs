//! Semantic Aim targets shared by observed output and portable live Preset compilation.
use crate::{FixtureMountFrame, ResolvedPointPose};
use light_core::programming::{PositionIntent, TargetReference};
use light_fixture::PatchedFixture;

pub fn aim_target_from_geometry(
    fixtures: &[PatchedFixture],
    points: &[ResolvedPointPose],
    mounts: &FixtureMountFrame,
    number: u32,
) -> Result<Option<PositionIntent>, String> {
    let target = fixtures
        .iter()
        .find(|fixture| fixture.fixture_number == Some(number))
        .ok_or_else(|| format!("no fixture numbered {number}"))?;
    // Point channels may belong to logical heads, but references always name the root UUID.
    if points
        .iter()
        .any(|point| point.fixture_id == target.fixture_id)
    {
        return Ok(Some(PositionIntent::target(
            TargetReference::Point {
                point_id: target.fixture_id.0,
            },
            [0.0; 3],
        )));
    }
    // The scalar mount projection may fall back to saved placement for a missing Point.
    // That fallback must not freeze an explicitly tracked relation into a fixed Origin target.
    if target
        .position_master
        .is_some_and(|id| !points.iter().any(|point| point.fixture_id.0 == id))
    {
        return Ok(None);
    }
    let Some(mount) = mounts.mount(target.fixture_id.0) else {
        return Ok(None);
    };
    let Some(world) = mount.world_from_fixture else {
        return Ok(None);
    };
    let world = world.point([0.0; 3]);
    let (reference, offset) = if let Some(point) = mount
        .position_master
        .and_then(|id| points.iter().find(|point| point.fixture_id == id))
    {
        let Some(rotation) =
            light_core::spatial::RigidTransform::euler_xyz(point.rotation_degrees.map(f64::from))
        else {
            return Ok(None);
        };
        let local = std::array::from_fn(|axis| {
            world[axis]
                - f64::from(point.origin_metres[axis])
                - f64::from(point.offset_metres[axis])
        });
        (
            TargetReference::Point {
                point_id: point.fixture_id.0,
            },
            rotation.inverse().direction(local),
        )
    } else {
        (TargetReference::Origin, world)
    };
    let intent = PositionIntent::target(reference, offset.map(|value| value as f32));
    intent.validate().map_err(|error| error.to_string())?;
    Ok(Some(intent))
}

/// Resolve only requested saved targets using the same Point/default/freeze and mount semantics
/// as an output generation, without creating an engine or consulting a different live show.
pub fn saved_aim_targets(
    fixtures: &[PatchedFixture],
    numbers: impl IntoIterator<Item = u32>,
) -> std::collections::HashMap<u32, PositionIntent> {
    let slots = crate::SlotTable::compile(crate::next_generation(), fixtures);
    let points_index = crate::point_projection::PointProjectionIndex::compile(fixtures, &slots);
    let values = crate::FrameValues::from_maps(
        crate::runtime_generation::compile_fixture_default_values(fixtures),
        Default::default(),
    );
    let points = points_index.resolve_saved(&values);
    let mounts_index =
        crate::mount_projection::MountProjectionIndex::compile(fixtures, &points_index);
    let mounts = mounts_index.resolve(&points, &mut Default::default());
    numbers
        .into_iter()
        .filter_map(|number| {
            aim_target_from_geometry(fixtures, &points, &mounts, number)
                .ok()
                .flatten()
                .map(|intent| (number, intent))
        })
        .collect()
}
