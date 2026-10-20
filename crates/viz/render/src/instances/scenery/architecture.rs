//! Parametric venue surfaces. Visibility never removes physical light blockers.
use super::*;
use light_fixture::{VenueArchitecture, VenueSlabPurpose};

pub(crate) fn push(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    style: &FrameStyle,
    hidden: bool,
) {
    push_context(frame, object, style, hidden, &[], None);
}
fn push_context(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    style: &FrameStyle,
    hidden: bool,
    stairs: &[light_fixture::VenueStairVolume],
    joined: Option<&light_fixture::VenueGeometry>,
) {
    let Some(architecture) = &object.architecture else {
        return;
    };
    let generated;
    let geometry = if let Some(joined) = joined {
        joined
    } else {
        let Ok(result) = light_fixture::venue_geometry_in_context(
            architecture,
            object.size.as_dvec3().to_array().map(|v| v * 1000.0),
            millimetre_pose(object),
            stairs,
        ) else {
            return;
        };
        generated = result;
        &generated
    };
    push_geometry(frame, object, style, hidden, architecture, geometry);
}
fn push_geometry(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    style: &FrameStyle,
    hidden: bool,
    architecture: &VenueArchitecture,
    geometry: &light_fixture::VenueGeometry,
) {
    let pose =
        Mat4::from_rotation_translation(euler_degrees(object.rotation_degrees), object.position);
    if !style.scenery_surfaces {
        if hidden {
            return;
        }
        let edges: Vec<[Vec3; 2]> =
            if style.plot && style.projection_view == viz_scene::ProjectionView::Top {
                geometry
                    .plan
                    .iter()
                    .map(|edge| {
                        edge.map(|p| {
                            pose.transform_point3(Vec3::new(
                                p[0] as f32 * 0.001,
                                0.0,
                                p[1] as f32 * 0.001,
                            ))
                        })
                    })
                    .collect()
            } else if geometry.joined_wall {
                geometry
                    .exterior
                    .iter()
                    .map(|edge| {
                        edge.map(|p| {
                            pose.transform_point3(Vec3::from_array(p.map(|v| v as f32 * 0.001)))
                        })
                    })
                    .collect()
            } else {
                geometry
                    .triangles
                    .iter()
                    .flat_map(|triangle| {
                        let points = triangle.map(|p| {
                            pose.transform_point3(Vec3::from_array(p.map(|v| v as f32 * 0.001)))
                        });
                        [
                            [points[0], points[1]],
                            [points[1], points[2]],
                            [points[2], points[0]],
                        ]
                    })
                    .collect()
            };
        for edge in edges {
            for position in edge {
                frame.lines.push(super::super::LineVertex {
                    position: position.to_array(),
                    _pad: 0.0,
                    colour: style.faint_ink.extend(1.0).to_array(),
                });
            }
        }
        return;
    }
    let colour = match architecture {
        VenueArchitecture::Chair {
            style: light_fixture::VenueChairStyle::White,
        } => Vec3::splat(0.88),
        VenueArchitecture::Chair { .. } => Vec3::splat(0.035),
        _ => Vec3::from(object.colour),
    };
    let physical = matches!(
        architecture,
        VenueArchitecture::Wall { .. }
            | VenueArchitecture::Slab { .. }
            | VenueArchitecture::Stairs { .. }
    );
    let mut triangles = geometry.triangles.iter().peekable();
    let mut triangle_index = 0;
    while let Some(triangle) = triangles.next() {
        let metallic = if geometry.metallic[triangle_index] {
            0.7
        } else {
            0.0
        };
        triangle_index += 1;
        let first = *triangle;
        if physical {
            if let Some(next) = triangles.peek()
                && first[0] == next[0]
                && first[2] == next[1]
            {
                let corners = [first[0], first[1], first[2], next[2]].map(|p| {
                    pose.transform_point3(Vec3::from_array(p.map(|v| v as f32 * 0.001)))
                        .extend(1.0)
                        .to_array()
                });
                frame.curtain_blockers.push(GpuCurtainBlocker {
                    world_to_local: corners,
                    half_size: [1.0, 1.0, 2.0, 0.0],
                });
                // The second triangle still draws, but its physical quad is already recorded.
                draw_triangle(
                    frame,
                    **triangles.peek().unwrap(),
                    pose,
                    colour,
                    object.roughness,
                    hidden,
                    0.0,
                );
                triangle_index += 1;
                triangles.next();
            } else {
                let [a, b, c] = first
                    .map(|p| pose.transform_point3(Vec3::from_array(p.map(|v| v as f32 * 0.001))));
                let normal = (b - a).cross(c - a).normalize_or_zero();
                if normal.length_squared() > 0.0 {
                    let transform = Mat4::from_cols(
                        (b - a).extend(0.0),
                        (c - a).extend(0.0),
                        normal.extend(0.0),
                        a.extend(1.0),
                    );
                    frame.curtain_blockers.push(GpuCurtainBlocker {
                        world_to_local: transform.inverse().to_cols_array_2d(),
                        half_size: [1.0, 1.0, 1.0, 0.0],
                    });
                }
            }
        }
        draw_triangle(
            frame,
            first,
            pose,
            if metallic > 0.0 {
                Vec3::splat(0.42)
            } else {
                colour
            },
            object.roughness,
            hidden,
            metallic,
        );
    }
}

