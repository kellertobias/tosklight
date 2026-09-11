use super::*;

fn location(x: i32, y: i32, z: i32) -> FixtureLocation {
    FixtureLocation { x, y, z }
}

fn rotation(x: f32, y: f32, z: f32) -> FixtureVector {
    FixtureVector { x, y, z }
}

fn assert_matrix_close(actual: [f64; 12], expected: [f64; 12], tolerance: f64) {
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() <= tolerance,
            "entry {index}: {actual} is not {expected}\n{actual:?}"
        );
    }
}

fn assert_rotation_close(actual: FixtureVector, expected: FixtureVector) {
    for (axis, actual, expected) in [
        ("x", actual.x, expected.x),
        ("y", actual.y, expected.y),
        ("z", actual.z, expected.z),
    ] {
        assert!(
            (actual - expected).abs() < 1e-3,
            "{axis}: {actual} is not {expected}"
        );
    }
}

/// Rotations away from every special case, each turn on every axis.
const TURNS: [(f32, f32, f32); 6] = [
    (30.0, -20.0, 45.0),
    (-75.0, 10.0, -60.0),
    (170.0, -120.0, 80.0),
    (-12.5, 33.0, 5.0),
    (0.0, 0.0, 0.0),
    (90.0, 45.0, -89.0),
];

#[test]
fn import_recovers_exactly_the_rotation_an_export_wrote() {
    for (x, y, z) in TURNS {
        let placed = location(1200, -3400, 6000);
        let written = mvr_matrix(placed, rotation(x, y, z), 0.0);
        let (read_location, read_rotation) = placement_from_mvr(written);
        assert_eq!(read_location, placed);
        assert_rotation_close(read_rotation, rotation(x, y, z));
    }
}

#[test]
fn any_orientation_survives_a_round_trip_even_where_its_angles_are_not_unique() {
    // Beyond a quarter turn about z, and exactly at it, the same orientation has other angles.
    for (x, y, z) in [
        (10.0, 20.0, 135.0),
        (-40.0, 70.0, 90.0),
        (25.0, -5.0, -90.0),
    ] {
        let written = mvr_matrix(location(0, 0, 0), rotation(x, y, z), 0.0);
        let (read_location, read_rotation) = placement_from_mvr(written);
        assert_matrix_close(mvr_matrix(read_location, read_rotation, 0.0), written, 1e-6);
    }
}

#[test]
fn the_bracket_is_folded_into_the_orientation_mvr_carries() {
    let written = mvr_matrix(location(0, 0, 5000), rotation(15.0, -30.0, 60.0), 25.0);
    let (read_location, read_rotation) = placement_from_mvr(written);
    assert_matrix_close(mvr_matrix(read_location, read_rotation, 0.0), written, 1e-6);
}

/// What each stored turn does, as the Stage and the visualizer draw it.
#[test]
fn stored_turns_move_the_fixture_axes_the_way_the_stage_draws_them() {
    let axes = |turn: FixtureVector, bracket: f32| {
        let m = mvr_matrix(location(0, 0, 0), turn, bracket);
        ([m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]])
    };
    let close = |actual: [f64; 3], expected: [f64; 3]| {
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| (actual - expected).abs() < 1e-9)
    };
    // z turns about the vertical: local +X swings towards upstage (+Y).
    let (u, _, _) = axes(rotation(0.0, 0.0, 90.0), 0.0);
    assert!(close(u, [0.0, 1.0, 0.0]), "{u:?}");
    // x turns about the cross-stage axis: local +Y tips up.
    let (_, v, _) = axes(rotation(90.0, 0.0, 0.0), 0.0);
    assert!(close(v, [0.0, 0.0, 1.0]), "{v:?}");
    // y is the renderer's roll about the audience axis: local +X tips up.
    let (u, _, _) = axes(rotation(0.0, 90.0, 0.0), 0.0);
    assert!(close(u, [0.0, 0.0, 1.0]), "{u:?}");
    // The bracket turns in the fixture's own frame, after the placement: turned a quarter about z,
    // the clamp's axis runs upstage, so the lantern's local +Z tips towards +X rather than towards
    // the audience.
    let (_, _, w) = axes(rotation(0.0, 0.0, 90.0), 30.0);
    let half = 30.0_f64.to_radians();
    assert!(close(w, [half.sin(), 0.0, half.cos()]), "{w:?}");
}

/// The Fixture node example in the MVR 1.6 specification.
const SPECIFICATION_FIXTURE: [f64; 12] = [
    0.158127,
    -0.987419,
    0.000000,
    0.987419,
    0.158127,
    0.000000,
    0.000000,
    0.000000,
    1.000000,
    6020.939200,
    2838.588955,
    4978.134459,
];

#[test]
fn the_specification_example_imports_as_a_turn_about_the_vertical() {
    let (read_location, read_rotation) = placement_from_mvr(SPECIFICATION_FIXTURE);
    assert_eq!(read_location, location(6021, 2839, 4978));
    let turn = (-0.987_419_f64).atan2(0.158_127).to_degrees() as f32;
    assert_rotation_close(read_rotation, rotation(0.0, 0.0, turn));
    assert!((turn + 80.902).abs() < 1e-2, "{turn}");

    // Exported again, it is the same matrix the specification wrote, to its printed precision.
    let written = mvr_matrix(read_location, read_rotation, 0.0);
    let mut expected = SPECIFICATION_FIXTURE;
    for (index, value) in expected.iter_mut().enumerate().skip(9) {
        *value = f64::from([6021, 2839, 4978][index - 9]);
    }
    assert_matrix_close(written, expected, 1e-5);
}

#[test]
fn scaled_axes_import_as_the_rotation_they_describe() {
    let mut scaled = mvr_matrix(location(0, 0, 0), rotation(20.0, -35.0, 50.0), 0.0);
    for value in &mut scaled[0..9] {
        *value *= 2.5;
    }
    let (_, read_rotation) = placement_from_mvr(scaled);
    assert_rotation_close(read_rotation, rotation(20.0, -35.0, 50.0));
}
