use super::*;

fn fixture_change(fixture_id: FixtureId, attribute: &str, value: f32) -> CueChange {
    CueChange::set(
        fixture_id,
        AttributeKey(attribute.into()),
        AttributeValue::Normalized(value),
    )
}

fn group_change(group_id: &str, attribute: &str, value: f32) -> GroupCueChange {
    GroupCueChange {
        group_id: group_id.into(),
        attribute: AttributeKey(attribute.into()),
        value: Some(AttributeValue::Normalized(value)),
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    }
}

fn content(changes: Vec<CueChange>) -> CueRecordingContent {
    CueRecordingContent {
        changes,
        ..Default::default()
    }
}

fn cue(number: f64, name: &str, changes: Vec<CueChange>) -> Cue {
    let mut cue = Cue::new(number);
    cue.name = name.into();
    cue.changes = changes;
    cue
}

fn cue_list(cues: Vec<Cue>) -> CueList {
    CueList {
        id: CueListId::new(),
        name: "Main".into(),
        priority: 12,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Off),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues,
    }
}

fn phaser(fixture_id: FixtureId) -> AttributePhaser {
    AttributePhaser {
        fixture_ids: vec![fixture_id],
        group_ids: vec![],
        attribute: AttributeKey("pan".into()),
        phaser: Phaser {
            mode: PhaserMode::Absolute,
            steps: vec![PhaserStep {
                position: 0.0,
                value: 0.5,
                curve_to_next: PhaserCurve::Linear,
            }],
            cycles_per_minute: 60.0,
            phase_start_degrees: 0.0,
            phase_end_degrees: 0.0,
            width: 1.0,
        },
    }
}

fn automatic_fixture(fixture_id: FixtureId, attribute: &str, value: Option<f32>) -> CueChange {
    CueChange {
        fixture_id,
        attribute: AttributeKey(attribute.into()),
        value: value.map(AttributeValue::Normalized),
        automatic_restore: true,
        fade_millis: None,
        delay_millis: None,
    }
}

fn automatic_group(group_id: &str, attribute: &str, value: Option<f32>) -> GroupCueChange {
    GroupCueChange {
        group_id: group_id.into(),
        attribute: AttributeKey(attribute.into()),
        value: value.map(AttributeValue::Normalized),
        automatic_restore: true,
        fade_millis: None,
        delay_millis: None,
    }
}

#[test]
fn append_uses_floor_of_maximum_plus_one_and_keeps_sparse_source_order() {
    let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let list = cue_list(vec![cue(1.2, "A", vec![]), cue(3.7, "B", vec![])]);
    let mut first = fixture_change(fixtures[2], "pan", 0.2);
    first.fade_millis = Some(900);
    first.delay_millis = Some(125);
    let mut first_group = group_change("front", "tilt", 0.4);
    first_group.fade_millis = Some(750);
    first_group.delay_millis = Some(80);
    let recorded = CueRecordingContent {
        changes: vec![first, fixture_change(fixtures[0], "intensity", 0.8)],
        group_changes: vec![first_group, group_change("back", "pan", 0.6)],
        ..Default::default()
    };

    let plan = list
        .plan_recording(recorded, CueRecordOperation::Append)
        .unwrap();
    let stored = plan.cue_list.cues.last().unwrap();

    assert!(plan.changed);
    assert_eq!(stored.number, 4.0);
    assert_eq!(stored.name, "");
    assert_eq!(stored.fade_millis, 0);
    assert_eq!(stored.delay_millis, 0);
    assert_eq!(stored.trigger, CueTrigger::Manual);
    assert_eq!(stored.changes.len(), 2);
    assert_eq!(stored.changes[0].fixture_id, fixtures[2]);
    assert_eq!(stored.changes[0].fade_millis, Some(900));
    assert_eq!(stored.changes[0].delay_millis, Some(125));
    assert_eq!(stored.changes[1].fixture_id, fixtures[0]);
    assert_eq!(stored.group_changes[0].group_id, "front");
    assert_eq!(stored.group_changes[0].fade_millis, Some(750));
    assert_eq!(stored.group_changes[0].delay_millis, Some(80));
    assert_eq!(stored.group_changes[1].group_id, "back");
    assert!(
        stored
            .changes
            .iter()
            .all(|change| change.fixture_id != fixtures[1])
    );
}