fn draw_triangle(
    frame: &mut FrameInstances,
    triangle: [[f64; 3]; 3],
    pose: Mat4,
    colour: Vec3,
    roughness: f32,
    hidden: bool,
    metallic: f32,
) {
    let [a, b, c] =
        triangle.map(|p| pose.transform_point3(Vec3::from_array(p.map(|v| v as f32 * 0.001))));
    let normal = (b - a).cross(c - a);
    if normal.length_squared() < 1e-12 {
        return;
    }
    let transform = Mat4::from_cols(
        (b - a).extend(0.0),
        (c - a).extend(0.0),
        normal.normalize().extend(0.0),
        a.extend(1.0),
    );
    let mut instance = MeshInstance::new(transform, colour, roughness, Vec3::ZERO, metallic);
    instance.surface[1] = if hidden { 1.0 } else { 0.0 };
    frame.mesh(MeshKind::Triangle).push(instance);
}

#[derive(Debug)]
struct Room {
    walls: Vec<(usize, bool)>,
    outline: Vec<glam::Vec2>,
    floor: f32,
    height: f32,
}

/// Walls connect at their physical centreline endpoints. Traversal keeps the directed edge for
/// an inward normal, supporting concave rooms and either authoring direction.
fn rooms(objects: &[SceneryObject]) -> Vec<Room> {
    let walls: Vec<_> = objects
        .iter()
        .enumerate()
        .filter_map(|(index, object)| {
            let Some(VenueArchitecture::Wall {
                ceiling_height_millimetres,
                ..
            }) = object.architecture
            else {
                return None;
            };
            let across = euler_degrees(object.rotation_degrees) * Vec3::X * object.size.x / 2.0;
            let start = object.position - across;
            let end = object.position + across;
            Some(light_fixture::VenueWallRun {
                id: Uuid::from_u128(index as u128 + 1),
                start_millimetres: [f64::from(start.x) * 1000.0, f64::from(start.z) * 1000.0],
                end_millimetres: [f64::from(end.x) * 1000.0, f64::from(end.z) * 1000.0],
                floor_millimetres: f64::from(object.position.y - object.size.y / 2.0) * 1000.0,
                height_millimetres: f64::from(object.size.y) * 1000.0,
                ceiling_height_millimetres,
            })
        })
        .collect();
    light_fixture::venue_rooms(&walls)
        .into_iter()
        .map(|r| Room {
            walls: r
                .wall_ids
                .iter()
                .map(|id| ((id.as_u128() - 1) as usize, true))
                .collect(),
            outline: r
                .outline_millimetres
                .iter()
                .map(|p| glam::Vec2::new(p[0] as f32 * 0.001, p[1] as f32 * 0.001))
                .collect(),
            floor: r.floor_millimetres as f32 * 0.001,
            height: r.height_millimetres as f32 * 0.001,
        })
        .collect()
}

fn inside(p: glam::Vec2, outline: &[glam::Vec2]) -> bool {
    let mut hit = false;
    for i in 0..outline.len() {
        let a = outline[i];
        let b = outline[(i + 1) % outline.len()];
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            hit = !hit;
        }
    }
    hit
}

