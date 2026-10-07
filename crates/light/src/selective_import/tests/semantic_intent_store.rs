//! TL-567: requested semantic intent through the application's real writers and show-open reader.
//!
//! Presets are written by `ActiveShowService::commit_programming_preset`, the Cue list by the
//! generic typed object writer. Each is then reopened from the SQLite file and compared both as
//! the exact persisted body and as typed intent; the Cue list is also recompiled through the
//! show-open candidate compiler. Identities are unchanged here; `semantic_intent.rs` covers import.
use super::semantic_intent::{
    CUE_LIST_ID, beam_preset, color_preset, cue_changes, cue_list, position_preset,
};
use super::support::*;
use crate::programming::semantic_intent_cases::{
    assert_persisted_eq, assert_semantic_attribute_map, color, warm_white_3200,
};
use crate::{
    ActionEnvelope, ActiveShowObjectBody, ActiveShowObjectKind, ActiveShowObjectMutation,
    ActiveShowObjectMutationKind, MutateActiveShowObjectsCommand, ProgrammingPresetActiveShowPorts,
    ProgrammingPresetCommit, ProgrammingPresetCommitResult, ProgrammingPresetRecordRequest,
    ProgrammingPresetRevisionExpectation, prepare_show_candidate,
};
use light_core::FixtureId;
use light_core::programming::{ColorIntent, ProgrammingOwner};
use light_programmer::{GroupDefinition, Preset, PresetAddress, PresetStoreMode};
use serde_json::json;

impl ProgrammingPresetActiveShowPorts for TestPorts {}

struct TargetShow {
    fixture: FixtureId,
    point: FixtureId,
}

fn seed_target(rig: &TestRig) -> TargetShow {
    let fixture = portable_fixture_record(571_000, 1);
    let point = portable_fixture_record(572_000, 2);
    for record in [&fixture, &point] {
        rig.target_profile(&record.profile);
        rig.target_object(
            "patched_fixture",
            &record.fixture_id.0.to_string(),
            record.body.clone(),
        );
    }
    rig.target_object(
        "group",
        "semantic-front",
        serde_json::to_value(GroupDefinition {
            id: "semantic-front".into(),
            name: "Front".into(),
            fixtures: vec![fixture.fixture_id],
            ..Default::default()
        })
        .unwrap(),
    );
    TargetShow {
        fixture: fixture.fixture_id,
        point: point.fixture_id,
    }
}

fn record(rig: &TestRig, preset: &Preset) -> ProgrammingPresetCommitResult {
    let request = ProgrammingPresetRecordRequest {
        show_id: rig.target_id,
        address: PresetAddress::new(preset.family, preset.number).unwrap(),
        name: preset.name.clone(),
        mode: PresetStoreMode::Overwrite,
        expected_object_revision: ProgrammingPresetRevisionExpectation::Current,
        expected_show_revision: None,
    };
    let commit = ProgrammingPresetCommit::new(&request, preset.clone());
    rig.active_show
        .commit_programming_preset(&context(), &commit, &rig.ports)
        .unwrap()
}

#[test]
fn recorded_semantic_presets_reopen_with_identical_bodies_and_typed_intent() {
    let rig = TestRig::new();
    let target = seed_target(&rig);
    for preset in [
        color_preset(target.fixture),
        position_preset(target.fixture, target.point.0),
        beam_preset(target.fixture),
    ] {
        let result = record(&rig, &preset);
        assert!(result.changed, "{}", preset.name);
        let document = rig.target_document();
        let object = document
            .object("preset", &result.projection.object_id)
            .unwrap();
        assert_persisted_eq(object.body(), &result.projection.raw_body);
        let loaded =
            ActiveShowObjectBody::decode(ActiveShowObjectKind::Preset, object.body().clone())
                .unwrap();
        let loaded = loaded.preset().unwrap().typed();
        assert_eq!(loaded.values, preset.values, "{}", preset.name);
        assert_eq!(loaded.group_values, preset.group_values, "{}", preset.name);
        assert_eq!(
            loaded.universal_values, preset.universal_values,
            "{}",
            preset.name
        );
        let allowed = ["color", "position", "focus", "zoom"];
        for map in object.body()["values"]
            .as_object()
            .unwrap()
            .values()
            .chain(object.body()["group_values"].as_object().unwrap().values())
            .chain([&object.body()["universal_values"]])
        {
            assert_semantic_attribute_map(map, &allowed);
        }
    }
}

