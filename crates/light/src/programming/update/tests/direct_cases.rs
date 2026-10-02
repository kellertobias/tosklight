//! TL-559 AC6: tagged Direct Color values through the actual Preset/Cue Update planners, Preset
//! recall materialization and Programmer session isolation. A Direct value is stored and recalled
//! as the exact tagged program (recipe, pinned identity and portable estimate); semantic values
//! beside it keep their ordinary portable representation.
use super::*;
use crate::programming::preset_recall_plan::plan_preset_selection_values;
use crate::programming::semantic_intent_cases::{magenta, uv_only_black};
use light_core::programming::{
    ColorProgram, NativeColorRecipe, PortableColorEstimate, PortableUv, PortableVisibleColor,
    ProgrammingOwner,
};
use light_core::{NativeColorIdentity, NativeColorValue, PhysicalDataQuality, SessionId, Xyz};
use light_programmer::{NormalProgrammerValueMutation, ProgrammerRegistry, ProgrammerSelection};
use std::sync::Arc;

pub(crate) fn direct_value(visible: Option<Xyz>, uv: Option<f32>, raw: u32) -> AttributeValue {
    let source = NativeColorIdentity {
        profile_id: Uuid::from_u128(0x559_01),
        profile_revision: 3,
        profile_digest: "tl559-profile-digest".into(),
        mode_id: Uuid::from_u128(0x559_02),
        head_id: Uuid::from_u128(0x559_03),
        path_id: Uuid::from_u128(0x559_04),
        model_revision: 2,
        native_layout_signature: "tl559-layout".into(),
    };
    let program = ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source,
            channels: (0..3u128)
                .map(|index| NativeColorValue {
                    channel_id: Uuid::from_u128(0x559_10 + index),
                    function_id: Uuid::from_u128(0x559_20 + index),
                    raw: raw + index as u32,
                })
                .collect(),
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 2,
            visible: visible.map(|xyz| PortableVisibleColor {
                xyz,
                relative_output: 1.0,
            }),
            uv: uv.map(|amount| PortableUv {
                amount,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec!["recorded".into()],
        },
    };
    program.validate().unwrap();
    AttributeValue::ColorProgram(Arc::new(program))
}

fn semantic(intent: light_core::programming::ColorIntent) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
}

const DIM: Xyz = Xyz {
    x: 0.11,
    y: 0.07,
    z: 0.02,
};

fn color_update(
    fixture_id: FixtureId,
    value: AttributeValue,
    order: u64,
) -> ProgrammerFixtureUpdate {
    ProgrammerFixtureUpdate {
        fixture_id,
        attribute: ProgrammingOwner::Color.key(),
        value,
        programmer_order: order,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    }
}

#[test]
fn preset_update_stores_tagged_direct_and_keeps_semantic_neighbours_portable() {
    let (a, b) = (fixture(1), fixture(2));
    let color = ProgrammingOwner::Color.key();
    let preset = Preset {
        name: "Color 5".into(),
        family: light_programmer::PresetFamily::Color,
        number: 5,
        values: [a, b]
            .into_iter()
            .map(|id| (id, HashMap::from([(color.clone(), semantic(magenta()))])))
            .collect(),
        group_values: HashMap::new(),
        aim_at_fixture_number: None,
        universal_values: HashMap::from([(color.clone(), semantic(uv_only_black()))]),
    };
    let reduced = direct_value(Some(DIM), Some(0.4), 1000);
    let plan = plan_preset_update(
        "5",
        &preset,
        2,
        2,
        ExistingContentMode::UpdateExisting,
        &content(vec![color_update(a, reduced.clone(), 1)]),
    )
    .unwrap();
    let updated = planned_preset(plan);
    assert_eq!(updated.values[&a][&color], reduced, "exact tagged Direct");
    assert_eq!(updated.values[&b][&color], semantic(magenta()));
    assert_eq!(updated.universal_values[&color], semantic(uv_only_black()));

    // Updating the Direct value back to Semantic replaces the whole tagged value.
    let plan = plan_preset_update(
        "5",
        &updated,
        3,
        3,
        ExistingContentMode::UpdateExisting,
        &content(vec![color_update(a, semantic(magenta()), 2)]),
    )
    .unwrap();
    assert_eq!(planned_preset(plan).values[&a][&color], semantic(magenta()));
}

