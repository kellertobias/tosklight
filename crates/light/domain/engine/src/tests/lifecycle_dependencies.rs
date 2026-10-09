use super::*;
use light_dynamics::{
    ActivationBoundary, ActivationPolicy, DynamicDefinition, DynamicDefinitionSnapshot,
    DynamicPhaseSpreadMode, DynamicReference, DynamicRunMode, DynamicSemanticValue, DynamicSpeed,
    DynamicTargetBinding, PhaseDistribution, PhaseOrdering, Rational,
};
use light_playback::{
    CueDynamicChange, DynamicPlaybackAssignment, DynamicPlaybackFaderMode,
    DynamicPlaybackResumePolicy, GroupCueChange,
};
use uuid::Uuid;

fn setup() -> (Engine, Arc<ManualClock>, FixtureId, FixtureId) {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 9, 29, 12, 0, 0).unwrap(),
    ));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let (first, first_id) = fixture();
    let (mut second, second_id) = fixture();
    second.address = Some(2);
    let mut cue_list = test_cue_list("Candidate inputs", vec![]);
    cue_list.cues[0].dynamic_changes.push(CueDynamicChange {
        fixture_id: first_id,
        attribute: AttributeKey::intensity(),
        value: DynamicSemanticValue::FixAt {
            value: 0.2,
            timing: Default::default(),
        },
        automatic_restore: false,
    });
    cue_list.cues[0].group_changes.push(GroupCueChange {
        preset_reference: None,
        group_id: "front".into(),
        attribute: AttributeKey::intensity(),
        value: Some(AttributeValue::Normalized(0.6)),
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    });
    let dynamic = DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 1,
        revision: 1,
        name: "Group-bound Dynamic".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::LiveGroup {
            group_id: "front".into(),
        },
        lanes: vec![light_dynamics::DynamicLane {
            id: Uuid::new_v4(),
            body: light_dynamics::DynamicLaneBody::LegacyScalar(
                light_dynamics::LegacyScalarLaneBody {
                    attribute: AttributeKey::intensity(),
                    mode: light_dynamics::DynamicLaneMode::MaxMin,
                    keyframes: light_dynamics::KeyframeConfiguration {
                        points: [0.0, 0.5]
                            .into_iter()
                            .map(|position| light_dynamics::DynamicKeyframe {
                                position,
                                source: light_dynamics::ScalarSource::Value { value: position },
                                interpolation: light_dynamics::ScalarInterpolation::Linear,
                            })
                            .collect(),
                        size: 1.0,
                    },
                    max_min: light_dynamics::MaxMinConfiguration {
                        minimum: light_dynamics::ScalarSource::Value { value: 0.0 },
                        maximum: light_dynamics::ScalarSource::Value { value: 1.0 },
                        function: light_dynamics::PeriodicFunction::Sinus,
                        size: 1.0,
                        pwm: Default::default(),
                    },
                    middle_amplitude: light_dynamics::MiddleAmplitudeConfiguration {
                        middle: light_dynamics::ScalarSource::Value { value: 0.5 },
                        amplitude: 0.5,
                        function: light_dynamics::PeriodicFunction::Sinus,
                        size: 1.0,
                        pwm: Default::default(),
                        invert_waveform: false,
                    },
                },
            ),
            speed_multiplier: Rational::ONE,
            width: 1.0,
            phase: None,
            random_group_id: None,
        }],
        random_groups: Vec::new(),
        phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: Default::default(),
        phase: PhaseDistribution {
            ordering: PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: Vec::new(),
        },
        speed: DynamicSpeed::Fixed {
            duration_millis: 1_000,
        },
        overall_speed_multiplier: Rational::ONE,
        run_mode: DynamicRunMode::Loop,
        default_activation: ActivationPolicy::StartNow,
        activation_boundary: ActivationBoundary::Beat,
    };
    let assignment = DynamicPlaybackAssignment {
        dynamic: DynamicReference {
            dynamic_id: Some(dynamic.id),
            last_known_pool_number: 1,
            embedded_fallback: DynamicDefinitionSnapshot {
                definition: Arc::new(dynamic.clone()),
            },
        },
        revision: 1,
        target_scope: None,
        fader_mode: DynamicPlaybackFaderMode::SizeAndMaster,
        priority: 0,
        activation_override: None,
        resume_policy: DynamicPlaybackResumePolicy::FollowDynamic,
        local_speed_multiplier: Rational::ONE,
        learned_duration_millis: None,
        crossfade_non_intensity: false,
        auto_off_at_zero: true,
        auto_off_flash_release: true,
        auto_off_full_control: true,
    };
    cue_list.cues[0].dynamic_changes[0].value = DynamicSemanticValue::DynamicOn {
        instance_link: Uuid::new_v4(),
        dynamic: assignment.dynamic.clone(),
        lane_id: dynamic.lanes[0].id,
        overrides: light_dynamics::DynamicInstanceOverrides {
            size: 1.0,
            speed_multiplier: Rational::ONE,
            phase_offset_degrees: 0.0,
        },
        timing: Default::default(),
    };
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![first, second].into(),
            playbacks: vec![
                test_playback(1, cue_list.id),
                test_playback_for_target(2, PlaybackTarget::Dynamic { assignment }),
            ]
            .into(),
            cue_lists: vec![cue_list].into(),
            groups: vec![GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                fixtures: vec![first_id],
                ..Default::default()
            }]
            .into(),
            dynamics: vec![dynamic].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::On);
    execute_pool(&engine, 2, PoolPlaybackAction::On);
    (engine, clock, first_id, second_id)
}

