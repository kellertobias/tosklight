use super::*;
use crate::{ProgrammingGroupRecordOperation, ProgrammingGroupRecordRequest};
use light_core::{AttributeKey, AttributeValue, SessionId, UserId};
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
fn minimal_legacy_body_keeps_omitted_defaults_and_extensions_losslessly() {
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
    let prepared = prepare_recording(&document.document, &commit).unwrap();
    let PreparedActiveShowTransaction::NoChange(state) = prepared else {
        let result = prepared_result(prepared);
        panic!(
            "unchanged legacy omissions must not be materialized: {:?}",
            result.projection.raw_body
        )
    };

    assert_eq!(state.result.projection.raw_body.as_deref(), Some(&body));
    assert_eq!(state.result.projection.object_id, "legacy minimal");
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
        master: 0.35,
        playback_fader: Some(4),
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
    assert!((body["master"].as_f64().unwrap() - 0.35).abs() < 0.000_001);
    assert_eq!(body["playback_fader"], 4);
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
    assert!(error.message.contains("dependent"));

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
    serde_json::to_value(GroupDefinition {
        id: id.into(),
        name: name.into(),
        fixtures: fixtures.into_iter().collect(),
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
    registry.start(session, UserId::new());
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
