use super::*;

#[test]
fn sparse_assignment_commits_one_page_and_preserves_extensions() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "page-one",
        &json!({"number":1,"name":"Main","assignments":{},"future":{"columns":12}}),
    );
    let changed = rig
        .handle(
            "assign",
            rig.show_revision(),
            assignment(1, 1000, playback(0, "House"), 1, Some("page-one")),
        )
        .unwrap();
    assert_eq!(
        changed.outcome.resolution(),
        PlaybackTopologyResolution::Playback {
            address: address(1, 1000)
        }
    );
    assert_eq!(changed.outcome.objects().len(), 1);
    assert_eq!(
        changed.outcome.objects()[0].kind(),
        ActiveShowObjectKind::PlaybackPage
    );
    assert_eq!(
        changed.outcome.objects()[0].raw_body().unwrap()["future"]["columns"],
        12
    );
    assert_eq!(assigned(&rig, 1, 1000).name, "House");
    assert!(rig.document().objects_of_kind("playback").next().is_none());
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 1);
}

#[test]
fn same_position_on_different_pages_is_independent_and_clear_is_local() {
    let rig = TestRig::new();
    for page in [1, 99] {
        rig.handle(
            &format!("assign-{page}"),
            rig.show_revision(),
            assignment(page, 1000, playback(0, &format!("Page {page}")), 0, None),
        )
        .unwrap();
    }
    rig.handle(
        "clear",
        rig.show_revision(),
        PlaybackTopologyAction::ClearPlayback {
            address: address(1, 1000),
            expected_page_revision: 1,
            expected_page_object_id: Some("1".into()),
        },
    )
    .unwrap();
    assert!(
        rig.document().object("playback_page", "1").unwrap().body()["assignments"]
            .as_object()
            .unwrap()
            .is_empty()
    );
    assert_eq!(assigned(&rig, 99, 1000).name, "Page 99");
    assert_eq!(
        rig.document()
            .object("playback_page", "99")
            .unwrap()
            .revision(),
        1
    );
}

#[test]
fn update_is_lossless_and_identical_update_has_no_transaction_or_event() {
    let rig = TestRig::new();
    let mut raw = serde_json::to_value(playback(0, "Before")).unwrap();
    raw["future_playback"] = json!({"keep":true});
    raw["target"]["future_target"] = json!([1, 2, 3]);
    rig.seed(
        "playback_page",
        "one",
        &json!({"number":1,"name":"Main","assignments":{"2":raw},"future_page":true}),
    );
    let mut value = assigned(&rig, 1, 2);
    value.name = "After".into();
    let action = PlaybackTopologyAction::UpdatePlayback {
        address: address(1, 2),
        expected_page_revision: 1,
        expected_page_object_id: Some("one".into()),
        playback: value.clone(),
    };
    rig.handle("update", rig.show_revision(), action).unwrap();
    let page = rig
        .document()
        .object("playback_page", "one")
        .unwrap()
        .body()
        .clone();
    assert_eq!(page["assignments"]["2"]["future_playback"]["keep"], true);
    assert_eq!(
        page["assignments"]["2"]["target"]["future_target"],
        json!([1, 2, 3])
    );
    assert_eq!(page["future_page"], true);
    rig.clear_steps();
    let unchanged = rig
        .handle(
            "noop",
            rig.show_revision(),
            PlaybackTopologyAction::UpdatePlayback {
                address: address(1, 2),
                expected_page_revision: 2,
                expected_page_object_id: Some("one".into()),
                playback: value,
            },
        )
        .unwrap();
    assert!(matches!(
        unchanged.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 1);
}

#[test]
fn assign_occupied_and_update_missing_reject_without_changes() {
    let rig = TestRig::new();
    rig.handle(
        "assign",
        rig.show_revision(),
        assignment(1, 2, playback(0, "Keep"), 0, None),
    )
    .unwrap();
    let revision = rig.show_revision();
    let conflict = rig
        .handle(
            "occupied",
            revision,
            assignment(1, 2, playback(0, "Overwrite"), 1, Some("1")),
        )
        .unwrap_err();
    assert_eq!(conflict.kind, ActionErrorKind::Conflict);
    let missing = rig
        .handle(
            "missing",
            revision,
            PlaybackTopologyAction::UpdatePlayback {
                address: address(1, 3),
                expected_page_revision: 1,
                expected_page_object_id: Some("1".into()),
                playback: playback(0, "Missing"),
            },
        )
        .unwrap_err();
    assert_eq!(missing.kind, ActionErrorKind::NotFound);
    assert_eq!(rig.show_revision(), revision);
    assert_eq!(assigned(&rig, 1, 2).name, "Keep");
}

#[test]
fn page_identity_collision_and_stale_identity_refuse_without_mutation() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":2,"name":"Occupied","assignments":{}}),
    );
    let revision = rig.show_revision();
    assert_eq!(
        rig.handle(
            "collision",
            revision,
            assignment(1, 1, playback(0, "New"), 0, None)
        )
        .unwrap_err()
        .kind,
        ActionErrorKind::Conflict
    );
    assert_eq!(
        rig.handle(
            "identity",
            revision,
            assignment(2, 1, playback(0, "New"), 1, Some("replacement"))
        )
        .unwrap_err()
        .kind,
        ActionErrorKind::Conflict
    );
    assert_eq!(rig.show_revision(), revision);
    assert_one_event(&rig, 0);
}