fn finalize(engine: &Engine, snapshot: EngineSnapshot, preserve: bool) -> FinalizedEngineSnapshot {
    engine
        .finalize_snapshot_playback(engine.prepare_snapshot(snapshot).unwrap(), preserve)
        .unwrap()
}

#[test]
fn cue_and_group_edits_publish_the_same_candidate_inputs_that_were_finalized() {
    let (engine, clock, first, second) = setup();
    engine
        .execute_playback(EnginePlaybackCommand::SetDynamicsPaused(true))
        .unwrap();
    let original = engine.snapshot();
    let old_rows = engine.active_cue_dynamic_values();
    let old_runtime = engine.playback_runtime();
    let mut candidate = (*original).clone();
    candidate.revision += 1;
    Arc::make_mut(&mut candidate.groups)[0].fixtures = vec![second];
    let change = &mut Arc::make_mut(&mut candidate.cue_lists)[0].cues[0].dynamic_changes[0];
    change.fixture_id = second;
    change.value = DynamicSemanticValue::FixAt {
        value: 0.9,
        timing: Default::default(),
    };
    let finalized = finalize(&engine, candidate, true);
    assert_eq!(finalized.sampled_at(), clock.now());
    assert_eq!(finalized.snapshot().groups[0].fixtures, vec![second]);
    assert_eq!(finalized.cue_dynamic_values().len(), 1);
    assert_eq!(finalized.cue_dynamic_values()[0].fixture_id, second);
    assert!(matches!(
        finalized.cue_dynamic_values()[0].value,
        DynamicSemanticValue::FixAt { value: 0.9, .. }
    ));
    assert!(finalized.playback_dynamics_paused());
    assert_eq!(finalized.dynamic_playbacks().len(), 1);
    assert!(Arc::ptr_eq(&original, &engine.snapshot()));
    assert_eq!(engine.active_cue_dynamic_values(), old_rows);
    assert_eq!(engine.playback_runtime(), old_runtime);
    let cue_rows = finalized.cue_dynamic_values().to_vec();
    let dynamic_rows = finalized.dynamic_playbacks().to_vec();
    let exact_snapshot = finalized.snapshot_arc();
    let installed: () = engine.install_finalized_snapshot(finalized);
    assert_eq!(installed, ());
    assert!(Arc::ptr_eq(&exact_snapshot, &engine.snapshot()));
    assert_eq!(engine.active_cue_dynamic_values(), cue_rows);
    assert_eq!(engine.active_dynamic_playbacks(), dynamic_rows);
    let contributions = engine.playback_contributions_at(clock.now());
    assert!(contributions.iter().any(|row| {
        row.value.fixture_id == second && row.value.value == AttributeValue::Normalized(0.6)
    }));
    assert!(
        !contributions
            .iter()
            .any(|row| row.value.fixture_id == first)
    );
}