/// Recording identical intent into a Preset reopened from disk must be a verified no-change.
///
/// Guards TL-571 (TL-567 defect D2): the store reparses `body_json`, and without `serde_json`'s
/// `float_roundtrip` some `f64` numbers came back one ULP off (magenta/warm-white `base_xyz.z`
/// 0.22964008152484894 reloaded as 0.22964008152484897), so `lossless_json::merge_typed` rewrote
/// the body and an identical re-record committed a new revision, undo entry and event.
#[test]
fn rerecording_identical_semantic_preset_after_reopen_is_a_verified_no_change() {
    let rig = TestRig::new();
    let target = seed_target(&rig);
    let preset = color_preset(target.fixture);
    let first = record(&rig, &preset);
    assert!(first.changed);
    let revision = rig.target_document().revision();
    let second = record(&rig, &preset);
    assert!(!second.changed);
    assert_eq!(
        second.projection.object_revision,
        first.projection.object_revision
    );
    assert_eq!(rig.target_document().revision(), revision);
}

/// Numeric fidelity must not hide a real authored change: a small White Blend edit and an
/// explicit zero relativeOutput both record new revisions after the no-change re-record.
#[test]
fn small_and_zero_authored_numeric_changes_still_record_after_reopen() {
    let rig = TestRig::new();
    let target = seed_target(&rig);
    let preset = color_preset(target.fixture);
    assert!(record(&rig, &preset).changed);
    assert!(!record(&rig, &preset).changed);
    let key = ProgrammingOwner::Color.key();
    let edited = |edit: &dyn Fn(&mut ColorIntent)| {
        let mut preset = preset.clone();
        let mut intent = warm_white_3200();
        edit(&mut intent);
        preset
            .values
            .get_mut(&target.fixture)
            .unwrap()
            .insert(key.clone(), color(intent));
        preset
    };
    let nudged = edited(&|intent| intent.white_blend += 0.001);
    assert!(
        record(&rig, &nudged).changed,
        "a 0.001 White Blend edit must record"
    );
    assert!(!record(&rig, &nudged).changed);
    let zero = edited(&|intent| {
        intent.white_blend += 0.001;
        intent.relative_output = 0.0;
    });
    let stored = record(&rig, &zero);
    assert!(
        stored.changed,
        "an explicit zero relativeOutput must record"
    );
    let document = rig.target_document();
    let body = document
        .object("preset", &stored.projection.object_id)
        .unwrap()
        .body()
        .clone();
    let decoded = ActiveShowObjectBody::decode(ActiveShowObjectKind::Preset, body).unwrap();
    assert_eq!(
        decoded.preset().unwrap().typed().values[&target.fixture][&key],
        color({
            let mut intent = warm_white_3200();
            intent.white_blend += 0.001;
            intent.relative_output = 0.0;
            intent
        })
    );
}

#[test]
fn semantic_cue_list_reopens_and_recompiles_through_the_show_open_reader() {
    let rig = TestRig::new();
    let target = seed_target(&rig);
    let body = cue_list(target.fixture, target.point.0);
    rig.active_show
        .mutate_objects(
            ActionEnvelope {
                context: context(),
                command: MutateActiveShowObjectsCommand {
                    show_id: rig.target_id,
                    mutations: vec![ActiveShowObjectMutation {
                        kind: ActiveShowObjectKind::CueList,
                        object_id: CUE_LIST_ID.into(),
                        expected_object_revision: 0,
                        mutation: ActiveShowObjectMutationKind::Put {
                            body: Box::new(
                                ActiveShowObjectBody::decode(
                                    ActiveShowObjectKind::CueList,
                                    body.clone(),
                                )
                                .unwrap(),
                            ),
                        },
                    }],
                },
            },
            &rig.ports,
        )
        .unwrap();

    let document = rig.target_document();
    let stored = document.object("cue_list", CUE_LIST_ID).unwrap().body();
    for pointer in ["/changes", "/group_changes"] {
        assert_persisted_eq(
            &stored["cues"][0][&pointer[1..]],
            &body["cues"][0][&pointer[1..]],
        );
    }
    assert_eq!(
        stored.pointer("/cues/0/changes/0/value/value/intent/relative_output"),
        Some(&json!(0.0)),
        "zero relative output must be stored explicitly"
    );
    let (_, snapshot) = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts();
    let list = snapshot
        .cue_lists
        .iter()
        .find(|list| list.name == "Semantic")
        .unwrap();
    let (changes, group_changes) = cue_changes(target.fixture, target.point.0);
    assert_eq!(list.cues[0].changes, changes);
    assert_eq!(list.cues[0].group_changes, group_changes);
}
