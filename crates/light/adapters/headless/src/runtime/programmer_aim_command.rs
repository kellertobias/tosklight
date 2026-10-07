//! `Fixture 1 AT Fixture 5`: point a selection at another object in the rig.
//!
//! The desk answers this from the show's own geometry, so it works while programming with nothing
//! drawn. What it aims at is where the target *is*, not where it was patched: a fixture slaved to
//! a 3D Point moves when the point moves, and the beam has to follow the object.

use super::*;

mod semantic;
pub(super) use semantic::{
    aim_target_intent, apply_position_mutations, apply_semantic_aim, resolve_aim_preset,
};

/// Where a rigged placement actually ends up, given the points in the show.
#[cfg(test)]
fn placed(
    position: [f32; 3],
    rotation: [f32; 3],
    master: Option<uuid::Uuid>,
    points: &HashMap<light_core::FixtureId, PointTransform>,
) -> light_core::Mount {
    let Some(master) = master.and_then(|master| points.get(&light_core::FixtureId(master))) else {
        return light_core::Mount {
            position,
            rotation_degrees: rotation,
        };
    };
    master.carry(position, rotation)
}

/// One 3D Point's live contribution, read from the resolved values.
#[derive(Clone, Copy, Debug, Default)]
#[cfg(test)]
struct PointTransform {
    pub origin: [f32; 3],
    pub offset: [f32; 3],
    pub rotation_degrees: [f32; 3],
}

#[cfg(test)]
impl PointTransform {
    /// Turn a slave about the point's own origin, then move it.
    ///
    /// Compose wholly in desk Z-up coordinates. No renderer is needed to resolve a mount.
    fn carry(&self, position: [f32; 3], rotation: [f32; 3]) -> light_core::Mount {
        use light_core::spatial::RigidTransform as Transform;
        let turn = Transform::euler_xyz(self.rotation_degrees.map(f64::from))
            .unwrap_or(Transform::IDENTITY);
        let local = std::array::from_fn(|i| f64::from(position[i] - self.origin[i]));
        let translated = turn.direction(local);
        let mounted = turn
            .compose(Transform::euler_xyz(rotation.map(f64::from)).unwrap_or(Transform::IDENTITY));
        light_core::Mount {
            position: std::array::from_fn(|i| {
                self.origin[i] + self.offset[i] + translated[i] as f32
            }),
            rotation_degrees: mounted.euler_xyz_degrees().map(|v| v as f32),
        }
    }
}

/// Normalize `degrees` onto the fixture's own pan or tilt range.
///
/// A fixture whose range does not reach the angle is aimed as close as it can turn rather than
/// wrapped to something it can physically do but that points somewhere else entirely.
pub(super) fn normalized_angle(
    fixture: &light_fixture::PatchedFixture,
    attribute: &str,
    degrees: f32,
) -> Option<(light_core::AttributeKey, light_core::AttributeValue)> {
    let parameter = fixture.definition.heads.iter().find_map(|head| {
        head.parameters
            .iter()
            .find(|parameter| parameter.attribute.0.as_ref() == attribute)
    })?;
    let low = parameter.metadata.physical_min;
    let high = parameter.metadata.physical_max;
    if !(high - low).is_finite() || (high - low).abs() < f32::EPSILON {
        return None;
    }
    let normalized = ((degrees - low) / (high - low)).clamp(0.0, 1.0);
    Some((
        light_core::AttributeKey(attribute.into()),
        light_core::AttributeValue::Normalized(normalized),
    ))
}

/// Aim every selected fixture at the fixture numbered `target`.
///
/// A fixture with no pan or tilt is skipped rather than refused: pointing a wash of movers and a
/// few fixed lanterns at the same object is an ordinary thing to ask, and the fixed ones simply
/// have nothing to turn.
pub(super) fn aim_selection(
    state: &AppState,
    fixtures: &[light_core::FixtureId],
    target: u32,
) -> Result<
    Vec<(
        light_core::FixtureId,
        light_core::AttributeKey,
        light_core::AttributeValue,
    )>,
    String,
