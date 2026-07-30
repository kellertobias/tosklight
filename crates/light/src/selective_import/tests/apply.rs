use super::support::*;
use crate::{
    ActionEnvelope, ActionErrorKind, ActiveShowObjectBody, ActiveShowObjectKind,
    ActiveShowObjectMutation, ActiveShowObjectMutationKind, ApplicationEvent, EventFilter,
    EventReplay, MutateActiveShowObjectsCommand, ShowEvent, selective_import::*,
};
use serde_json::json;
use std::{
    sync::{atomic::Ordering, mpsc},
    time::Duration,
};
use uuid::Uuid;

#[test]
fn apply_commits_dependency_closure_once_and_preserves_unknown_extensions() {
    let rig = TestRig::new();
    rig.source_object(
        "macro",
        "root",
        json!({"id":"root","macro_id":"child","future":{"root":true}}),
    );
    rig.source_object(
        "macro",
        "child",
        json!({"id":"child","future":{"nested":[{"opaque":7}]}}),
    );
    let preview = rig.preview(rig.request("macro", "root"));
    let before = rig.target_document().revision();
    let event_before = rig.active_show.events().latest_sequence();
    rig.clear_steps();

    let result = rig.apply(&preview).unwrap();

    assert!(result.changed);
    assert_eq!(result.change.show_revision.value(), before.value() + 1);
    assert_eq!(
        rig.steps(),
        vec![
            "source",
            "begin",
            "begin",
            "prepare",
            "backup",
            "commit",
            "install",
            "reconcile"
        ]
    );
    assert_eq!(result.event_sequence, Some(event_before + 1));
    let EventReplay::Events(events) = rig
        .active_show
        .events()
        .replay(event_before, &EventFilter::default())
    else {
        panic!("selective import event should be retained");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].sequence, result.event_sequence.unwrap());
    assert_eq!(
        events[0].correlation_id,
        Some(result.context.correlation_id)
    );
    assert!(matches!(
        events[0].payload,
        ApplicationEvent::Show(ShowEvent::SelectiveImportApplied(_))
    ));
    let target = rig.target_document();
    assert_eq!(
        target.object("macro", "root").unwrap().body()["future"]["root"],
        true
    );
    assert_eq!(
        target.object("macro", "child").unwrap().body()["future"]["nested"][0]["opaque"],
        7
    );
}

#[test]
fn selective_import_undo_restores_replaced_objects_and_deletes_created_objects_once() {
    let rig = TestRig::new();
    rig.source_object("macro", "root", json!({"id":"root","macro_id":"child"}));
    rig.source_object("macro", "child", json!({"id":"child","name":"Source"}));
    let previous_child = json!({"id":"child","name":"Destination"});
    rig.target_object("macro", "child", previous_child.clone());
    let preview = rig.preview(rig.request("macro", "root"));

    let applied = rig.apply(&preview).unwrap();
    let undo = applied.undo.expect("changed import undo target");
    let imported_revision = applied.change.show_revision;

    let undone_revision = rig.service.undo(&context(), &undo, &rig.ports).unwrap();

    let target = rig.target_document();
    assert_eq!(undone_revision, imported_revision.value() + 1);
    assert!(target.object("macro", "root").is_none());
    assert_eq!(
        target.object("macro", "child").unwrap().body(),
        &previous_child
    );
}

#[test]
fn apply_imports_dynamic_and_its_dependency_closure_as_one_valid_candidate() {
    let rig = TestRig::new();
    let dynamic_id = Uuid::new_v4();
    rig.source_object(
        "group",
        "front",
        serde_json::to_value(light_programmer::GroupDefinition {
            id: "front".into(),
            name: "Front".into(),
            ..Default::default()
        })
        .unwrap(),
    );
    rig.source_object(
        "preset",
        "1.1",
        json!({
            "name": "Dim",
            "family": "Intensity",
            "number": 1,
            "values": {},
            "group_values": {}
        }),
    );
    rig.source_object(
        "dynamic",
        &dynamic_id.to_string(),
        dynamic_with_dependencies(dynamic_id, "front", "1.1"),
    );
    let preview = rig.preview(rig.request("dynamic", &dynamic_id.to_string()));

    let result = rig.apply(&preview).unwrap();

    assert!(result.changed);
    let target = rig.target_document();
    assert!(target.object("group", "front").is_some());
    assert!(target.object("preset", "1.1").is_some());
    let dynamic = target
        .object("dynamic", &dynamic_id.to_string())
        .expect("Dynamic is imported with its dependencies");
    assert_eq!(
        dynamic.body().pointer("/target_binding/group_id"),
        Some(&json!("front"))
    );
    assert_eq!(
        dynamic
            .body()
            .pointer("/lanes/0/keyframes/points/0/source/preset_id"),
        Some(&json!("1.1"))
    );
    assert!(
        rig.ports
            .installed
            .lock()
            .as_ref()
            .is_some_and(|snapshot| snapshot.dynamics.len() == 1)
    );
}

