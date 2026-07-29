use super::*;

#[test]
fn configure_virtual_persists_one_sparse_page_qualified_assignment_and_normalizes_controls() {
    let rig = TestRig::new();
    let raw_page = json!({
        "number": 1,
        "name": "Main",
        "slots": {},
        "virtual_playbacks": {},
        "future_layout": {"columns": 12}
    });
    rig.seed("playback_page", "legacy-page-one", &raw_page);
    let mut requested = playback(7, "Audience");
    requested.buttons = [
        light_playback::PlaybackButtonAction::Blackout,
        light_playback::PlaybackButtonAction::Flash,
        light_playback::PlaybackButtonAction::PauseDynamics,
    ];

    let result = rig
        .handle(
            "configure-virtual-1-1001",
            rig.show_revision(),
            PlaybackTopologyAction::ConfigureVirtual {
                page: 1,
                number: 1001,
                expected_page_revision: 1,
                expected_page_object_id: Some("legacy-page-one".into()),
                playback: requested,
            },
        )
        .unwrap();

    assert_eq!(
        result.outcome.resolution(),
        PlaybackTopologyResolution::Virtual {
            page: 1,
            playback_number: 1001,
        }
    );
    assert_eq!(result.outcome.objects().len(), 1);
    let projection = &result.outcome.objects()[0];
    assert_eq!(projection.kind(), ActiveShowObjectKind::PlaybackPage);
    assert_eq!(projection.object_id(), "legacy-page-one");
    let raw = projection.raw_body().unwrap();
    assert_eq!(raw["future_layout"]["columns"], 12);
    assert_eq!(raw["slots"], json!({}));
    assert_eq!(
        raw["virtual_playbacks"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        vec!["1001"]
    );

    let page: light_playback::PlaybackPage = serde_json::from_value(raw.as_ref().clone()).unwrap();
    let assigned = page.virtual_playbacks.get(&1001).unwrap();
    assert_eq!(assigned.number, 1001);
    assert_eq!(assigned.name, "Audience");
    assert!(!assigned.has_fader);
    assert_eq!(assigned.button_count, 1);
    assert_eq!(
        assigned.buttons,
        [
            light_playback::PlaybackButtonAction::Blackout,
            light_playback::PlaybackButtonAction::None,
            light_playback::PlaybackButtonAction::None,
        ]
    );
    assert!(rig.document().objects_of_kind("playback").next().is_none());
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 1);
}

#[test]
fn banked_virtual_numbers_on_different_pages_remain_independent() {
    let rig = TestRig::new();

    let first = rig
        .handle(
            "configure-page-1",
            rig.show_revision(),
            PlaybackTopologyAction::ConfigureVirtual {
                page: 1,
                number: 1001,
                expected_page_revision: 0,
                expected_page_object_id: None,
                playback: playback(1001, "Page one"),
            },
        )
        .unwrap();
    assert_eq!(
        first.outcome.resolution(),
        PlaybackTopologyResolution::Virtual {
            page: 1,
            playback_number: 1001,
        }
    );

    rig.clear_steps();
    let second = rig
        .handle(
            "configure-page-2",
            rig.show_revision(),
            PlaybackTopologyAction::ConfigureVirtual {
                page: 2,
                number: 1301,
                expected_page_revision: 0,
                expected_page_object_id: None,
                playback: playback(1301, "Page two"),
            },
        )
        .unwrap();
    assert_eq!(
        second.outcome.resolution(),
        PlaybackTopologyResolution::Virtual {
            page: 2,
            playback_number: 1301,
        }
    );

    let document = rig.document();
    let page_one: light_playback::PlaybackPage = serde_json::from_value(
        document
            .object("playback_page", "1")
            .unwrap()
            .body()
            .clone(),
    )
    .unwrap();
    let page_two: light_playback::PlaybackPage = serde_json::from_value(
        document
            .object("playback_page", "2")
            .unwrap()
            .body()
            .clone(),
    )
    .unwrap();
    assert_eq!(page_one.virtual_playbacks[&1001].name, "Page one");
    assert_eq!(page_two.virtual_playbacks[&1301].name, "Page two");
    assert_eq!(page_one.virtual_playbacks.len(), 1);
    assert_eq!(page_two.virtual_playbacks.len(), 1);
    assert_eq!(rig.steps(), mutation_steps());
    assert_event_object_counts(&rig, &[1, 1]);
}

