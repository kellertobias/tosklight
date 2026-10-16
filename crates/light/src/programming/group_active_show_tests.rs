use super::*;
use crate::{ProgrammingGroupRecordOperation, ProgrammingGroupRecordRequest};
use light_core::{AttributeKey, AttributeValue, SessionId};
use light_programmer::{DerivedGroup, ProgrammerRegistry, SelectionExpression, SelectionRule};
use light_show::ShowStore;
use serde_json::{Value, json};
use std::path::PathBuf;
use uuid::Uuid;

#[test]
fn absent_empty_overwrite_creates_an_intentionally_stored_group_with_opaque_id() {
    let document = TestDocument::new([]);
    let commit = recording(
        &document,
        "look A.01 / house",
        [],
        ProgrammingGroupRecordOperation::Overwrite,
    );
    let result = prepared_result(prepare_recording(&document.document, &commit).unwrap());

    assert!(result.changed);
    assert_eq!(result.projection.object_id, "look A.01 / house");
    assert_eq!(result.projection.object_revision, 1);
    let body = result.projection.raw_body.as_deref().unwrap();
    assert_eq!(body["id"], "look A.01 / house");
    assert_eq!(body["name"], "Group look A.01 / house");
    assert_eq!(body["fixtures"], json!([]));
    assert_eq!(body["programming"], json!({}));
}

#[test]
fn identical_overwrite_is_a_no_change_and_preserves_unknown_fields() {
    let fixture = light_core::FixtureId::new();
    let mut body = group_body("opaque", "Kept", [fixture]);
    body["future_extension"] = json!({"retain":[1, 2, 3]});
    let document = TestDocument::new([("opaque", body)]);
    let commit = recording(
        &document,
        "opaque",
        [fixture],
        ProgrammingGroupRecordOperation::Overwrite,
    );
    let prepared = prepare_recording(&document.document, &commit).unwrap();
    let PreparedActiveShowTransaction::NoChange(state) = prepared else {
        panic!("an identical Group body must not prepare a commit")
    };

    assert!(!state.result.changed);
    assert_eq!(state.result.event_sequence, None);
    assert_eq!(state.result.projection.object_revision, 1);
    assert_eq!(
        state.result.projection.raw_body.as_deref().unwrap()["future_extension"],
        json!({"retain":[1, 2, 3]})
    );
}

#[test]
fn minimal_legacy_body_writes_canonical_source_and_keeps_extensions_losslessly() {
    let fixture = light_core::FixtureId::new();
    let body = json!({
        "name": "Legacy minimal",
        "fixtures": [fixture],
        "future_extension": {"retain": true}
    });
    let document = TestDocument::new([("legacy minimal", body.clone())]);
    let commit = recording(
        &document,
        "legacy minimal",
        [fixture],
        ProgrammingGroupRecordOperation::Overwrite,
    );
    let result = prepared_result(prepare_recording(&document.document, &commit).unwrap());
    assert!(result.changed);
    let stored = result.projection.raw_body.as_deref().unwrap();
    assert_eq!(stored["name"], body["name"]);
    assert_eq!(stored["fixtures"], body["fixtures"]);
    assert_eq!(stored["future_extension"], body["future_extension"]);
    assert_eq!(
        stored["source"],
        json!({"type":"explicit","fixture_ids":[fixture]})
    );
    assert_eq!(result.projection.object_id, "legacy minimal");
}

#[test]
fn changed_legacy_body_updates_only_semantic_fields_without_materializing_defaults() {
    let before = light_core::FixtureId::new();
    let after = light_core::FixtureId::new();
    let body = json!({
        "name": "Legacy minimal",
        "fixtures": [before],
        "future_extension": {"retain": true}
    });
    let document = TestDocument::new([("legacy", body)]);
    let commit = recording(
        &document,
        "legacy",
        [after],
        ProgrammingGroupRecordOperation::Overwrite,
    );
    let result = prepared_result(prepare_recording(&document.document, &commit).unwrap());
    let body = result.projection.raw_body.as_deref().unwrap();

    assert_eq!(
        body,
        &json!({
            "name": "Legacy minimal",
            "fixtures": [after],
            "source": {"type":"explicit","fixture_ids":[after]},
            "future_extension": {"retain": true}
        })
    );
}

