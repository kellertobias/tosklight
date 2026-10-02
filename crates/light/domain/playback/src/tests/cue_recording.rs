use super::*;
use light_dynamics::{DynamicSemanticValue, DynamicValueTiming};

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
    let mut cue = Cue::new(cue_number(number));
    cue.name = name.into();
    cue.changes = changes;
    cue
}

fn fixed_at(fixture_id: FixtureId, value: f32) -> CueDynamicChange {
    CueDynamicChange {
        fixture_id,
        attribute: AttributeKey::intensity(),
        value: DynamicSemanticValue::FixAt {
            value,
            timing: DynamicValueTiming::default(),
        },
        automatic_restore: false,
    }
}

fn position_mask(
    fixture_id: FixtureId,
    component: Option<light_core::programming::ProgrammingComponent>,
    pan: f32,
    tilt: f32,
) -> CueDynamicChange {
    CueDynamicChange {
        fixture_id,
        attribute: AttributeKey("position".into()),
        automatic_restore: false,
        value: DynamicSemanticValue::ProgrammingFixAt {
            mask: light_dynamics::ProgrammingFamilyFixAt {
                address: light_dynamics::DynamicValueAddress {
                    representation: light_dynamics::DynamicFamilyRepresentation::Angles,
                    component,
                },
                family: AttributeValue::Position(std::sync::Arc::new(
                    light_core::programming::PositionIntent::angles(pan, tilt),
                )),
            },
            timing: DynamicValueTiming::default(),
        },
    }
}

#[test]
fn cue_only_component_and_whole_holds_release_only_the_introduced_track() {
    use light_core::programming::ProgrammingComponent;
    for component in [Some(ProgrammingComponent::Pan), None] {
        let fixture = FixtureId::new();
        let tilt = position_mask(fixture, Some(ProgrammingComponent::Tilt), 10.0, -30.0);
        let mut baseline = cue(1.0, "Tilt hold", vec![]);
        baseline.dynamic_changes = vec![tilt.clone()];
        let mut temporary = cue(2.0, "Temporary", vec![]);
        temporary.cue_only = true;
        temporary.dynamic_changes = vec![position_mask(fixture, component, 720.0, 90.0)];
        let mut list = cue_list(vec![baseline, temporary, cue(3.0, "Continue Tilt", vec![])]);
        refresh_cue_only_restorations(&mut list);
        let restored = &list.cues[2].dynamic_changes;
        assert_eq!(restored.len(), 1);
        assert!(restored[0].automatic_restore);
        assert_eq!(
            restored[0].value,
            DynamicSemanticValue::ProgrammingRelease { component }
        );
        let saved = serde_json::to_string(&list).unwrap();
        refresh_cue_only_restorations(&mut list);
        assert_eq!(serde_json::to_string(&list).unwrap(), saved);
        let id = list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(list).unwrap();
        for expected in [1, 2, 1] {
            engine.go_at(id, Utc::now()).unwrap();
            assert_eq!(engine.active_cue_dynamic_values().len(), expected);
        }
        assert_eq!(engine.active_cue_dynamic_values()[0].value, tilt.value);
    }
}