#[test]
fn new_recording_and_playback_use_backend_canonical_defaults_and_explicit_zeroes() {
    let cue_list_id = CueListId::new();
    let fixture = FixtureId::new();
    let first = CueRecordingContent {
        changes: vec![fixture_change(fixture, "intensity", 0.5)],
        timing: CueRecordingTiming {
            fade_millis: Some(0),
            delay_millis: Some(0),
        },
        name: Some("Opening".into()),
        ..Default::default()
    };

    let plan = CueList::new_recording(cue_list_id, "Cuelist 7", first, None).unwrap();
    let list = &plan.cue_list;
    let first = &list.cues[0];
    assert!(plan.changed);
    assert_eq!(list.priority, 0);
    assert_eq!(list.mode, CueListMode::Sequence);
    assert_eq!(list.wrap_mode, Some(WrapMode::Off));
    assert_eq!(list.chaser_xfade_percent, Some(0));
    assert_eq!(first.number, 1.0);
    assert_eq!(first.name, "Opening");
    assert_eq!(first.fade_millis, 0);
    assert_eq!(first.delay_millis, 0);
    assert_eq!(first.trigger, CueTrigger::Follow { delay_millis: 0 });
    assert!(first.phasers.is_empty());

    let playback = PlaybackDefinition::new_cue_list(7, "Cuelist 7", cue_list_id);
    assert_eq!(
        playback.buttons,
        [
            PlaybackButtonAction::GoMinus,
            PlaybackButtonAction::Go,
            PlaybackButtonAction::Flash,
        ]
    );
    assert_eq!(playback.button_count, 3);
    assert_eq!(playback.fader, PlaybackFaderMode::Master);
}

#[test]
fn overwrite_preserves_existing_identity_and_name_but_clears_phasers() {
    let fixture = FixtureId::new();
    let mut target = cue(2.5, "Keep", vec![fixture_change(fixture, "pan", 0.2)]);
    let target_id = target.id;
    target.fade_millis = 5_000;
    target.delay_millis = 900;
    target.trigger = CueTrigger::Wait { delay_millis: 50 };
    target.cue_only = true;
    target.phasers = vec![phaser(fixture)];
    let list = cue_list(vec![cue(1.0, "First", vec![]), target]);

    let plan = list
        .plan_recording(
            content(vec![fixture_change(fixture, "tilt", 0.7)]),
            CueRecordOperation::Overwrite { cue_number: 2.5 },
        )
        .unwrap();
    let stored = &plan.cue_list.cues[1];

    assert_eq!(stored.id, target_id);
    assert_eq!(stored.name, "Keep");
    assert_eq!(stored.changes[0].attribute, AttributeKey("tilt".into()));
    assert_eq!(stored.fade_millis, 0);
    assert_eq!(stored.delay_millis, 0);
    assert_eq!(stored.trigger, CueTrigger::Manual);
    assert!(!stored.cue_only);
    assert!(stored.phasers.is_empty());

    let named_recording = plan
        .cue_list
        .plan_recording(
            CueRecordingContent {
                changes: vec![fixture_change(fixture, "tilt", 0.7)],
                name: Some("Renamed".into()),
                ..Default::default()
            },
            CueRecordOperation::Overwrite { cue_number: 2.5 },
        )
        .unwrap();
    assert_eq!(named_recording.cue_list.cues[1].id, target_id);
    assert_eq!(named_recording.cue_list.cues[1].name, "Keep");
}

#[test]
fn missing_overwrite_inserts_in_decimal_numeric_order() {
    let fixture = FixtureId::new();
    let list = cue_list(vec![
        cue(1.0, "One", vec![]),
        cue(2.5, "Two point five", vec![]),
    ]);

    let plan = list
        .plan_recording(
            content(vec![fixture_change(fixture, "intensity", 0.4)]),
            CueRecordOperation::Overwrite { cue_number: 1.75 },
        )
        .unwrap();

    assert!(plan.changed);
    assert_eq!(
        plan.cue_list
            .cues
            .iter()
            .map(|cue| cue.number)
            .collect::<Vec<_>>(),
        vec![1.0, 1.75, 2.5]
    );
}

