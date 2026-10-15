//! Stable live Preset identities are copied with their linked Cue dependency closure.
use super::support::*;
use crate::selective_import::*;
use light_core::FixtureId;
use light_programmer::GroupDefinition;
use light_show::PortableShowObjectKey;
use serde_json::{Value, json};
use uuid::Uuid;

fn preset(instance: Uuid, number: u32) -> Value {
    json!({"instance_id":instance,"family":"Intensity","number":number,"name":"Source",
        "values":{},"group_values":{},"universal_values":{"intensity":{"kind":"normalized","value":0.7}}})
}

fn reference(instance: Uuid, owner: Value) -> Value {
    json!({"preset_instance_id":instance,"source_owner":owner,"source_attribute":"intensity"})
}

fn linked_list(id: Uuid, fixture: FixtureId, group: &str, instance: Uuid) -> Value {
    let body = json!({"id":id,"name":"Linked","priority":0,"mode":"sequence","looped":false,"chaser_xfade_percent":0,
        "cues":[{"id":Uuid::new_v4(),"number":"1","name":"Linked","fade_millis":0,
            "delay_millis":0,"trigger":{"type":"manual"},
            "changes":[{"fixture_id":fixture,"attribute":"intensity",
                "value":{"kind":"normalized","value":0.2},"automatic_restore":false,
                "preset_reference":reference(instance,json!({"type":"universal"}))}],
            "group_changes":[{"group_id":group,"attribute":"intensity",
                "value":{"kind":"normalized","value":0.2},"automatic_restore":false,
                "preset_reference":reference(instance,json!({"type":"universal"}))}]}]});
    let typed: light_playback::CueList = serde_json::from_value(body).unwrap();
    serde_json::to_value(typed).unwrap()
}