#[test]
fn cue_only_owner_release_restores_every_component_hold() {
    use light_core::programming::ProgrammingComponent;
    let fixture = FixtureId::new();
    let baseline_values = vec![
        position_mask(fixture, Some(ProgrammingComponent::Pan), 720.0, 10.0),
        position_mask(fixture, Some(ProgrammingComponent::Tilt), 20.0, -30.0),
    ];
    let mut baseline = cue(1.0, "Pair", vec![]);
    baseline.dynamic_changes = baseline_values.clone();
    let mut temporary = cue(2.0, "Release owner", vec![]);
    temporary.cue_only = true;
    temporary.dynamic_changes = vec![CueDynamicChange {
        fixture_id: fixture,
        attribute: AttributeKey("position".into()),
        value: DynamicSemanticValue::Release,
        automatic_restore: false,
    }];
    let mut list = cue_list(vec![baseline, temporary, cue(3.0, "Restore pair", vec![])]);
    refresh_cue_only_restorations(&mut list);
    assert_eq!(list.cues[2].dynamic_changes.len(), 2);
    let id = list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(list).unwrap();
    for expected in [2, 0, 2] {
        engine.go_at(id, Utc::now()).unwrap();
        assert_eq!(engine.active_cue_dynamic_values().len(), expected);
    }
    let result = engine.active_cue_dynamic_values();
    for expected in baseline_values {
        assert!(result.iter().any(|value| value.value == expected.value));
    }
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
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues,
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
    assert_eq!(stored.number, cue_number(4.0));
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

    let plan = CueList::new_recording(cue_list_id, "Cuelist 7", first, None, false, false).unwrap();
    let list = &plan.cue_list;
    let first = &list.cues[0];
    assert!(plan.changed);
    assert_eq!(list.priority, 0);
    assert_eq!(list.mode, CueListMode::Sequence);
    assert_eq!(list.wrap_mode, Some(WrapMode::Off));
    assert_eq!(list.chaser_xfade_percent, Some(0));
    assert_eq!(first.number, cue_number(1.0));
    assert_eq!(first.name, "Opening");
    assert_eq!(first.fade_millis, 0);
    assert_eq!(first.delay_millis, 0);
    assert_eq!(first.trigger, CueTrigger::Follow { delay_millis: 0 });
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
fn overwrite_preserves_existing_identity_and_name() {
    let fixture = FixtureId::new();
    let mut target = cue(2.5, "Keep", vec![fixture_change(fixture, "pan", 0.2)]);
    let target_id = target.id;
    target.fade_millis = 5_000;
    target.delay_millis = 900;
    target.trigger = CueTrigger::Wait { delay_millis: 50 };
    target.cue_only = true;
    let list = cue_list(vec![cue(1.0, "First", vec![]), target]);

    let plan = list
        .plan_recording(
            content(vec![fixture_change(fixture, "tilt", 0.7)]),
            CueRecordOperation::Overwrite {
                cue_number: cue_number(2.5),
            },
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

    let named_recording = plan
        .cue_list
        .plan_recording(
            CueRecordingContent {
                changes: vec![fixture_change(fixture, "tilt", 0.7)],
                name: Some("Renamed".into()),
                ..Default::default()
            },
            CueRecordOperation::Overwrite {
                cue_number: cue_number(2.5),
            },
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
            CueRecordOperation::Overwrite {
                cue_number: cue_number(1.75),
            },
        )
        .unwrap();

    assert!(plan.changed);
    assert_eq!(
        plan.cue_list
            .cues
            .iter()
            .map(|cue| cue.number.clone())
            .collect::<Vec<_>>(),
        vec![cue_number(1.0), cue_number(1.75), cue_number(2.5)]
    );
}

#[test]
fn merge_replaces_only_source_addresses_and_preserves_metadata() {
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
    let target_id = target.id;
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
            CueRecordOperation::Merge {
                cue_number: cue_number(2.0),
            },
        )
        .unwrap();
    let stored = &plan.cue_list.cues[1];
    assert_eq!(stored.id, target_id);
    assert_eq!(stored.name, "Old");
    assert_eq!(stored.fade_millis, 2_000);
    assert!(stored.cue_only);
    assert_eq!(stored.changes.len(), 3);
    assert_eq!(stored.changes[0].fixture_id, fixtures[0]);
    assert_eq!(stored.changes[1].fixture_id, fixtures[1]);
    assert_eq!(stored.changes[2].fixture_id, fixtures[2]);

    let repeated = plan
        .cue_list
        .plan_recording(
            recorded,
            CueRecordOperation::Merge {
                cue_number: cue_number(2.0),
            },
        )
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
            CueRecordOperation::Merge {
                cue_number: cue_number(2.0)
            },
        ),
        Err(CueRecordingPlanError::CueDoesNotExist {
            cue_number: cue_number(2.0)
        })
    );
    assert_eq!(
        list.plan_recording(
            recorded,
            CueRecordOperation::Subtract {
                cue_number: cue_number(2.0)
            },
        ),
        Err(CueRecordingPlanError::CueDoesNotExist {
            cue_number: cue_number(2.0)
        })
    );
}

