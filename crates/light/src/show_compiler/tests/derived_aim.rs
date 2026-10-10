use super::super::compile_show_candidate;
use super::support::portable_fixture;
use light_core::{
    AttributeValue, PresetValueOwner, PresetValueReference,
    programming::{PositionIntent, TargetReference},
};
use light_programmer::{Preset, PresetFamily};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn derived_aim_link_materializes_current_source_instead_of_recorded_fallback() {
    let (_, mut fixture, _) = portable_fixture();
    fixture.fixture_number = Some(5);
    fixture.location.x = 2000;
    let identity = Uuid::new_v4();
    let preset = Preset {
        instance_id: Some(identity),
        family: PresetFamily::Position,
        number: 7,
        aim_at_fixture_number: Some(5),
        ..Default::default()
    };
    let fallback = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [9.0, 0.0, 0.0],
    )));
    let reference = PresetValueReference {
        preset_instance_id: identity,
        source_owner: PresetValueOwner::Universal,
        source_attribute: light_core::AttributeKey("position".into()),
        sample_rank: None,
        member_fixture: None,
    };
    let id = Uuid::new_v4();
    let cue = json!({"id":id,"name":"Aim","priority":0,"mode":"sequence","looped":false,"cues":[{"id":Uuid::new_v4(),"number":"1","name":"Aim","changes":[{"fixture_id":fixture.fixture_id,"attribute":"position","value":fallback,"preset_reference":reference}],"dynamic_changes":[],"fade_millis":0,"delay_millis":0,"trigger":{"type":"manual"}}]});
    let path = std::path::PathBuf::from(std::env::var_os("LIGHT_TMP_DIR").unwrap())
        .join(format!("derived-aim-{}.sqlite", Uuid::new_v4()));
    let (store, _) = light_show::ShowStore::create(&path, "Derived Aim reference").unwrap();
    for (kind, id, body) in &[
        (
            "patched_fixture",
            fixture.fixture_id.0.to_string().as_str(),
            serde_json::to_value(&fixture).unwrap(),
        ),
        ("preset", "3.7", serde_json::to_value(&preset).unwrap()),
        ("cue_list", id.to_string().as_str(), cue),
    ] {
        store.put_object(kind, id, body, 0).unwrap();
    }
    let profile = light_show::FixtureProfileRevision::from_profile(
        serde_json::to_value(fixture.definition.profile_snapshot.as_ref().unwrap()).unwrap(),
    )
    .unwrap();
    store.insert_fixture_profile_revision(&profile).unwrap();
    let document = store.portable_document().unwrap();
    let snapshot =
        compile_show_candidate(document.candidate(&document.transaction()).unwrap()).unwrap();
    assert_eq!(
        snapshot.cue_lists[0].cues[0].changes[0].value,
        Some(AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [2.0, 0.0, 0.0]
        ))))
    );
    let (migration, _) = super::super::prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts();
    store.apply_portable_transaction(migration).unwrap();
    let document = store.portable_document().unwrap();
    let previous = super::super::prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let fixture_key = fixture.fixture_id.0.to_string();
    let mut body = document
        .object("patched_fixture", &fixture_key)
        .unwrap()
        .body()
        .clone();
    body["location"]["x"] = json!(3000);
    let mut edit = document.transaction();
    edit.put("patched_fixture", &fixture_key, body);
    let (edit, updated) =
        super::super::prepare_normalized_show_candidate_incremental(&document, edit, &previous)
            .unwrap()
            .into_parts();
    assert_eq!(
        updated.cue_lists[0].cues[0].changes[0].value,
        Some(AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [3.0, 0.0, 0.0]
        ))))
    );
    store.apply_portable_transaction(edit).unwrap();
    let document = store.portable_document().unwrap();
    let mut changed_source = preset.clone();
    changed_source.aim_at_fixture_number = Some(999);
    let mut edit = document.transaction();
    edit.put(
        "preset",
        "3.7",
        serde_json::to_value(&changed_source).unwrap(),
    );
    let (edit, missing) =
        super::super::prepare_normalized_show_candidate_incremental(&document, edit, &updated)
            .unwrap()
            .into_parts();
    assert_eq!(
        missing.cue_lists[0].cues[0].changes[0].value,
        Some(fallback.clone())
    );
    store.apply_portable_transaction(edit).unwrap();
    let document = store.portable_document().unwrap();
    let mut edit = document.transaction();
    edit.put("preset", "3.7", serde_json::to_value(&preset).unwrap());
    let (edit, restored) =
        super::super::prepare_normalized_show_candidate_incremental(&document, edit, &missing)
            .unwrap()
            .into_parts();
    assert_eq!(
        restored.cue_lists[0].cues[0].changes[0].value,
        updated.cue_lists[0].cues[0].changes[0].value
    );
    store.apply_portable_transaction(edit).unwrap();
    let document = store.portable_document().unwrap();
    assert_eq!(
        document.object("cue_list", &id.to_string()).unwrap().body()["cues"][0]["changes"][0]["value"],
        serde_json::to_value(&fallback).unwrap(),
        "live compilation never rewrites recorded fallback"
    );
    assert!(
        document.object("preset", "3.7").unwrap().body()["universal_values"]
            .as_object()
            .is_none_or(|values| values.is_empty())
    );
    let mut edit = document.transaction();
    edit.delete("patched_fixture", &fixture_key);
    let (_, deleted) =
        super::super::prepare_normalized_show_candidate_incremental(&document, edit, &restored)
            .unwrap()
            .into_parts();
    assert_eq!(
        deleted.cue_lists[0].cues[0].changes[0].value,
        Some(fallback.clone())
    );
    let mut edit = document.transaction();
    edit.delete("preset", "3.7");
    let (_, deleted_source) =
        super::super::prepare_normalized_show_candidate_incremental(&document, edit, &restored)
            .unwrap()
            .into_parts();
    assert_eq!(
        deleted_source.cue_lists[0].cues[0].changes[0].value,
        Some(fallback.clone())
    );
    drop(store);
    let reopened = light_show::ShowStore::open(&path).unwrap();
    let document = reopened.portable_document().unwrap();
    let reopened_snapshot = super::super::prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    assert_eq!(
        reopened_snapshot.cue_lists[0].cues[0].changes[0].value,
        updated.cue_lists[0].cues[0].changes[0].value
    );
    // An explicit modern Record into the same legacy source replaces its relation. The live
    // Cue identity remains linked, but moving the old target must no longer retarget it.
    let modern_value = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [8.0, 1.0, 4.0],
    )));
    let mut modern = preset.clone();
    modern.store(
        Preset {
            family: PresetFamily::Position,
            number: 7,
            universal_values: [(
                light_core::AttributeKey("position".into()),
                modern_value.clone(),
            )]
            .into(),
            ..Default::default()
        },
        light_programmer::PresetStoreMode::Overwrite,
    );
    assert_eq!(modern.instance_id, Some(identity));
    assert_eq!(modern.aim_at_fixture_number, None);
    let mut edit = document.transaction();
    edit.put("preset", "3.7", serde_json::to_value(&modern).unwrap());
    let (edit, converted) = super::super::prepare_normalized_show_candidate_incremental(
        &document,
        edit,
        &reopened_snapshot,
    )
    .unwrap()
    .into_parts();
    assert_eq!(
        converted.cue_lists[0].cues[0].changes[0].value,
        Some(modern_value.clone())
    );
    reopened.apply_portable_transaction(edit).unwrap();
    let document = reopened.portable_document().unwrap();
    let mut old_target = document
        .object("patched_fixture", &fixture_key)
        .unwrap()
        .body()
        .clone();
    old_target["location"]["x"] = json!(7000);
    let mut edit = document.transaction();
    edit.put("patched_fixture", &fixture_key, old_target);
    let (edit, moved) =
        super::super::prepare_normalized_show_candidate_incremental(&document, edit, &converted)
            .unwrap()
            .into_parts();
    assert_eq!(
        moved.cue_lists[0].cues[0].changes[0].value,
        Some(modern_value.clone())
    );
    reopened.apply_portable_transaction(edit).unwrap();
    drop(reopened);
    let reopened = light_show::ShowStore::open(&path).unwrap();
    let document = reopened.portable_document().unwrap();
    let persisted: Preset =
        serde_json::from_value(document.object("preset", "3.7").unwrap().body().clone()).unwrap();
    assert_eq!(persisted.instance_id, Some(identity));
    assert_eq!(persisted.aim_at_fixture_number, None);
    let (_, snapshot) = super::super::prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts();
    assert_eq!(
        snapshot.cue_lists[0].cues[0].changes[0].value,
        Some(modern_value)
    );
    assert_eq!(
        document.object("cue_list", &id.to_string()).unwrap().body()["cues"][0]["changes"][0]["value"],
        serde_json::to_value(&fallback).unwrap()
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}
