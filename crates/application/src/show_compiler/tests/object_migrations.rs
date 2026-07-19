use super::super::{compile_show_candidate, stage_candidate_migrations};
use super::support::{document_with_objects, snapshot_without_revision, stored_body};
use light_core::CueListId;
use light_output::{DeliveryMode, OutputRoute, Protocol};
use light_playback::{Cue, CueList, CueListMode, IntensityPriorityMode, RestartMode, WrapMode};
use serde_json::json;
use uuid::Uuid;

#[test]
fn defaults_are_raw_preserving_side_effect_free_and_compile_equivalent() {
    let cue_list_id = CueListId::new();
    let cue_list = CueList {
        id: cue_list_id,
        name: "Legacy Chaser".into(),
        priority: 0,
        mode: CueListMode::Chaser,
        looped: true,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Tracking),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 250,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues: vec![Cue::new(1.0)],
    };
    let mut legacy_cue = serde_json::to_value(cue_list).unwrap();
    legacy_cue["cues"][0].as_object_mut().unwrap().remove("id");
    legacy_cue["cues"][0]["future_cue"] = json!({"kept": [3, 1, 2]});
    legacy_cue["future_list"] = json!({"kept": true});
    let group = json!({
        "name": "Front",
        "fixtures": [],
        "future_group": {"kept": true}
    });
    let preset = json!({
        "name": "Dim",
        "family": "Intensity",
        "values": {"00000000-0000-0000-0000-000000000123": {
            "intensity": {"kind": "normalized", "value": 0.5, "future_value": {"kept": true}}
        }},
        "future_preset": {"kept": true}
    });
    let playback = json!({
        "number": 1,
        "name": "Speed",
        "target": {"type": "speed_group", "group": "A", "future_target": {"kept": true}},
        "fader": "speed",
        "future_playback": {"kept": true}
    });
    let route = OutputRoute {
        protocol: Protocol::ArtNet,
        logical_universe: 1,
        destination_universe: 1,
        delivery_mode: Some(DeliveryMode::Broadcast),
        destination: None,
        enabled: true,
        minimum_slots: 512,
    };
    let mut legacy_route = serde_json::to_value(route).unwrap();
    for field in ["delivery_mode", "destination", "minimum_slots"] {
        legacy_route.as_object_mut().unwrap().remove(field);
    }
    legacy_route["future_route"] = json!({"kept": true});

    let originals = vec![
        ("cue_list", "legacy", legacy_cue),
        ("group", "7", group),
        ("playback", "1", playback),
        ("preset", "1.5", preset),
        ("route", "one", legacy_route),
    ];
    let (store, document) = document_with_objects(&originals);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();

    for (kind, id, body) in &originals {
        assert_eq!(stored_body(&store, kind, id), *body);
    }
    let candidate = document.candidate(&transaction).unwrap();
    let cue = candidate.object("cue_list", "legacy").unwrap().body();
    assert_eq!(cue["chaser_xfade_percent"], 25);
    assert!(cue.get("chaser_xfade_millis").is_none());
    assert!(Uuid::parse_str(cue["cues"][0]["id"].as_str().unwrap()).is_ok());
    assert_eq!(cue["cues"][0]["future_cue"], json!({"kept": [3, 1, 2]}));
    assert_eq!(candidate.object("group", "7").unwrap().body()["id"], "7");
    let preset = candidate.object("preset", "1.5").unwrap().body();
    assert_eq!(preset["number"], 5);
    assert_eq!(
        preset["values"]["00000000-0000-0000-0000-000000000123"]["intensity"]["future_value"],
        json!({"kept": true})
    );
    let playback = candidate.object("playback", "1").unwrap().body();
    assert_eq!(playback["fader"], "learned_percentage");
    assert_eq!(playback["buttons"], json!(["double", "half", "learn"]));
    assert_eq!(playback["target"]["future_target"], json!({"kept": true}));
    let route = candidate.object("route", "one").unwrap().body();
    assert_eq!(route["delivery_mode"], "broadcast");
    assert!(route["destination"].is_null());
    assert_eq!(route["minimum_slots"], 512);
    assert_eq!(route["future_route"], json!({"kept": true}));

    let migrated = candidate
        .objects()
        .map(|object| {
            (
                object.key().kind().to_owned(),
                object.key().id().to_owned(),
                object.body().clone(),
            )
        })
        .collect::<Vec<_>>();
    let legacy_snapshot = compile_show_candidate(candidate).unwrap();
    let migrated_refs = migrated
        .iter()
        .map(|(kind, id, body)| (kind.as_str(), id.as_str(), body.clone()))
        .collect::<Vec<_>>();
    let (_, current_document) = document_with_objects(&migrated_refs);
    let mut current_transaction = current_document.transaction();
    stage_candidate_migrations(&current_document, &mut current_transaction).unwrap();
    assert!(current_transaction.is_empty());
    let current_snapshot =
        compile_show_candidate(current_document.candidate(&current_transaction).unwrap()).unwrap();
    assert_eq!(
        snapshot_without_revision(legacy_snapshot),
        snapshot_without_revision(current_snapshot)
    );
}
