//! `Fixture 1 AT Fixture 5`: point a selection at another object in the rig.
//!
//! The desk answers this from the show's own geometry, so it works while programming with nothing
//! drawn. What it aims at is where the target *is*, not where it was patched: a fixture slaved to
//! a 3D Point moves when the point moves, and the beam has to follow the object.

use super::*;

/// A fixture's placement once any 3D Point it is slaved to has been applied, in metres.
pub(super) fn world_mount(
    fixture: &light_fixture::PatchedFixture,
    points: &HashMap<light_core::FixtureId, PointTransform>,
) -> light_core::Mount {
    placed(
        [
            fixture.location.x as f32 / 1000.0,
            fixture.location.y as f32 / 1000.0,
            fixture.location.z as f32 / 1000.0,
        ],
        [fixture.rotation.x, fixture.rotation.y, fixture.rotation.z],
        fixture.position_master,
        points,
    )
}

/// Where a rigged placement actually ends up, given the points in the show.
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
pub(super) struct PointTransform {
    pub origin: [f32; 3],
    pub offset: [f32; 3],
    pub rotation_degrees: [f32; 3],
}

impl PointTransform {
    /// Turn a slave about the point's own origin, then move it. Same order as the visualizer, so
    /// the beam is aimed at the object an operator can see.
    fn carry(&self, position: [f32; 3], rotation: [f32; 3]) -> light_core::Mount {
        let local = [
            position[0] - self.origin[0],
            position[1] - self.origin[1],
            position[2] - self.origin[2],
        ];
        let turned = rotate(local, self.rotation_degrees);
        light_core::Mount {
            position: [
                self.origin[0] + turned[0] + self.offset[0],
                self.origin[1] + turned[1] + self.offset[1],
                self.origin[2] + turned[2] + self.offset[2],
            ],
            rotation_degrees: [
                rotation[0] + self.rotation_degrees[0],
                rotation[1] + self.rotation_degrees[1],
                rotation[2] + self.rotation_degrees[2],
            ],
        }
    }
}

/// `Rx * Ry * Rz`, matching how a mounting rotation is applied everywhere else.
fn rotate(v: [f32; 3], degrees: [f32; 3]) -> [f32; 3] {
    let [rx, ry, rz] = degrees;
    let (sz, cz) = rz.to_radians().sin_cos();
    let after_z = [v[0] * cz - v[1] * sz, v[0] * sz + v[1] * cz, v[2]];
    let (sy, cy) = ry.to_radians().sin_cos();
    let after_y = [
        after_z[0] * cy + after_z[2] * sy,
        after_z[1],
        -after_z[0] * sy + after_z[2] * cy,
    ];
    let (sx, cx) = rx.to_radians().sin_cos();
    [
        after_y[0],
        after_y[1] * cx - after_y[2] * sx,
        after_y[1] * sx + after_y[2] * cx,
    ]
}

/// The live poses of every 3D Point in the show.
pub(super) fn point_transforms(
    snapshot: &light_engine::EngineSnapshot,
    resolved: &light_engine::ResolvedValues,
) -> HashMap<light_core::FixtureId, PointTransform> {
    let mut points = HashMap::new();
    for fixture in snapshot.fixtures.iter() {
        if !is_point(fixture) {
            continue;
        }
        let axis = |name: &str, low: f32, high: f32| {
            resolved
                .get(&(fixture.fixture_id, light_core::AttributeKey(name.into())))
                .and_then(light_core::AttributeValue::normalized)
                .map_or(0.0, |value| low + value * (high - low))
        };
        points.insert(
            fixture.fixture_id,
            PointTransform {
                origin: [
                    fixture.location.x as f32 / 1000.0,
                    fixture.location.y as f32 / 1000.0,
                    fixture.location.z as f32 / 1000.0,
                ],
                offset: [
                    axis("point.position.x", -100.0, 100.0),
                    axis("point.position.y", -100.0, 100.0),
                    axis("point.position.z", -100.0, 100.0),
                ],
                rotation_degrees: [
                    axis("point.rotation.x", -180.0, 180.0),
                    axis("point.rotation.y", -180.0, 180.0),
                    axis("point.rotation.z", -180.0, 180.0),
                ],
            },
        );
    }
    points
}

fn is_point(fixture: &light_fixture::PatchedFixture) -> bool {
    fixture.definition.heads.iter().any(|head| {
        head.parameters
            .iter()
            .any(|parameter| parameter.attribute.0.as_ref() == "point.position.x")
    })
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
    let snapshot = state.output.snapshot();
    let resolved = state.output.resolved_values();
    let points = point_transforms(&snapshot, &resolved);
    let target = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_number == Some(target))
        .ok_or_else(|| format!("no fixture numbered {target}"))?;
    let aim_at = world_mount(target, &points).position;
    let mut assignments = Vec::new();
    for fixture_id in fixtures {
        let Some(fixture) = snapshot
            .fixtures
            .iter()
            .find(|candidate| candidate.fixture_id == *fixture_id)
        else {
            continue;
        };
        let mount = world_mount(fixture, &points);
        let Some((pan, tilt)) = light_core::pan_tilt_towards(mount, aim_at) else {
            continue;
        };
        for (attribute, degrees) in [("pan", pan), ("tilt", tilt)] {
            if let Some((key, value)) = normalized_angle(fixture, attribute, degrees) {
                assignments.push((*fixture_id, key, value));
            }
        }
    }
    if assignments.is_empty() {
        return Err("nothing in the selection can be aimed".into());
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
            // Two metres up the stage and a quarter turn about the point's own origin.
            point([2.0, 6.0, 0.0], [0.0, -1.0, 0.0], [0.0, 90.0, 0.0]),
        )]);
        let mount = placed([4.0, 6.0, 0.0], [0.0; 3], Some(master), &points);
        // The same answer the visualizer gives: turned about the point, then moved with it.
        assert!(
            (mount.position[0] - 2.0).abs() < 1e-4,
            "{:?}",
            mount.position
        );
        assert!(
            (mount.position[1] - 5.0).abs() < 1e-4,
            "{:?}",
            mount.position
        );
        assert!(
            (mount.position[2] + 2.0).abs() < 1e-4,
            "{:?}",
            mount.position
        );
        assert!((mount.rotation_degrees[1] - 90.0).abs() < 1e-4);
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
