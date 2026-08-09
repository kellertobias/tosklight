use std::collections::{BTreeMap, HashSet};

use light_core::{AttributeKey, FixtureId};
use light_engine::{Engine, EngineSnapshot, RenderOptions};
use light_fixture::{
    CanonicalTransform, ChannelBehavior, ChannelFunction, ChannelResolution, FixtureChannel,
    FixtureProfile, PatchedFixture,
};
use light_programmer::{HighlightOutputLayer, HighlightOutputRole, ProgrammerRegistry};

#[test]
fn temporary_high_low_layers_and_explicit_fallthrough_restore_exact_output() {
    let (fixture, fixture_id) = intensity_fixture(64);
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .set_highlight_look(light_fixture::HighlightLook::default())
        .unwrap();
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    let ordinary = engine.render(RenderOptions::default()).unwrap().universes[&1];
    assert_eq!(ordinary[0], 64);

    engine.set_highlight_layers([layer(
        fixture_id,
        HighlightOutputRole::LowLight,
        HashSet::new(),
    )]);
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        26
    );

    engine.set_highlight_layers([layer(
        fixture_id,
        HighlightOutputRole::Highlight,
        HashSet::new(),
    )]);
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        255
    );

    engine.set_highlight_layers([layer(
        fixture_id,
        HighlightOutputRole::Highlight,
        HashSet::from([AttributeKey::intensity()]),
    )]);
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1][0],
        64,
        "an explicit Programmer attribute falls through the temporary look"
    );

    engine.clear_highlighted_fixtures();
    assert_eq!(
        engine.render(RenderOptions::default()).unwrap().universes[&1],
        ordinary
    );
}

fn layer(
    fixture_id: FixtureId,
    role: HighlightOutputRole,
    suppressed_attributes: HashSet<AttributeKey>,
) -> HighlightOutputLayer {
    HighlightOutputLayer {
        fixture_id,
        role,
        suppressed_attributes,
    }
}

fn intensity_fixture(default_raw: u32) -> (PatchedFixture, FixtureId) {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Highlight layer".into();
    profile.short_name = "Highlight".into();
    profile.revision = 1;
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    mode.splits[0].footprint = 1;
    mode.channels = vec![FixtureChannel {
        id: uuid::Uuid::new_v4(),
        head_id,
        split: 1,
        fixture_attribute: AttributeKey::intensity(),
        attribute: AttributeKey::intensity(),
        canonical_transform: CanonicalTransform::Identity,
        resolution: ChannelResolution::U8,
        secondary_slots: vec![],
        default_raw,
        highlight_raw: 255,
        physical_min: Some(0.0),
        physical_max: Some(1.0),
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: false,
        reacts_to_group_master: false,
        reacts_to_grand_master: false,
        behavior: ChannelBehavior::Controlled,
        functions: vec![ChannelFunction::continuous(
            "Intensity",
            AttributeKey::intensity(),
            255,
        )],
    }];
    let mode_id = mode.id;
    let definition = profile.resolved_definition(mode_id).unwrap();
    let fixture_id = FixtureId::new();
    (
        PatchedFixture {
            fixture_id,
            fixture_number: Some(1),
            virtual_fixture_number: None,
            name: "Highlight layer".into(),
            definition,
            universe: Some(1),
            address: Some(1),
            split_patches: vec![],
            layer_id: "default".into(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![],
            group_masters_enabled: true,
            grand_master_enabled: true,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.0,
            shaper_angle: None,
            installed_appearance: Default::default(),
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        },
        fixture_id,
    )
}