#[test]
fn merge_replaces_only_source_addresses_and_preserves_metadata_and_phasers() {
    let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let mut target = cue(
        2.0,
        "Old",
        vec![
            fixture_change(fixtures[0], "intensity", 0.1),
            fixture_change(fixtures[1], "pan", 0.2),
        ],
    );
    target.fade_millis = 2_000;
    target.cue_only = true;
    target.phasers = vec![phaser(fixtures[0])];
    let target_id = target.id;
    let phasers = target.phasers.clone();
    let list = cue_list(vec![cue(1.0, "First", vec![]), target]);
    let recorded = CueRecordingContent {
        changes: vec![
            fixture_change(fixtures[1], "pan", 0.9),
            fixture_change(fixtures[2], "tilt", 0.7),
        ],
        name: Some("Merged".into()),
        ..Default::default()
    };

    let plan = list
        .plan_recording(
            recorded.clone(),
            CueRecordOperation::Merge { cue_number: 2.0 },
        )
        .unwrap();
    let stored = &plan.cue_list.cues[1];
    assert_eq!(stored.id, target_id);
    assert_eq!(stored.name, "Old");
    assert_eq!(stored.fade_millis, 2_000);
    assert!(stored.cue_only);
    assert_eq!(stored.phasers, phasers);
    assert_eq!(stored.changes.len(), 3);
    assert_eq!(stored.changes[0].fixture_id, fixtures[0]);
    assert_eq!(stored.changes[1].fixture_id, fixtures[1]);
    assert_eq!(stored.changes[2].fixture_id, fixtures[2]);

    let repeated = plan
        .cue_list
        .plan_recording(recorded, CueRecordOperation::Merge { cue_number: 2.0 })
        .unwrap();
    assert!(!repeated.changed);
}

#[test]
fn missing_explicit_merge_and_subtract_are_rejected() {
    let fixture = FixtureId::new();
    let list = cue_list(vec![cue(1.0, "Only", vec![])]);
    let recorded = content(vec![fixture_change(fixture, "pan", 0.4)]);

    assert_eq!(
        list.plan_recording(
            recorded.clone(),
            CueRecordOperation::Merge { cue_number: 2.0 },
        ),
        Err(CueRecordingPlanError::CueDoesNotExist { cue_number: 2.0 })
    );
    assert_eq!(
        list.plan_recording(recorded, CueRecordOperation::Subtract { cue_number: 2.0 },),
        Err(CueRecordingPlanError::CueDoesNotExist { cue_number: 2.0 })
    );
}

#[test]
fn subtract_is_sparse_and_empty_source_deletes_except_for_the_only_cue() {
    let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let mut target = cue(
        2.0,
        "Target",
        vec![
            fixture_change(fixtures[0], "intensity", 0.1),
            fixture_change(fixtures[1], "pan", 0.2),
        ],
    );
    target.phasers = vec![phaser(fixtures[0])];
    let target_id = target.id;
    let list = cue_list(vec![cue(1.0, "First", vec![]), target]);

    let no_match = list
        .plan_recording(
            content(vec![fixture_change(fixtures[2], "tilt", 0.5)]),
            CueRecordOperation::Subtract { cue_number: 2.0 },
        )
        .unwrap();
    assert!(!no_match.changed);

    let subtracted = list
        .plan_recording(
            content(vec![fixture_change(fixtures[1], "pan", 0.0)]),
            CueRecordOperation::Subtract { cue_number: 2.0 },
        )
        .unwrap();
    assert!(subtracted.changed);
    assert_eq!(subtracted.cue_list.cues[1].changes.len(), 1);
    assert_eq!(subtracted.cue_list.cues[1].id, target_id);
    assert_eq!(subtracted.cue_list.cues[1].phasers.len(), 1);

    let deleted = list
        .plan_recording(
            CueRecordingContent::default(),
            CueRecordOperation::Subtract { cue_number: 2.0 },
        )
        .unwrap();
    assert!(deleted.changed && deleted.deleted);
    assert_eq!(deleted.cue_id, target_id);
    assert_eq!(deleted.cue_list.cues.len(), 1);

    assert_eq!(
        deleted.cue_list.plan_recording(
            CueRecordingContent::default(),
            CueRecordOperation::Subtract { cue_number: 1.0 },
        ),
        Err(CueRecordingPlanError::CannotDeleteOnlyCue)
    );
}