#[test]
fn apply_rewrites_duplicate_identity_and_all_imported_references() {
    let rig = TestRig::new();
    rig.source_object("macro", "root", json!({"id":"root","macro_id":"child"}));
    rig.source_object(
        "macro",
        "child",
        json!({"id":"child","future_extension":{"kept":"yes"}}),
    );
    rig.target_object("macro", "child", json!({"id":"child","destination":true}));
    let request = rig
        .request("macro", "root")
        .resolve(key("macro", "child"), ImportConflictResolution::Duplicate);
    let preview = rig.preview(request);
    let duplicate = preview
        .objects
        .iter()
        .find(|object| object.source == key("macro", "child"))
        .unwrap()
        .destination
        .clone();

    rig.apply(&preview).unwrap();
    let target = rig.target_document();
    assert_eq!(
        target.object("macro", "root").unwrap().body()["macro_id"],
        duplicate.id()
    );
    let copied = target.object("macro", duplicate.id()).unwrap().body();
    assert_eq!(copied["id"], duplicate.id());
    assert_eq!(copied["future_extension"]["kept"], "yes");
    assert_eq!(
        target.object("macro", "child").unwrap().body()["destination"],
        true
    );
}

#[test]
fn importing_schedule_assigns_a_new_identity_and_resets_recurrence_anchors() {
    let rig = TestRig::new();
    let interval_id = uuid::Uuid::new_v4().to_string();
    let calendar_id = uuid::Uuid::new_v4().to_string();
    rig.source_object("macro", "doors", json!({"id":"doors"}));
    rig.source_object(
        "schedule",
        &interval_id,
        json!({
            "id":interval_id,
            "name":"Interval",
            "enabled":true,
            "trigger":{
                "type":"interval",
                "every_seconds":300,
                "enabled_at":"2000-01-01T00:00:00Z"
            },
            "target":{"type":"macro","macro_id":"doors"}
        }),
    );
    rig.source_object(
        "schedule",
        &calendar_id,
        json!({
            "id":calendar_id,
            "name":"Every two days",
            "enabled":true,
            "trigger":{
                "type":"calendar",
                "rule":{
                    "type":"every_n_days",
                    "every_days":2,
                    "anchor":"2000-01-01",
                    "at":"14:00:00"
                }
            },
            "target":{"type":"macro","macro_id":"doors"}
        }),
    );
    let request = SelectiveShowImportRequest::new(
        rig.source_id,
        rig.target_id,
        [key("schedule", &interval_id), key("schedule", &calendar_id)],
    );
    let preview = rig.preview(request);
    let imported = preview
        .objects
        .iter()
        .filter(|object| object.source.kind() == "schedule")
        .collect::<Vec<_>>();
    assert_eq!(imported.len(), 2);
    assert!(
        imported
            .iter()
            .all(|object| object.destination != object.source)
    );

    rig.apply(&preview).unwrap();
    let target = rig.target_document();
    let interval = target
        .object("schedule", imported[0].destination.id())
        .unwrap()
        .body();
    let calendar = target
        .object("schedule", imported[1].destination.id())
        .unwrap()
        .body();
    let (interval, calendar) = if interval["trigger"]["type"] == "interval" {
        (interval, calendar)
    } else {
        (calendar, interval)
    };
    assert_ne!(interval["trigger"]["enabled_at"], "2000-01-01T00:00:00Z");
    assert_eq!(
        calendar["trigger"]["rule"]["anchor"],
        chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string()
    );
}

