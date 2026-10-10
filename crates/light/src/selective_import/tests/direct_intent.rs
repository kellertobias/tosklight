//! TL-559 AC2/AC6: tagged Direct Color values through the application's real Preset writer,
//! the SQLite reopen, selective import (with its undo) and the show-open compiler.
//!
//! The portable estimate (known black, reduced output with a nondefault relative output,
//! UV-only black), the exact recipe and the pinned original identity are compared field by
//! field after every boundary: nothing is normalized, re-predicted or re-identified.
use super::programming_native::{NativeFixture, native_fixture};
use super::support::*;
use crate::selective_import::*;
use crate::{
    ActiveShowObjectBody, ActiveShowObjectKind, ProgrammingPresetCommit,
    ProgrammingPresetCommitResult, ProgrammingPresetRecordRequest,
    ProgrammingPresetRevisionExpectation, prepare_show_candidate,
};
use light_core::programming::{ColorProgram, PortableColorEstimate, ProgrammingOwner};
use light_core::{AttributeValue, NativeColorIdentity};
use light_programmer::{Preset, PresetAddress, PresetStoreMode};
use serde_json::{Value, json};
use uuid::Uuid;

const CUE_LIST_ID: &str = "00000000-0000-0000-0000-00000559c0e1";

/// A tagged Direct value with an explicit portable estimate.
fn direct(source: &NativeColorIdentity, channels: &Value, portable: Value) -> Value {
    json!({"kind":"color_program","value":{"kind":"direct",
        "recipe":{"source":source,"channels":channels,"spreads":[]},"portable":portable}})
}

fn portable(xyz: [f64; 3], relative_output: f64, uv: f64) -> Value {
    json!({"model_revision":1,
        "visible":{"xyz":{"x":xyz[0],"y":xyz[1],"z":xyz[2]},"relative_output":relative_output},
        "uv":{"amount":uv,"quality":"estimated"},"quality":"estimated",
        "limitations":["tl559 recorded estimate"]})
}