#[test]
fn macro_assignment_requires_portable_target_and_persists_it() {
    let rig = TestRig::new();
    let macro_id = Uuid::from_u128(71);
    let mut value = playback(0, "Macro");
    value.target = PlaybackTarget::Macro { macro_id };
    value.buttons = PlaybackDefinition::default_buttons(&value.target);
    value.fader = PlaybackDefinition::default_fader(&value.target);
    value.has_fader = false;
    assert_eq!(
        rig.handle(
            "missing",
            rig.show_revision(),
            assignment(1, 401, value.clone(), 0, None)
        )
        .unwrap_err()
        .kind,
        ActionErrorKind::NotFound
    );
    rig.seed(
        "macro",
        &macro_id.to_string(),
        &serde_json::to_value(crate::CommandMacroDefinition {
            id: macro_id,
            number: 71,
            name: "Preset fixtures".into(),
            source: "Fixture 1 At 50".into(),
            presentation: crate::MacroPresentation::default(),
        })
        .unwrap(),
    );
    rig.handle(
        "assign",
        rig.show_revision(),
        assignment(1, 401, value.clone(), 0, None),
    )
    .unwrap();
    assert_eq!(assigned(&rig, 1, 401).target, value.target);
    assert!(!assigned(&rig, 1, 401).has_fader);
}

#[test]
fn timecode_target_is_supported_at_every_canonical_position() {
    for (page, position) in [(1, 1), (99, 1000)] {
        let rig = TestRig::new();
        let timecode_id = light_playback::TimecodeId(Uuid::from_u128(70));
        rig.seed(
            "timecode",
            &timecode_id.0.to_string(),
            &serde_json::to_value(light_playback::TimecodeDefinition {
                id: timecode_id,
                number: 70,
                name: "Timeline".into(),
                duration: Some(light_playback::TimecodeFrame(1000)),
                transport_offset: light_playback::TimecodeFrame::ZERO,
                auto_start: false,
                audio: None,
                markers: vec![],
                lanes: vec![],
            })
            .unwrap(),
        );
        let mut value = playback(0, "Timeline");
        value.target = PlaybackTarget::Timecode { timecode_id };
        value.buttons = PlaybackDefinition::default_buttons(&value.target);
        value.fader = PlaybackDefinition::default_fader(&value.target);
        value.has_fader = false;
        rig.handle(
            "assign",
            rig.show_revision(),
            assignment(page, position, value.clone(), 0, None),
        )
        .unwrap();
        assert_eq!(assigned(&rig, page, position).target, value.target);
    }
}

#[test]
fn missing_cuelist_and_group_targets_are_not_persisted() {
    for target in [
        PlaybackTarget::CueList {
            cue_list_id: CueListId::new(),
        },
        PlaybackTarget::Group {
            group_id: "absent".into(),
            initial_master: None,
        },
    ] {
        let rig = TestRig::new();
        let before = rig.show_revision();
        let mut value = playback(0, "Missing");
        value.target = target;
        value.buttons = PlaybackDefinition::default_buttons(&value.target);
        value.fader = PlaybackDefinition::default_fader(&value.target);
        assert_eq!(
            rig.handle("missing", before, assignment(1, 1, value, 0, None))
                .unwrap_err()
                .kind,
            ActionErrorKind::NotFound
        );
        assert_eq!(rig.show_revision(), before);
        assert_one_event(&rig, 0);
    }
}

#[test]
fn exact_address_bounds_are_enforced_without_legacy_bank_numbers() {
    for (page, position) in [(0, 1), (100, 1), (1, 0), (1, 1001)] {
        assert!(light_playback::PlaybackAddress::new(page, position).is_err());
    }
    assert_eq!(address(99, 1000).to_string(), "99.1000");
}

