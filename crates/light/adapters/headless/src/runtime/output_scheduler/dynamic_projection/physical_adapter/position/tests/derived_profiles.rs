//! TL-552: a shipped mover without an authored Position graph is driven through the derived
//! nominal graph of the runtime profile projection, by the unchanged Live Position adapter.
use super::*;

fn package(name: &str) -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

fn robin(projected: bool) -> FixtureProfile {
    let mut profile = package("robe--robin-300-ledwash");
    profile.modes.retain(|m| m.name == "Mode 2");
    if projected {
        apply_runtime_profile_compatibility(&mut profile);
    }
    profile.validate().unwrap();
    profile
}

fn rig(profile: &FixtureProfile, inverted: bool) -> Rig {
    let root = FixtureId::new();
    let mut fixture = patched(profile, root, 1);
    (fixture.invert_pan, fixture.invert_tilt) = (inverted, inverted);
    Rig::new(vec![fixture], root)
}

/// Fitted raw words of Pan and Tilt, and the achieved Angles.
fn fit(rig: &Rig, profile: &FixtureProfile, pan: f32, tilt: f32) -> ([u32; 2], [f64; 2]) {
    // `Rig::verify` assumes the synthetic all-U16 layout; the shipped mode has 8-bit channels.
    let resolved = rig.resolve(&[(rig.root, angles(pan, tilt))]);
    let result = &resolved.results[0];
    let outcome = &result.achieved.outcomes[0].result;
    assert_eq!(outcome.status, PositionFitStatus::Fitted);
    let index = |attribute: &str| {
        profile.modes[0]
            .channels
            .iter()
            .position(|c| &*c.attribute.0 == attribute)
            .unwrap() as u32
    };
    let raw = |attribute| {
        result
            .writes
            .iter()
            .find(|w| w.slot.channel_index == index(attribute))
            .unwrap()
            .raw
    };
    ([raw("pan"), raw("tilt")], outcome.achieved.unwrap())
}

#[test]
fn without_the_derived_graph_the_robin_has_no_position_destination() {
    // The gap this closes: the authored package alone gives the adapter nothing to fit.
    let profile = robin(false);
    let rig = rig(&profile, false);
    assert!(
        rig.adapter
            .compile(&rig.engine.snapshot(), rig.root)
            .unwrap()
            .is_none()
    );
}

#[test]
fn derived_robin_angles_render_sixteen_bit_words_and_inversion_mirrors_only_the_wire() {
    let profile = robin(true);
    let upright = rig(&profile, false);
    // Declared Pan 450° and Tilt 300°, centred on the neutral pose.
    let expected = |degrees: f64, travel: f64| ((degrees / travel + 0.5) * 65535.).round() as i64;
    let (words, achieved) = fit(&upright, &profile, 67.5, -75.);
    assert!(
        (i64::from(words[0]) - expected(67.5, 450.)).abs() <= 1,
        "{words:?}"
    );
    assert!(
        (i64::from(words[1]) - expected(-75., 300.)).abs() <= 1,
        "{words:?}"
    );
    assert!((achieved[0] - 67.5).abs() < 0.01 && (achieved[1] + 75.).abs() < 0.01);

    let inverted = rig(&profile, true);
    let (mirrored, reported) = fit(&inverted, &profile, 67.5, -75.);
    for axis in 0..2 {
        let sum = words[axis] + mirrored[axis];
        assert!((65534..=65536).contains(&sum), "axis {axis}: {sum}");
        assert!(
            (reported[axis] - achieved[axis]).abs() < 0.01,
            "reported angle unchanged"
        );
    }
}