#[test]
fn apply_copies_profile_snapshot_without_inferencing_unknown_asset_fields() {
    let rig = TestRig::new();
    let id = light_core::FixtureId::new();
    let snapshot = profile(
        id,
        4,
        json!({
            "managed_asset_id":"audio",
            "future_asset":{"bytes":"AAECAwQ="},
            "model_asset":"data:model/gltf-binary;base64,Z2xURg=="
        }),
    );
    rig.source_profile(&snapshot);
    rig.source_object(
        "managed_asset",
        "audio",
        json!({"id":"audio","checksum":"sha256:x"}),
    );
    rig.source_object(
        "effect",
        "one",
        json!({"id":"one","profile_id":id.0,"profile_revision":4}),
    );
    let preview = rig.preview(rig.request("effect", "one"));

    rig.apply(&preview).unwrap();

    let target = rig.target_document();
    let stored = target.fixture_profile_revision(id, 4).unwrap();
    assert_eq!(stored.digest(), snapshot.digest());
    assert_eq!(stored.profile()["future_asset"]["bytes"], "AAECAwQ=");
    assert!(target.object("managed_asset", "audio").is_none());
}

#[test]
fn identical_selection_is_a_true_noop() {
    let rig = TestRig::new();
    let body = json!({"id":"same","opaque":true});
    rig.source_object("macro", "same", body.clone());
    rig.target_object("macro", "same", body);
    let preview = rig.preview(rig.request("macro", "same"));
    let before = rig.target_document().revision();
    let event_before = rig.active_show.events().latest_sequence();
    rig.clear_steps();

    let result = rig.apply(&preview).unwrap();

    assert!(!result.changed);
    assert_eq!(result.event_sequence, None);
    assert_eq!(rig.active_show.events().latest_sequence(), event_before);
    assert_eq!(result.change.show_revision, before);
    assert_eq!(rig.steps(), vec!["source", "begin", "begin"]);
}

#[test]
fn unresolved_preview_cannot_partially_write() {
    let rig = TestRig::new();
    rig.source_object("macro", "root", json!({"id":"root","macro_id":"missing"}));
    let preview = rig.preview(rig.request("macro", "root"));
    let before = rig.target_document();
    rig.clear_steps();

    let error = rig.apply(&preview).unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(rig.target_document(), before);
    assert_eq!(rig.steps(), vec!["source", "begin"]);
}

#[test]
fn candidate_compile_failure_leaves_every_object_unwritten() {
    let rig = TestRig::new();
    rig.source_object("macro", "valid", json!({"id":"valid"}));
    rig.source_object("cue_list", "broken", json!({"id":"broken"}));
    let request = SelectiveShowImportRequest::new(
        rig.source_id,
        rig.target_id,
        [key("macro", "valid"), key("cue_list", "broken")],
    );
    let preview = rig.preview(request);
    let before = rig.target_document();
    rig.clear_steps();

    let error = rig.apply(&preview).unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(rig.target_document(), before);
    assert_eq!(rig.steps(), vec!["source", "begin"]);
}

#[test]
fn runtime_preparation_and_commit_failures_are_atomic() {
    let prepare = TestRig::new();
    prepare.source_object("macro", "one", json!({"id":"one"}));
    let preview = prepare.preview(prepare.request("macro", "one"));
    let before = prepare.target_document();
    prepare.ports.fail_prepare.store(true, Ordering::SeqCst);
    prepare.clear_steps();
    assert!(prepare.apply(&preview).is_err());
    assert_eq!(prepare.target_document(), before);
    assert_eq!(prepare.steps(), vec!["source", "begin", "begin", "prepare"]);

    let commit = TestRig::new();
    commit.source_object("macro", "one", json!({"id":"one"}));
    let preview = commit.preview(commit.request("macro", "one"));
    let before = commit.target_document();
    commit.ports.fail_commit.store(true, Ordering::SeqCst);
    commit.clear_steps();
    assert!(commit.apply(&preview).is_err());
    assert_eq!(commit.target_document(), before);
    assert_eq!(
        commit.steps(),
        vec!["source", "begin", "begin", "prepare", "backup", "commit"]
    );
    assert!(commit.ports.installed.lock().is_none());
    assert!(commit.ports.reconciled.lock().is_empty());
}