#[test]
fn candidate_assignment_moves_and_removals_reconcile_before_publication() {
    for remove in [false, true] {
        let (engine, _, _, _) = setup();
        let mut candidate = (*engine.snapshot()).clone();
        candidate.revision += 1;
        if remove {
            candidate.playbacks = Arc::default();
        } else {
            let definitions = Arc::make_mut(&mut candidate.playbacks);
            definitions[0].number = 3;
            definitions[1].number = 7;
        }
        let finalized = finalize(&engine, candidate, true);
        if remove {
            assert!(finalized.cue_dynamic_values().is_empty());
            assert!(finalized.dynamic_playbacks().is_empty());
        } else {
            assert_eq!(finalized.cue_dynamic_values()[0].playback_number, Some(3));
            assert_eq!(finalized.dynamic_playbacks()[0].playback_number, 7);
        }
        assert_eq!(
            engine.active_cue_dynamic_values()[0].playback_number,
            Some(1)
        );
        assert_eq!(engine.active_dynamic_playbacks()[0].playback_number, 2);
        let cues = finalized.cue_dynamic_values().to_vec();
        let dynamics = finalized.dynamic_playbacks().to_vec();
        engine.install_finalized_snapshot(finalized);
        assert_eq!(engine.active_cue_dynamic_values(), cues);
        assert_eq!(engine.active_dynamic_playbacks(), dynamics);
    }
}

#[test]
fn shared_playback_is_detached_and_release_clears_normal_temporary_and_dynamic_state() {
    for preserve in [false, true] {
        let (engine, clock, _, _) = setup();
        execute_pool(&engine, 1, PoolPlaybackAction::SetTempButton(true));
        engine
            .execute_playback(EnginePlaybackCommand::SetDynamicsPaused(true))
            .unwrap();
        engine.reserve_playback_source_occurrence_watermark(200);
        let live_playback = engine.generation.load_full().playback_arc();
        let old_cues = engine.active_cue_dynamic_values();
        assert_eq!(old_cues.len(), 2);
        let old_dynamics = engine.active_dynamic_playbacks();
        let mut candidate = (*engine.snapshot()).clone();
        candidate.revision += 1; // All compiled dependency Arcs are unchanged.
        let finalized = finalize(&engine, candidate, preserve);
        assert_eq!(finalized.playback_dynamics_paused(), preserve);
        if preserve {
            assert_eq!(finalized.cue_dynamic_values(), old_cues);
            assert_eq!(finalized.dynamic_playbacks(), old_dynamics);
        } else {
            assert!(finalized.cue_dynamic_values().is_empty());
            assert!(finalized.dynamic_playbacks().is_empty());
        }
        assert_eq!(engine.active_cue_dynamic_values(), old_cues);
        assert_eq!(engine.active_dynamic_playbacks(), old_dynamics);
        assert!(engine.playback_dynamics().paused);
        let cues = finalized.cue_dynamic_values().to_vec();
        let dynamics = finalized.dynamic_playbacks().to_vec();
        clock.advance_millis(500);
        engine.install_finalized_snapshot(finalized);
        let installed = engine.generation.load_full().playback_arc();
        assert!(!Arc::ptr_eq(&live_playback, &installed));
        assert!(Arc::ptr_eq(
            &live_playback.read().clock(),
            &installed.read().clock(),
        ));
        assert_eq!(engine.active_cue_dynamic_values(), cues);
        assert_eq!(engine.active_dynamic_playbacks(), dynamics);
        assert_eq!(engine.playback_dynamics().paused, preserve);
        assert_eq!(engine.playback_source_occurrence_watermark(), 200);
        if !preserve {
            assert!(engine.playback_runtime().is_empty());
        }
    }
}

#[test]
fn abandoning_finalized_playback_leaves_live_generation_controls_and_history_unchanged() {
    for preserve in [false, true] {
        let (engine, _, _, _) = setup();
        execute_pool(&engine, 1, PoolPlaybackAction::SetTempButton(true));
        let original = engine.generation.load_full();
        let runtime = engine.playback_runtime();
        let cues = engine.active_cue_dynamic_values();
        let dynamics = engine.active_dynamic_playbacks();
        let watermark = engine.playback_source_occurrence_watermark();
        let mut candidate = (*engine.snapshot()).clone();
        candidate.playbacks = Arc::default();
        drop(finalize(&engine, candidate, preserve));
        assert!(Arc::ptr_eq(&original, &engine.generation.load_full()));
        assert_eq!(engine.playback_runtime(), runtime);
        assert_eq!(engine.active_cue_dynamic_values(), cues);
        assert_eq!(engine.active_dynamic_playbacks(), dynamics);
        assert_eq!(engine.playback_source_occurrence_watermark(), watermark);
    }
}

