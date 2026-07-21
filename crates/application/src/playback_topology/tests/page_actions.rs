use super::*;

#[test]
fn create_page_commits_one_default_projection_and_replays_once() {
    let rig = TestRig::new();
    let revision = rig.show_revision();
    let action = PlaybackTopologyAction::CreatePage {
        page: 4,
        expected_page_revision: 0,
        expected_page_object_id: None,
    };

    let changed = rig.handle("create-page", revision, action.clone()).unwrap();

    assert!(matches!(
        changed.outcome,
        PlaybackTopologyOutcome::Changed { .. }
    ));
    assert_eq!(
        changed.outcome.resolution(),
        PlaybackTopologyResolution::Page { page: 4 }
    );
    assert_eq!(changed.outcome.objects().len(), 1);
    let projection = &changed.outcome.objects()[0];
    assert_eq!(projection.kind(), ActiveShowObjectKind::PlaybackPage);
    assert_eq!(projection.object_id(), "4");
    assert_eq!(
        projection.raw_body().unwrap().as_ref(),
        &json!({"number":4,"name":"Page 4","slots":{}})
    );
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 1);

    rig.clear_steps();
    let replay = rig.handle("create-page", revision, action).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, changed.outcome);
    assert_eq!(rig.steps(), ["authorize"]);
    assert_one_event(&rig, 1);
}

#[test]
fn create_existing_page_is_no_change_without_normalizing_legacy_json() {
    let rig = TestRig::new();
    let raw = json!({
        "number":2,"name":"Wing","slots":{"7":19},
        "future_layout":{"columns":12}
    });
    rig.seed("playback_page", "legacy-page-two", &raw);

    let result = rig
        .handle(
            "ensure-page",
            rig.show_revision(),
            PlaybackTopologyAction::CreatePage {
                page: 2,
                expected_page_revision: 1,
                expected_page_object_id: Some("legacy-page-two".into()),
            },
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    assert_eq!(result.outcome.objects().len(), 1);
    assert_eq!(result.outcome.objects()[0].object_id(), "legacy-page-two");
    assert_eq!(
        result.outcome.objects()[0].raw_body().unwrap().as_ref(),
        &raw
    );
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn rename_page_is_lossless_replayable_and_semantically_idempotent() {
    let rig = TestRig::new();
    rig.seed(
        "playback",
        "8",
        &serde_json::to_value(playback(8, "House")).unwrap(),
    );
    rig.seed(
        "playback_page",
        "legacy-page-three",
        &json!({
            "number":3,"name":"Before","slots":{"2":8},
            "future_layout":{"columns":8}
        }),
    );
    let revision = rig.show_revision();
    let action = PlaybackTopologyAction::RenamePage {
        page: 3,
        name: "Act One".into(),
        expected_page_revision: 1,
        expected_page_object_id: Some("legacy-page-three".into()),
    };

    let changed = rig.handle("rename-page", revision, action.clone()).unwrap();

    assert_eq!(changed.outcome.objects().len(), 1);
    assert_eq!(
        changed.outcome.objects()[0].object_id(),
        "legacy-page-three"
    );
    let page = changed.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(page["name"], "Act One");
    assert_eq!(page["slots"], json!({"2":8}));
    assert_eq!(page["future_layout"]["columns"], 8);
    assert_one_event(&rig, 1);

    rig.clear_steps();
    let replay = rig.handle("rename-page", revision, action).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, changed.outcome);
    assert_eq!(rig.steps(), ["authorize"]);

    rig.clear_steps();
    let no_change = rig
        .handle(
            "rename-page-no-change",
            rig.show_revision(),
            PlaybackTopologyAction::RenamePage {
                page: 3,
                name: "Act One".into(),
                expected_page_revision: 2,
                expected_page_object_id: Some("legacy-page-three".into()),
            },
        )
        .unwrap();
    assert!(matches!(
        no_change.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    assert_eq!(no_change.outcome.objects().len(), 1);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 1);
}

#[test]
fn page_actions_reject_stale_authority_invalid_names_and_canonical_collisions() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "legacy-page-one",
        &json!({"number":1,"name":"Main","slots":{}}),
    );
    rig.seed(
        "playback_page",
        "2",
        &json!({"number":9,"name":"Occupied","slots":{}}),
    );
    let revision = rig.show_revision();

    let stale = rig
        .handle(
            "stale-page",
            revision,
            PlaybackTopologyAction::RenamePage {
                page: 1,
                name: "Changed".into(),
                expected_page_revision: 0,
                expected_page_object_id: Some("legacy-page-one".into()),
            },
        )
        .unwrap_err();
    assert_eq!(stale.kind, ActionErrorKind::Conflict);
    assert_eq!(stale.current_revision, Some(revision));
    assert_eq!(stale.current_related_revision, Some(1));

    rig.clear_steps();
    let invalid = rig
        .handle(
            "invalid-page-name",
            revision,
            PlaybackTopologyAction::RenamePage {
                page: 1,
                name: " padded ".into(),
                expected_page_revision: 1,
                expected_page_object_id: Some("legacy-page-one".into()),
            },
        )
        .unwrap_err();
    assert_eq!(invalid.kind, ActionErrorKind::Invalid);

    rig.clear_steps();
    let collision = rig
        .handle(
            "create-page-collision",
            revision,
            PlaybackTopologyAction::CreatePage {
                page: 2,
                expected_page_revision: 0,
                expected_page_object_id: None,
            },
        )
        .unwrap_err();
    assert_eq!(collision.kind, ActionErrorKind::Conflict);
    assert_eq!(collision.current_related_revision, Some(1));
    assert_eq!(rig.show_revision(), revision);
    assert_one_event(&rig, 0);
}
