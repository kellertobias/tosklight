//! A stage element is placed by its feet: its origin is the floor it stands on, and its height
//! raises or lowers the deck rather than moving where the feet start. A flight of stairs and a
//! handrail stand on their feet the same way.
use super::model_tests::shipped_venue;
use super::*;

const STAGE_ELEMENTS: [&str; 8] = [
    "venue--stage-element-2-1-m",
    "venue--stage-element-1-1-m",
    "venue--stage-element-1-0-5-m",
    "venue--stage-deck-2-1-m",
    "venue--stage-deck-1-1-m",
    "venue--stage-deck-1-0-5-m",
    "venue--stage-stairs",
    "venue--stage-stairs-with-handrails",
];

/// What the plan builds an element as: a deck is a riser, a flight of stairs is its own kind.
fn expected_kind(name: &str) -> viz_scene::SceneryKind {
    if name.contains("stairs") {
        viz_scene::SceneryKind::Stairs
    } else {
        viz_scene::SceneryKind::Riser
    }
}

/// The lowest and highest the compiled box reaches.
fn vertical_extent(object: &viz_scene::SceneryObject) -> (f32, f32) {
    (
        object.position.y - object.size.y * 0.5,
        object.position.y + object.size.y * 0.5,
    )
}

#[test]
fn a_stage_element_placed_on_the_floor_spans_from_the_floor_up_its_height() {
    for name in STAGE_ELEMENTS {
        let mut fixture = shipped_venue(name);
        fixture.instances[0].position = Vec3::new(1.5, 0.0, -2.0);
        for height in [0.2, 0.5, 1.0] {
            fixture.instances[0].scenery_size_metres = Some(Vec3::new(2.0, height, 1.0));
            let object = compile(std::slice::from_ref(&fixture)).scene.scenery[0].clone();
            assert_eq!(object.kind, expected_kind(name), "{name}");
            let (bottom, top) = vertical_extent(&object);
            assert!(
                bottom.abs() < 1e-5,
                "{name} at {height} m: feet at {bottom}"
            );
            assert!(
                (top - height).abs() < 1e-5,
                "{name} at {height} m: deck at {top}"
            );
            // Only the height moves: the footprint stays where it was placed.
            assert_eq!(
                (object.position.x, object.position.z),
                (1.5, -2.0),
                "{name}"
            );
        }
    }
}

#[test]
fn changing_a_stage_elements_height_keeps_its_feet_on_a_raised_floor() {
    for name in STAGE_ELEMENTS {
        let mut fixture = shipped_venue(name);
        // Standing on something 0.8 m up, such as another deck.
        fixture.instances[0].position = Vec3::new(0.0, 0.8, 0.0);
        fixture.instances[0].scenery_size_metres = Some(Vec3::new(1.0, 0.3, 1.0));
        let low = compile(std::slice::from_ref(&fixture)).scene.scenery[0].clone();
        fixture.instances[0].scenery_size_metres = Some(Vec3::new(1.0, 0.9, 1.0));
        let high = compile(std::slice::from_ref(&fixture)).scene.scenery[0].clone();
        let (low_bottom, low_top) = vertical_extent(&low);
        let (high_bottom, high_top) = vertical_extent(&high);
        assert!((low_bottom - 0.8).abs() < 1e-5, "{name}: {low_bottom}");
        assert!((high_bottom - 0.8).abs() < 1e-5, "{name}: {high_bottom}");
        assert!((high_top - low_top - 0.6).abs() < 1e-5, "{name}");
    }
}

/// The feet follow the height the profile allows, and the model scale, not the one asked for.
#[test]
fn a_stage_elements_feet_stay_put_when_its_height_is_clamped_or_scaled() {
    let mut fixture = shipped_venue("venue--stage-element-2-1-m");
    fixture.instances[0].position = Vec3::ZERO;
    fixture.instances[0].scenery_size_metres = Some(Vec3::new(2.0, 5.0, 1.0));
    fixture.instances[0].model_scale = 0.5;
    let object = compile(std::slice::from_ref(&fixture)).scene.scenery[0].clone();
    let (bottom, top) = vertical_extent(&object);
    assert!(bottom.abs() < 1e-5, "{bottom}");
    // 1.2 m is the tallest the profile builds, drawn at half scale.
    assert!((top - 0.6).abs() < 1e-5, "{top}");
}

/// A handrail stands on the floor it is placed on, like the deck it guards.
#[test]
fn a_handrail_stands_on_the_floor_it_is_placed_on() {
    let mut fixture = shipped_venue("venue--stage-handrail");
    fixture.instances[0].position = Vec3::new(0.0, 0.6, 0.0);
    let object = &compile(std::slice::from_ref(&fixture)).scene.scenery[0];
    let (bottom, top) = vertical_extent(object);
    // Placed on a 0.6 m deck, a 1 m guard reaches 1.6 m off the floor.
    assert!((bottom - 0.6).abs() < 1e-5, "{bottom}");
    assert!((top - 1.6).abs() < 1e-5, "{top}");
}

/// Truss, curtains, chains and the primitive shapes keep their centre origin.
#[test]
fn other_generated_scenery_keeps_its_centre_origin() {
    for name in ["venue--two-point-truss", "venue--curtain", "venue--box"] {
        let mut fixture = shipped_venue(name);
        fixture.instances[0].position = Vec3::new(0.0, 2.0, 0.0);
        let object = &compile(std::slice::from_ref(&fixture)).scene.scenery[0];
        assert_eq!(object.position, Vec3::new(0.0, 2.0, 0.0), "{name}");
    }
}
