use super::*;
use crate::state::VersionedState;
use serde_json::json;

fn state(objects: &[(&str, &str, Value)]) -> SyncState {
    SyncState {
        objects: objects
            .iter()
            .map(|(kind, id, body)| (ObjectKey::new(*kind, *id), body.clone()))
            .collect(),
        metadata: BTreeMap::new(),
    }
}

fn no_profile(_: (Uuid, u64)) -> Option<Value> {
    None
}

fn capture(before: &SyncState, after: &SyncState, desk: &VersionedState) -> Option<ShowEditIntent> {
    ShowEditIntent::capture(before, after, desk, &no_profile)
}

#[test]
fn a_gesture_that_changes_nothing_persistent_is_not_an_intent() {
    // Live control — previews, DMX input, Highlight — never touches the show file, so the state
    // read after it is the state read before it, and nothing can be journaled.
    let rig = state(&[("patch_layer", "truss", json!({"name": "Truss", "order": 1}))]);
    assert_eq!(
        capture(&rig, &rig.clone(), &VersionedState::default()),
        None
    );
}

#[test]
fn one_gesture_over_several_objects_is_one_intent_with_field_edits() {
    let before = state(&[
        (
            "patch_layer",
            "truss",
            json!({"name": "Truss", "order": 1, "locked": false}),
        ),
        ("cad_annotation", "gone", json!({"text": "old"})),
    ]);
    let after = state(&[
        (
            "patch_layer",
            "truss",
            json!({"name": "Back Truss", "order": 1, "locked": false}),
        ),
        ("cad_annotation", "new", json!({"text": "hello"})),
    ]);
    let mut desk = VersionedState::default();
    desk.revisions
        .insert(ObjectKey::new("cad_annotation", "gone"), 7);
    let intent = capture(&before, &after, &desk).expect("an intent");
    assert_eq!(
        intent.operations(),
        &[
            PendingOperation::DeleteObject {
                key: ObjectKey::new("cad_annotation", "gone"),
                base_revision: Some(7),
            },
            PendingOperation::CreateObject {
                key: ObjectKey::new("cad_annotation", "new"),
                body: json!({"text": "hello"}),
            },
            PendingOperation::UpdateObject {
                key: ObjectKey::new("patch_layer", "truss"),
                fields: vec![ShowSyncFieldEdit {
                    path: "/name".into(),
                    base: Some(json!("Truss")),
                    value: Some(json!("Back Truss")),
                }],
            },
        ]
    );
}

#[test]
fn nested_fields_are_separate_and_arrays_travel_whole() {
    let mut edits = Vec::new();
    field_edits_between(
        &json!({"location": {"x": 1, "y": 2}, "tags": ["a"], "gone": 1, "a/b": 0}),
        &json!({"location": {"x": 5, "y": 2}, "tags": ["a", "b"], "a/b": 1}),
        &mut edits,
    );
    let paths: Vec<&str> = edits.iter().map(|edit| edit.path.as_str()).collect();
    assert_eq!(paths, ["/a~1b", "/gone", "/location/x", "/tags"]);
    assert_eq!(edits[1].value, None);
    assert_eq!(edits[3].value, Some(json!(["a", "b"])));
}

#[test]
fn metadata_changes_carry_their_base() {
    let mut before = SyncState::default();
    before
        .metadata
        .insert("previs.show_version".into(), "1".into());
    let mut after = SyncState::default();
    after
        .metadata
        .insert("previs.show_version".into(), "2".into());
    let intent = capture(&before, &after, &VersionedState::default()).unwrap();
    assert_eq!(
        intent.operations(),
        &[PendingOperation::SetMetadata {
            key: "previs.show_version".into(),
            base: Some("1".into()),
            value: Some("2".into()),
        }]
    );
}