#[test]
fn cue_update_replaces_the_tracked_source_with_the_exact_direct_value() {
    let (a, b) = (fixture(1), fixture(2));
    let color = ProgrammingOwner::Color.key();
    let set = |fixture_id, value| CueChange::set(fixture_id, color.clone(), value);
    let list = cue_list(vec![
        cue(
            1.0,
            vec![set(a, semantic(magenta())), set(b, semantic(magenta()))],
        ),
        cue(2.0, vec![]),
    ]);
    let black = direct_value(
        Some(Xyz {
            x: 0.,
            y: 0.,
            z: 0.,
        }),
        Some(0.9),
        0,
    );
    let plan = plan_cue_update(
        &list,
        4,
        4,
        &target(&list, 1, Some(1)),
        CueUpdateMode::ExistingOnly,
        &content(vec![color_update(a, black.clone(), 1)]),
    )
    .unwrap();
    let updated = planned_cue_list(plan);
    let value = |cue: &Cue, fixture_id| {
        cue.changes
            .iter()
            .find(|change| change.fixture_id == fixture_id)
            .and_then(|change| change.value.clone())
    };
    assert_eq!(
        value(&updated.cues[0], a),
        Some(black),
        "UV-only black kept"
    );
    assert_eq!(value(&updated.cues[0], b), Some(semantic(magenta())));
}

#[test]
fn preset_recall_materializes_tagged_direct_without_baking_destination_values() {
    let (a, b, c) = (fixture(1), fixture(2), fixture(3));
    let color = ProgrammingOwner::Color.key();
    let unknown = direct_value(None, None, 77);
    let universal = direct_value(Some(DIM), Some(0.), 5);
    let preset = Preset {
        name: "Direct".into(),
        family: light_programmer::PresetFamily::Color,
        number: 9,
        values: HashMap::from([
            (a, HashMap::from([(color.clone(), unknown.clone())])),
            (b, HashMap::from([(color.clone(), semantic(magenta()))])),
        ]),
        group_values: HashMap::new(),
        aim_at_fixture_number: None,
        universal_values: HashMap::from([(color.clone(), universal.clone())]),
    };
    let selection = ProgrammerSelection {
        selected: vec![a, b, c],
        ..Default::default()
    };
    let planned =
        plan_preset_selection_values(&selection, &preset, &HashMap::new(), &HashMap::new(), 0)
            .unwrap();
    let recalled = |fixture_id| {
        planned.iter().find_map(|mutation| match mutation {
            NormalProgrammerValueMutation::SetFixture {
                fixture_id: id,
                attribute,
                value,
                ..
            } if *id == fixture_id && attribute == &color => Some(value.clone()),
            _ => None,
        })
    };
    assert_eq!(
        recalled(a),
        Some(unknown),
        "unknown appearance stays tagged Direct"
    );
    assert_eq!(
        recalled(b),
        Some(semantic(magenta())),
        "semantic stays portable"
    );
    assert_eq!(
        recalled(c),
        Some(universal),
        "universal Direct template, not baked"
    );
}

/// The Programmer (one desk: every session of it reads the same authoritative state) keeps
/// the exact tagged Direct value beside semantic edits of other fixtures and through a Group
/// assignment of another owner value.
#[test]
fn programmer_sessions_read_the_exact_tagged_direct_value() {
    let registry = ProgrammerRegistry::default();
    let (first, second) = (SessionId::new(), SessionId::new());
    registry.start(first);
    registry.start(second);
    let (a, b) = (fixture(1), fixture(2));
    let color = ProgrammingOwner::Color.key();
    let value = direct_value(Some(DIM), Some(0.4), 1000);
    registry.set(first, a, color.clone(), value.clone());
    registry.set(second, b, color.clone(), semantic(magenta()));
    let get = |session, fixture_id| {
        registry.get(session).and_then(|state| {
            state
                .values
                .iter()
                .find(|v| v.fixture_id == fixture_id && v.attribute == color)
                .map(|v| v.value.clone())
        })
    };
    for session in [first, second] {
        assert_eq!(get(session, a), Some(value.clone()), "exact tagged Direct");
        assert_eq!(get(session, b), Some(semantic(magenta())));
    }
}