#[test]
fn unmatched_subtract_does_not_reorder_existing_automatic_restorations() {
    let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let baseline = cue(
        1.0,
        "Baseline",
        vec![
            fixture_change(fixtures[0], "intensity", 0.1),
            fixture_change(fixtures[1], "pan", 0.2),
        ],
    );
    let mut cue_only = cue(
        2.0,
        "Cue only",
        vec![
            fixture_change(fixtures[0], "intensity", 0.8),
            fixture_change(fixtures[1], "pan", 0.9),
        ],
    );
    cue_only.cue_only = true;
    let mut following = cue(3.0, "Following", vec![]);
    following.changes = vec![
        automatic_fixture(fixtures[1], "pan", Some(0.2)),
        automatic_fixture(fixtures[0], "intensity", Some(0.1)),
    ];
    let list = cue_list(vec![baseline, cue_only, following]);

    let plan = list
        .plan_recording(
            content(vec![fixture_change(fixtures[2], "tilt", 0.5)]),
            CueRecordOperation::Subtract { cue_number: 2.0 },
        )
        .unwrap();

    assert!(!plan.changed);
    assert_eq!(plan.cue_list, list);
}

#[test]
fn merge_active_preserves_phasers_or_appends_when_no_cue_is_active() {
    let fixtures = [FixtureId::new(), FixtureId::new()];
    let mut target = cue(1.0, "Active", vec![fixture_change(fixtures[0], "pan", 0.1)]);
    target.phasers = vec![phaser(fixtures[0])];
    let target_id = target.id;
    let list = cue_list(vec![target]);

    let merged = list
        .plan_recording(
            content(vec![fixture_change(fixtures[1], "tilt", 0.8)]),
            CueRecordOperation::MergeActive {
                active_cue_id: Some(target_id),
            },
        )
        .unwrap();
    assert_eq!(merged.cue_list.cues.len(), 1);
    assert_eq!(merged.cue_list.cues[0].phasers.len(), 1);

    let appended = merged
        .cue_list
        .plan_recording(
            content(vec![fixture_change(fixtures[1], "tilt", 0.8)]),
            CueRecordOperation::MergeActive {
                active_cue_id: None,
            },
        )
        .unwrap();
    assert!(appended.changed);
    assert_eq!(appended.cue_list.cues.len(), 2);
    assert_eq!(appended.cue_number, 2.0);

    let missing = Uuid::new_v4();
    assert_eq!(
        list.plan_recording(
            content(vec![fixture_change(fixtures[1], "tilt", 0.8)]),
            CueRecordOperation::MergeActive {
                active_cue_id: Some(missing),
            },
        ),
        Err(CueRecordingPlanError::ActiveCueDoesNotExist { cue_id: missing })
    );
}

#[test]
fn byte_identical_overwrite_is_no_change_but_append_always_changes() {
    let fixture = FixtureId::new();
    let target = cue(
        1.0,
        "Stable",
        vec![fixture_change(fixture, "intensity", 0.5)],
    );
    let target_id = target.id;
    let list = cue_list(vec![target]);
    let recorded = content(vec![fixture_change(fixture, "intensity", 0.5)]);

    let unchanged = list
        .plan_recording(
            recorded.clone(),
            CueRecordOperation::Overwrite { cue_number: 1.0 },
        )
        .unwrap();
    assert!(!unchanged.changed);
    assert_eq!(unchanged.cue_id, target_id);
    assert_eq!(unchanged.cue_list, list);

    let appended = list
        .plan_recording(recorded, CueRecordOperation::Append)
        .unwrap();
    assert!(appended.changed);
    assert_ne!(appended.cue_id, target_id);
}

#[test]
fn cue_only_restoration_is_regenerated_after_insertion() {
    let fixture = FixtureId::new();
    let mut cue_only = cue(
        1.0,
        "Cue only",
        vec![fixture_change(fixture, "intensity", 0.8)],
    );
    cue_only.cue_only = true;
    let mut following = cue(2.0, "Following", vec![]);
    following.changes = vec![automatic_fixture(fixture, "intensity", None)];
    let list = cue_list(vec![cue_only, following]);

    let inserted = list
        .plan_recording(
            content(vec![fixture_change(FixtureId::new(), "pan", 0.5)]),
            CueRecordOperation::Overwrite { cue_number: 1.5 },
        )
        .unwrap();

    assert_eq!(inserted.cue_list.cues[1].number, 1.5);
    assert!(inserted.cue_list.cues[1].changes[1].automatic_restore);
    assert_eq!(inserted.cue_list.cues[1].changes[1].fixture_id, fixture);
    assert!(inserted.cue_list.cues[1].changes[1].value.is_none());
    assert!(inserted.cue_list.cues[2].changes.is_empty());
}

