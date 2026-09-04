use super::*;
use light_core::FrameAddressResolver;

/// A compiled cue list learns where the patch generation keeps its pairs, so a playback
/// contribution says where it lives; a pair the patch never numbered says nothing and is offered
/// by name. A repatch is a new generation, and the addresses follow it.
#[test]
fn playback_contributions_carry_the_current_generation_address() {
    let programmers = ProgrammerRegistry::default();
    let (fixture, fixture_id) = schema_v2_fixture(&[
        ("intensity", false, false, false, false, true),
        ("tilt", false, false, false, false, false),
    ]);
    let cue_list = test_cue_list(
        "Addressed",
        vec![
            CueChange::set(
                fixture_id,
                AttributeKey::intensity(),
                AttributeValue::Normalized(1.0),
            ),
            CueChange::set(
                fixture_id,
                AttributeKey("zoom".into()),
                AttributeValue::Normalized(0.5),
            ),
        ],
    );
    let playback = test_playback(1, cue_list.id);
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture.clone()].into(),
            cue_lists: vec![cue_list.clone()].into(),
            playbacks: vec![playback.clone()].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);

    let generation = engine.frame_addresser().generation();
    let contributions = engine.playback_contributions_at(Utc::now());
    let address_of = |name: &str| {
        contributions
            .iter()
            .find(|contribution| &*contribution.value.attribute.0 == name)
            .expect("the cue contributes the attribute")
            .address
    };
    assert_eq!(
        address_of("intensity").map(|address| address.generation),
        Some(generation),
        "a declared pair is offered by number for this generation"
    );
    assert_eq!(
        address_of("zoom"),
        None,
        "an undeclared pair is offered by name"
    );
    let frame = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(frame.universes[&1][0], u8::MAX);

    // A repatch: a second fixture ahead of the first renumbers everything.
    let (mut other, _) = schema_v2_fixture(&[("intensity", false, false, false, false, true)]);
    other.fixture_number = Some(2);
    other.address = Some(10);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![other, fixture].into(),
            cue_lists: vec![cue_list].into(),
            playbacks: vec![playback].into(),
            revision: 2,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let repatched = engine.frame_addresser().generation();
    assert_ne!(repatched, generation);
    let contributions = engine.playback_contributions_at(Utc::now());
    let intensity = contributions
        .iter()
        .find(|contribution| contribution.value.attribute.is_intensity())
        .expect("the playback survives the repatch");
    assert_eq!(
        intensity.address.map(|address| address.generation),
        Some(repatched),
        "the address follows the new generation"
    );
    let frame = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(
        frame.universes[&1][0],
        u8::MAX,
        "the value still lands on its channel"
    );
}