#[test]
fn present_legacy_body_identity_is_repaired_to_the_authoritative_object_key() {
    let fixture = light_core::FixtureId::new();
    let document = TestDocument::new([(
        "authoritative",
        json!({"id":"wrong","name":"Legacy","fixtures":[fixture]}),
    )]);
    let commit = recording(
        &document,
        "authoritative",
        [fixture],
        ProgrammingGroupRecordOperation::Overwrite,
    );
    let result = prepared_result(prepare_recording(&document.document, &commit).unwrap());
    assert_eq!(
        result.projection.raw_body.as_deref().unwrap()["id"],
        "authoritative"
    );
}

#[test]
fn merge_materializes_derived_membership_and_preserves_metadata_and_extensions() {
    let fixtures = (0..3)
        .map(|_| light_core::FixtureId::new())
        .collect::<Vec<_>>();
    let source = group_body("source", "Source", fixtures[..2].iter().copied());
    let mut derived = serde_json::to_value(GroupDefinition {
        id: "derived".into(),
        name: "Presentation".into(),
        color: Some("#112233".into()),
        icon: Some("wash".into()),
        derived_from: Some(DerivedGroup {
            source_group_id: "source".into(),
            rule: SelectionRule::All,
        }),
        programming: HashMap::from([(
            AttributeKey("portable.future".into()),
            AttributeValue::Normalized(0.25),
        )]),
        ..Default::default()
    })
    .unwrap();
    derived["future_extension"] = json!({"keep":true});
    let document = TestDocument::new([("source", source), ("derived", derived)]);
    let commit = recording(
        &document,
        "derived",
        [fixtures[1], fixtures[2]],
        ProgrammingGroupRecordOperation::Merge,
    );

    let result = prepared_result(prepare_recording(&document.document, &commit).unwrap());
    let body = result.projection.raw_body.as_deref().unwrap();
    assert_eq!(body["fixtures"], json!(fixtures));
    assert_eq!(body["derived_from"], Value::Null);
    assert_eq!(body["name"], "Presentation");
    assert_eq!(body["color"], "#112233");
    assert_eq!(body["icon"], "wash");
    assert!(body.get("master").is_none());
    assert!(body.get("playback_fader").is_none());
    assert_eq!(body["future_extension"], json!({"keep":true}));
    assert_eq!(body["programming"]["portable.future"]["value"], 0.25);
}

#[test]
fn empty_subtract_deletes_but_a_direct_live_dependency_blocks_it() {
    let source = group_body("source", "Source", []);
    let dependent = serde_json::to_value(GroupDefinition {
        id: "dependent".into(),
        derived_from: Some(DerivedGroup {
            source_group_id: "source".into(),
            rule: SelectionRule::All,
        }),
        ..Default::default()
    })
    .unwrap();
    let blocked = TestDocument::new([("source", source.clone()), ("dependent", dependent)]);
    let commit = recording(
        &blocked,
        "source",
        [],
        ProgrammingGroupRecordOperation::Subtract,
    );
    let error = recording_error(prepare_recording(&blocked.document, &commit));
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(
        error.message,
        "Group source cannot be deleted: derived Group dependent depends on it. Retarget that Group first."
    );
    let unchanged = ShowStore::open(&blocked.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(unchanged.revision(), blocked.document.revision());
    for object in blocked.document.objects() {
        let retained = unchanged
            .object(object.key().kind(), object.key().id())
            .unwrap();
        assert_eq!(retained.revision(), object.revision());
        assert_eq!(retained.body(), object.body());
    }

    let allowed = TestDocument::new([("source", source)]);
    let commit = recording(
        &allowed,
        "source",
        [],
        ProgrammingGroupRecordOperation::Subtract,
    );
    let result = prepared_result(prepare_recording(&allowed.document, &commit).unwrap());
    assert!(result.changed);
    assert!(result.projection.deleted);
    assert_eq!(result.projection.raw_body, None);
    assert_eq!(result.projection.object_revision, 2);
}

#[test]
fn exact_object_and_show_revisions_are_validated_before_mutation() {
    let document = TestDocument::new([("target", group_body("target", "Target", []))]);
    let mut stale = recording(
        &document,
        "target",
        [],
        ProgrammingGroupRecordOperation::Overwrite,
    );
    stale.expected_object_revision = ProgrammingGroupRevisionExpectation::Exact(0);
    let error = recording_error(prepare_recording(&document.document, &stale));
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(1));

    let mut stale_show = recording(
        &document,
        "target",
        [],
        ProgrammingGroupRecordOperation::Overwrite,
    );
    stale_show.expected_show_revision = Some(PortableShowRevision::from_value(0));
    let error = recording_error(prepare_recording(&document.document, &stale_show));
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(
        error.current_related_revision,
        Some(document.document.revision().value())
    );
}