#[test]
fn subtract_is_sparse_and_empty_source_deletes_except_for_the_only_cue() {
    let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let target = cue(
        2.0,
        "Target",
        vec![
            fixture_change(fixtures[0], "intensity", 0.1),
            fixture_change(fixtures[1], "pan", 0.2),
        ],
    );
    let target_id = target.id;
    let list = cue_list(vec![cue(1.0, "First", vec![]), target]);

    let no_match = list
        .plan_recording(
            content(vec![fixture_change(fixtures[2], "tilt", 0.5)]),
            CueRecordOperation::Subtract {
                cue_number: cue_number(2.0),
            },
        )
        .unwrap();
    assert!(!no_match.changed);

    let subtracted = list
        .plan_recording(
            content(vec![fixture_change(fixtures[1], "pan", 0.0)]),
            CueRecordOperation::Subtract {
                cue_number: cue_number(2.0),
            },
        )
        .unwrap();
    assert!(subtracted.changed);
    assert_eq!(subtracted.cue_list.cues[1].changes.len(), 1);
    assert_eq!(subtracted.cue_list.cues[1].id, target_id);

    let deleted = list
        .plan_recording(
            CueRecordingContent::default(),
            CueRecordOperation::Subtract {
                cue_number: cue_number(2.0),
            },
        )
        .unwrap();
    assert!(deleted.changed && deleted.deleted);
    assert_eq!(deleted.cue_id, target_id);
    assert_eq!(deleted.cue_list.cues.len(), 1);

    assert_eq!(
        deleted.cue_list.plan_recording(
            CueRecordingContent::default(),
            CueRecordOperation::Subtract {
                cue_number: cue_number(1.0)
            },
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
            CueRecordOperation::Subtract {
                cue_number: cue_number(2.0),
            },
        )
        .unwrap();

    assert!(!plan.changed);
    assert_eq!(plan.cue_list, list);
}

#[test]
fn merge_active_updates_the_active_cue_or_appends_when_no_cue_is_active() {
    let fixtures = [FixtureId::new(), FixtureId::new()];
    let target = cue(1.0, "Active", vec![fixture_change(fixtures[0], "pan", 0.1)]);
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
    assert_eq!(appended.cue_number, cue_number(2.0));

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
            CueRecordOperation::Overwrite {
                cue_number: cue_number(1.0),
            },
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
            CueRecordOperation::Overwrite {
                cue_number: cue_number(1.5),
            },
        )
        .unwrap();

    assert_eq!(inserted.cue_list.cues[1].number, cue_number(1.5));
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
            CueRecordOperation::Overwrite {
                cue_number: cue_number(1.0),
            },
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
            CueRecordOperation::Subtract {
                cue_number: cue_number(2.0),
            },
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
            CueRecordOperation::Subtract {
                cue_number: cue_number(2.0),
            },
        )
        .unwrap();

    assert!(deleted.deleted);
    assert_eq!(deleted.cue_list.cues.len(), 2);
    assert!(deleted.cue_list.cues[1].changes.is_empty());
}