#[test]
fn cue_only_restoration_is_regenerated_after_overwrite() {
    let fixture = FixtureId::new();
    let mut cue_only = cue(
        1.0,
        "Cue only",
        vec![fixture_change(fixture, "intensity", 0.8)],
    );
    cue_only.cue_only = true;
    let mut following = cue(2.0, "Following", vec![]);
    following.changes = vec![automatic_fixture(fixture, "intensity", None)];
    let list = cue_list(vec![cue_only, following]);

    let overwritten = list
        .plan_recording(
            content(vec![fixture_change(FixtureId::new(), "pan", 0.5)]),
            CueRecordOperation::Overwrite { cue_number: 1.0 },
        )
        .unwrap();

    assert!(!overwritten.cue_list.cues[0].cue_only);
    assert!(overwritten.cue_list.cues[1].changes.is_empty());
}

#[test]
fn cue_only_fixture_and_group_restoration_is_regenerated_after_subtract() {
    let fixtures = [FixtureId::new(), FixtureId::new()];
    let mut baseline = cue(
        1.0,
        "Baseline",
        vec![
            fixture_change(fixtures[0], "intensity", 0.1),
            fixture_change(fixtures[1], "pan", 0.2),
        ],
    );
    baseline.group_changes = vec![group_change("front", "tilt", 0.3)];
    let mut cue_only = cue(
        2.0,
        "Cue only",
        vec![
            fixture_change(fixtures[0], "intensity", 0.8),
            fixture_change(fixtures[1], "pan", 0.9),
        ],
    );
    cue_only.cue_only = true;
    cue_only.group_changes = vec![group_change("front", "tilt", 0.7)];
    let mut following = cue(3.0, "Following", vec![]);
    following.changes = vec![
        automatic_fixture(fixtures[0], "intensity", Some(0.1)),
        automatic_fixture(fixtures[1], "pan", Some(0.2)),
    ];
    following.group_changes = vec![automatic_group("front", "tilt", Some(0.3))];
    let list = cue_list(vec![baseline, cue_only, following]);

    let subtracted = list
        .plan_recording(
            content(vec![fixture_change(fixtures[1], "pan", 0.0)]),
            CueRecordOperation::Subtract { cue_number: 2.0 },
        )
        .unwrap();
    let restores = &subtracted.cue_list.cues[2];

    assert_eq!(restores.changes.len(), 1);
    assert_eq!(restores.changes[0].fixture_id, fixtures[0]);
    assert_eq!(
        restores.changes[0].value,
        Some(AttributeValue::Normalized(0.1))
    );
    assert!(restores.changes[0].automatic_restore);
    assert_eq!(restores.group_changes.len(), 1);
    assert_eq!(
        restores.group_changes[0].value,
        Some(AttributeValue::Normalized(0.3))
    );
}

#[test]
fn cue_only_restoration_is_regenerated_after_delete() {
    let fixture = FixtureId::new();
    let baseline = cue(
        1.0,
        "Baseline",
        vec![fixture_change(fixture, "intensity", 0.1)],
    );
    let mut cue_only = cue(
        2.0,
        "Cue only",
        vec![fixture_change(fixture, "intensity", 0.8)],
    );
    cue_only.cue_only = true;
    let mut following = cue(3.0, "Following", vec![]);
    following.changes = vec![automatic_fixture(fixture, "intensity", Some(0.1))];
    let list = cue_list(vec![baseline, cue_only, following]);

    let deleted = list
        .plan_recording(
            CueRecordingContent::default(),
            CueRecordOperation::Subtract { cue_number: 2.0 },
        )
        .unwrap();

    assert!(deleted.deleted);
    assert_eq!(deleted.cue_list.cues.len(), 2);
    assert!(deleted.cue_list.cues[1].changes.is_empty());
}
