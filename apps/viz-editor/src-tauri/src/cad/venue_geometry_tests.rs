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