#[test]
fn cue_only_dynamic_layer_restores_prior_fat_without_touching_static_tracking() {
    let fixture = FixtureId::new();
    let mut baseline = cue(
        1.0,
        "Baseline",
        vec![fixture_change(fixture, "intensity", 0.3)],
    );
    baseline.dynamic_changes = vec![fixed_at(fixture, 0.4)];
    let cue_only = cue(2.0, "Cue only", vec![]);
    let following = cue(3.0, "Following", vec![]);
    let list = cue_list(vec![baseline, cue_only, following]);
    let recording = CueRecordingContent {
        dynamic_changes: vec![fixed_at(fixture, 0.8)],
        cue_only: true,
        ..Default::default()
    };

    let plan = list
        .plan_recording(
            recording,
            CueRecordOperation::Overwrite {
                cue_number: cue_number(2.0),
            },
        )
        .unwrap();

    assert_eq!(plan.cue_list.cues[0].changes.len(), 1);
    let restore = &plan.cue_list.cues[2].dynamic_changes;
    assert_eq!(restore.len(), 1);
    assert!(restore[0].automatic_restore);
    assert!(matches!(
        restore[0].value,
        DynamicSemanticValue::FixAt { value: 0.4, .. }
    ));
}

#[test]
fn authored_release_records_both_ordinary_and_scalar_track_removal_without_zero() {
    let fixture = FixtureId::new();
    let attribute = AttributeKey::intensity();
    let mut first = cue(
        1.0,
        "Owned",
        vec![fixture_change(fixture, "intensity", 1.0)],
    );
    first.dynamic_changes = vec![fixed_at(fixture, 0.8)];
    let list = cue_list(vec![first]);
    let release = CueRecordingContent {
        changes: vec![CueChange {
            fixture_id: fixture,
            attribute: attribute.clone(),
            value: None,
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        }],
        dynamic_changes: vec![CueDynamicChange {
            fixture_id: fixture,
            attribute: attribute.clone(),
            value: DynamicSemanticValue::Release,
            automatic_restore: false,
        }],
        ..Default::default()
    };

    let plan = list
        .plan_recording(release, CueRecordOperation::Append)
        .unwrap();
    let stored = &plan.cue_list.cues[1];
    assert_eq!(stored.changes.len(), 1);
    assert_eq!(stored.changes[0].value, None);
    assert!(matches!(
        stored.dynamic_changes[0].value,
        DynamicSemanticValue::Release
    ));
    assert!(
        !plan
            .cue_list
            .state_at_index(1)
            .contains_key(&(fixture, attribute))
    );
}

#[test]
fn add_missing_adds_only_addresses_the_cue_does_not_store() {
    let fixtures = [FixtureId::new(), FixtureId::new()];
    let mut target = cue(1.0, "Look", vec![fixture_change(fixtures[0], "pan", 0.1)]);
    target.group_changes = vec![group_change("7", "dimmer", 0.2)];
    target.dynamic_changes = vec![fixed_at(fixtures[0], 0.3)];
    let target_id = target.id;
    let list = cue_list(vec![target]);
    let recorded = CueRecordingContent {
        changes: vec![
            fixture_change(fixtures[0], "pan", 0.9),
            fixture_change(fixtures[1], "tilt", 0.5),
        ],
        group_changes: vec![
            group_change("7", "dimmer", 0.9),
            group_change("8", "dimmer", 0.4),
        ],
        dynamic_changes: vec![fixed_at(fixtures[0], 0.9), fixed_at(fixtures[1], 0.6)],
        ..Default::default()
    };

    let numbered = list
        .plan_recording(
            recorded.clone(),
            CueRecordOperation::AddMissing {
                cue_number: cue_number(1.0),
            },
        )
        .unwrap();
    let stored = &numbered.cue_list.cues[0];
    assert_eq!(stored.id, target_id);
    assert_eq!(stored.changes.len(), 2);
    assert_eq!(
        stored.changes[0].value,
        Some(AttributeValue::Normalized(0.1))
    );
    assert_eq!(stored.changes[1].fixture_id, fixtures[1]);
    assert_eq!(stored.group_changes.len(), 2);
    assert_eq!(
        stored.group_changes[0].value,
        Some(AttributeValue::Normalized(0.2))
    );
    assert_eq!(stored.dynamic_changes.len(), 2);
    assert_eq!(stored.dynamic_changes[0], fixed_at(fixtures[0], 0.3));

    let active = list
        .plan_recording(
            recorded.clone(),
            CueRecordOperation::AddMissingActive {
                active_cue_id: Some(target_id),
            },
        )
        .unwrap();
    assert_eq!(active.cue_list, numbered.cue_list);

    let inactive = list
        .plan_recording(
            recorded.clone(),
            CueRecordOperation::AddMissingActive {
                active_cue_id: None,
            },
        )
        .unwrap();
    assert_eq!(inactive.cue_list.cues.len(), 2);
    assert_eq!(inactive.cue_number, cue_number(2.0));

    assert_eq!(
        list.plan_recording(
            recorded,
            CueRecordOperation::AddMissing {
                cue_number: cue_number(4.0)
            },
        ),
        Err(CueRecordingPlanError::CueDoesNotExist {
            cue_number: cue_number(4.0)
        })
    );
}

