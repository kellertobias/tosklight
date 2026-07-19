use super::{apply_delta, merge_typed, merge_typed_request};
use serde_json::json;

#[test]
fn typed_delta_retains_unknown_fields_on_changed_keyed_items() {
    let before = json!({"items":[{"id":"a","known":1}]});
    let after = json!({"items":[{"id":"a","known":2}]});
    let mut stored = json!({"items":[{"id":"a","known":1,"future":true}]});

    apply_delta(&mut stored, &before, &after);

    assert_eq!(
        stored,
        json!({"items":[{"id":"a","known":2,"future":true}]})
    );
}

#[test]
fn keyed_items_are_matched_by_identity_instead_of_array_position() {
    let before = json!({"items":[{"id":"a","known":1},{"id":"b","known":2}]});
    let after = json!({"items":[{"id":"a","known":3},{"id":"b","known":4}]});
    let mut stored = json!({"items":[
        {"id":"b","known":2,"future":"b"},
        {"id":"a","known":1,"future":"a"}
    ]});

    apply_delta(&mut stored, &before, &after);

    assert_eq!(
        stored,
        json!({"items":[
            {"id":"a","known":3,"future":"a"},
            {"id":"b","known":4,"future":"b"}
        ]})
    );
}

#[test]
fn duplicate_identities_are_matched_by_stable_occurrence() {
    let before = json!({"items":[
        {"id":"duplicate","known":1},
        {"id":"duplicate","known":2}
    ]});
    let stored = json!({"items":[
        {"id":"duplicate","known":1,"future":"first"},
        {"id":"duplicate","known":2,"future":"second"}
    ]});
    let after = json!({"items":[
        {"id":"duplicate","known":1},
        {"id":"new","known":3},
        {"id":"duplicate","known":2}
    ]});

    let merged = merge_typed_request(Some(&stored), Some(&before), &after, &after, &after).unwrap();

    assert_eq!(merged["items"][0]["future"], "first");
    assert_eq!(merged["items"][1], json!({"id":"new","known":3}));
    assert_eq!(merged["items"][2]["future"], "second");
}

#[test]
fn request_extensions_merge_inside_keyed_items() {
    let stored_typed = json!({"items":[{"id":"a","known":1}]});
    let stored = json!({"items":[{"id":"a","known":1,"server_future":true}]});
    let request_typed = json!({"items":[{"id":"a","known":2}]});
    let request = json!({"items":[{
        "id":"a",
        "known":2,
        "client_future":{"accepted":true}
    }]});

    let merged = merge_typed_request(
        Some(&stored),
        Some(&stored_typed),
        &request,
        &request_typed,
        &request_typed,
    )
    .unwrap();

    assert_eq!(
        merged,
        json!({"items":[{
            "id":"a",
            "known":2,
            "server_future":true,
            "client_future":{"accepted":true}
        }]})
    );
}

#[test]
fn composite_address_keeps_extensions_on_nested_cue_changes() {
    let before = json!({"items":[{
        "group_id":"front",
        "attribute":"intensity",
        "known":1
    }]});
    let after = json!({"items":[{
        "group_id":"front",
        "attribute":"intensity",
        "known":2
    }]});
    let mut stored = json!({"items":[{
        "group_id":"front",
        "attribute":"intensity",
        "known":1,
        "future_curve":"soft"
    }]});

    apply_delta(&mut stored, &before, &after);

    assert_eq!(stored["items"][0]["known"], 2);
    assert_eq!(stored["items"][0]["future_curve"], "soft");
}

#[test]
fn public_typed_merge_removes_known_map_entries_without_losing_siblings() {
    let before = json!({"values":{"remove":1,"keep":2}});
    let after = json!({"values":{"keep":3}});
    let stored = json!({"values":{"remove":{"future":true},"keep":2},"future":true});

    let merged = merge_typed(&stored, &before, &after).unwrap();

    assert_eq!(merged, json!({"values":{"keep":3},"future":true}));
}

#[test]
fn public_typed_merge_materializes_missing_defaults_and_normalizes_known_values() {
    let before = json!({"known":"legacy-alias","default":[]});
    let after = json!({"known":"canonical","default":[]});
    let stored = json!({"known":"legacy-alias","future":true});

    let merged = merge_typed(&stored, &before, &after).unwrap();

    assert_eq!(
        merged,
        json!({"known":"canonical","default":[],"future":true})
    );
}
