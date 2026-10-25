use super::*;

#[test]
fn bounds_fall_back_to_a_default_room_when_the_scene_is_empty() {
    let mut scene = Scene::default();
    scene.recompute_bounds();
    assert!(!scene.bounds.is_empty());
    assert!(scene.bounds.radius() > 1.0);
}

/// The bracket turns the fixture in its own frame, after the mounting rotation.
///
/// A lantern turned to face across the stage and then angled down in its clamp looks down and
/// across; adding the two angles as world Euler terms would instead have it look somewhere
/// neither the bar nor the bracket says.
#[test]
fn the_bracket_angle_turns_the_fixture_about_its_own_transverse_axis() {
    let mut fixture = FixtureInstance {
        lamp_response: Default::default(),
        drawn_as_scenery: false,
        invisible: false,
        instance_id: Uuid::nil(),
        fixture_id: Uuid::nil(),
        name: "Lantern".into(),
        number: None,
        position: Vec3::ZERO,
        rotation_degrees: Vec3::new(0.0, 90.0, 0.0),
        position_master: None,
        bracket_degrees: 0.0,
        bracket_hinge: None,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        installed_shaper_blades: [0.0; 4],
        body: FixtureBody {
            size: Vec3::splat(0.3),
            kind: BodyKind::Lantern,
        },
        patched: true,
        address: None,
        model: None,
        fallback: None,
    };
    // Hung level and facing stage right, an emitter's rest direction is straight down.
    let level = fixture.orientation() * Vec3::NEG_Y;
    assert!((level.y + 1.0).abs() < 1e-5, "{level:?}");

    // Angled 90 degrees in its bracket, it looks along the way the yoke turned it.
    fixture.bracket_degrees = -90.0;
    let angled = fixture.orientation() * Vec3::NEG_Y;
    assert!(angled.y.abs() < 1e-5, "no longer looking down: {angled:?}");
    assert!(
        angled.x.abs() > 0.99,
        "it looks the way the mounting rotation faces it: {angled:?}"
    );
}

#[test]
fn a_fixture_with_no_bracket_angle_keeps_its_mounting_rotation_exactly() {
    let fixture = FixtureInstance {
        lamp_response: Default::default(),
        drawn_as_scenery: false,
        invisible: false,
        instance_id: Uuid::nil(),
        fixture_id: Uuid::nil(),
        name: "Lantern".into(),
        number: None,
        position: Vec3::ZERO,
        rotation_degrees: Vec3::new(12.0, 34.0, 56.0),
        position_master: None,
        bracket_degrees: 0.0,
        bracket_hinge: None,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        installed_shaper_blades: [0.0; 4],
        body: FixtureBody {
            size: Vec3::splat(0.3),
            kind: BodyKind::Lantern,
        },
        patched: true,
        address: None,
        model: None,
        fallback: None,
    };
    let expected = euler_degrees(fixture.rotation_degrees);
    assert!((fixture.orientation().dot(expected).abs() - 1.0).abs() < 1e-6);
}

#[test]
fn a_wide_rig_still_leaves_the_house_view_standing_on_the_floor() {
    // Framing pads the rig outward on every axis by a share of its widest span, so a wide
    // rig with fixtures standing on the deck pushes the underside of the bounds metres below
    // the floor. The house view must not sink with it.
    let mut scene = Scene::default();
    for step in 0..9_u8 {
        let x = -20.0 + f32::from(step) * 5.0;
        scene.fixtures.push(rigged_spot(Vec3::new(x, 0.4, -4.0)));
        scene.fixtures.push(rigged_spot(Vec3::new(x, 6.0, -4.0)));
    }
    scene.recompute_bounds();
    let framing = scene.framing_bounds();
    assert!(
        framing.min.y < -1.65,
        "this rig pads the bounds below standing height: {framing:?}"
    );

    let camera = crate::Camera::audience(framing);
    assert!(
        camera.position.y > 1.0,
        "the audience stands in the room, not under it: {}",
        camera.position.y
    );
    assert!(
        (camera.target - camera.position).normalize().y > 0.0,
        "and looks up at the rig"
    );
}