#[test]
fn insert_stores_a_new_cue_and_never_replaces_one() {
    let fixture = FixtureId::new();
    let list = cue_list(vec![cue(
        1.0,
        "Only",
        vec![fixture_change(fixture, "pan", 0.1)],
    )]);
    let recorded = content(vec![fixture_change(fixture, "pan", 0.4)]);

    let inserted = list
        .plan_recording(
            recorded.clone(),
            CueRecordOperation::Insert {
                cue_number: cue_number(1.5),
            },
        )
        .unwrap();
    assert_eq!(inserted.cue_list.cues.len(), 2);
    assert_eq!(inserted.cue_list.cues[1].number, cue_number(1.5));
    assert_eq!(
        list.plan_recording(
            recorded,
            CueRecordOperation::Insert {
                cue_number: cue_number(1.0)
            },
        ),
        Err(CueRecordingPlanError::CueAlreadyExists {
            cue_number: cue_number(1.0)
        })
    );
}

fn lane_on(instance: Uuid, lane: u128) -> light_dynamics::DynamicSemanticValue {
    serde_json::from_value(serde_json::json!({
        "type": "dynamic_on", "instance_link": instance, "lane_id": Uuid::from_u128(lane),
        "dynamic": { "dynamic_id": null, "last_known_pool_number": 1,
            "embedded_fallback": { "definition": {
                "id": Uuid::from_u128(100), "pool_number": 1, "revision": 1, "name": "Position",
                "target_binding": {"type":"targetless"}, "lanes": ([1, 2].map(|id| serde_json::json!({
                    "id": Uuid::from_u128(id), "speed_multiplier": {"numerator":1,"denominator":1}, "width":1.0,
                    "programming": {"address":{"representation":{"kind":"angles"},"component": {"kind": if id == 1 { "pan" } else { "tilt" }}},
                        "configuration":{"mode":"keyframes","configuration":{"points":[
                            {"position":0.0,"source":{"kind":"value","value":{"kind":"scalar","value":45.0}},"interpolation":"linear"}
                        ],"size":1.0}}}
                }))),
                "phase": {"ordering":{"type":"selection"},"offset_degrees":0,"span_degrees":360,"block_size":1,"repeats":1,"wings":false},
                "speed":{"type":"fixed","duration_millis":1000},"default_activation":"start_now"
            }}
        }, "overrides":{"size":1.0,"speed_multiplier":{"numerator":1,"denominator":1},"phase_offset_degrees":0},
        "timing":{}
    })).unwrap()
}