> {
    let source = state.output.engine().observe_source_frame(&[]);
    let snapshot = source.snapshot();
    let mount = |fixture: light_core::FixtureId| {
        let transform = source.mounts().mount(fixture.0)?.world_from_fixture?;
        Some(light_core::Mount {
            position: transform.point([0.0; 3]).map(|value| value as f32),
            rotation_degrees: transform.euler_xyz_degrees().map(|value| value as f32),
        })
    };
    let target = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_number == Some(target))
        .ok_or_else(|| format!("no fixture numbered {target}"))?;
    if state.output.supported_programming_contract()
        >= light_core::programming::PROGRAMMING_CONTRACT_VERSION
    {
        let Some(intent) =
            semantic::target_intent_from_frame(&source, target.fixture_number.unwrap())?
        else {
            return Ok(Vec::new());
        };
        let value = light_core::AttributeValue::Position(Arc::new(intent));
        return Ok(fixtures
            .iter()
            .filter(|id| {
                snapshot.fixtures.iter().any(|fixture| {
                    fixture.fixture_id == **id
                        || fixture
                            .logical_heads
                            .iter()
                            .any(|head| head.fixture_id == **id)
                })
            })
            .map(|id| {
                (
                    *id,
                    light_core::programming::ProgrammingOwner::Position.key(),
                    value.clone(),
                )
            })
            .collect());
    }
    // The explicit target is validated first: an empty selection makes a valid Aim a quiet
    // no-op, but never turns a missing target into a success (NOTICE-002).
    if fixtures.is_empty() {
        return Ok(Vec::new());
    }
    let aim_at = source
        .points()
        .iter()
        .find(|point| point.fixture_id == target.fixture_id)
        .map(|point| {
            std::array::from_fn(|axis| point.origin_metres[axis] + point.offset_metres[axis])
        })
        .or_else(|| mount(target.fixture_id).map(|mount| mount.position));
    let Some(aim_at) = aim_at else {
        // Unknown placement is passive; do not invent a stage-origin target.
        return Ok(Vec::new());
    };
    let mut assignments = Vec::new();
    for fixture_id in fixtures {
        let Some(fixture) = snapshot
            .fixtures
            .iter()
            .find(|candidate| candidate.fixture_id == *fixture_id)
        else {
            continue;
        };
        let Some(mount) = mount(fixture.fixture_id) else {
            continue;
        };
        let Some((pan, tilt)) = light_core::pan_tilt_towards(mount, aim_at) else {
            continue;
        };
        for (attribute, degrees) in [("pan", pan), ("tilt", tilt)] {
            if let Some((key, value)) = normalized_angle(fixture, attribute, degrees) {
                assignments.push((*fixture_id, key, value));
            }
        }
    }
    Ok(assignments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(origin: [f32; 3], offset: [f32; 3], rotation: [f32; 3]) -> PointTransform {
        PointTransform {
            origin,
            offset,
            rotation_degrees: rotation,
        }
    }

    #[test]
    fn a_fixture_with_no_master_stands_where_the_patch_put_it() {
        let mount = placed([2.0, 6.0, -1.0], [10.0, 0.0, 0.0], None, &HashMap::new());
        assert_eq!(mount.position, [2.0, 6.0, -1.0]);
        assert_eq!(mount.rotation_degrees, [10.0, 0.0, 0.0]);
    }

    #[test]
    fn a_master_that_has_not_moved_changes_nothing() {
        let master = uuid::Uuid::from_u128(4);
        let points = HashMap::from([(
            light_core::FixtureId(master),
            point([0.0, 6.0, 0.0], [0.0; 3], [0.0; 3]),
        )]);
        let mount = placed([2.0, 6.0, -1.0], [0.0; 3], Some(master), &points);
        assert_eq!(mount.position, [2.0, 6.0, -1.0]);
    }

    #[test]
    fn a_master_carries_its_slave_when_it_moves_and_turns() {
        let master = uuid::Uuid::from_u128(4);
        let points = HashMap::from([(
            light_core::FixtureId(master),
            // One metre downstage and a quarter turn about the up axis, about the point's own
            // origin: a truss point flown in and swung round.
            point([2.0, 6.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 90.0]),
        )]);
        let mount = placed([4.0, 6.0, 0.0], [0.0; 3], Some(master), &points);
        // Two metres stage-right of the point becomes two metres upstage of it, then the whole
        // assembly comes one metre downstage. Turned about the point first, then moved with it.
        let expected = [2.0, 7.0, 0.0];
        for axis in 0..3 {
            assert!(
                (mount.position[axis] - expected[axis]).abs() < 1e-4,
                "{:?}",
                mount.position
            );
        }
        assert!((mount.rotation_degrees[2] - 90.0).abs() < 1e-4);
    }

    /// The desk and the renderer answer where a slaved fixture is with one implementation. This
    /// pins the axis conversion between them for a turn about every axis at once, which is where
    /// two separate implementations used to disagree.
    #[test]
    fn the_desk_places_a_slave_exactly_where_the_renderer_draws_it() {
        let transform = point([1.0, 2.0, 3.0], [0.5, -0.25, 0.75], [20.0, 35.0, -50.0]);
        let mount = transform.carry([4.0, 1.0, 5.0], [5.0, 10.0, 15.0]);
        let renderer = viz_project::viz_scene::slaved_to_point(
            viz_project::viz_scene::glam::Vec3::new(4.0, 5.0, -1.0),
            viz_project::viz_scene::desk_rotation_to_world([5.0, 10.0, 15.0]),
            &viz_project::viz_scene::PointPose {
                fixture_id: uuid::Uuid::nil(),
                origin_metres: [1.0, 3.0, -2.0],
                offset_metres: [0.5, 0.75, 0.25],
                rotation_degrees: viz_project::viz_scene::desk_rotation_to_world([
                    20.0, 35.0, -50.0,
                ])
                .to_array(),
            },
        );
        let drawn = [renderer.0.x, -renderer.0.z, renderer.0.y];
        for axis in 0..3 {
            assert!(
                (mount.position[axis] - drawn[axis]).abs() < 1e-4,
                "desk {:?} renderer {drawn:?}",
                mount.position
            );
        }
        let expected = viz_project::viz_scene::euler_degrees(renderer.1);
        let actual = viz_project::viz_scene::euler_degrees(
            viz_project::viz_scene::desk_rotation_to_world(mount.rotation_degrees),
        );
        for axis in [
            viz_project::viz_scene::glam::Vec3::X,
            viz_project::viz_scene::glam::Vec3::Y,
            viz_project::viz_scene::glam::Vec3::Z,
        ] {
            assert!((actual * axis - expected * axis).length() < 1e-4);
        }
    }

    #[test]
    fn an_unknown_master_leaves_the_fixture_alone() {
        // A point that has been deleted must not silently move the rig to the stage origin.
        let mount = placed(
            [2.0, 6.0, -1.0],
            [0.0; 3],
            Some(uuid::Uuid::from_u128(9)),
            &HashMap::new(),
        );
        assert_eq!(mount.position, [2.0, 6.0, -1.0]);
    }
}