#[test]
fn source_or_target_changes_after_preview_are_rejected() {
    let source = TestRig::new();
    source.source_object("macro", "one", json!({"id":"one"}));
    let preview = source.preview(source.request("macro", "one"));
    source.source_object("macro", "later", json!({"id":"later"}));
    assert_eq!(
        source.apply(&preview).unwrap_err().kind,
        ActionErrorKind::Conflict
    );
    assert!(source.target_document().object("macro", "one").is_none());

    let target = TestRig::new();
    target.source_object("macro", "one", json!({"id":"one"}));
    let preview = target.preview(target.request("macro", "one"));
    target.target_object("macro", "later", json!({"id":"later"}));
    assert_eq!(
        target.apply(&preview).unwrap_err().kind,
        ActionErrorKind::Conflict
    );
    assert!(target.target_document().object("macro", "one").is_none());
}

#[test]
fn stage_layout_references_follow_duplicated_fixture_children() {
    let rig = TestRig::new();
    let source = portable_fixture_record(100_000, 1);
    let mut destination = portable_fixture_record(110_000, 2);
    destination.body["fixture_id"] = json!(source.fixture_id.0);
    rig.source_profile(&source.profile);
    rig.target_profile(&destination.profile);
    rig.source_object(
        "patched_fixture",
        &source.fixture_id.0.to_string(),
        source.body.clone(),
    );
    rig.target_object(
        "patched_fixture",
        &source.fixture_id.0.to_string(),
        destination.body,
    );
    let mut positions = serde_json::Map::new();
    positions.insert(
        source.head_id.0.to_string(),
        json!({"x":1,"y":2,"rotation":0}),
    );
    let mut positions3d = serde_json::Map::new();
    positions3d.insert(
        source.multipatch_id.to_string(),
        json!({"x":1,"y":2,"z":3,"rotation":{"x":0,"y":0,"z":0}}),
    );
    rig.source_object(
        "stage_layout",
        "main",
        json!({
            "version":2,
            "positions":positions,
            "positions3d":positions3d,
            "future_layout":{"retained":true}
        }),
    );
    let fixture_key = key("patched_fixture", &source.fixture_id.0.to_string());
    let request = rig
        .request("stage_layout", "main")
        .resolve(fixture_key.clone(), ImportConflictResolution::Duplicate);
    let preview = rig.preview(request);
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let duplicate = preview
        .objects
        .iter()
        .find(|object| object.source == fixture_key)
        .unwrap()
        .destination
        .clone();

    let result = rig.apply(&preview).unwrap();

    let target = rig.target_document();
    let fixture = target
        .object("patched_fixture", duplicate.id())
        .unwrap()
        .body();
    let duplicated_head = fixture["logical_heads"][0]["fixture_id"].as_str().unwrap();
    let duplicated_multipatch = fixture["multipatch"][0]["id"].as_str().unwrap();
    assert_ne!(duplicated_head, source.head_id.0.to_string());
    assert_ne!(duplicated_multipatch, source.multipatch_id.to_string());
    let layout = target.object("stage_layout", "main").unwrap().body();
    assert!(layout["positions"].get(duplicated_head).is_some());
    assert!(layout["positions3d"].get(duplicated_multipatch).is_some());
    assert!(
        layout["positions"]
            .get(source.head_id.0.to_string())
            .is_none()
    );
    assert_eq!(layout["future_layout"]["retained"], true);
    for committed in &result.change.objects {
        assert_eq!(
            target
                .object(committed.key.kind(), committed.key.id())
                .unwrap()
                .body(),
            &committed.body
        );
    }
}

