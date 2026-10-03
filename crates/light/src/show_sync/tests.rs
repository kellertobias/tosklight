use super::fields::{FieldResolution, apply_field_edits, parse_pointer};
use super::{ShowSyncFieldEdit, is_sync_metadata_key, is_sync_object_kind};
use serde_json::{Value, json};

fn edit(path: &str, base: Option<Value>, value: Option<Value>) -> ShowSyncFieldEdit {
    ShowSyncFieldEdit {
        path: path.into(),
        base,
        value,
    }
}

#[test]
fn independent_fields_merge_and_a_changed_field_keeps_the_other_users_value() {
    // Control moved the note's colour; Architect, from the old base, edited its text and colour.
    let mut body = json!({"text": "Old", "colour": "red", "layer": "a"});
    let resolutions = apply_field_edits(
        &mut body,
        &[
            edit("/text", Some(json!("Old")), Some(json!("New"))),
            edit("/colour", Some(json!("blue")), Some(json!("green"))),
        ],
    )
    .unwrap();
    assert_eq!(
        resolutions,
        vec![
            FieldResolution::Applied,
            FieldResolution::Conflict {
                theirs: Some(json!("red"))
            }
        ]
    );
    assert_eq!(body, json!({"text": "New", "colour": "red", "layer": "a"}));
}

#[test]
fn a_retried_edit_finds_its_value_already_there() {
    let mut body = json!({"text": "New"});
    let resolutions = apply_field_edits(
        &mut body,
        &[edit("/text", Some(json!("Old")), Some(json!("New")))],
    )
    .unwrap();
    assert_eq!(resolutions, vec![FieldResolution::Unchanged]);
}

#[test]
fn null_and_absent_are_one_state_and_removal_deletes_the_field() {
    let mut body = json!({"note": null, "keep": 1});
    let resolutions = apply_field_edits(
        &mut body,
        &[
            edit("/note", None, Some(json!("hello"))),
            edit("/keep", Some(json!(1)), None),
        ],
    )
    .unwrap();
    assert_eq!(
        resolutions,
        vec![FieldResolution::Applied, FieldResolution::Applied]
    );
    assert_eq!(body, json!({"note": "hello"}));
}

#[test]
fn nested_fields_create_their_object_levels_and_address_array_elements() {
    let mut body = json!({"points": [{"x": 1}, {"x": 2}]});
    apply_field_edits(
        &mut body,
        &[
            edit("/transform/position/x", None, Some(json!(3.5))),
            edit("/points/1/x", Some(json!(2)), Some(json!(9))),
            edit("/a~1b", None, Some(json!(true))),
        ],
    )
    .unwrap();
    assert_eq!(
        body,
        json!({"points": [{"x": 1}, {"x": 9}], "transform": {"position": {"x": 3.5}}, "a/b": true})
    );
}

#[test]
fn malformed_pointers_and_out_of_range_indices_are_refused() {
    assert!(parse_pointer("text").is_err());
    let mut body = json!({"points": []});
    assert!(apply_field_edits(&mut body, &[edit("/points/4/x", None, Some(json!(1)))]).is_err());
}

#[test]
fn the_root_path_replaces_the_whole_body_under_the_same_compare() {
    let mut body = json!({"a": 1});
    let resolutions = apply_field_edits(
        &mut body,
        &[edit("", Some(json!({"a": 1})), Some(json!({"b": 2})))],
    )
    .unwrap();
    assert_eq!(resolutions, vec![FieldResolution::Applied]);
    assert_eq!(body, json!({"b": 2}));
}

#[test]
fn only_architect_owned_kinds_and_metadata_synchronize() {
    for kind in [
        "rig_attachment",
        "cad_annotation",
        "media_surface",
        "patch_layer",
    ] {
        assert!(is_sync_object_kind(kind), "{kind}");
    }
    for kind in [
        "cue_list",
        "playback",
        "preset",
        "route",
        "user_layout",
        "patched_fixture",
    ] {
        assert!(!is_sync_object_kind(kind), "{kind}");
    }
    assert!(is_sync_metadata_key("architect.venue"));
    assert!(is_sync_metadata_key("previs.lighting_designer"));
    assert!(!is_sync_metadata_key("name"));
    assert!(!is_sync_metadata_key("show_id"));
    assert!(!is_sync_metadata_key("architect."));
}

#[test]
fn the_sync_feed_reaches_only_subscriptions_that_opt_in() {
    use crate::show_sync::{ShowSyncGapChange, ShowSyncGapReason, ShowSyncPublication};
    use crate::{EventBus, EventDraft, EventFilter, EventReplay, EventTopic, SubscriptionOptions};
    let bus = EventBus::default();
    assert!(!bus.has_subscriber_for(EventTopic::ShowSync));
    let default = bus.subscribe(EventFilter::default(), SubscriptionOptions::default());
    let opted_in = bus.subscribe(
        EventFilter::default().with_topic(EventTopic::ShowSync),
        SubscriptionOptions::default(),
    );
    assert!(bus.has_subscriber_for(EventTopic::ShowSync));
    bus.publish(EventDraft::show_sync_published(ShowSyncPublication::Gap(
        ShowSyncGapChange {
            show_id: light_core::ShowId::new(),
            show_revision: 4,
            reason: ShowSyncGapReason::ShowReplaced,
        },
    )));
    assert!(
        default.try_next().is_none(),
        "the default stream is unchanged"
    );
    assert!(opted_in.try_next().is_some());
    let EventReplay::Events(replayed) = bus.replay(0, &EventFilter::default()) else {
        panic!("replayable")
    };
    assert!(replayed.is_empty());
    drop(opted_in);
    assert!(!bus.has_subscriber_for(EventTopic::ShowSync));
}