#[test]
fn a_fixture_patched_with_a_profile_the_desk_lacks_carries_the_profile_first() {
    let profile_id = Uuid::new_v4();
    let fixture_id = Uuid::new_v4().to_string();
    let fixture =
        json!({"fixture_id": fixture_id, "profile_id": profile_id, "profile_revision": 3});
    let after = state(&[(PATCHED_FIXTURE_KIND, &fixture_id, fixture)]);
    let intent = ShowEditIntent::capture(
        &SyncState::default(),
        &after,
        &VersionedState::default(),
        &|key| (key == (profile_id, 3)).then(|| json!({"id": profile_id})),
    )
    .unwrap();
    assert!(matches!(
        &intent.operations()[0],
        PendingOperation::RetainProfile { profile_id: id, revision: 3, .. } if *id == profile_id
    ));
    assert!(matches!(
        &intent.operations()[1],
        PendingOperation::CreateObject { .. }
    ));
    let mut desk = VersionedState::default();
    desk.profiles.insert((profile_id, 3));
    let known = ShowEditIntent::capture(&SyncState::default(), &after, &desk, &|_| Some(json!({})))
        .unwrap();
    assert_eq!(
        known.operations().len(),
        1,
        "a profile the desk holds is not re-sent"
    );
}

#[test]
fn the_overlay_applies_pending_edits_but_keeps_fields_the_desk_kept() {
    let mut mirror = state(&[(
        "patch_layer",
        "truss",
        json!({"name": "Front Truss", "order": 1}),
    )]);
    let operations = vec![PendingOperation::UpdateObject {
        key: ObjectKey::new("patch_layer", "truss"),
        fields: vec![
            ShowSyncFieldEdit {
                path: "/name".into(),
                base: Some(json!("Truss")),
                value: Some(json!("Back Truss")),
            },
            ShowSyncFieldEdit {
                path: "/order".into(),
                base: Some(json!(1)),
                value: Some(json!(4)),
            },
        ],
    }];
    let mut held = HeldFields::new();
    held.entry(ObjectKey::new("patch_layer", "truss"))
        .or_default()
        .insert("/name".into());
    overlay(&mut mirror, &operations, &held);
    assert_eq!(
        mirror.objects[&ObjectKey::new("patch_layer", "truss")],
        json!({"name": "Front Truss", "order": 4})
    );
}

#[test]
fn set_field_creates_levels_and_removes_values() {
    let mut body = json!({});
    set_field(&mut body, "/location/x", Some(json!(3)));
    assert_eq!(body, json!({"location": {"x": 3}}));
    set_field(&mut body, "/location/x", None);
    assert_eq!(body, json!({"location": {}}));
    set_field(&mut body, "/missing/deep", None);
    assert_eq!(
        body,
        json!({"location": {}}),
        "removing an absent field is a no-op"
    );
}

#[test]
fn unnumbered_deletes_resolve_when_sent() {
    let operation = PendingOperation::DeleteObject {
        key: ObjectKey::new("cad_annotation", "fresh"),
        base_revision: None,
    };
    let wire = operation.to_wire(|_| Some(4)).unwrap();
    assert_eq!(
        wire,
        ShowSyncOperation::DeleteObject {
            kind: "cad_annotation".into(),
            id: "fresh".into(),
            base_object_revision: 4,
        }
    );
}

#[test]
fn a_bulk_gesture_is_split_into_transactions_the_desk_accepts() {
    let after = SyncState {
        objects: (0..2_500)
            .map(|index| {
                (
                    ObjectKey::new("cad_annotation", format!("note-{index:04}")),
                    json!({"text": index}),
                )
            })
            .collect(),
        metadata: BTreeMap::new(),
    };
    let transactions = capture(&SyncState::default(), &after, &VersionedState::default())
        .unwrap()
        .into_transactions();
    assert_eq!(
        transactions
            .iter()
            .map(|transaction| transaction.operations().len())
            .collect::<Vec<_>>(),
        [1_000, 1_000, 500]
    );
}
