use super::{apply_delta, merge_typed};
use serde_json::json;

#[test]
fn group_reference_extensions_follow_identity_through_insert_delete_and_reorder() {
    let before = json!({"references": [
        {"group_id": "front", "rule": {"type": "all"}},
        {"group_id": "side", "rule": {"type": "odd"}},
        {"group_id": "remove", "rule": {"type": "even"}}
    ]});
    let stored = json!({"references": [
        {
            "group_id": "front",
            "rule": {"type": "all", "future_rule": "front"},
            "future_reference": "front"
        },
        {
            "group_id": "side",
            "rule": {"type": "odd", "future_rule": "side"},
            "future_reference": "side"
        },
        {
            "group_id": "remove",
            "rule": {"type": "even", "future_rule": "remove"},
            "future_reference": "remove"
        }
    ]});
    let after = json!({"references": [
        {"group_id": "side", "rule": {"type": "even"}},
        {"group_id": "insert", "rule": {"type": "all"}},
        {"group_id": "front", "rule": {"type": "odd"}}
    ]});

    let merged = merge_typed(&stored, &before, &after).unwrap();

    assert_eq!(merged["references"][0]["future_reference"], "side");
    assert_eq!(merged["references"][0]["rule"]["future_rule"], "side");
    assert_eq!(merged["references"][0]["rule"]["type"], "even");
    assert_eq!(
        merged["references"][1],
        json!({"group_id": "insert", "rule": {"type": "all"}})
    );
    assert_eq!(merged["references"][2]["future_reference"], "front");
    assert_eq!(merged["references"][2]["rule"]["future_rule"], "front");
    assert_eq!(merged["references"][2]["rule"]["type"], "odd");
    assert!(
        merged["references"]
            .as_array()
            .unwrap()
            .iter()
            .all(|reference| reference["group_id"] != "remove")
    );
}

#[test]
fn repeated_group_references_keep_extensions_by_occurrence_order() {
    let before = json!({"references": [
        {"group_id": "repeat", "rule": {"type": "odd"}},
        {"group_id": "repeat", "rule": {"type": "even"}},
        {"group_id": "other", "rule": {"type": "all"}}
    ]});
    let stored = json!({"references": [
        {
            "group_id": "repeat",
            "rule": {"type": "odd"},
            "future_reference": "first"
        },
        {
            "group_id": "repeat",
            "rule": {"type": "even"},
            "future_reference": "second"
        },
        {
            "group_id": "other",
            "rule": {"type": "all"},
            "future_reference": "other"
        }
    ]});
    let after = json!({"references": [
        {"group_id": "other", "rule": {"type": "all"}},
        {"group_id": "repeat", "rule": {"type": "even"}},
        {"group_id": "repeat", "rule": {"type": "odd"}}
    ]});

    let merged = merge_typed(&stored, &before, &after).unwrap();

    assert_eq!(merged["references"][0]["future_reference"], "other");
    assert_eq!(merged["references"][1]["future_reference"], "first");
    assert_eq!(merged["references"][1]["rule"]["type"], "even");
    assert_eq!(merged["references"][2]["future_reference"], "second");
    assert_eq!(merged["references"][2]["rule"]["type"], "odd");
}

#[test]
fn established_id_and_group_attribute_identities_keep_precedence() {
    let before = json!({"items": [
        {"id": "a", "group_id": "same", "known": 1},
        {"id": "b", "group_id": "same", "known": 2}
    ]});
    let after = json!({"items": [
        {"id": "b", "group_id": "same", "known": 3},
        {"id": "a", "group_id": "same", "known": 4}
    ]});
    let mut stored = json!({"items": [
        {"id": "a", "group_id": "same", "known": 1, "future": "a"},
        {"id": "b", "group_id": "same", "known": 2, "future": "b"}
    ]});

    apply_delta(&mut stored, &before, &after);

    assert_eq!(stored["items"][0]["future"], "b");
    assert_eq!(stored["items"][1]["future"], "a");

    let before = json!({"items": [
        {"group_id": "same", "attribute": "intensity", "known": 1},
        {"group_id": "same", "attribute": "color", "known": 2}
    ]});
    let after = json!({"items": [
        {"group_id": "same", "attribute": "color", "known": 3},
        {"group_id": "same", "attribute": "intensity", "known": 4}
    ]});
    let mut stored = json!({"items": [
        {
            "group_id": "same",
            "attribute": "intensity",
            "known": 1,
            "future": "intensity"
        },
        {
            "group_id": "same",
            "attribute": "color",
            "known": 2,
            "future": "color"
        }
    ]});

    apply_delta(&mut stored, &before, &after);

    assert_eq!(stored["items"][0]["future"], "color");
    assert_eq!(stored["items"][1]["future"], "intensity");
}