#[test]
fn dynamic_lane_provenance_tracks_each_authored_cue_and_instance_off_replacement() {
    let fixture = FixtureId::new();
    let instance = Uuid::new_v4();
    let change = |value| CueDynamicChange {
        fixture_id: fixture,
        attribute: AttributeKey("position".into()),
        value,
        automatic_restore: false,
    };
    let mut first = cue(1.0, "Pan", vec![]);
    first.dynamic_changes.push(change(lane_on(instance, 1)));
    let mut second = cue(2.0, "Tilt", vec![]);
    second.dynamic_changes.push(change(lane_on(instance, 2)));
    let mut third = cue(3.0, "Off", vec![]);
    third
        .dynamic_changes
        .push(change(DynamicSemanticValue::DynamicOff {
            instance_link: instance,
            timing: Default::default(),
        }));
    let mut fourth = cue(4.0, "Pan again", vec![]);
    fourth.dynamic_changes.push(change(lane_on(instance, 1)));
    let ids = [first.id, second.id, third.id, fourth.id];
    let list = cue_list(vec![first, second, third, fourth]);
    let list_id = list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(list).unwrap();
    engine.go_at(list_id, Utc::now()).unwrap();
    engine.go_at(list_id, Utc::now()).unwrap();
    let tracked = engine.active_cue_dynamic_values();
    assert_eq!(tracked.len(), 2);
    for (lane, authored) in [(1, ids[0]), (2, ids[1])] {
        let row = tracked
            .iter()
            .find(|row| row.value.track_key().lane_id == Some(Uuid::from_u128(lane)))
            .unwrap();
        assert_eq!(row.authored_cue_id, authored);
        assert_eq!(row.current_cue_id, ids[1]);
    }
    engine.go_at(list_id, Utc::now()).unwrap();
    let off = engine.active_cue_dynamic_values();
    assert_eq!(off.len(), 1);
    assert_eq!(off[0].authored_cue_id, ids[2]);
    assert!(matches!(
        off[0].value,
        DynamicSemanticValue::DynamicOff { .. }
    ));
    engine.go_at(list_id, Utc::now()).unwrap();
    let resumed = engine.active_cue_dynamic_values();
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].authored_cue_id, ids[3]);
    assert_eq!(
        resumed[0].value.track_key().lane_id,
        Some(Uuid::from_u128(1))
    );
}

#[test]
fn cue_only_restores_complete_multifixture_lane_set_before_following_explicit_changes() {
    let fixtures = [FixtureId::new(), FixtureId::new()];
    let instance = Uuid::new_v4();
    let on = |fixture, lane| CueDynamicChange {
        fixture_id: fixture,
        attribute: AttributeKey("position".into()),
        value: lane_on(instance, lane),
        automatic_restore: false,
    };
    let baseline_values = vec![on(fixtures[0], 1), on(fixtures[0], 2), on(fixtures[1], 1)];
    let mut baseline = cue(1.0, "Baseline", vec![]);
    baseline.dynamic_changes = baseline_values.clone();
    let mut temporary = cue(2.0, "Temporary", vec![]);
    temporary.cue_only = true;
    temporary.dynamic_changes = vec![on(fixtures[1], 2)];
    let mut consecutive = cue(3.0, "Second temporary", vec![]);
    consecutive.cue_only = true;
    consecutive.dynamic_changes = vec![on(fixtures[1], 2)];
    let following = cue(4.0, "Restored", vec![]);
    let mut list = cue_list(vec![baseline, temporary, consecutive, following]);
    refresh_cue_only_restorations(&mut list);
    let snapshot = serde_json::to_string(&list).unwrap();
    refresh_cue_only_restorations(&mut list);
    assert_eq!(
        serde_json::to_string(&list).unwrap(),
        snapshot,
        "regeneration must be idempotent"
    );
    let id = list.id;
    let now = Utc::now();
    let mut engine = PlaybackEngine::default();
    engine.register(list).unwrap();
    for expected in [3, 4, 4, 3] {
        engine.go_at(id, now).unwrap();
        let values = engine.active_cue_dynamic_values();
        assert_eq!(values.len(), expected);
        assert!(
            values
                .iter()
                .all(|value| matches!(value.value, DynamicSemanticValue::DynamicOn { .. }))
        );
    }
    let values = engine.active_cue_dynamic_values();
    for expected in baseline_values {
        assert!(
            values
                .iter()
                .any(|value| value.fixture_id == expected.fixture_id
                    && value.value == expected.value)
        );
    }
}
