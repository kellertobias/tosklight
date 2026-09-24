//! The show's colour programming model at the output: Direct keeps its exact behaviour, Intent
//! drives every fixture from one device-independent target.

use super::*;

fn rgb_engine() -> (Engine, ProgrammerRegistry, SessionId, FixtureId) {
    let (fixture, fixture_id) = schema_v2_fixture(&[
        ("intensity", false, false, false, false, false),
        ("color.red", false, false, false, false, false),
        ("color.green", false, false, false, false, false),
        ("color.blue", false, false, false, false, false),
    ]);
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    (engine, programmers, session, fixture_id)
}

fn dim_red() -> AttributeValue {
    // sRGB red at a tenth of its luminance: the colour is red, the level is not the colour's job.
    AttributeValue::ColorXyz(Xyz {
        x: 0.041_246,
        y: 0.021_267,
        z: 0.001_933,
    })
}

#[test]
fn direct_shows_keep_their_exact_output_and_intent_shows_drive_unauthored_rgb_at_full_chroma() {
    let (engine, programmers, session, fixture_id) = rgb_engine();
    programmers.set(
        session,
        fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    );
    programmers.set(session, fixture_id, AttributeKey::color(), dim_red());

    assert_eq!(
        engine.color_model(),
        light_core::ColorProgrammingModel::Direct
    );
    let direct = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(
        &direct.universes[&1][0..4],
        &[255, 0, 0, 0],
        "a Direct head without an authored colour system leaves a whole-colour value unresolved"
    );

    engine.set_color_model(light_core::ColorProgrammingModel::Intent);
    let intent = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(
        &intent.universes[&1][0..4],
        &[255, 255, 0, 0],
        "Intent reproduces the chromaticity at the engine's full reach"
    );

    // Intensity alone dims it.
    programmers.set(
        session,
        fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let dimmed = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(dimmed.universes[&1][0], 128);
    assert_eq!(&dimmed.universes[&1][1..4], &[255, 0, 0]);

    engine.set_color_model(light_core::ColorProgrammingModel::Direct);
    let back = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(
        &back.universes[&1][1..4],
        &[0, 0, 0],
        "switching back restores Direct exactly"
    );
}

#[test]
fn the_report_names_each_heads_quality_for_its_current_target() {
    let (engine, programmers, session, fixture_id) = rgb_engine();
    let report = engine.color_intent_report(None).unwrap();
    assert_eq!(report.len(), 1);
    assert_eq!(
        report[0].target, None,
        "no colour programmed yet: reported against white"
    );
    assert_eq!(
        report[0].quality,
        light_core::ColorResolutionQuality::Uncalibrated
    );

    programmers.set(session, fixture_id, AttributeKey::color(), dim_red());
    let report = engine.color_intent_report(None).unwrap();
    assert_eq!(
        report[0].target,
        Some(match dim_red() {
            AttributeValue::ColorXyz(color) => color,
            _ => unreachable!(),
        })
    );
    assert_eq!(
        report[0].engine,
        Some(light_fixture::ColorIntentEngine::Additive)
    );

    let (dimmer, dimmer_id) =
        schema_v2_fixture(&[("intensity", false, false, false, false, false)]);
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![dimmer].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let report = engine
        .color_intent_report(Some(&std::collections::HashSet::from([dimmer_id])))
        .unwrap();
    assert_eq!(
        report[0].quality,
        light_core::ColorResolutionQuality::Unsupported
    );
}