#[test]
fn deleted_cue_hold_uses_finalization_time_and_is_not_recaptured_at_installation() {
    let (engine, clock, first, _) = setup();
    let mut initial = (*engine.snapshot()).clone();
    let list = &mut Arc::make_mut(&mut initial.cue_lists)[0];
    list.cues[0].group_changes.clear();
    list.cues[0].changes = vec![CueChange::set(
        first,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.0),
    )];
    let mut second = Cue::new(2_u16.into());
    second.fade_millis = 1_000;
    second.changes.push(CueChange::set(
        first,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    ));
    list.cues.push(second);
    engine.replace_snapshot(initial).unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    clock.advance_millis(250);
    let mut candidate = (*engine.snapshot()).clone();
    Arc::make_mut(&mut candidate.cue_lists)[0].cues.pop();
    let finalized = finalize(&engine, candidate, true);
    let captured_at = finalized.sampled_at();
    assert!(engine.playback_runtime()[0].deleted_cue_hold.is_none());
    clock.advance_millis(500);
    engine.install_finalized_snapshot(finalized);
    let runtime = engine.playback_runtime();
    let hold = runtime[0].deleted_cue_hold.as_ref().unwrap();
    assert_eq!(hold.contributions.len(), 1);
    assert_eq!(
        hold.contributions[0].timed.value,
        AttributeValue::Normalized(0.25)
    );
    assert_eq!(clock.now() - captured_at, ChronoDuration::milliseconds(500));
    assert_eq!(
        engine.playback_contributions_at(clock.now())[0].value.value,
        AttributeValue::Normalized(0.25),
    );
}

#[test]
fn reused_preparation_cannot_resurrect_temporary_sources_from_an_older_generation() {
    let (engine, _, _, _) = setup();
    execute_pool(&engine, 1, PoolPlaybackAction::SetTempButton(true));
    let mut old_candidate = (*engine.snapshot()).clone();
    old_candidate.revision = 3;
    let prepared = engine.prepare_snapshot(old_candidate).unwrap();
    let mut newer = (*engine.snapshot()).clone();
    newer.revision = 2;
    Arc::make_mut(&mut newer.cue_lists)[0].name = "New compiled generation".into();
    engine.replace_snapshot(newer).unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Off);
    assert!(engine.active_cue_dynamic_values().is_empty());
    let finalized = engine.finalize_snapshot_playback(prepared, true).unwrap();
    assert!(finalized.cue_dynamic_values().is_empty());
    engine.install_finalized_snapshot(finalized);
    assert!(engine.active_cue_dynamic_values().is_empty());
}

#[test]
fn finalization_and_installation_do_not_tick_overdue_automatic_cues() {
    let (engine, clock, _, _) = setup();
    let mut candidate = (*engine.snapshot()).clone();
    let list = &mut Arc::make_mut(&mut candidate.cue_lists)[0];
    let current_id = list.cues[0].id;
    let mut next = Cue::new(2_u16.into());
    next.trigger = light_playback::CueTrigger::Wait { delay_millis: 100 };
    list.cues.push(next);
    clock.advance_millis(500);
    let finalized = finalize(&engine, candidate, true);
    assert_eq!(finalized.cue_dynamic_values()[0].current_cue_id, current_id);
    engine.install_finalized_snapshot(finalized);
    assert_eq!(
        engine.playback_runtime()[0].current_cue_id,
        Some(current_id)
    );
    let tick = engine
        .generation
        .load_full()
        .playback()
        .write()
        .tick(clock.now(), None);
    assert_eq!(
        tick.transitions.len(),
        1,
        "the due action remains for the scheduler"
    );
}

#[path = "lifecycle_restore_dynamics.rs"]
mod restore_dynamics;