/// Known black, reduced output (relative output 0.35) and UV-only black.
fn portables() -> [(&'static str, Value); 3] {
    [
        ("2.11", portable([0., 0., 0.], 1., 0.)),
        ("2.12", portable([0.11, 0.07, 0.02], 0.35, 0.4)),
        ("2.13", portable([0., 0., 0.], 1., 1.)),
    ]
}

fn preset_body(source: &NativeFixture, number: u16, portable: Value) -> Value {
    let mut values = serde_json::Map::new();
    values.insert(
        source.fixture_id.0.to_string(),
        json!({"color": direct(&source.identity, &source.channels, portable)}),
    );
    json!({"instance_id": Uuid::from_u128(source.fixture_id.0.as_u128() ^ (u128::from(number) << 96)), "name":format!("Direct {number}"),"family":"Color","number":number,
        "values":values,"group_values":{},"universal_values":{}})
}

fn static_cue_list(source: &NativeFixture, portable: Value) -> Value {
    json!({
        "id": CUE_LIST_ID, "name": "Direct static", "priority": 10,
        "mode": "sequence", "looped": false,
        "cues": [{
            "id": Uuid::new_v4(), "number": "1", "name": "Direct",
            "changes": [{"fixture_id": source.fixture_id, "attribute": "color",
                "automatic_restore": false,
                "value": direct(&source.identity, &source.channels, portable)}],
            "dynamic_changes": [], "fade_millis": 0, "delay_millis": 0,
            "trigger": {"type":"manual"}
        }]
    })
}

/// The stored tagged program and its exact portable estimate.
fn stored_program(body: &Value, pointer: &str) -> ColorProgram {
    serde_json::from_value(body.pointer(pointer).unwrap().clone()).unwrap()
}

fn assert_direct(program: &ColorProgram, source: &NativeFixture, expected: &Value) {
    let ColorProgram::Direct { recipe, portable } = program else {
        panic!("tagged Direct value expected")
    };
    assert_eq!(&recipe.source, &source.identity, "pinned original identity");
    assert_eq!(
        serde_json::to_value(&recipe.channels).unwrap(),
        source.channels,
        "exact recipe"
    );
    let expected: PortableColorEstimate = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(portable, &expected, "portable estimate unchanged");
}

fn member_pointer(source: &NativeFixture) -> String {
    format!("/values/{}/color/value", source.fixture_id.0)
}

fn seed_source(rig: &TestRig, source: &NativeFixture) {
    rig.source_profile(&source.revision);
    rig.source_object(
        "patched_fixture",
        &source.fixture_id.0.to_string(),
        source.fixture_body.clone(),
    );
}

/// AC2: black, reduced output/relative output, UV and pinned provenance survive import of
/// Presets and a static Cue list and recompile through the show-open reader.
#[test]
fn import_keeps_direct_portable_appearance_relative_output_uv_and_pinned_identity() {
    let rig = TestRig::new();
    let source = native_fixture();
    seed_source(&rig, &source);
    let mut keys = vec![key("cue_list", CUE_LIST_ID)];
    for (index, (id, portable)) in portables().into_iter().enumerate() {
        rig.source_object(
            "preset",
            id,
            preset_body(&source, 11 + index as u16, portable),
        );
        keys.push(key("preset", id));
    }
    let reduced = portables()[1].1.clone();
    rig.source_object(
        "cue_list",
        CUE_LIST_ID,
        static_cue_list(&source, reduced.clone()),
    );
    let preview = rig.preview(SelectiveShowImportRequest::new(
        rig.source_id,
        rig.target_id,
        keys,
    ));
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    rig.apply(&preview).unwrap();

    let target = rig.target_document();
    for (id, portable) in portables() {
        let body = target.object("preset", id).unwrap().body();
        assert_direct(
            &stored_program(body, &member_pointer(&source)),
            &source,
            &portable,
        );
    }
    let list = target.object("cue_list", CUE_LIST_ID).unwrap().body();
    assert_direct(
        &stored_program(list, "/cues/0/changes/0/value/value"),
        &source,
        &reduced,
    );
    let (_, snapshot) = prepare_show_candidate(&target, target.transaction())
        .unwrap()
        .into_parts();
    let compiled = &snapshot
        .cue_lists
        .iter()
        .find(|list| list.name == "Direct static")
        .unwrap()
        .cues[0]
        .changes[0];
    let Some(AttributeValue::ColorProgram(program)) = &compiled.value else {
        panic!("compiled Color program")
    };
    assert_direct(program, &source, &reduced);
}

/// AC6: selective import undo restores the replaced destination Direct Preset byte for byte.
#[test]
fn import_undo_restores_the_replaced_direct_preset_exactly() {
    let rig = TestRig::new();
    let source = native_fixture();
    seed_source(&rig, &source);
    rig.target_profile(&source.revision);
    rig.target_object(
        "patched_fixture",
        &source.fixture_id.0.to_string(),
        source.fixture_body.clone(),
    );
    let [(id, black), (_, reduced), _] = portables();
    let original = preset_body(&source, 11, black.clone());
    rig.target_object("preset", id, original.clone());
    rig.source_object("preset", id, preset_body(&source, 11, reduced.clone()));
    let preview = rig.preview(rig.request("preset", id).resolve(
        key("preset", id),
        ImportConflictResolution::ReplaceDestination,
    ));
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let applied = rig.apply(&preview).unwrap();
    let replaced = rig
        .target_document()
        .object("preset", id)
        .unwrap()
        .body()
        .clone();
    assert_direct(
        &stored_program(&replaced, &member_pointer(&source)),
        &source,
        &reduced,
    );
    let undo = applied.undo.expect("changed import has an undo target");
    rig.service.undo(&context(), &undo, &rig.ports).unwrap();
    let restored = rig
        .target_document()
        .object("preset", id)
        .unwrap()
        .body()
        .clone();
    assert_eq!(restored, original, "exact prior body");
    assert_direct(
        &stored_program(&restored, &member_pointer(&source)),
        &source,
        &black,
    );
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

/// AC6: Preset record through the real writer reopens with the identical tagged value; an
/// identical re-record is a verified no-change; a semantic value beside it stays semantic.
#[test]
fn recorded_direct_preset_reopens_identically_and_rerecord_is_a_no_change() {
    let rig = TestRig::new();
    let source = native_fixture();
    rig.target_profile(&source.revision);
    rig.target_object(
        "patched_fixture",
        &source.fixture_id.0.to_string(),
        source.fixture_body.clone(),
    );
    let reduced = portables()[1].1.clone();
    let body = preset_body(&source, 12, reduced.clone());
    let mut preset: Preset = serde_json::from_value(body).unwrap();
    let semantic = crate::programming::semantic_intent_cases::color(
        crate::programming::semantic_intent_cases::uv_only_black(),
    );
    preset
        .universal_values
        .insert(ProgrammingOwner::Color.key(), semantic.clone());
    let first = record(&rig, &preset);
    assert!(first.changed);
    let document = rig.target_document();
    let object = document
        .object("preset", &first.projection.object_id)
        .unwrap();
    assert_direct(
        &stored_program(object.body(), &member_pointer(&source)),
        &source,
        &reduced,
    );
    let decoded =
        ActiveShowObjectBody::decode(ActiveShowObjectKind::Preset, object.body().clone()).unwrap();
    let typed = decoded.preset().unwrap().typed();
    assert_eq!(typed.values, preset.values);
    assert_eq!(
        typed.universal_values[&ProgrammingOwner::Color.key()],
        semantic,
        "semantic portability kept separately"
    );
    assert!(!record(&rig, &preset).changed, "identical re-record");
}
