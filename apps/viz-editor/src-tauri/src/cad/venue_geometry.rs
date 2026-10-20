//! CAD and printed plans use the same solid geometry as the renderer, before camera cutaway.
use light_fixture::{PatchedFixturePatch, VenueArchitecture};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadVenueGeometry {
    pub triangles: Vec<[[f64; 3]; 3]>,
    pub plan: Vec<[[f64; 2]; 2]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub elevation: Vec<[[f64; 3]; 2]>,
}

pub fn for_fixture(patch: &PatchedFixturePatch, size: [f32; 3]) -> Option<CadVenueGeometry> {
    let architecture = patch.scenery_options.architecture.as_ref()?;
    let geometry =
        light_fixture::venue_geometry(architecture, [size[0], size[2], size[1]].map(f64::from))
            .ok()?;
    let lift = if matches!(architecture, VenueArchitecture::Slab { .. }) {
        0.0
    } else {
        f64::from(size[2]) / 2.0
    };
    Some(CadVenueGeometry {
        triangles: geometry
            .triangles
            .into_iter()
            .map(|t| t.map(|p| [p[0], p[1] + lift, p[2]]))
            .collect(),
        plan: geometry.plan,
        elevation: match architecture {
            VenueArchitecture::Wall { openings, .. } => light_fixture::venue_wall_drawing(
                [size[0], size[2], size[1]].map(f64::from),
                openings,
            )
            .into_iter()
            .map(|line| line.map(|p| [p[0], p[1] + lift, p[2]]))
            .collect(),
            _ => vec![],
        },
    })
}

/// Generated ceiling entities are presentation-only: the enclosing walls remain their authority.
pub fn with_ceilings(mut entities: Vec<super::CadEntity>) -> Vec<super::CadEntity> {
    use light_fixture::{VenueWallRun, venue_rooms};
    let walls: Vec<_> = entities
        .iter()
        .filter_map(|entity| {
            let VenueArchitecture::Wall {
                ceiling_height_millimetres,
                ..
            } = entity.scenery.as_ref()?.architecture.as_ref()?
            else {
                return None;
            };
            let angle = f64::from(entity.rotation_degrees[2]).to_radians();
            let half = f64::from(entity.size_millimetres[0]) / 2.0;
            let x = f64::from(entity.position_millimetres[0]);
            let z = -f64::from(entity.position_millimetres[1]);
            Some(VenueWallRun {
                id: entity.id,
                start_millimetres: [x - half * angle.cos(), z + half * angle.sin()],
                end_millimetres: [x + half * angle.cos(), z - half * angle.sin()],
                floor_millimetres: f64::from(entity.position_millimetres[2]),
                height_millimetres: f64::from(entity.size_millimetres[2]),
                ceiling_height_millimetres: *ceiling_height_millimetres,
            })
        })
        .collect();
    for room in venue_rooms(&walls) {
        let Some(roof) = room.ceiling() else { continue };
        let Some(template) = entities
            .iter()
            .filter(|e| room.wall_ids.contains(&e.id))
            .min_by_key(|e| e.id)
        else {
            continue;
        };
        let Ok(geometry) = light_fixture::venue_geometry(&roof, [1.0; 3]) else {
            continue;
        };
        let mut ceiling = template.clone();
        ceiling.id = uuid::Uuid::from_u128(
            template.id.as_u128() ^ 0x7390_1100_ee52_ffff_8033_1000_ffff_1111,
        );
        ceiling.name = "Automatic ceiling".into();
        ceiling.selectable = false;
        ceiling.position_millimetres = [0; 3];
        ceiling.rotation_degrees = [0.0; 3];
        ceiling.venue_geometry = Some(CadVenueGeometry {
            triangles: geometry.triangles,
            plan: geometry.plan,
            elevation: vec![],
        });
        ceiling.scenery.as_mut().unwrap().architecture = Some(roof);
        entities.push(ceiling);
    }
    context::apply(&mut entities);
    entities
}

#[cfg(test)]
#[path = "venue_geometry_tests.rs"]
mod tests;

#[path = "venue_geometry_context.rs"]
mod context;
