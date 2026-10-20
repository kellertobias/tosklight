use super::*;
use crate::cad::{CadEntity, scenery::CadScenery};
use light_fixture::{PatchedFixturePatch, VenueArchitecture, VenuePlanPoint, VenueSlabPurpose};
use uuid::Uuid;

fn patch(architecture: VenueArchitecture) -> PatchedFixturePatch {
    serde_json::from_value(serde_json::json!({
        "fixture_id":Uuid::from_u128(1),
        "scenery_options":{"architecture":architecture}
    }))
    .unwrap()
}
fn heights(geometry: &CadVenueGeometry) -> [f64; 2] {
    geometry
        .triangles
        .iter()
        .flatten()
        .fold([f64::INFINITY, f64::NEG_INFINITY], |[low, high], point| {
            [low.min(point[1]), high.max(point[1])]
        })
}
#[test]
fn authored_walls_and_furniture_start_on_the_placement_floor() {
    for architecture in [
        VenueArchitecture::Wall {
            openings: vec![],
            ceiling_height_millimetres: Some(4000),
        },
        VenueArchitecture::Bar,
    ] {
        let geometry = for_fixture(&patch(architecture), [6000.0, 200.0, 3000.0]).unwrap();
        assert_eq!(heights(&geometry), [0.0, 3000.0]);
        assert!(!geometry.plan.is_empty());
    }
}
#[test]
fn slab_elevation_is_not_lifted_again_by_half_the_box_height() {
    let architecture = VenueArchitecture::Slab {
        purpose: VenueSlabPurpose::Balcony,
        outline_millimetres: vec![
            VenuePlanPoint { x: 0, y: 0 },
            VenuePlanPoint { x: 3000, y: 0 },
            VenuePlanPoint { x: 3000, y: 2000 },
            VenuePlanPoint { x: 0, y: 2000 },
        ],
        elevation_millimetres: 4200,
        thickness_millimetres: 300,
    };
    let geometry = for_fixture(&patch(architecture), [3000.0, 2000.0, 300.0]).unwrap();
    assert_eq!(heights(&geometry), [4200.0, 4500.0]);
}
fn wall(id: u128, position: [i32; 3], angle: f32, length: f32) -> CadEntity {
    let id = Uuid::from_u128(id);
    CadEntity {
        id,
        logical_fixture_id: id,
        name: format!("Wall {id}"),
        fixture_number: None,
        fixture_display_id: "0.1".into(),
        dmx_address: "Visual only".into(),
        fixture_profile: String::new(),
        mode: String::new(),
        note: String::new(),
        kind: "venue".into(),
        fixture_type: "venue".into(),
        drawing_id: String::new(),
        layer_id: "default".into(),
        selectable: true,
        position_millimetres: position,
        rotation_degrees: [0.0, 0.0, angle],
        size_millimetres: [length, 200.0, 4000.0],
        output_direction: [0.0; 3],
        aim: Default::default(),
        scenery: Some(CadScenery {
            architecture: Some(VenueArchitecture::Wall {
                openings: vec![],
                ceiling_height_millimetres: Some(4000),
            }),
            kind: "box".into(),
            chords: 0,
            pattern: "standard".into(),
            feet: "scissor".into(),
            handrails: light_fixture::StairHandrails::None,
            chain: None,
            anchor: None,
        }),
        venue_geometry: None,
        mounting: None,
        imported_model: false,
        position_reference: None,
    }
}
fn room() -> Vec<CadEntity> {
    vec![
        wall(1, [0, 0, 500], 0.0, 6000.0),
        wall(2, [3000, 2000, 500], 90.0, 4000.0),
        wall(3, [0, 4000, 500], 0.0, 6000.0),
        wall(4, [-3000, 2000, 500], 90.0, 4000.0),
    ]
}
#[test]
fn closed_wall_loop_adds_one_nonselectable_ceiling_without_changing_the_walls() {
    let walls = room();
    let result = with_ceilings(walls.clone());
    assert_eq!(result.len(), 5);
    let ceiling = &result[4];
    assert!(!ceiling.selectable);
    assert_eq!(ceiling.position_millimetres, [0; 3]);
    assert_eq!(ceiling.rotation_degrees, [0.0; 3]);
    assert_eq!(
        heights(ceiling.venue_geometry.as_ref().unwrap()),
        [4500.0, 4650.0]
    );
    assert!(matches!(
        ceiling.scenery.as_ref().unwrap().architecture,
        Some(VenueArchitecture::Slab {
            purpose: VenueSlabPurpose::Roof,
            ..
        })
    ));
    for (before, after) in walls.iter().zip(&result) {
        assert_eq!(before.id, after.id);
        assert_eq!(before.position_millimetres, after.position_millimetres);
        assert!(after.selectable);
    }
    assert_eq!(ceiling.id, with_ceilings(walls)[4].id);
}
#[test]
fn an_open_wall_chain_never_generates_a_roof() {
    let mut walls = room();
    walls.pop();
    assert_eq!(with_ceilings(walls).len(), 3);
}
#[test]
fn generated_ceiling_identity_does_not_depend_on_patch_iteration_order() {
    let walls = room();
    let first = with_ceilings(walls.clone())[4].id;
    let mut reversed = walls;
    reversed.reverse();
    assert_eq!(with_ceilings(reversed)[4].id, first);
}