#[test]
fn clear_virtual_removes_only_the_addressed_page_assignment_and_replays_once() {
    let rig = TestRig::new();
    for (page, number, name) in [(1, 1001, "Page one"), (2, 1301, "Page two")] {
        rig.handle(
            &format!("seed-page-{page}"),
            rig.show_revision(),
            PlaybackTopologyAction::ConfigureVirtual {
                page,
                number,
                expected_page_revision: 0,
                expected_page_object_id: None,
                playback: playback(number, name),
            },
        )
        .unwrap();
    }
    rig.clear_steps();
    let revision = rig.show_revision();
    let action = PlaybackTopologyAction::ClearVirtual {
        page: 1,
        number: 1001,
        expected_page_revision: 1,
        expected_page_object_id: Some("1".into()),
    };

    let changed = rig
        .handle("clear-page-1-1001", revision, action.clone())
        .unwrap();

    assert_eq!(
        changed.outcome.resolution(),
        PlaybackTopologyResolution::Virtual {
            page: 1,
            playback_number: 1001,
        }
    );
    let document = rig.document();
    let page_one: light_playback::PlaybackPage = serde_json::from_value(
        document
            .object("playback_page", "1")
            .unwrap()
            .body()
            .clone(),
    )
    .unwrap();
    let page_two: light_playback::PlaybackPage = serde_json::from_value(
        document
            .object("playback_page", "2")
            .unwrap()
            .body()
            .clone(),
    )
    .unwrap();
    assert!(page_one.virtual_playbacks.is_empty());
    assert_eq!(page_two.virtual_playbacks[&1301].name, "Page two");
    assert_eq!(rig.steps(), mutation_steps());
    assert_event_object_counts(&rig, &[1, 1, 1]);

    rig.clear_steps();
    let replay = rig.handle("clear-page-1-1001", revision, action).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, changed.outcome);
    assert_eq!(rig.steps(), ["authorize"]);
    assert_event_object_counts(&rig, &[1, 1, 1]);
}

#[test]
fn virtual_configuration_enforces_bounds_and_page_optimistic_authority() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "legacy-page-one",
        &json!({"number":1,"name":"Main","slots":{},"virtual_playbacks":{}}),
    );
    let revision = rig.show_revision();

    for (request_id, number) in [("below-range", 1000), ("wrong-page-bank", 1301)] {
        rig.clear_steps();
        let error = rig
            .handle(
                request_id,
                revision,
                PlaybackTopologyAction::ConfigureVirtual {
                    page: 1,
                    number,
                    expected_page_revision: 1,
                    expected_page_object_id: Some("legacy-page-one".into()),
                    playback: playback(number, "Invalid"),
                },
            )
            .unwrap_err();
        assert_eq!(error.kind, ActionErrorKind::Invalid);
        assert_eq!(rig.show_revision(), revision);
        assert_eq!(rig.steps(), ["authorize", "begin"]);
    }

    rig.clear_steps();
    let stale_revision = rig
        .handle(
            "stale-virtual-page",
            revision,
            PlaybackTopologyAction::ConfigureVirtual {
                page: 1,
                number: 1001,
                expected_page_revision: 0,
                expected_page_object_id: Some("legacy-page-one".into()),
                playback: playback(1001, "Stale"),
            },
        )
        .unwrap_err();
    assert_eq!(stale_revision.kind, ActionErrorKind::Conflict);
    assert_eq!(stale_revision.current_revision, Some(revision));
    assert_eq!(stale_revision.current_related_revision, Some(1));
    assert_eq!(rig.steps(), ["authorize", "begin"]);

    rig.clear_steps();
    let stale_identity = rig
        .handle(
            "wrong-virtual-page-id",
            revision,
            PlaybackTopologyAction::ClearVirtual {
                page: 1,
                number: 1001,
                expected_page_revision: 1,
                expected_page_object_id: Some("replacement-page-one".into()),
            },
        )
        .unwrap_err();
    assert_eq!(stale_identity.kind, ActionErrorKind::Conflict);
    assert_eq!(stale_identity.current_revision, Some(revision));
    assert_eq!(stale_identity.current_related_revision, Some(1));
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn virtual_replay_fingerprint_rejects_page_or_number_changes() {
    let rig = TestRig::new();
    let revision = rig.show_revision();
    let action = PlaybackTopologyAction::ConfigureVirtual {
        page: 1,
        number: 1001,
        expected_page_revision: 0,
        expected_page_object_id: None,
        playback: playback(1001, "Replay"),
    };
    let first = rig
        .handle("virtual-replay", revision, action.clone())
        .unwrap();
    rig.clear_steps();

    let replay = rig.handle("virtual-replay", revision, action).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(rig.steps(), ["authorize"]);

    rig.clear_steps();
    let collision = rig
        .handle(
            "virtual-replay",
            revision,
            PlaybackTopologyAction::ConfigureVirtual {
                page: 2,
                number: 1002,
                expected_page_revision: 0,
                expected_page_object_id: None,
                playback: playback(1002, "Replay"),
            },
        )
        .unwrap_err();
    assert_eq!(collision.kind, ActionErrorKind::Conflict);
    assert_eq!(rig.steps(), ["authorize"]);
    assert_one_event(&rig, 1);
}

fn assert_event_object_counts(rig: &TestRig, expected: &[usize]) {
    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained events")
    };
    let actual = events
        .iter()
        .map(|event| {
            let ApplicationEvent::Show(ShowEvent::ObjectsChanged(change)) = &event.payload else {
                panic!("expected Show Objects event")
            };
            change.changes.len()
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}