#[test]
fn one_patch_layer_merges_only_its_fixtures_and_stage_geometry() {
    let rig = TestRig::new();
    let mut front = portable_fixture_record(130_000, 1);
    let mut rear = portable_fixture_record(140_000, 2);
    front.body["layer_id"] = json!("front");
    rear.body["layer_id"] = json!("rear");
    rig.source_profile(&front.profile);
    rig.source_profile(&rear.profile);
    rig.target_profile(&rear.profile);
    rig.source_object(
        "patch_layer",
        "front",
        json!({"id":"front","name":"Front","order":1}),
    );
    rig.source_object(
        "patch_layer",
        "rear",
        json!({"id":"rear","name":"Rear","order":2}),
    );
    rig.target_object(
        "patch_layer",
        "rear",
        json!({"id":"rear","name":"Rear Target","order":2}),
    );
    rig.source_object(
        "patched_fixture",
        &front.fixture_id.0.to_string(),
        front.body.clone(),
    );
    rig.source_object(
        "patched_fixture",
        &rear.fixture_id.0.to_string(),
        rear.body.clone(),
    );
    let mut target_rear = rear.body.clone();
    target_rear["name"] = json!("Rear Must Stay");
    target_rear["split_patches"] = json!([{"split":1,"universe":null,"address":null}]);
    target_rear["multipatch"][0]["split_patches"] =
        json!([{"split":1,"universe":null,"address":null}]);
    rig.target_object(
        "patched_fixture",
        &rear.fixture_id.0.to_string(),
        target_rear.clone(),
    );
    rig.source_object(
        "stage_layout",
        "main",
        json!({
            "version":2,
            "positions":{
                (front.head_id.0.to_string()):{"x":1,"y":2,"rotation":0},
                (rear.head_id.0.to_string()):{"x":30,"y":40,"rotation":0}
            },
            "positions3d":{}
        }),
    );
    rig.target_object(
        "stage_layout",
        "main",
        json!({
            "version":2,
            "positions":{
                (rear.head_id.0.to_string()):{"x":300,"y":400,"rotation":0}
            },
            "positions3d":{},
            "destination_extension":{"retained":true}
        }),
    );

    let preview = rig.preview(rig.request("patch_layer", "front"));
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert!(preview.objects.iter().any(|object| {
        object.source == key("stage_layout", "main")
            && object.action == ImportObjectAction::MergeScoped
    }));
    assert!(preview.objects.iter().any(|object| {
        object.source == key("patched_fixture", &front.fixture_id.0.to_string())
    }));
    assert!(
        !preview.objects.iter().any(|object| {
            object.source == key("patched_fixture", &rear.fixture_id.0.to_string())
        })
    );

    rig.apply(&preview).unwrap();

    let target = rig.target_document();
    assert_eq!(
        target
            .object("patched_fixture", &rear.fixture_id.0.to_string())
            .unwrap()
            .body(),
        &target_rear
    );
    assert!(
        target
            .object("patched_fixture", &front.fixture_id.0.to_string())
            .is_some()
    );
    let layout = target.object("stage_layout", "main").unwrap().body();
    assert_eq!(layout["positions"][front.head_id.0.to_string()]["x"], 1);
    assert_eq!(layout["positions"][rear.head_id.0.to_string()]["x"], 300);
    assert_eq!(layout["destination_extension"]["retained"], true);
}

#[test]
fn additive_patch_layer_rewrites_membership_and_appends_fixture_numbers() {
    let rig = TestRig::new();
    let mut source = portable_fixture_record(150_000, 2);
    let target = portable_fixture_record(160_000, 10);
    source.body["layer_id"] = json!("front");
    rig.source_profile(&source.profile);
    rig.target_profile(&target.profile);
    rig.source_object(
        "patch_layer",
        "front",
        json!({"id":"front","name":"Front","order":1}),
    );
    rig.target_object(
        "patch_layer",
        "front",
        json!({"id":"front","name":"Existing Front","order":1}),
    );
    rig.source_object(
        "patched_fixture",
        &source.fixture_id.0.to_string(),
        source.body,
    );
    rig.target_object(
        "patched_fixture",
        &target.fixture_id.0.to_string(),
        target.body,
    );

    let preview = rig.preview(
        rig.request("patch_layer", "front")
            .with_mode(ImportLoadMode::AddToEnd),
    );

    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let imported_layer = preview
        .objects
        .iter()
        .find(|object| object.source == key("patch_layer", "front"))
        .unwrap()
        .destination
        .clone();
    let imported_fixture = preview
        .objects
        .iter()
        .find(|object| object.source == key("patched_fixture", &source.fixture_id.0.to_string()))
        .unwrap()
        .destination
        .clone();
    rig.apply(&preview).unwrap();

    let target = rig.target_document();
    let fixture = target
        .object(imported_fixture.kind(), imported_fixture.id())
        .unwrap();
    assert_eq!(fixture.body()["layer_id"], imported_layer.id());
    assert_eq!(fixture.body()["fixture_number"], 11);
}

