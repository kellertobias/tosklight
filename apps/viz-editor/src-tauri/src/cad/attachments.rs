//! Persist only profile-declared lamp clamps onto independent stationary rig pipes.
//! Geometry snapping remains the CAD view's job; connector alignment is not lamp mounting.
use super::*;

type Point = [f32; 3];

pub(super) fn snap_attachments(
    session: &Session,
    moved: &[EntityTransform],
) -> Result<Vec<RigAttachment>, String> {
    let scene =
        session.with(|document| document.patch_snapshot().map_err(|error| error.to_string()))?;
    let entities = entities(&scene, &BTreeSet::new(), &fixture_notes(session)?);
    let moving: BTreeSet<_> = moved.iter().map(|fixture| fixture.id).collect();
    // Retained profile authority is required; cad_mounting's legacy drawing fallback is not consent.
    let declared: BTreeSet<_> = scene
        .fixtures
        .iter()
        .filter(|fixture| {
            scene
                .profile_revisions
                .iter()
                .find(|profile| {
                    profile.profile_id == fixture.profile.profile_id
                        && profile.profile_revision == fixture.profile.profile_revision
                })
                .is_some_and(|profile| {
                    profile.patch_policy == light_fixture::PatchPolicy::Dmx
                        && profile
                            .profile_snapshot
                            .get("mounting")
                            .is_some_and(|value| value.is_object())
                })
        })
        .map(|fixture| fixture.patch.fixture_id.0)
        .collect();
    let still: Vec<_> = entities
        .iter()
        .filter(|entity| !moving.contains(&entity.logical_fixture_id))
        .filter_map(|entity| pipes(entity).map(|pipes| (entity, pipes)))
        .collect();
    let mut created = Vec::new();
    for fixture in moved {
        let Some(lamp) = entities.iter().find(|entity| {
            entity.id == fixture.id
                && entity.id == entity.logical_fixture_id
                && entity.kind != "venue"
                && declared.contains(&entity.logical_fixture_id)
        }) else {
            continue;
        };
        let Some(clip) = lamp.mounting.as_ref().filter(|clip| {
            clip.hardware == "clamp"
                && clip
                    .half_extent
                    .iter()
                    .all(|value| value.is_finite() && *value > 0.0)
                && clip
                    .centre
                    .iter()
                    .chain(clip.pipe.iter())
                    .all(|value| value.is_finite())
        }) else {
            continue;
        };
        let centre = placed(lamp, clip.centre);
        let target = still
            .iter()
            .flat_map(|(rig, segments)| {
                segments.iter().filter_map(|(start, end, radius)| {
                    let point = closest(centre, *start, *end);
                    let local = unrotate(sub(point, centre), lamp.rotation_degrees);
                    if (0..3).any(|axis| local[axis].abs() > clip.half_extent[axis] + radius) {
                        return None;
                    }
                    let distance = sub(point, placed(lamp, clip.pipe))
                        .iter()
                        .map(|value| value * value)
                        .sum::<f32>();
                    Some((*rig, point, distance))
                })
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let Some((rig, point, _)) = target else {
            continue;
        };
        let local = sub(point, rig.position_millimetres.map(|value| value as f32));
        let attachment = RigAttachment {
            fixture_id: fixture.id,
            truss_member_id: rig.id,
            mounting_point_id: format!(
                "{}:pipe:{:.0}:{:.0}:{:.0}",
                rig.id, local[0], local[1], local[2]
            ),
            local_transform: EntityTransform {
                stage_position_set: None,
                copy_stage_position_set: None,
                id: fixture.id,
                position_millimetres: std::array::from_fn(|axis| {
                    fixture.position_millimetres[axis] - rig.position_millimetres[axis]
                }),
                rotation_degrees: fixture.rotation_degrees,
            },
        };
        session.change(|document| {
            document
                .put_object(
                    "rig_attachment",
                    &fixture.id.to_string(),
                    &serde_json::to_value(&attachment).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())
        })?;
        created.push(attachment);
    }
    let attached: BTreeSet<_> = created
        .iter()
        .map(|attachment| attachment.fixture_id)
        .collect();
    for fixture in moved
        .iter()
        .filter(|fixture| !attached.contains(&fixture.id))
    {
        session.change(|document| {
            document
                .delete_object("rig_attachment", &fixture.id.to_string())
                .map(|_| ())
                .map_err(|error| error.to_string())
        })?;
    }
    Ok(created)
}

/// The same straight-run chord geometry as venueShapes.trussPipes/trussPlan.trussParts.
fn pipes(entity: &CadEntity) -> Option<Vec<(Point, Point, f32)>> {
    let scenery = entity
        .scenery
        .as_ref()
        .filter(|scenery| scenery.kind == "truss" && (1..=4).contains(&scenery.chords))?;
    let size = entity.size_millimetres;
    if size.iter().any(|value| !value.is_finite() || *value <= 0.0) {
        return None;
    }
    let run = (0..3).max_by(|a, b| size[*a].total_cmp(&size[*b]).then_with(|| b.cmp(a)))?;
    let (across, up) = match run {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    let section = size[across].max(size[up]);
    let diameter = if scenery.chords == 1 {
        section.clamp(20.0, 100.0)
    } else {
        (section * 0.17).clamp(25.0, 60.0)
    };
    let half = diameter.max(section - diameter) / 2.0;
    let offsets = match scenery.chords {
        1 => vec![(0.0, 0.0)],
        2 => vec![(0.0, half), (0.0, -half)],
        3 => vec![(0.0, half), (half, -half), (-half, -half)],
        _ => vec![(half, half), (-half, half), (-half, -half), (half, -half)],
    };
    Some(
        offsets
            .into_iter()
            .map(|(side, height)| {
                let at = |along| {
                    let mut point = [0.0; 3];
                    point[run] = along;
                    point[across] = side;
                    point[up] = height;
                    placed(entity, point)
                };
                (at(-size[run] / 2.0), at(size[run] / 2.0), diameter / 2.0)
            })
            .collect(),
    )
}
fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|axis| a[axis] - b[axis])
}
fn closest(point: Point, start: Point, end: Point) -> Point {
    let along = sub(end, start);
    let offset = sub(point, start);
    let length = along.iter().map(|value| value * value).sum::<f32>();
    let t = if length > 0.0 {
        (0..3).map(|axis| offset[axis] * along[axis]).sum::<f32>() / length
    } else {
        0.0
    };
    std::array::from_fn(|axis| start[axis] + along[axis] * t.clamp(0.0, 1.0))
}
fn placed(entity: &CadEntity, point: Point) -> Point {
    let turned = rotate(point, entity.rotation_degrees);
    std::array::from_fn(|axis| turned[axis] + entity.position_millimetres[axis] as f32)
}
/// Desk-space XYZ placement: apply Z, then Y, then X (CAD projection's exact convention).
fn rotate(mut point: Point, degrees: Point) -> Point {
    for axis in [2, 1, 0] {
        point = turn(point, axis, degrees[axis].to_radians());
    }
    point
}
fn unrotate(mut point: Point, degrees: Point) -> Point {
    for axis in [0, 1, 2] {
        point = turn(point, axis, -degrees[axis].to_radians());
    }
    point
}
fn turn(p: Point, axis: usize, angle: f32) -> Point {
    let (s, c) = angle.sin_cos();
    match axis {
        0 => [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c],
        1 => [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c],
        _ => [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]],
    }
}
