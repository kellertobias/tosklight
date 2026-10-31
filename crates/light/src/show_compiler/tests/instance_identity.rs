//! Instance identities for operator-numbered objects (TL-858): a Group or Preset keeps one UUID
//! for its whole life, so a merge can tell "Group 7 changed" from "two copies made a Group 7".

use super::super::{stage_candidate_migrations, stage_instance_identities};
use super::support::document_with_objects;
use serde_json::{Value, json};
use uuid::Uuid;

fn group(name: &str) -> Value {
    json!({"id":"7","name":name,"fixtures":[],"future":{"kept":true}})
}

fn uid(body: &Value) -> Uuid {
    Uuid::parse_str(body["uid"].as_str().expect("uid is stamped")).unwrap()
}

#[test]
fn a_new_group_gets_an_identity_when_it_is_written() {
    let (_, document) = document_with_objects(&[]);
    let mut transaction = document.transaction();
    transaction.put("group", "7", group("Front"));
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();
    let body = candidate.object("group", "7").unwrap().body();
    uid(body);
    assert_eq!(body["future"], json!({"kept": true}));
}

#[test]
fn rewriting_a_group_without_its_identity_keeps_the_stored_one() {
    let identity = Uuid::new_v4();
    let mut stored = group("Front");
    stored["uid"] = json!(identity);
    let (_, document) = document_with_objects(&[("group", "7", stored)]);
    let mut transaction = document.transaction();
    // A typed writer rebuilds the body and drops the field it does not know.
    transaction.put("group", "7", group("Front wash"));
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();
    let body = candidate.object("group", "7").unwrap().body();
    assert_eq!(uid(body), identity);
    assert_eq!(body["name"], "Front wash");
}

#[test]
fn a_copy_that_reuses_another_objects_identity_gets_its_own() {
    let identity = Uuid::new_v4();
    let mut stored = group("Front");
    stored["uid"] = json!(identity);
    let (_, document) = document_with_objects(&[("group", "7", stored.clone())]);
    let mut transaction = document.transaction();
    transaction.put("group", "8", stored);
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();
    assert_eq!(
        uid(candidate.object("group", "7").unwrap().body()),
        identity
    );
    assert_ne!(
        uid(candidate.object("group", "8").unwrap().body()),
        identity
    );
}

#[test]
fn an_unchanged_legacy_show_is_not_rewritten_until_it_is_forked() {
    let preset_identity = Uuid::new_v4();
    let (_, document) = document_with_objects(&[
        ("group", "7", group("Front")),
        ("group", "8", group("Back")),
        (
            "preset",
            "2.1",
            json!({"family":"Color","number":1,"name":"Red","instance_id":preset_identity}),
        ),
    ]);
    let mut opened = document.transaction();
    stage_candidate_migrations(&document, &mut opened).unwrap();
    let candidate = document.candidate(&opened).unwrap();
    assert!(
        candidate
            .object("group", "7")
            .unwrap()
            .body()
            .get("uid")
            .is_none()
    );

    let mut fork = document.transaction();
    stage_instance_identities(&document, &mut fork).unwrap();
    let candidate = document.candidate(&fork).unwrap();
    let front = uid(candidate.object("group", "7").unwrap().body());
    let back = uid(candidate.object("group", "8").unwrap().body());
    assert_ne!(front, back);
    assert!(
        fork.object_writes().all(|(key, _)| key.kind() == "group"),
        "a Preset that already has an identity is left alone"
    );
    assert_eq!(
        candidate.object("preset", "2.1").unwrap().body()["instance_id"],
        json!(preset_identity)
    );
}