#[test]
fn legacy_inline_fixture_materializes_profile_and_retains_unknown_data() {
    let rig = TestRig::new();
    let legacy = legacy_fixture_record(120_000, 3);
    let fixture_id = legacy.fixture_id.0.to_string();
    rig.source_object("patched_fixture", &fixture_id, legacy.body);
    let preview = rig.preview(rig.request("patched_fixture", &fixture_id));
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert_eq!(preview.profiles[0].action, ImportProfileAction::Copy);

    let result = rig.apply(&preview).unwrap();

    let target = rig.target_document();
    assert!(
        target
            .fixture_profile_revision(
                legacy.profile.id().profile_id(),
                legacy.profile.id().revision(),
            )
            .is_some()
    );
    let stored = target
        .object("patched_fixture", &fixture_id)
        .unwrap()
        .body();
    assert_eq!(stored["future_fixture"]["retained"], true);
    let committed = result
        .change
        .objects
        .iter()
        .find(|change| change.key == key("patched_fixture", &fixture_id))
        .unwrap();
    assert_eq!(&committed.body, stored);
    assert_eq!(committed.object_revision, 1);
}

#[test]
fn duplicate_profile_rewrites_only_the_registered_reference() {
    let rig = TestRig::new();
    let source = portable_fixture_record(130_000, 4);
    let mut conflicting = source.profile.profile().clone();
    conflicting["future_profile"] = json!({"retained":false,"destination":true});
    let conflicting = light_show::FixtureProfileRevision::from_profile(conflicting).unwrap();
    rig.source_profile(&source.profile);
    rig.target_profile(&conflicting);
    let fixture_key = source.fixture_id.0.to_string();
    rig.source_object("patched_fixture", &fixture_key, source.body.clone());
    let source_profile = ImportProfileKey {
        profile_id: source.profile.id().profile_id(),
        revision: source.profile.id().revision(),
    };
    let request = rig
        .request("patched_fixture", &fixture_key)
        .resolve_profile(source_profile, ImportProfileConflictResolution::Duplicate);
    let preview = rig.preview(request);
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let destination = preview.profiles[0].destination;
    assert_ne!(destination.profile_id, source_profile.profile_id);

    let result = rig.apply(&preview).unwrap();

    let target = rig.target_document();
    let body = target
        .object("patched_fixture", &fixture_key)
        .unwrap()
        .body();
    assert_eq!(body["profile_id"], destination.profile_id.0.to_string());
    assert_eq!(body["future_fixture"]["profile_id"], "must-not-change");
    let duplicated = target
        .fixture_profile_revision(destination.profile_id, destination.revision)
        .unwrap();
    assert_eq!(duplicated.profile()["future_profile"]["retained"], true);
    assert_eq!(
        target
            .fixture_profile_revision(source_profile.profile_id, source_profile.revision)
            .unwrap()
            .digest(),
        conflicting.digest()
    );
    assert!(
        result.change.profiles.iter().any(|profile| {
            profile.source == source_profile && profile.destination == destination
        })
    );
}