#[test]
fn an_unrelated_show_edit_does_not_conflict_with_a_current_group() {
    // A stale show revision alongside an exact object guard is the false conflict this path used
    // to raise: the Group named here has not moved, only something else in the show has.
    let document = TestDocument::new([("target", group_body("target", "Target", []))]);
    let mut commit = recording(
        &document,
        "target",
        [],
        ProgrammingGroupRecordOperation::Overwrite,
    );
    commit.expected_object_revision = ProgrammingGroupRevisionExpectation::Exact(1);
    commit.expected_show_revision = Some(PortableShowRevision::from_value(0));

    let result = prepare_recording(&document.document, &commit);

    assert!(
        result.is_ok(),
        "an unrelated show edit must not refuse a write guarded by its own object revision"
    );
}

fn recording_error(
    result: Result<PreparedActiveShowTransaction<PreparedRecording>, ActionError>,
) -> ActionError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("recording unexpectedly succeeded"),
    }
}

fn prepared_result(
    prepared: PreparedActiveShowTransaction<PreparedRecording>,
) -> ProgrammingGroupCommitResult {
    match prepared {
        PreparedActiveShowTransaction::NoChange(state)
        | PreparedActiveShowTransaction::PreparedCommit { state, .. } => state.result,
    }
}

fn group_body(
    id: &str,
    name: &str,
    fixtures: impl IntoIterator<Item = light_core::FixtureId>,
) -> Value {
    let fixtures = fixtures.into_iter().collect::<Vec<_>>();
    serde_json::to_value(GroupDefinition {
        id: id.into(),
        name: name.into(),
        fixtures: fixtures.clone(),
        source: Some(light_programmer::GroupFixtureSource::Explicit {
            fixture_ids: fixtures,
        }),
        ..Default::default()
    })
    .unwrap()
}

fn recording(
    document: &TestDocument,
    group_id: &str,
    fixtures: impl IntoIterator<Item = light_core::FixtureId>,
    operation: ProgrammingGroupRecordOperation,
) -> ProgrammingGroupCommit {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    registry.select_expression(
        session,
        fixtures.into_iter().collect(),
        SelectionExpression::Static,
    );
    let capture = registry.capture_group_recording_selection(session).unwrap();
    ProgrammingGroupCommit::new(
        &ProgrammingGroupRecordRequest {
            show_id: document.document.id(),
            group_id: group_id.into(),
            operation,
            expected_object_revision: ProgrammingGroupRevisionExpectation::Current,
            expected_show_revision: None,
        },
        capture,
        session,
        false,
    )
}

struct TestDocument {
    path: PathBuf,
    document: PortableShowDocument,
}

impl TestDocument {
    fn new<const N: usize>(objects: [(&str, Value); N]) -> Self {
        let path =
            std::env::temp_dir().join(format!("light-programming-group-{}.sqlite", Uuid::new_v4()));
        let (store, _) = ShowStore::create(&path, "Group test").unwrap();
        for (id, body) in objects {
            store.put_object("group", id, &body, 0).unwrap();
        }
        let document = store.portable_document().unwrap();
        drop(store);
        Self { path, document }
    }
}

impl Drop for TestDocument {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.path.display(), suffix));
        }
    }
}

#[test]
fn delete_group_with_stored_dynamic_dependency_is_actionable_and_deterministic() {
    for reversed in [false, true] {
        let mut document = TestDocument::new([("source", group_body("source", "Source", []))]);
        let store = ShowStore::open(&document.path).unwrap();
        let mut dependencies = [(Uuid::from_u128(7), 7), (Uuid::from_u128(3), 3)];
        if reversed {
            dependencies.reverse();
        }
        for (id, pool) in dependencies {
            let mut body = stored_dependency_dynamic(id, pool);
            body["future_operator_content"] = json!({"retain":true});
            store
                .put_object("dynamic", &id.to_string(), &body, 0)
                .unwrap();
        }
        document.document = store.portable_document().unwrap();
        let before = document.document.clone();
        let commit = recording(
            &document,
            "source",
            [],
            ProgrammingGroupRecordOperation::Subtract,
        );
        let error = recording_error(prepare_recording(&document.document, &commit));
        assert_eq!(error.kind, ActionErrorKind::Conflict);
        assert_eq!(
            error.message,
            "Group source cannot be deleted: Dynamic 3 depends on it. Retarget that Dynamic first."
        );
        assert_eq!(
            store.portable_document().unwrap().revision(),
            before.revision()
        );
        for object in before.objects() {
            let current = document
                .document
                .object(object.key().kind(), object.key().id())
                .unwrap();
            assert_eq!(current.revision(), object.revision());
            assert_eq!(current.body(), object.body());
        }
    }
}