fn rigged_spot(position: Vec3) -> FixtureInstance {
    FixtureInstance {
        lamp_response: Default::default(),
        drawn_as_scenery: false,
        invisible: false,
        instance_id: Uuid::nil(),
        fixture_id: Uuid::nil(),
        name: "Spot".into(),
        number: Some(1),
        position,
        rotation_degrees: Vec3::ZERO,
        position_master: None,
        bracket_degrees: 0.0,
        bracket_hinge: None,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        installed_shaper_blades: [0.0; 4],
        body: FixtureBody::default(),
        patched: true,
        address: None,
        model: None,
        fallback: None,
    }
}

#[test]
fn a_slaved_fixture_stands_where_its_point_puts_it() {
    let master = Uuid::from_u128(7);
    let mut spot = rigged_spot(Vec3::new(4.0, 6.0, 0.0));
    spot.position_master = Some(master);
    let pose = crate::PointPose {
        fixture_id: master,
        origin_metres: [2.0, 6.0, 0.0],
        offset_metres: [0.0, -1.0, 0.0],
        rotation_degrees: [0.0, 90.0, 0.0],
    };
    let (position, _) = spot.placed_by(&[pose]);
    // Swung a quarter turn about the point, then flown down a metre with it.
    assert!((position - Vec3::new(2.0, 5.0, -2.0)).length() < 1e-4);
}

#[test]
fn an_unslaved_fixture_ignores_every_point_in_the_show() {
    let spot = rigged_spot(Vec3::new(4.0, 6.0, 0.0));
    let pose = crate::PointPose {
        fixture_id: Uuid::from_u128(7),
        origin_metres: [2.0, 6.0, 0.0],
        offset_metres: [0.0, -1.0, 0.0],
        rotation_degrees: [0.0, 90.0, 0.0],
    };
    assert_eq!(spot.placed_by(&[pose]).0, Vec3::new(4.0, 6.0, 0.0));
}

#[test]
fn a_slave_whose_point_has_not_reported_stands_where_the_rig_put_it() {
    let mut spot = rigged_spot(Vec3::new(4.0, 6.0, 0.0));
    spot.position_master = Some(Uuid::from_u128(7));
    // A point that is gone, or that has not sent a pose yet, must not guess an offset.
    assert_eq!(spot.placed_by(&[]).0, Vec3::new(4.0, 6.0, 0.0));
}

#[test]
fn framing_ignores_a_floor_far_wider_than_the_rig() {
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
        lamp_response: Default::default(),
        drawn_as_scenery: false,
        invisible: false,
        instance_id: Uuid::nil(),
        fixture_id: Uuid::nil(),
        name: "Spot".into(),
        number: Some(1),
        position: Vec3::new(0.0, 5.0, 0.0),
        rotation_degrees: Vec3::ZERO,
        position_master: None,
        bracket_degrees: 0.0,
        bracket_hinge: None,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        installed_shaper_blades: [0.0; 4],
        body: FixtureBody::default(),
        patched: true,
        address: None,
        model: None,
        fallback: None,
    });
    scene.scenery.push(SceneryObject {
        id: Uuid::nil(),
        name: "House floor".into(),
        position: Vec3::new(0.0, 0.0, 0.0),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(200.0, 0.1, 200.0),
        colour: [0.2, 0.2, 0.2],
        roughness: 0.8,
        kind: SceneryKind::Floor,
        chords: 0,
        detail: Default::default(),
        position_master: None,
        architecture: None,
    });
    scene.recompute_bounds();
    assert!(scene.bounds.radius() > 100.0);
    let framing = scene.framing_bounds();
    assert!(
        framing.radius() < 10.0,
        "the rig, not the room, decides the frame: {framing:?}"
    );
    assert!(framing.min.y <= 0.0, "the stage floor stays in shot");
}

#[test]
fn cone_half_angle_interpolates_between_beam_and_field() {
    let emitter = EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "Main".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 10.0,
        field_angle_degrees: 30.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    };
    assert!((emitter.cone_half_angle(0.0) - 5.0_f32.to_radians()).abs() < 1e-6);
    assert!((emitter.cone_half_angle(1.0) - 15.0_f32.to_radians()).abs() < 1e-6);
    assert!((emitter.cone_half_angle(0.5) - 10.0_f32.to_radians()).abs() < 1e-6);
}