#[test]
fn managed_asset_copy_is_exact_and_failures_compensate() {
    let success = TestRig::new();
    let asset = crate::AssetReference {
        id: crate::AssetId(Uuid::from_u128(140_000)),
        revision: crate::AssetRevision(2),
    };
    success.source_object(
        "audio_cue",
        "one",
        json!({"id":"one","asset_id":asset.id.0,"asset_revision":asset.revision.0}),
    );
    success.asset_action(asset, ImportManagedAssetAction::Copy);
    let preview = success.preview(success.request("audio_cue", "one"));
    success.clear_steps();
    let result = success.apply(&preview).unwrap();
    assert_eq!(result.change.managed_assets, vec![asset]);
    assert_eq!(
        success.asset_steps(),
        vec!["asset_prepare", "asset_publish"]
    );
    assert_eq!(
        success.steps(),
        vec![
            "source",
            "begin",
            "asset_prepare",
            "begin",
            "prepare",
            "backup",
            "commit",
            "install",
            "asset_publish",
            "reconcile",
        ]
    );

    let runtime = TestRig::new();
    runtime.source_object(
        "audio_cue",
        "one",
        json!({"id":"one","asset_id":asset.id.0,"asset_revision":asset.revision.0}),
    );
    runtime.asset_action(asset, ImportManagedAssetAction::Copy);
    let preview = runtime.preview(runtime.request("audio_cue", "one"));
    runtime.ports.fail_prepare.store(true, Ordering::SeqCst);
    assert!(runtime.apply(&preview).is_err());
    assert!(
        runtime
            .target_document()
            .object("audio_cue", "one")
            .is_none()
    );
    assert_eq!(
        runtime.asset_steps(),
        vec!["asset_prepare", "asset_compensate"]
    );

    let mismatched = TestRig::new();
    mismatched.source_object(
        "audio_cue",
        "one",
        json!({"id":"one","asset_id":asset.id.0,"asset_revision":asset.revision.0}),
    );
    mismatched.asset_action(asset, ImportManagedAssetAction::Copy);
    let preview = mismatched.preview(mismatched.request("audio_cue", "one"));
    mismatched
        .ports
        .mismatch_prepared_assets
        .store(true, Ordering::SeqCst);
    let error = mismatched.apply(&preview).unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert!(error.message.contains("different revision set"));
    assert!(
        mismatched
            .target_document()
            .object("audio_cue", "one")
            .is_none()
    );
    assert_eq!(
        mismatched.asset_steps(),
        vec!["asset_prepare", "asset_compensate"]
    );

    let persistence = TestRig::new();
    persistence.source_object(
        "audio_cue",
        "one",
        json!({"id":"one","asset_id":asset.id.0,"asset_revision":asset.revision.0}),
    );
    persistence.asset_action(asset, ImportManagedAssetAction::Copy);
    let preview = persistence.preview(persistence.request("audio_cue", "one"));
    persistence.ports.fail_commit.store(true, Ordering::SeqCst);
    assert!(persistence.apply(&preview).is_err());
    assert!(
        persistence
            .target_document()
            .object("audio_cue", "one")
            .is_none()
    );
    assert_eq!(
        persistence.asset_steps(),
        vec!["asset_prepare", "asset_compensate"]
    );

    let prepare = TestRig::new();
    prepare.source_object(
        "audio_cue",
        "one",
        json!({"id":"one","asset_id":asset.id.0,"asset_revision":asset.revision.0}),
    );
    prepare.asset_action(asset, ImportManagedAssetAction::Copy);
    let preview = prepare.preview(prepare.request("audio_cue", "one"));
    prepare
        .ports
        .fail_asset_prepare
        .store(true, Ordering::SeqCst);
    assert!(prepare.apply(&preview).is_err());
    assert_eq!(
        prepare.asset_steps(),
        vec!["asset_prepare", "asset_partial_cleanup"]
    );
}

#[test]
fn selective_and_ordinary_active_show_mutations_share_one_ordering_gate() {
    let rig = TestRig::new();
    rig.source_object("macro", "imported", json!({"id":"imported"}));
    let preview = rig.preview(rig.request("macro", "imported"));
    let before = rig.target_document().revision().value();
    rig.pause_next_runtime_prepare();

    std::thread::scope(|scope| {
        let import_rig = &rig;
        let import_preview = &preview;
        let import = scope.spawn(move || import_rig.apply(import_preview));
        rig.wait_for_runtime_prepare();
        let (ordinary_tx, ordinary_rx) = mpsc::channel();
        let ordinary_rig = &rig;
        scope.spawn(move || {
            ordinary_tx
                .send(
                    ordinary_rig.active_show.mutate_objects(
                        group_put(ordinary_rig, "after-import"),
                        &ordinary_rig.ports,
                    ),
                )
                .unwrap();
        });
        assert!(ordinary_rx.recv_timeout(Duration::from_millis(50)).is_err());
        rig.release_runtime_prepare();

        let imported = import.join().unwrap().unwrap();
        let ordinary = ordinary_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        assert_eq!(imported.change.show_revision.value(), before + 1);
        assert_eq!(ordinary.show_revision.value(), before + 2);
    });

    let target = rig.target_document();
    assert!(target.object("macro", "imported").is_some());
    assert!(target.object("group", "after-import").is_some());
}

