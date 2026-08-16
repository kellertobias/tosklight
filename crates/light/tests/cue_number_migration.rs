use light_application::show_compiler::prepare_show_candidate;
use light_core::CueListId;
use light_playback::{Cue, CueList, CueListMode, CueNumber, IntensityPriorityMode, RestartMode};
use light_show::ShowStore;
use serde_json::{Value, json};

fn number(value: &str) -> CueNumber {
    value.parse().unwrap()
}

fn cue_list(numbers: &[&str]) -> CueList {
    CueList {
        id: CueListId::new(),
        name: "Migration".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: None,
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues: numbers
            .iter()
            .map(|value| Cue::new(number(value)))
            .collect(),
    }
}

fn store_with(body: &Value) -> (ShowStore, light_show::PortableShowDocument) {
    let (store, _) = ShowStore::create(":memory:", "Cue migration").unwrap();
    store.put_object("cue_list", "main", body, 0).unwrap();
    let document = store.portable_document().unwrap();
    (store, document)
}

#[test]
fn rewrites_legacy_numbers_once_without_changing_ids_or_extensions() {
    let mut body = serde_json::to_value(cue_list(&["2", "3"])).unwrap();
    let ids = body["cues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|cue| cue["id"].clone())
        .collect::<Vec<_>>();
    body["cues"][0]["number"] = json!(2.0);
    body["cues"][1]["number"] = json!(2.1);
    body["cues"][0]["future"] = json!({"kept": true});
    let (store, document) = store_with(&body);

    let prepared = prepare_show_candidate(&document, document.transaction()).unwrap();
    let (transaction, _) = prepared.into_parts();
    store.apply_portable_transaction(transaction).unwrap();
    let migrated = store.objects("cue_list").unwrap().remove(0).body;
    assert_eq!(migrated["cues"][0]["number"], "2");
    assert_eq!(migrated["cues"][1]["number"], "2.1");
    assert_eq!(migrated["cues"][0]["id"], ids[0]);
    assert_eq!(migrated["cues"][1]["id"], ids[1]);
    assert_eq!(migrated["cues"][0]["future"], json!({"kept": true}));

    let document = store.portable_document().unwrap();
    let prepared = prepare_show_candidate(&document, document.transaction()).unwrap();
    let (second, _) = prepared.into_parts();
    assert!(second.is_empty(), "Cue migration must be idempotent");
}

#[test]
fn blocks_collisions_and_legacy_order_changes_with_manual_renumber_guidance() {
    for (raw, expected) in [
        ([2.05, 2.5], "both normalize to 2.5"),
        ([2.11, 2.2], "legacy Cue order would change"),
    ] {
        let mut body = serde_json::to_value(cue_list(&["2", "3"])).unwrap();
        body["cues"][0]["number"] = json!(raw[0]);
        body["cues"][1]["number"] = json!(raw[1]);
        let (_store, document) = store_with(&body);
        let error = prepare_show_candidate(&document, document.transaction())
            .err()
            .expect("ambiguous legacy numbering must stop migration");
        assert!(error.message.contains(expected), "{}", error.message);
        assert!(
            error.message.contains("manually renumber"),
            "{}",
            error.message
        );
    }
}

#[test]
fn portable_show_round_trip_preserves_distinct_and_deep_paths() {
    let body =
        serde_json::to_value(cue_list(&["2", "2.0", "2.1", "2.1.0", "2.1.1", "2.2", "3"])).unwrap();
    let (store, document) = store_with(&body);

    let prepared = prepare_show_candidate(&document, document.transaction()).unwrap();
    let (transaction, _) = prepared.into_parts();
    store.apply_portable_transaction(transaction).unwrap();

    let reopened = store.portable_document().unwrap();
    let object = reopened
        .objects()
        .find(|object| object.key().kind() == "cue_list")
        .unwrap();
    let numbers = object.body()["cues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|cue| cue["number"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(numbers, ["2", "2.0", "2.1", "2.1.0", "2.1.1", "2.2", "3"]);
}