pub(crate) fn push_all(frame: &mut FrameInstances, objects: &[SceneryObject], style: &FrameStyle) {
    let stairs: Vec<_> = objects
        .iter()
        .filter(|object| matches!(object.architecture, Some(VenueArchitecture::Stairs { .. })))
        .map(|object| {
            light_fixture::venue_stair_volume(
                object.size.as_dvec3().to_array().map(|v| v * 1000.0),
                millimetre_pose(object),
            )
        })
        .collect();
    let joined = joins::resolve(objects);
    let rooms = rooms(objects);
    let mut hidden = std::collections::HashSet::new();
    if let Some(camera) = style.camera_position {
        for room in &rooms {
            let interior = inside(glam::Vec2::new(camera.x, camera.z), &room.outline)
                && camera.y >= room.floor
                && camera.y < room.floor + room.height;
            if interior {
                continue;
            }
            let area: f32 = (0..room.outline.len())
                .map(|i| room.outline[i].perp_dot(room.outline[(i + 1) % room.outline.len()]))
                .sum();
            for (edge, (index, _)) in room.walls.iter().enumerate() {
                let a = room.outline[edge];
                let b = room.outline[(edge + 1) % room.outline.len()];
                let side =
                    (b - a).perp_dot(glam::Vec2::new(camera.x, camera.z) - a) * area.signum();
                if side < 0.0 {
                    hidden.insert(*index);
                }
            }
        }
    }
    for (index, object) in objects.iter().enumerate() {
        if object.architecture.is_none() {
            continue;
        }
        let roof_hidden = match (&object.architecture, style.camera_position) {
            (
                Some(VenueArchitecture::Slab {
                    purpose: VenueSlabPurpose::Roof,
                    elevation_millimetres,
                    ..
                }),
                Some(camera),
            ) => camera.y > object.position.y + *elevation_millimetres as f32 * 0.001,
            _ => false,
        };
        push_context(
            frame,
            object,
            style,
            hidden.contains(&index) || roof_hidden,
            &stairs,
            joined.get(&joins::key(index, object)),
        );
    }
    for room in rooms {
        let Some(first) = room.walls.iter().map(|(i, _)| &objects[*i]).find(|o| {
            matches!(
                o.architecture,
                Some(VenueArchitecture::Wall {
                    ceiling_height_millimetres: Some(_),
                    ..
                })
            )
        }) else {
            continue;
        };
        let roof = SceneryObject {
            architecture: Some(VenueArchitecture::Slab {
                purpose: VenueSlabPurpose::Roof,
                outline_millimetres: room
                    .outline
                    .iter()
                    .map(|p| light_fixture::VenuePlanPoint {
                        x: (p.x * 1000.0).round() as i32,
                        y: (p.y * 1000.0).round() as i32,
                    })
                    .collect(),
                elevation_millimetres: ((room.floor + room.height) * 1000.0).round() as i32,
                thickness_millimetres: 150,
            }),
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            ..first.clone()
        };
        let roof_hidden = style.camera_position.is_some_and(|p| {
            p.y > room.floor + room.height || !inside(glam::Vec2::new(p.x, p.z), &room.outline)
        });
        push_context(frame, &roof, style, roof_hidden, &stairs, None);
    }
}
#[cfg(test)]
mod tests;

fn millimetre_pose(object: &SceneryObject) -> [[f64; 4]; 4] {
    Mat4::from_rotation_translation(
        euler_degrees(object.rotation_degrees),
        object.position * 1000.0,
    )
    .to_cols_array_2d()
    .map(|column| column.map(f64::from))
}

/// Only a legacy floor actually intersected by stairs changes its rendering path.
pub(crate) fn apply_floor_cutouts(objects: &mut [SceneryObject]) {
    let stairs: Vec<_> = objects
        .iter()
        .filter(|o| matches!(o.architecture, Some(VenueArchitecture::Stairs { .. })))
        .map(|o| {
            light_fixture::venue_stair_volume(
                o.size.as_dvec3().to_array().map(|v| v * 1000.0),
                millimetre_pose(o),
            )
        })
        .collect();
    if stairs.is_empty() {
        return;
    }
    for object in objects {
        if object.kind != SceneryKind::Floor || object.architecture.is_some() {
            continue;
        }
        let size = object.size.as_dvec3().to_array().map(|v| v * 1000.0);
        let floor = light_fixture::venue_floor_slab(size);
        if light_fixture::venue_geometry_in_context(&floor, size, millimetre_pose(object), &stairs)
            .is_ok_and(|g| g.has_stair_cutouts)
        {
            object.architecture = Some(floor);
        }
    }
}

mod joins;