fn group(id: &str, fixture: FixtureId) -> Value {
    serde_json::to_value(GroupDefinition {
        id: id.into(),
        name: id.into(),
        fixtures: vec![fixture],
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn stable_preset_add_to_end_does_not_duplicate_destination_live_identity() {
    let rig = TestRig::new();
    let instance = Uuid::new_v4();
    let fixture = portable_fixture_record(810_001, 1);
    rig.source_object("preset", "1.1", preset(instance, 1));
    rig.target_profile(&fixture.profile);
    rig.target_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body,
    );
    rig.target_object("group", "front", group("front", fixture.fixture_id));
    rig.target_object("preset", "1.1", preset(instance, 1));
    let list_id = Uuid::new_v4();
    let original_list = linked_list(list_id, fixture.fixture_id, "front", instance);
    rig.target_object("cue_list", &list_id.to_string(), original_list.clone());
    let before = rig.target_document();
    let request = rig
        .request("preset", "1.1")
        .with_mode(ImportLoadMode::AddToEnd);
    let preview = rig.preview(request.clone());
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let again = rig.preview(request);
    assert_eq!(
        preview.objects, again.objects,
        "preview identities must be deterministic"
    );
    let destination = preview
        .objects
        .iter()
        .find(|o| o.source == key("preset", "1.1"))
        .unwrap()
        .destination
        .clone();
    let applied = rig.apply(&preview).unwrap();
    let after = rig.target_document();
    let copied = after.object("preset", destination.id()).unwrap().body();
    assert_ne!(copied["instance_id"], json!(instance));
    assert!(Uuid::parse_str(copied["instance_id"].as_str().unwrap()).is_ok());
    assert_eq!(
        copied["universal_values"],
        preset(instance, 1)["universal_values"]
    );
    assert_eq!(
        after.object("preset", "1.1").unwrap(),
        before.object("preset", "1.1").unwrap()
    );
    assert_eq!(
        after
            .object("cue_list", &list_id.to_string())
            .unwrap()
            .body(),
        before
            .object("cue_list", &list_id.to_string())
            .unwrap()
            .body()
    );
    let undo = applied.undo.unwrap();
    rig.service.undo(&context(), &undo, &rig.ports).unwrap();
    assert!(
        rig.target_document()
            .object("preset", destination.id())
            .is_none()
    );
    assert_eq!(
        rig.target_document()
            .object("preset", "1.1")
            .unwrap()
            .body(),
        before.object("preset", "1.1").unwrap().body()
    );
}

#[test]
fn stable_preset_linked_cue_copy_imports_and_rebinds_fixture_and_group_links() {
    let rig = TestRig::new();
    let instance = Uuid::new_v4();
    let fixture = portable_fixture_record(810_002, 1);
    rig.source_profile(&fixture.profile);
    rig.source_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body,
    );
    rig.source_object("group", "front", group("front", fixture.fixture_id));
    rig.source_object("preset", "1.1", preset(instance, 1));
    let id = Uuid::new_v4();
    let source_list = linked_list(id, fixture.fixture_id, "front", instance);
    rig.source_object("cue_list", &id.to_string(), source_list.clone());
    let preview = rig.preview(
        rig.request("cue_list", &id.to_string())
            .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let mapped = |kind: &str, source: &str| {
        preview
            .objects
            .iter()
            .find(|o| o.source == key(kind, source))
            .unwrap()
            .destination
            .id()
            .to_owned()
    };
    assert!(
        preview
            .objects
            .iter()
            .any(|object| object.source == key("preset", "1.1")),
        "linked Preset dependency must be included: {:?}",
        preview.dependencies
    );
    let new_preset = mapped("preset", "1.1");
    let new_list = mapped("cue_list", &id.to_string());
    let new_fixture = mapped("patched_fixture", &fixture.fixture_id.0.to_string());
    let new_group = mapped("group", "front");
    rig.apply(&preview).unwrap();
    let after = rig.target_document();
    let imported_preset = after.object("preset", &new_preset).unwrap().body();
    let imported_list = after.object("cue_list", &new_list).unwrap().body();
    assert_ne!(imported_preset["instance_id"], json!(instance));
    for pointer in [
        "/cues/0/changes/0/preset_reference",
        "/cues/0/group_changes/0/preset_reference",
    ] {
        assert_eq!(
            imported_list.pointer(pointer).unwrap()["preset_instance_id"],
            imported_preset["instance_id"]
        );
        assert_eq!(
            imported_list.pointer(pointer).unwrap()["source_owner"],
            json!({"type":"universal"})
        );
    }
    assert_eq!(
        imported_list["cues"][0]["changes"][0]["fixture_id"],
        new_fixture
    );
    assert_eq!(
        imported_list["cues"][0]["group_changes"][0]["group_id"],
        new_group
    );
    assert_eq!(
        rig.source_document()
            .object("cue_list", &id.to_string())
            .unwrap()
            .body(),
        &source_list
    );
}

#[test]
fn stable_preset_explicit_duplicate_allocates_instance_but_preserving_import_keeps_it() {
    for duplicate in [false, true] {
        let rig = TestRig::new();
        let instance = Uuid::new_v4();
        rig.source_object("preset", "1.1", preset(instance, 1));
        let mut request = rig.request("preset", "1.1");
        if duplicate {
            request = request.resolve(key("preset", "1.1"), ImportConflictResolution::Duplicate);
        }
        let preview = rig.preview(request);
        assert!(preview.can_apply(), "{:?}", preview.blockers);
        let destination = &preview.objects[0].destination;
        rig.apply(&preview).unwrap();
        let after = rig.target_document();
        let actual = &after.object("preset", destination.id()).unwrap().body()["instance_id"];
        assert_eq!(actual == &json!(instance), !duplicate);
    }
}

fn key(kind: &str, id: &str) -> PortableShowObjectKey {
    PortableShowObjectKey::new(kind, id)
}

fn scoped_preset(instance: Uuid, fixture: FixtureId) -> Value {
    let mut body = preset(instance, 1);
    body["values"] = json!({fixture.0.to_string():{"intensity":{"kind":"normalized","value":0.7}}});
    body["group_values"] = json!({"front":{"intensity":{"kind":"normalized","value":0.7}}});
    body
}

fn scoped_list(id: Uuid, fixture: FixtureId, instance: Uuid) -> Value {
    let mut body = linked_list(id, fixture, "front", instance);
    body["cues"][0]["changes"][0]["preset_reference"]["source_owner"] =
        json!({"type":"fixture","fixture_id":fixture});
    body["cues"][0]["group_changes"][0]["preset_reference"]["source_owner"] =
        json!({"type":"group","group_id":"front"});
    body["cues"][0]["group_changes"][0]["preset_reference"]["member_fixture"] = json!(fixture);
    body
}

#[test]
fn copied_preset_live_owner_metadata_follows_imported_fixture_group_and_member() {
    let rig = TestRig::new();
    let fixture = portable_fixture_record(810_003, 1);
    let instance = Uuid::new_v4();
    let id = Uuid::new_v4();
    rig.source_profile(&fixture.profile);
    rig.source_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body,
    );
    rig.source_object("group", "front", group("front", fixture.fixture_id));
    rig.source_object("preset", "1.1", scoped_preset(instance, fixture.fixture_id));
    rig.source_object(
        "cue_list",
        &id.to_string(),
        scoped_list(id, fixture.fixture_id, instance),
    );
    let preview = rig.preview(
        rig.request("cue_list", &id.to_string())
            .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let destination = |kind: &str, source: &str| {
        preview
            .objects
            .iter()
            .find(|o| o.source == key(kind, source))
            .unwrap()
            .destination
            .id()
            .to_owned()
    };
    let fixture_id = destination("patched_fixture", &fixture.fixture_id.0.to_string());
    let group_id = destination("group", "front");
    let list_id = destination("cue_list", &id.to_string());
    rig.apply(&preview).unwrap();
    let after = rig.target_document();
    let cue = &after.object("cue_list", &list_id).unwrap().body()["cues"][0];
    assert_eq!(
        cue["changes"][0]["preset_reference"]["source_owner"]["fixture_id"],
        fixture_id
    );
    assert_eq!(
        cue["group_changes"][0]["preset_reference"]["source_owner"]["group_id"],
        group_id
    );
    assert_eq!(
        cue["group_changes"][0]["preset_reference"]["member_fixture"],
        fixture_id
    );
}

#[test]
fn kept_or_destination_only_preset_retains_its_owners_when_cue_targets_are_copied() {
    for external in [false, true] {
        let rig = TestRig::new();
        let fixture = portable_fixture_record(810_004, 1);
        let source_instance = Uuid::new_v4();
        let target_instance = if external {
            source_instance
        } else {
            Uuid::new_v4()
        };
        let id = Uuid::new_v4();
        rig.source_profile(&fixture.profile);
        rig.target_profile(&fixture.profile);
        rig.source_object(
            "patched_fixture",
            &fixture.fixture_id.0.to_string(),
            fixture.body.clone(),
        );
        rig.target_object(
            "patched_fixture",
            &fixture.fixture_id.0.to_string(),
            fixture.body,
        );
        rig.source_object("group", "front", group("front", fixture.fixture_id));
        rig.target_object("group", "front", group("front", fixture.fixture_id));
        if !external {
            rig.source_object(
                "preset",
                "1.1",
                scoped_preset(source_instance, fixture.fixture_id),
            );
        }
        let retained = scoped_preset(target_instance, fixture.fixture_id);
        rig.target_object("preset", "1.1", retained.clone());
        rig.source_object(
            "cue_list",
            &id.to_string(),
            scoped_list(id, fixture.fixture_id, source_instance),
        );
        let mut request = rig
            .request("cue_list", &id.to_string())
            .with_mode(ImportLoadMode::AddToEnd);
        if !external {
            request = request.resolve(
                key("preset", "1.1"),
                ImportConflictResolution::KeepDestination,
            );
        }
        let preview = rig.preview(request);
        assert!(preview.can_apply(), "{:?}", preview.blockers);
        let destination = preview
            .objects
            .iter()
            .find(|o| o.source == key("cue_list", &id.to_string()))
            .unwrap()
            .destination
            .id()
            .to_owned();
        rig.apply(&preview).unwrap();
        let after = rig.target_document();
        assert_eq!(after.object("preset", "1.1").unwrap().body(), &retained);
        let cue = &after.object("cue_list", &destination).unwrap().body()["cues"][0];
        assert_ne!(cue["changes"][0]["fixture_id"], json!(fixture.fixture_id));
        assert_eq!(
            cue["changes"][0]["preset_reference"]["preset_instance_id"],
            json!(target_instance)
        );
        assert_eq!(
            cue["changes"][0]["preset_reference"]["source_owner"]["fixture_id"],
            json!(fixture.fixture_id)
        );
        assert_eq!(
            cue["group_changes"][0]["preset_reference"]["source_owner"]["group_id"],
            "front"
        );
        assert_eq!(
            cue["group_changes"][0]["preset_reference"]["member_fixture"],
            json!(fixture.fixture_id)
        );
    }
}

#[test]
fn deleted_preset_reference_keeps_exact_identity_owner_and_recorded_fallback() {
    let rig = TestRig::new();
    let fixture = portable_fixture_record(810_005, 1);
    let instance = Uuid::new_v4();
    let id = Uuid::new_v4();
    rig.source_profile(&fixture.profile);
    rig.source_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body,
    );
    rig.source_object("group", "front", group("front", fixture.fixture_id));
    let original = scoped_list(id, fixture.fixture_id, instance);
    rig.source_object("cue_list", &id.to_string(), original.clone());
    let preview = rig.preview(
        rig.request("cue_list", &id.to_string())
            .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert!(
        !preview
            .objects
            .iter()
            .any(|object| object.source.kind() == "preset")
    );
    let destination = preview
        .objects
        .iter()
        .find(|object| object.source == key("cue_list", &id.to_string()))
        .unwrap()
        .destination
        .id()
        .to_owned();
    rig.apply(&preview).unwrap();
    let after = rig.target_document();
    let copied = &after.object("cue_list", &destination).unwrap().body()["cues"][0];
    for scope in ["changes", "group_changes"] {
        assert_eq!(
            copied[scope][0]["preset_reference"],
            original["cues"][0][scope][0]["preset_reference"]
        );
        assert_eq!(
            copied[scope][0]["value"],
            original["cues"][0][scope][0]["value"]
        );
    }
}

#[test]
fn legacy_preset_import_uses_existing_one_time_identity_migration() {
    for null in [false, true] {
        let rig = TestRig::new();
        let mut body = preset(Uuid::new_v4(), 1);
        if null {
            body["instance_id"] = Value::Null;
        } else {
            body.as_object_mut().unwrap().remove("instance_id");
        }
        rig.source_object("preset", "1.1", body);
        let source_before = rig.source_document();
        let preview = rig.preview(
            rig.request("preset", "1.1")
                .with_mode(ImportLoadMode::AddToEnd),
        );
        assert!(preview.can_apply(), "{:?}", preview.blockers);
        let destination = preview.objects[0].destination.id().to_owned();
        rig.apply(&preview).unwrap();
        let reopened = rig.target_document();
        let identity =
            reopened.object("preset", &destination).unwrap().body()["instance_id"].clone();
        let id = Uuid::parse_str(
            identity
                .as_str()
                .expect("existing candidate migration assigns identity"),
        )
        .unwrap();
        assert!(!id.is_nil());
        assert_eq!(
            rig.target_document()
                .object("preset", &destination)
                .unwrap()
                .body()["instance_id"],
            identity,
            "a second portable read must retain the once-seeded identity"
        );
        assert_eq!(
            rig.source_document(),
            source_before,
            "reading/importing may not migrate the source file in place"
        );
    }
}

#[test]
fn ambiguous_source_preset_identity_blocks_atomically_without_guessing_an_owner() {
    let rig = TestRig::new();
    let fixture = portable_fixture_record(810_006, 1);
    let instance = Uuid::new_v4();
    let id = Uuid::new_v4();
    rig.source_profile(&fixture.profile);
    rig.source_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body,
    );
    rig.source_object("group", "front", group("front", fixture.fixture_id));
    rig.source_object("preset", "1.1", preset(instance, 1));
    rig.source_object("preset", "1.2", preset(instance, 2));
    rig.source_object(
        "cue_list",
        &id.to_string(),
        linked_list(id, fixture.fixture_id, "front", instance),
    );
    let before = rig.target_document();
    let preview = rig.preview(
        rig.request("cue_list", &id.to_string())
            .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(!preview.can_apply());
    assert!(format!("{:?}", preview.blockers).contains("ambiguous Preset instance"));
    assert!(rig.apply(&preview).is_err());
    assert_eq!(rig.target_document(), before);
}

#[test]
fn replaced_preset_keeps_destination_instance_and_existing_live_cue_links() {
    let rig = TestRig::new();
    let fixture = portable_fixture_record(810_007, 1);
    let source_instance = Uuid::new_v4();
    let target_instance = Uuid::new_v4();
    let source_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    rig.source_profile(&fixture.profile);
    rig.target_profile(&fixture.profile);
    rig.source_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body.clone(),
    );
    rig.target_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body,
    );
    rig.source_object("group", "front", group("front", fixture.fixture_id));
    rig.target_object("group", "front", group("front", fixture.fixture_id));
    rig.source_object(
        "preset",
        "1.1",
        scoped_preset(source_instance, fixture.fixture_id),
    );
    let mut original_preset = scoped_preset(target_instance, fixture.fixture_id);
    original_preset["name"] = json!("Original");
    rig.target_object("preset", "1.1", original_preset.clone());
    rig.source_object(
        "cue_list",
        &source_id.to_string(),
        scoped_list(source_id, fixture.fixture_id, source_instance),
    );
    let original_cue = scoped_list(target_id, fixture.fixture_id, target_instance);
    rig.target_object("cue_list", &target_id.to_string(), original_cue.clone());
    let before = rig.target_document();
    let request = rig
        .request("cue_list", &source_id.to_string())
        .with_mode(ImportLoadMode::AddToEnd)
        .resolve(
            key("preset", "1.1"),
            ImportConflictResolution::ReplaceDestination,
        );
    let preview = rig.preview(request.clone());
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let destination = preview
        .objects
        .iter()
        .find(|o| o.source == key("cue_list", &source_id.to_string()))
        .unwrap()
        .destination
        .id()
        .to_owned();
    let applied = rig.apply(&preview).unwrap();
    let after = rig.target_document();
    assert_eq!(
        after.object("preset", "1.1").unwrap().body()["instance_id"],
        json!(target_instance)
    );
    assert_eq!(
        after
            .object("cue_list", &target_id.to_string())
            .unwrap()
            .body(),
        before
            .object("cue_list", &target_id.to_string())
            .unwrap()
            .body()
    );
    assert_eq!(
        after.object("cue_list", &destination).unwrap().body()["cues"][0]["changes"][0]["preset_reference"]
            ["preset_instance_id"],
        json!(target_instance)
    );
    // Reopen the SQLite reader, Undo the one import transaction, then replay as a fresh
    // import; the service has no separate Redo entry point. Native desk Undo/Redo is separate.
    rig.service
        .undo(&context(), &applied.undo.unwrap(), &rig.ports)
        .unwrap();
    assert_eq!(
        rig.target_document()
            .object("preset", "1.1")
            .unwrap()
            .body(),
        &original_preset
    );
    let replay = rig.preview(request);
    assert!(replay.can_apply(), "{:?}", replay.blockers);
    rig.apply(&replay).unwrap();
    assert_eq!(
        rig.target_document()
            .object("preset", "1.1")
            .unwrap()
            .body()["instance_id"],
        json!(target_instance)
    );
}

