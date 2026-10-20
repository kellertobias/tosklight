//! Scene context is projected after all authored objects and automatic ceilings exist.
use super::{super::CadEntity, CadVenueGeometry};
use light_fixture::{
    VenueArchitecture, VenueWallPlacement, venue_floor_slab, venue_geometry_in_context,
    venue_joined_walls, venue_stair_volume,
};
use viz_scene::glam::{Mat4, Vec3};

pub(super) fn apply(entities: &mut [CadEntity]) {
    let stairs: Vec<_> = entities
        .iter()
        .filter(|entity| {
            matches!(
                entity
                    .scenery
                    .as_ref()
                    .and_then(|s| s.architecture.as_ref()),
                Some(VenueArchitecture::Stairs { .. })
            )
        })
        .map(|entity| venue_stair_volume(size(entity), pose(entity, true)))
        .collect();
    let walls: Vec<_> = entities
        .iter()
        .filter_map(|entity| {
            let VenueArchitecture::Wall { openings, .. } =
                entity.scenery.as_ref()?.architecture.as_ref()?
            else {
                return None;
            };
            Some(VenueWallPlacement {
                id: entity.id,
                size_millimetres: size(entity),
                local_to_world: pose(entity, true),
                openings: openings.clone(),
            })
        })
        .collect();
    let mut joined = venue_joined_walls(&walls);
    for entity in entities {
        if let Some(geometry) = joined.remove(&entity.id) {
            let lift = f64::from(entity.size_millimetres[2]) / 2.0;
            entity.venue_geometry = Some(CadVenueGeometry {
                joined_wall: true,
                exterior: geometry
                    .exterior
                    .into_iter()
                    .map(|line| line.map(|p| [p[0], p[1] + lift, p[2]]))
                    .collect(),
                triangles: geometry
                    .triangles
                    .into_iter()
                    .map(|triangle| triangle.map(|p| [p[0], p[1] + lift, p[2]]))
                    .collect(),
                plan: geometry.plan,
                elevation: geometry
                    .elevation
                    .into_iter()
                    .map(|line| line.map(|p| [p[0], p[1] + lift, p[2]]))
                    .collect(),
            });
            continue;
        }
        let Some(scenery) = &entity.scenery else {
            continue;
        };
        let implicit = scenery.architecture.is_none() && scenery.kind == "floor";
        let architecture = if implicit {
            venue_floor_slab(size(entity))
        } else if let Some(architecture @ VenueArchitecture::Slab { .. }) = &scenery.architecture {
            architecture.clone()
        } else {
            continue;
        };
        let Ok(geometry) =
            venue_geometry_in_context(&architecture, size(entity), pose(entity, false), &stairs)
        else {
            continue;
        };
        if implicit && !geometry.has_stair_cutouts {
            continue;
        }
        entity.venue_geometry = Some(CadVenueGeometry {
            joined_wall: false,
            exterior: vec![],
            triangles: geometry.triangles,
            plan: geometry.plan,
            elevation: vec![],
        });
        if implicit {
            entity.scenery.as_mut().unwrap().architecture = Some(architecture);
        }
    }
}
fn size(entity: &CadEntity) -> [f64; 3] {
    let [width, depth, height] = entity.size_millimetres;
    [width, height, depth].map(f64::from)
}
fn pose(entity: &CadEntity, lift: bool) -> [[f64; 4]; 4] {
    let rotation =
        viz_scene::euler_degrees(viz_scene::desk_rotation_to_world(entity.rotation_degrees));
    let [x, y, z] = entity.position_millimetres;
    let position = Vec3::new(x as f32, z as f32, -y as f32)
        + if lift {
            rotation * Vec3::Y * (entity.size_millimetres[2] / 2.0)
        } else {
            Vec3::ZERO
        };
    Mat4::from_rotation_translation(rotation, position)
        .to_cols_array_2d()
        .map(|column| column.map(f64::from))
}
