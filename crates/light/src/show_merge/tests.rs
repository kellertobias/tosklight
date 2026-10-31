//! The merge cases an offline-programming session produces (TL-858).

use super::{MergeChoice, MergeInputs, MergeObjects, MergeResolutions, merge_shows};
use light_show::PortableShowObjectKey;
use serde_json::{Value, json};

fn key(kind: &str, id: &str) -> PortableShowObjectKey {
    PortableShowObjectKey::new(kind, id)
}

fn objects(entries: &[(&str, &str, Value)]) -> MergeObjects {
    entries
        .iter()
        .map(|(kind, id, body)| (key(kind, id), body.clone()))
        .collect()
}

fn cue(id: &str, number: &str, changes: Value) -> Value {
    json!({"id": id, "number": number, "name": "", "changes": changes})
}

fn change(fixture: &str, level: f64) -> Value {
    json!({"fixture_id": fixture, "attribute": "intensity", "value": level})
}

fn list(cues: Vec<Value>) -> Value {
    json!({"id": "L", "name": "Main", "cues": cues})
}

fn merge(base: MergeObjects, mine: MergeObjects, theirs: MergeObjects) -> super::MergeOutcome {
    merge_shows(
        &MergeInputs { base, mine, theirs },
        &MergeResolutions::new(),
    )
}

#[test]
fn edits_to_different_objects_and_different_cues_merge_without_conflicts() {
    let base = objects(&[
        (
            "cue_list",
            "L",
            list(vec![cue("c1", "1", json!([])), cue("c2", "2", json!([]))]),
        ),
        (
            "group",
            "1",
            json!({"id": "1", "name": "Front", "fixtures": ["a"]}),
        ),
    ]);
    let mut mine = base.clone();
    mine.insert(
        key("cue_list", "L"),
        list(vec![
            cue("c1", "1", json!([change("a", 1.0)])),
            cue("c2", "2", json!([])),
        ]),
    );
    let mut theirs = base.clone();
    theirs.insert(
        key("cue_list", "L"),
        list(vec![
            cue("c1", "1", json!([])),
            cue("c2", "2", json!([change("b", 0.5)])),
        ]),
    );
    theirs.insert(
        key("group", "1"),
        json!({"id": "1", "name": "Front wash", "fixtures": ["a"]}),
    );
    let outcome = merge(base, mine, theirs);
    assert!(outcome.conflicts.is_empty(), "{:?}", outcome.conflicts);
    assert_eq!(outcome.auto_merged, 1);
    let merged = outcome.writes[&key("cue_list", "L")].as_ref().unwrap();
    assert_eq!(merged["cues"][0]["changes"], json!([change("a", 1.0)]));
    assert_eq!(merged["cues"][1]["changes"], json!([change("b", 0.5)]));
    assert_eq!(
        outcome.writes[&key("group", "1")].as_ref().unwrap()["name"],
        "Front wash"
    );
}

#[test]
fn different_fixtures_in_the_same_cue_merge_and_a_new_cue_keeps_its_place() {
    let base = objects(&[(
        "cue_list",
        "L",
        list(vec![cue("c1", "1", json!([change("a", 0.2)]))]),
    )]);
    let mine = objects(&[(
        "cue_list",
        "L",
        list(vec![cue(
            "c1",
            "1",
            json!([change("a", 0.2), change("b", 1.0)]),
        )]),
    )]);
    let theirs = objects(&[(
        "cue_list",
        "L",
        list(vec![
            cue("c1", "1", json!([change("a", 0.2), change("c", 0.7)])),
            cue("c3", "1.5", json!([])),
        ]),
    )]);
    let outcome = merge(base, mine, theirs);
    assert!(outcome.conflicts.is_empty());
    let merged = outcome.writes[&key("cue_list", "L")].as_ref().unwrap();
    assert_eq!(
        merged["cues"][0]["changes"],
        // Theirs' new change follows its predecessor in theirs.
        json!([change("a", 0.2), change("c", 0.7), change("b", 1.0)])
    );
    assert_eq!(merged["cues"][1]["id"], "c3");
}

#[test]
fn the_same_fixture_attribute_changed_on_both_sides_is_a_conflict_the_operator_decides() {
    let base = objects(&[(
        "cue_list",
        "L",
        list(vec![cue("c1", "3", json!([change("a", 0.2)]))]),
    )]);
    let mine = objects(&[(
        "cue_list",
        "L",
        list(vec![cue("c1", "3", json!([change("a", 0.5)]))]),
    )]);
    let theirs = objects(&[(
        "cue_list",
        "L",
        list(vec![cue("c1", "3", json!([change("a", 0.9)]))]),
    )]);
    let inputs = MergeInputs { base, mine, theirs };
    let outcome = merge_shows(&inputs, &MergeResolutions::new());
    assert_eq!(outcome.conflicts.len(), 1);
    let conflict = &outcome.conflicts[0];
    assert_eq!(
        conflict.path,
        ["cues", "Cue 3", "changes", "a|intensity", "value"]
    );
    assert_eq!(
        (conflict.mine.clone(), conflict.theirs.clone()),
        (Some(json!(0.5)), Some(json!(0.9)))
    );
    // Unresolved, the show keeps its value.
    assert!(outcome.writes.is_empty());

    let resolved = merge_shows(
        &inputs,
        &MergeResolutions::from([(conflict.id.clone(), MergeChoice::Theirs)]),
    );
    assert!(resolved.conflicts.is_empty());
    let merged = resolved.writes[&key("cue_list", "L")].as_ref().unwrap();
    assert_eq!(merged["cues"][0]["changes"][0]["value"], 0.9);
}