fn stairs_entity(id: u128, position: [i32; 3], yaw: f32, rise: f32, run: f32) -> CadEntity {
    let mut entity = wall(id, position, yaw, 1200.0);
    entity.name = "Stairs".into();
    entity.size_millimetres = [1200.0, run, rise];
    entity.scenery.as_mut().unwrap().architecture = Some(VenueArchitecture::Stairs { steps: 16 });
    entity
}
fn balcony_entity() -> CadEntity {
    let mut entity = wall(91, [0, 0, 0], 0.0, 8000.0);
    entity.name = "Balcony".into();
    entity.size_millimetres = [8000.0, 8000.0, 200.0];
    entity.scenery.as_mut().unwrap().architecture = Some(VenueArchitecture::Slab {
        purpose: VenueSlabPurpose::Balcony,
        outline_millimetres: vec![
            VenuePlanPoint { x: -4000, y: -4000 },
            VenuePlanPoint { x: 4000, y: -4000 },
            VenuePlanPoint { x: 4000, y: 4000 },
            VenuePlanPoint { x: -4000, y: 4000 },
        ],
        elevation_millimetres: 1500,
        thickness_millimetres: 200,
    });
    entity
}
fn top_area(geometry: &CadVenueGeometry, height: f64) -> f64 {
    geometry
        .triangles
        .iter()
        .filter(|t| t.iter().all(|p| (p[1] - height).abs() < 1e-5))
        .map(|t| {
            ((t[1][0] - t[0][0]) * (t[2][2] - t[0][2]) - (t[1][2] - t[0][2]) * (t[2][0] - t[0][0]))
                .abs()
                / 2.0
        })
        .sum()
}
#[test]
fn moving_and_deleting_stairs_recomputes_every_reached_slab_mesh_and_export_contour() {
    let stair = stairs_entity(90, [0, 0, 0], 90.0, 3000.0, 4500.0);
    let balcony = balcony_entity();
    let scene = with_ceilings(vec![stair.clone(), balcony.clone()]);
    let geometry = scene[1].venue_geometry.as_ref().unwrap();
    assert!((top_area(geometry, 1700.0) - 58_600_000.0).abs() < 1.0);
    let perimeter: f64 = geometry
        .plan
        .iter()
        .map(|line| (line[0][0] - line[1][0]).hypot(line[0][1] - line[1][1]))
        .sum();
    assert!((perimeter - 43_400.0).abs() < 0.1);
    let mut moved = stair;
    moved.position_millimetres[0] = 20000;
    let moved_scene = with_ceilings(vec![moved, balcony.clone()]);
    assert!(
        (top_area(moved_scene[1].venue_geometry.as_ref().unwrap(), 1700.0) - 64_000_000.0).abs()
            < 0.1
    );
    let deleted_scene = with_ceilings(vec![balcony]);
    assert!(
        (top_area(deleted_scene[0].venue_geometry.as_ref().unwrap(), 1700.0) - 64_000_000.0).abs()
            < 0.1
    );
}
#[test]
fn stairs_cut_generated_ceilings_using_the_same_world_pose_as_native_rendering() {
    let mut entities = room();
    entities.push(stairs_entity(90, [0, 2000, 500], 0.0, 4000.0, 2000.0));
    let scene = with_ceilings(entities);
    let ceiling = scene.iter().find(|entity| !entity.selectable).unwrap();
    assert!(
        (top_area(ceiling.venue_geometry.as_ref().unwrap(), 4650.0) - 21_600_000.0).abs() < 1.0
    );
}
#[test]
fn legacy_floors_are_adapted_only_while_stairs_actually_intersect_them() {
    let stair = stairs_entity(90, [0, 0, 0], 0.0, 3000.0, 4500.0);
    let mut floor = balcony_entity();
    floor.position_millimetres[2] = 1600;
    let scenery = floor.scenery.as_mut().unwrap();
    scenery.architecture = None;
    scenery.kind = "floor".into();
    let scene = with_ceilings(vec![stair, floor.clone()]);
    assert!(matches!(
        scene[1].scenery.as_ref().unwrap().architecture,
        Some(VenueArchitecture::Slab {
            purpose: VenueSlabPurpose::Floor,
            ..
        })
    ));
    assert!(
        (top_area(scene[1].venue_geometry.as_ref().unwrap(), 100.0) - 58_600_000.0).abs() < 0.1
    );
    let scene = with_ceilings(vec![floor]);
    assert!(scene[0].venue_geometry.is_none());
    assert!(scene[0].scenery.as_ref().unwrap().architecture.is_none());
}

#[test]
fn connected_wall_geometry_tracks_neighbour_move_and_delete_without_changing_identity() {
    let a = wall(101, [-1000, 0, 0], 0.0, 2000.0);
    let b = wall(102, [0, -1000, 0], -90.0, 2000.0);
    let joined = with_ceilings(vec![a.clone(), b.clone()]);
    assert_eq!(joined[0].id, a.id);
    assert!(
        joined
            .iter()
            .all(|e| e.venue_geometry.as_ref().unwrap().joined_wall)
    );
    assert_eq!(
        heights(joined[0].venue_geometry.as_ref().unwrap()),
        [0.0, 4000.0]
    );
    let mut moved = b;
    moved.position_millimetres[0] += 100;
    let moved = with_ceilings(vec![a.clone(), moved]);
    // Base projection comes from fresh fixture snapshots in production; this fixture helper has none.
    assert!(moved[0].venue_geometry.is_none());
    assert!(with_ceilings(vec![a])[0].venue_geometry.is_none());
}