#[test]
fn external_preset_instance_does_not_bind_same_number_different_source_preset() {
    let rig = TestRig::new();
    let fixture = portable_fixture_record(810_008, 1);
    let source_instance = Uuid::new_v4();
    let external_instance = Uuid::new_v4();
    let id = Uuid::new_v4();
    rig.source_profile(&fixture.profile);
    rig.target_profile(&fixture.profile);
    for target in [false, true] {
        let body = fixture.body.clone();
        if target {
            rig.target_object("patched_fixture", &fixture.fixture_id.0.to_string(), body);
            rig.target_object("group", "front", group("front", fixture.fixture_id));
        } else {
            rig.source_object("patched_fixture", &fixture.fixture_id.0.to_string(), body);
            rig.source_object("group", "front", group("front", fixture.fixture_id));
        }
    }
    rig.source_object(
        "preset",
        "1.1",
        scoped_preset(source_instance, fixture.fixture_id),
    );
    let original = scoped_preset(external_instance, fixture.fixture_id);
    rig.target_object("preset", "1.1", original.clone());
    rig.source_object(
        "cue_list",
        &id.to_string(),
        scoped_list(id, fixture.fixture_id, external_instance),
    );
    let preview = rig.preview(
        rig.request("cue_list", &id.to_string())
            .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert!(
        !preview
            .objects
            .iter()
            .any(|object| object.source.kind() == "preset"),
        "external stable instance must not traverse a different source Preset at the same pool number"
    );
    let destination = preview
        .objects
        .iter()
        .find(|object| object.source == key("cue_list", &id.to_string()))
        .unwrap()
        .destination
        .id()
        .to_owned();
    rig.apply(&preview).unwrap();
    let after = rig.target_document();
    assert_eq!(after.object("preset", "1.1").unwrap().body(), &original);
    let cue = &after.object("cue_list", &destination).unwrap().body()["cues"][0];
    for scope in ["changes", "group_changes"] {
        assert_eq!(
            cue[scope][0]["preset_reference"]["preset_instance_id"],
            json!(external_instance)
        );
    }
    assert_eq!(
        cue["changes"][0]["preset_reference"]["source_owner"]["fixture_id"],
        json!(fixture.fixture_id)
    );
    assert_eq!(
        cue["group_changes"][0]["preset_reference"]["source_owner"]["group_id"],
        "front"
    );
}

#[test]
fn legacy_source_replacement_retains_modern_destination_instance_and_live_links() {
    for null in [false, true] {
        let rig = TestRig::new();
        let fixture = portable_fixture_record(810_009, 1);
        let retained_instance = Uuid::new_v4();
        let list_id = Uuid::new_v4();
        rig.target_profile(&fixture.profile);
        rig.target_object(
            "patched_fixture",
            &fixture.fixture_id.0.to_string(),
            fixture.body,
        );
        rig.target_object("group", "front", group("front", fixture.fixture_id));
        let original_preset = preset(retained_instance, 1);
        rig.target_object("preset", "1.1", original_preset.clone());
        rig.target_object(
            "cue_list",
            &list_id.to_string(),
            linked_list(list_id, fixture.fixture_id, "front", retained_instance),
        );
        let before = rig.target_document();
        let mut legacy = preset(Uuid::new_v4(), 1);
        if null {
            legacy["instance_id"] = Value::Null;
        } else {
            legacy.as_object_mut().unwrap().remove("instance_id");
        }
        legacy["name"] = json!("Legacy replacement");
        rig.source_object("preset", "1.1", legacy);
        let source_before = rig.source_document();
        let preview = rig.preview(rig.request("preset", "1.1").resolve(
            key("preset", "1.1"),
            ImportConflictResolution::ReplaceDestination,
        ));
        assert!(preview.can_apply(), "{:?}", preview.blockers);
        let applied = rig.apply(&preview).unwrap();
        let after = rig.target_document();
        assert_eq!(
            after.object("preset", "1.1").unwrap().body()["instance_id"],
            json!(retained_instance),
            "a legacy source with no identity must not recreate the modern destination Preset"
        );
        assert_eq!(
            after.object("preset", "1.1").unwrap().body()["name"],
            "Legacy replacement"
        );
        assert_eq!(
            after
                .object("cue_list", &list_id.to_string())
                .unwrap()
                .body(),
            before
                .object("cue_list", &list_id.to_string())
                .unwrap()
                .body()
        );
        assert_eq!(
            rig.source_document(),
            source_before,
            "source legacy file is immutable"
        );
        rig.service
            .undo(&context(), &applied.undo.unwrap(), &rig.ports)
            .unwrap();
        assert_eq!(
            rig.target_document()
                .object("preset", "1.1")
                .unwrap()
                .body(),
            &original_preset
        );
    }
}