#[test]
fn the_same_group_and_preset_number_created_on_both_sides_are_both_kept_and_references_follow() {
    let base = objects(&[(
        "group",
        "6",
        json!({"id": "6", "name": "Back", "uid": "u6"}),
    )]);
    let mut mine = base.clone();
    mine.insert(
        key("group", "7"),
        json!({"id": "7", "name": "Mine", "uid": "um"}),
    );
    mine.insert(
        key("preset", "2.4"),
        json!({"family": "Color", "number": 4, "name": "Red", "instance_id": "pm"}),
    );
    let mut theirs = base.clone();
    theirs.insert(
        key("group", "7"),
        json!({"id": "7", "name": "Theirs", "uid": "ut"}),
    );
    theirs.insert(
        key("preset", "2.4"),
        json!({"family": "Color", "number": 4, "name": "Blue", "instance_id": "pt"}),
    );
    theirs.insert(
        key("cue_list", "L"),
        list(vec![cue("c1", "1", json!([]))])
            .as_object()
            .map(|body| {
                let mut body = body.clone();
                body.insert("cues".into(), json!([{"id": "c1", "number": "1", "group_changes": [{"group_id": "7", "attribute": "intensity", "value": 1.0}]}]));
                Value::Object(body)
            })
            .unwrap(),
    );
    theirs.insert(
        key("macro", "m"),
        json!({"name": "Look", "source": "Group 7 At Full"}),
    );
    let outcome = merge(base, mine, theirs);
    assert!(outcome.conflicts.is_empty(), "{:?}", outcome.conflicts);
    assert!(
        !outcome.writes.contains_key(&key("group", "7")),
        "the show keeps its Group 7"
    );
    assert_eq!(
        outcome.writes[&key("group", "8")].as_ref().unwrap()["name"],
        "Theirs"
    );
    assert_eq!(
        outcome.writes[&key("group", "8")].as_ref().unwrap()["id"],
        "8"
    );
    assert_eq!(
        outcome.writes[&key("preset", "2.5")].as_ref().unwrap()["number"],
        5
    );
    let cues = outcome.writes[&key("cue_list", "L")].as_ref().unwrap();
    assert_eq!(cues["cues"][0]["group_changes"][0]["group_id"], "8");
    let moved = outcome
        .renumbers
        .iter()
        .map(|renumber| (renumber.from.id().to_owned(), renumber.to.id().to_owned()))
        .collect::<Vec<_>>();
    assert_eq!(
        moved,
        [("7".into(), "8".into()), ("2.4".into(), "2.5".into())]
    );
    assert_eq!(outcome.macro_warnings.len(), 1);
    assert_eq!(outcome.macro_warnings[0].mentions, ["Group 7 → 8"]);
}

#[test]
fn deleting_an_object_the_other_side_changed_is_a_conflict() {
    let base = objects(&[("group", "1", json!({"id": "1", "name": "Front"}))]);
    let mine = objects(&[("group", "1", json!({"id": "1", "name": "Front wash"}))]);
    let theirs = objects(&[]);
    let outcome = merge(base.clone(), mine.clone(), theirs.clone());
    assert_eq!(outcome.conflicts.len(), 1);
    assert!(outcome.conflicts[0].path.is_empty());
    assert_eq!(outcome.conflicts[0].theirs, None);

    // A deletion of an object the show left alone simply applies.
    let outcome = merge(base.clone(), base, theirs);
    assert!(outcome.conflicts.is_empty());
    assert_eq!(outcome.writes[&key("group", "1")], None);
}

#[test]
fn patch_changes_on_both_sides_merge_per_field() {
    let fixture = |number: u64, address: u64, name: &str| json!({"fixture_id": "f1", "fixture_number": number, "name": name, "address": {"universe": 1, "address": address}});
    let base = objects(&[("patched_fixture", "f1", fixture(1, 1, "Spot"))]);
    let mine = objects(&[("patched_fixture", "f1", fixture(1, 101, "Spot"))]);
    let mut theirs = objects(&[("patched_fixture", "f1", fixture(1, 1, "Spot SL"))]);
    theirs.insert(
        key("patched_fixture", "f2"),
        json!({"fixture_id": "f2", "fixture_number": 2}),
    );
    let outcome = merge(base, mine, theirs);
    assert!(outcome.conflicts.is_empty());
    let merged = outcome.writes[&key("patched_fixture", "f1")]
        .as_ref()
        .unwrap();
    assert_eq!(merged, &fixture(1, 101, "Spot SL"));
    assert!(outcome.writes.contains_key(&key("patched_fixture", "f2")));
    let patch = outcome
        .summary
        .iter()
        .find(|kind| kind.kind == "patched_fixture")
        .unwrap();
    assert_eq!((patch.added, patch.changed), (1, 1));
}