#[test]
fn unrelated_or_frozen_dynamic_targets_do_not_block_group_deletion() {
    let mut document = TestDocument::new([("source", group_body("source", "Source", []))]);
    let store = ShowStore::open(&document.path).unwrap();
    for (id, binding) in [
        (
            "frozen",
            json!({"type":"frozen_targets", "targets":[Uuid::from_u128(22)]}),
        ),
        (
            "unrelated",
            json!({"type":"live_group", "group_id":"other"}),
        ),
        ("targetless", json!({"type":"targetless"})),
    ] {
        let mut body = stored_dependency_dynamic(Uuid::from_u128(21), 1);
        body["target_binding"] = binding;
        let definition: light_dynamics::DynamicDefinition =
            serde_json::from_value(body.clone()).unwrap();
        light_dynamics::validate_definition(&definition).unwrap();
        store.put_object("dynamic", id, &body, 0).unwrap();
    }
    document.document = store.portable_document().unwrap();
    let commit = recording(
        &document,
        "source",
        [],
        ProgrammingGroupRecordOperation::Subtract,
    );
    let result = prepared_result(prepare_recording(&document.document, &commit).unwrap());
    assert!(result.projection.deleted);
}

fn stored_dependency_dynamic(id: Uuid, pool: u16) -> Value {
    let body = json!({
        "id":id,"pool_number":pool,"revision":1,"name":"Stopped unassigned dependency",
        "target_binding":{"type":"live_group","group_id":"source"},
        "lanes":[{"id":Uuid::from_u128(102),"attribute":"intensity","mode":"keyframes",
            "keyframes":{"points":[
                {"position":0.0,"source":{"type":"value","value":0.25},"interpolation":"linear"},
                {"position":0.5,"source":{"type":"value","value":0.75},"interpolation":"linear"}],"size":1.0},
            "max_min":{"minimum":{"type":"value","value":0.25},"maximum":{"type":"value","value":0.75},"function":"sinus","size":1.0},
            "middle_amplitude":{"middle":{"type":"current"},"amplitude":0.25,"function":"sinus","size":1.0},
            "speed_multiplier":{"numerator":1,"denominator":1},"width":1.0,"random_group_id":null}],
        "phase":{"ordering":{"type":"selection"},"offset_degrees":0.0,"span_degrees":360.0,
            "block_size":1,"repeats":1,"wings":false,"anchors_degrees":[]},
        "speed":{"type":"fixed","duration_millis":1000},"default_activation":"start_now"
    });
    let definition: light_dynamics::DynamicDefinition =
        serde_json::from_value(body.clone()).unwrap();
    light_dynamics::validate_definition(&definition).unwrap();
    body
}

#[test]
fn repairable_invalid_dynamics_do_not_gain_runtime_group_dependencies() {
    let mut document = TestDocument::new([("source", group_body("source", "Source", []))]);
    let store = ShowStore::open(&document.path).unwrap();
    let mut invalid = stored_dependency_dynamic(Uuid::from_u128(8), 8);
    invalid["lanes"] = json!([]);
    assert!(
        light_dynamics::validate_definition(
            &serde_json::from_value::<light_dynamics::DynamicDefinition>(invalid.clone()).unwrap()
        )
        .is_err()
    );
    let malformed = json!({"target_binding":{"type":"live_group","group_id":"source"}});
    for (id, body) in [("invalid", &invalid), ("malformed", &malformed)] {
        store.put_object("dynamic", id, body, 0).unwrap();
    }
    document.document = store.portable_document().unwrap();
    let commit = recording(
        &document,
        "source",
        [],
        ProgrammingGroupRecordOperation::Subtract,
    );
    let result = prepared_result(prepare_recording(&document.document, &commit).unwrap());
    assert!(result.projection.deleted);
    let unchanged = store.portable_document().unwrap();
    assert_eq!(unchanged.revision(), document.document.revision());
    for (id, body) in [("invalid", invalid), ("malformed", malformed)] {
        assert_eq!(unchanged.object("dynamic", id).unwrap().body(), &body);
    }
}
