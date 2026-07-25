use super::*;

#[test]
fn map_existing_is_page_only_lossless_replayable_and_semantically_idempotent() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        &serde_json::to_value(cue_list(cue_list_id, "Mapped source")).unwrap(),
    );
    let mut source = serde_json::to_value(cue_list_playback(12, cue_list_id)).unwrap();
    source["future_playback"] = json!({"keep":"source"});
    source.as_object_mut().unwrap().remove("color");
    rig.seed("playback", "legacy-twelve", &source);
    rig.seed(
        "playback_page",
        "legacy-page-two",
        &json!({"number":2,"name":"Wing","slots":{},"future_page":{"columns":8}}),
    );
    let action = map_existing_action(2, 4, 12, 1, "legacy-page-two");
    let before = rig.show_revision();

    let first = rig.handle("map-existing", before, action.clone()).unwrap();

    assert!(matches!(
        first.outcome,
        PlaybackTopologyOutcome::Changed { .. }
    ));
    assert_eq!(first.outcome.objects().len(), 1);
    assert_eq!(
        first.outcome.objects()[0].kind(),
        ActiveShowObjectKind::PlaybackPage
    );
    assert_eq!(first.outcome.objects()[0].object_id(), "legacy-page-two");
    let page = first.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(page["slots"]["4"], 12);
    assert_eq!(page["future_page"]["columns"], 8);
    let document = rig.document();
    let stored_source = document.object("playback", "legacy-twelve").unwrap();
    assert_eq!(stored_source.revision(), 1);
    assert_eq!(stored_source.body()["future_playback"]["keep"], "source");
    assert!(stored_source.body().get("color").is_none());
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 1);

    rig.clear_steps();
    let replay = rig.handle("map-existing", before, action).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(rig.steps(), ["authorize"]);
    assert_one_event(&rig, 1);

    rig.clear_steps();
    let no_change = rig
        .handle(
            "map-existing-no-change",
            rig.show_revision(),
            map_existing_action(2, 4, 12, 2, "legacy-page-two"),
        )
        .unwrap();
    assert!(matches!(
        no_change.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    assert_eq!(no_change.outcome.objects().len(), 1);
    assert_eq!(no_change.outcome.event_sequence(), None);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 1);
}

#[test]
fn map_existing_creates_default_page_without_rewriting_source() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        &serde_json::to_value(cue_list(cue_list_id, "Source")).unwrap(),
    );
    rig.seed(
        "playback",
        "source-seven",
        &serde_json::to_value(cue_list_playback(7, cue_list_id)).unwrap(),
    );

    let outcome = rig
        .handle(
            "map-new-page",
            rig.show_revision(),
            PlaybackTopologyAction::MapExistingPlayback {
                page: 3,
                slot: 6,
                playback_number: 7,
                expected_page_revision: 0,
                expected_page_object_id: None,
                expected_playback_revision: 1,
                expected_playback_object_id: Some("source-seven".into()),
            },
        )
        .unwrap();

    assert_eq!(outcome.outcome.objects().len(), 1);
    let page = outcome.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(page["number"], 3);
    assert_eq!(page["name"], "Page 3");
    assert_eq!(page["slots"]["6"], 7);
    assert_eq!(
        rig.document()
            .object("playback", "source-seven")
            .unwrap()
            .revision(),
        1
    );
    assert_one_event(&rig, 1);
}

#[test]
fn map_existing_rejects_an_occupied_default_page_storage_identity() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        &serde_json::to_value(cue_list(cue_list_id, "Source")).unwrap(),
    );
    rig.seed(
        "playback",
        "source-seven",
        &serde_json::to_value(cue_list_playback(7, cue_list_id)).unwrap(),
    );
    rig.seed(
        "playback_page",
        "3",
        &json!({"number":9,"name":"Legacy Page","slots":{}}),
    );
    let revision = rig.show_revision();

    let error = rig
        .handle(
            "map-page-key-collision",
            revision,
            PlaybackTopologyAction::MapExistingPlayback {
                page: 3,
                slot: 6,
                playback_number: 7,
                expected_page_revision: 0,
                expected_page_object_id: None,
                expected_playback_revision: 1,
                expected_playback_object_id: Some("source-seven".into()),
            },
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(revision));
    assert_eq!(error.current_related_revision, Some(1));
    assert_eq!(rig.show_revision(), revision);
    let document = rig.document();
    let occupied = document.object("playback_page", "3").unwrap();
    assert_eq!(occupied.body()["number"], 9);
    assert_eq!(occupied.revision(), 1);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn map_existing_rejects_stale_source_identity_and_non_cuelist_target() {
    let rig = TestRig::new();
    rig.seed(
        "playback",
        "legacy-seven",
        &serde_json::to_value(playback(7, "Grand master")).unwrap(),
    );
    let revision = rig.show_revision();
    let stale = PlaybackTopologyAction::MapExistingPlayback {
        page: 1,
        slot: 1,
        playback_number: 7,
        expected_page_revision: 0,
        expected_page_object_id: None,
        expected_playback_revision: 1,
        expected_playback_object_id: Some("replacement-seven".into()),
    };

    let error = rig.handle("map-stale-source", revision, stale).unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_related_revision, Some(1));
    assert_one_event(&rig, 0);

    rig.clear_steps();
    let invalid = rig
        .handle(
            "map-wrong-target",
            revision,
            PlaybackTopologyAction::MapExistingPlayback {
                page: 1,
                slot: 1,
                playback_number: 7,
                expected_page_revision: 0,
                expected_page_object_id: None,
                expected_playback_revision: 1,
                expected_playback_object_id: Some("legacy-seven".into()),
            },
        )
        .unwrap_err();
    assert_eq!(invalid.kind, ActionErrorKind::Invalid);
    assert_eq!(rig.show_revision(), revision);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}
