use super::super::{
    compile_show_candidate, prepare_normalized_show_candidate_incremental, prepare_show_candidate,
};
use super::support::{document_with_objects, snapshot_without_revision};
use light_core::CueListId;
use light_playback::{Cue, CueList, CueListMode, IntensityPriorityMode, RestartMode, WrapMode};
use serde_json::{Value, json};
use std::sync::Arc;

fn normalized_document() -> (light_show::ShowStore, light_show::PortableShowDocument) {
    let cue = CueList {
        id: CueListId::new(),
        name: "Main".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Tracking),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues: vec![Cue::new(1.0)],
    };
    let (store, document) = document_with_objects(&[
        ("cue_list", "main", serde_json::to_value(cue).unwrap()),
        (
            "group",
            "front",
            json!({"id": "front", "name": "Front", "fixtures": []}),
        ),
        (
            "preset",
            "1.1",
            json!({"number": 1, "decimal": 1, "name": "Dim", "family": "Intensity", "values": {}}),
        ),
    ]);
    let transaction = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .0;
    if !transaction.is_empty() {
        store.apply_portable_transaction(transaction).unwrap();
    }
    let document = store.portable_document().unwrap();
    (store, document)
}

#[test]
fn cue_edit_rebuilds_only_the_playback_subgraph() {
    let (_store, document) = normalized_document();
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut cue = document.object("cue_list", "main").unwrap().body().clone();
    cue["name"] = json!("Updated");
    let mut transaction = document.transaction();
    transaction.put("cue_list", "main", cue);

    let incremental =
        prepare_normalized_show_candidate_incremental(&document, transaction.clone(), &previous)
            .unwrap()
            .into_parts()
            .1;
    let full = compile_show_candidate(document.candidate(&transaction).unwrap()).unwrap();

    assert_eq!(
        snapshot_without_revision(incremental.clone()),
        snapshot_without_revision(full)
    );
    assert!(Arc::ptr_eq(&incremental.fixtures, &previous.fixtures));
    assert!(Arc::ptr_eq(&incremental.groups, &previous.groups));
    assert!(Arc::ptr_eq(&incremental.routes, &previous.routes));
    assert!(Arc::ptr_eq(
        &incremental.control_mappings,
        &previous.control_mappings
    ));
    assert!(!Arc::ptr_eq(&incremental.cue_lists, &previous.cue_lists));
}

#[test]
fn preset_edit_reuses_every_runtime_projection() {
    let (_store, document) = normalized_document();
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut preset = document.object("preset", "1.1").unwrap().body().clone();
    preset["name"] = json!("Updated");
    let mut transaction = document.transaction();
    transaction.put("preset", "1.1", preset);
    let next = prepare_normalized_show_candidate_incremental(&document, transaction, &previous)
        .unwrap()
        .into_parts()
        .1;

    assert!(Arc::ptr_eq(&next.fixtures, &previous.fixtures));
    assert!(Arc::ptr_eq(&next.cue_lists, &previous.cue_lists));
    assert!(Arc::ptr_eq(&next.playbacks, &previous.playbacks));
    assert!(Arc::ptr_eq(&next.playback_pages, &previous.playback_pages));
    assert!(Arc::ptr_eq(&next.routes, &previous.routes));
    assert!(Arc::ptr_eq(
        &next.control_mappings,
        &previous.control_mappings
    ));
    assert!(Arc::ptr_eq(&next.groups, &previous.groups));
    assert_eq!(next.revision, previous.revision + 1);
}

#[test]
fn touched_legacy_object_is_normalized_without_sweeping_unrelated_objects() {
    let (_store, document) = normalized_document();
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut cue = document.object("cue_list", "main").unwrap().body().clone();
    let cue = cue.as_object_mut().unwrap();
    cue.remove("chaser_xfade_percent");
    cue.insert("chaser_xfade_millis".into(), json!(250));
    for field in [
        "intensity_priority_mode",
        "wrap_mode",
        "restart_mode",
        "force_cue_timing",
        "disable_cue_timing",
        "speed_multiplier",
    ] {
        cue.remove(field);
    }
    let mut transaction = document.transaction();
    transaction.put("cue_list", "main", Value::Object(cue.clone()));

    let prepared =
        prepare_normalized_show_candidate_incremental(&document, transaction, &previous).unwrap();
    let (transaction, next) = prepared.into_parts();
    let candidate = document.candidate(&transaction).unwrap();
    let migrated = candidate.object("cue_list", "main").unwrap().body();

    assert_eq!(migrated["chaser_xfade_percent"], 25);
    assert!(migrated.get("chaser_xfade_millis").is_none());
    assert_eq!(migrated["restart_mode"], "first_cue");
    assert_eq!(migrated["intensity_priority_mode"], "htp");
    assert!(Arc::ptr_eq(&next.groups, &previous.groups));
    assert!(Arc::ptr_eq(&next.fixtures, &previous.fixtures));
}