#[test]
fn replay_fingerprint_includes_page_position_and_definition() {
    let rig = TestRig::new();
    let before = rig.show_revision();
    let action = assignment(1, 401, playback(0, "First"), 0, None);
    rig.handle("same", before, action.clone()).unwrap();
    assert!(rig.handle("same", before, action).unwrap().replayed);
    for changed in [
        assignment(2, 401, playback(0, "First"), 0, None),
        assignment(1, 402, playback(0, "First"), 0, None),
        assignment(1, 401, playback(0, "Changed"), 0, None),
    ] {
        assert_eq!(
            rig.handle("same", before, changed).unwrap_err().kind,
            ActionErrorKind::Conflict
        );
    }
    assert_one_event(&rig, 1);
}

#[test]
fn storage_identity_conflicts_stop_before_topology_mutation() {
    let cue_rig = TestRig::new();
    let cue_list_id = CueListId::new();
    let cue_list = cue_list(cue_list_id, "Original");
    let cue_body = serde_json::to_value(&cue_list).unwrap();
    cue_rig.seed("cue_list", "legacy-main", &cue_body);

    let cue_error = cue_rig
        .handle(
            "stale-cue-identity",
            cue_rig.show_revision(),
            PlaybackTopologyAction::SaveCueList {
                cue_list_id,
                expected_revision: 1,
                expected_object_id: Some("replacement-main".into()),
                cue_list,
                raw_body: Arc::new(cue_body),
            },
        )
        .unwrap_err();

    assert_eq!(cue_error.kind, ActionErrorKind::Conflict);
    assert_eq!(cue_error.current_related_revision, Some(1));
    assert_eq!(cue_rig.steps(), ["authorize", "begin"]);
    assert_one_event(&cue_rig, 0);
}

#[test]
fn canonical_storage_key_collisions_stop_before_topology_mutation() {
    let cue_rig = TestRig::new();
    let requested_id = CueListId::new();
    let occupied = cue_list(CueListId::new(), "Occupied");
    cue_rig.seed(
        "cue_list",
        &requested_id.0.to_string(),
        &serde_json::to_value(&occupied).unwrap(),
    );
    let requested = cue_list(requested_id, "Requested");
    let requested_body = serde_json::to_value(&requested).unwrap();
    let cue_revision = cue_rig.show_revision();

    let cue_error = cue_rig
        .handle(
            "occupied-cue-key",
            cue_revision,
            PlaybackTopologyAction::SaveCueList {
                cue_list_id: requested_id,
                expected_revision: 0,
                expected_object_id: None,
                cue_list: requested,
                raw_body: Arc::new(requested_body),
            },
        )
        .unwrap_err();

    assert_eq!(cue_error.kind, ActionErrorKind::Conflict);
    assert_eq!(cue_error.current_related_revision, Some(1));
    assert_eq!(cue_rig.show_revision(), cue_revision);
    let cue_document = cue_rig.document();
    assert_eq!(
        cue_document
            .object("cue_list", &requested_id.0.to_string())
            .unwrap()
            .body()["id"],
        occupied.id.0.to_string()
    );
    assert_eq!(cue_rig.steps(), ["authorize", "begin"]);
    assert_one_event(&cue_rig, 0);
}

#[test]
fn clearing_assignment_keeps_cuelist_source_and_unrelated_same_page_assignment() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        &serde_json::to_value(cue_list(cue_list_id, "Keep source")).unwrap(),
    );
    rig.seed("playback_page","one",&json!({"number":1,"name":"Main","assignments":{"1":cue_list_playback(0,cue_list_id),"2":playback(0,"Keep neighbor")},"future":"retain"}));
    let before = rig.show_revision();
    let result = rig
        .handle(
            "clear",
            before,
            PlaybackTopologyAction::ClearPlayback {
                address: address(1, 1),
                expected_page_revision: 1,
                expected_page_object_id: Some("one".into()),
            },
        )
        .unwrap();
    assert_eq!(result.outcome.show_revision().value(), before + 1);
    assert_eq!(result.outcome.objects().len(), 1);
    assert!(
        rig.document()
            .object("cue_list", &cue_list_id.0.to_string())
            .is_some()
    );
    assert_eq!(assigned(&rig, 1, 2).name, "Keep neighbor");
    assert_eq!(
        rig.document()
            .object("playback_page", "one")
            .unwrap()
            .body()["future"],
        "retain"
    );
    assert_one_event(&rig, 1);
}

#[test]
fn clearing_absent_assignment_is_no_change_without_runtime_prepare_or_event() {
    let rig = TestRig::new();
    let before = rig.show_revision();
    let result = rig
        .handle(
            "clear",
            before,
            PlaybackTopologyAction::ClearPlayback {
                address: address(99, 1000),
                expected_page_revision: 0,
                expected_page_object_id: None,
            },
        )
        .unwrap();
    assert!(matches!(
        result.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    assert_eq!(rig.show_revision(), before);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}