#[test]
fn import_planning_does_not_hold_the_active_show_ordering_gate() {
    let rig = TestRig::new();
    rig.source_object(
        "audio_cue",
        "slow-descriptor",
        json!({
            "id": "slow-descriptor",
            "asset_id": Uuid::new_v4(),
            "asset_revision": 1
        }),
    );
    let before = rig.target_document().revision();
    rig.pause_next_import_descriptor();

    std::thread::scope(|scope| {
        let preview_rig = &rig;
        let preview = scope.spawn(move || {
            preview_rig.preview(preview_rig.request("audio_cue", "slow-descriptor"))
        });
        rig.wait_for_import_descriptor();

        let (ordinary_tx, ordinary_rx) = mpsc::channel();
        let ordinary_rig = &rig;
        scope.spawn(move || {
            ordinary_tx
                .send(ordinary_rig.active_show.mutate_objects(
                    group_put(ordinary_rig, "during-import-planning"),
                    &ordinary_rig.ports,
                ))
                .unwrap();
        });
        let ordinary = ordinary_rx.recv_timeout(Duration::from_secs(2));
        rig.release_import_descriptor();

        let ordinary = ordinary
            .expect("ordinary mutation must not wait for import adapter reads")
            .unwrap();
        assert_eq!(ordinary.show_revision.value(), before.value() + 1);
        assert_eq!(preview.join().unwrap().target_revision, before);
    });

    assert!(
        rig.target_document()
            .object("group", "during-import-planning")
            .is_some()
    );
}

#[test]
fn waiting_selective_import_rejects_a_target_changed_by_an_ordered_mutation() {
    let rig = TestRig::new();
    rig.source_object("macro", "imported", json!({"id":"imported"}));
    let preview = rig.preview(rig.request("macro", "imported"));
    rig.pause_next_runtime_prepare();

    std::thread::scope(|scope| {
        let ordinary_rig = &rig;
        let ordinary = scope.spawn(move || {
            ordinary_rig
                .active_show
                .mutate_objects(group_put(ordinary_rig, "first"), &ordinary_rig.ports)
        });
        rig.wait_for_runtime_prepare();
        let (import_tx, import_rx) = mpsc::channel();
        let import_rig = &rig;
        let import_preview = &preview;
        scope.spawn(move || import_tx.send(import_rig.apply(import_preview)).unwrap());
        assert!(import_rx.recv_timeout(Duration::from_millis(50)).is_err());
        rig.release_runtime_prepare();

        ordinary.join().unwrap().unwrap();
        let error = import_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap_err();
        assert_eq!(error.kind, ActionErrorKind::Conflict);
    });

    let target = rig.target_document();
    assert!(target.object("group", "first").is_some());
    assert!(target.object("macro", "imported").is_none());
}

#[test]
fn apply_uses_the_exact_immutable_source_snapshot_opened_for_the_preview_revision() {
    let rig = TestRig::new();
    rig.source_object(
        "macro",
        "snapshot",
        json!({"id":"snapshot","version":"previewed"}),
    );
    let preview = rig.preview(rig.request("macro", "snapshot"));
    rig.pause_next_source_snapshot();

    std::thread::scope(|scope| {
        let apply_rig = &rig;
        let apply_preview = &preview;
        let apply = scope.spawn(move || apply_rig.apply(apply_preview));
        rig.wait_for_source_snapshot();
        rig.update_source_object(
            "macro",
            "snapshot",
            json!({"id":"snapshot","version":"changed-after-open"}),
        );
        rig.release_source_snapshot();
        apply.join().unwrap().unwrap();
    });

    assert_eq!(
        rig.target_document()
            .object("macro", "snapshot")
            .unwrap()
            .body()["version"],
        "previewed"
    );
}

fn group_put(rig: &TestRig, id: &str) -> ActionEnvelope<MutateActiveShowObjectsCommand> {
    ActionEnvelope {
        context: context(),
        command: MutateActiveShowObjectsCommand {
            show_id: rig.target_id,
            mutations: vec![ActiveShowObjectMutation {
                kind: ActiveShowObjectKind::Group,
                object_id: id.into(),
                expected_object_revision: 0,
                mutation: ActiveShowObjectMutationKind::Put {
                    body: ActiveShowObjectBody::decode(
                        ActiveShowObjectKind::Group,
                        json!({"id":id,"name":"Group","fixtures":[]}),
                    )
                    .unwrap(),
                },
            }],
        },
    }
}
