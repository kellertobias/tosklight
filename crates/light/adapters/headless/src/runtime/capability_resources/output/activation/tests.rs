use super::super::publication_tests;
use super::*;
use crate::runtime::dynamic_snapshot_publication::{ColdGenerationCursor, InputCaptureCursor};
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use light_core::{AttributeKey, FixtureId, SessionId};
use light_dynamics::{
    DynamicAddressValue, DynamicControlCursor, DynamicDefinition, DynamicDefinitionSnapshot,
    DynamicInstanceOverrides, DynamicReference, DynamicRuntimeSnapshot, DynamicSampleBoundary,
    DynamicSemanticValue, DynamicSpeedTransport, DynamicValueTiming, Rational,
    ScalarSourceResolver,
};
use light_programmer::{ProgrammerRegistry, ProgrammerState};
use std::num::NonZeroUsize;

fn playback() -> PlaybackRenderCapability {
    PlaybackRenderCapability::new(
        PlaybackService::new(EventBus::default()),
        Arc::new(
            crate::runtime::playback_telemetry::PlaybackTelemetrySampler::new(Arc::new(
                AtomicU16::new(40),
            )),
        ),
    )
}

fn definition() -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Restored owners",
        "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": Uuid::new_v4(), "attribute": "intensity", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0.0, "source": {"type": "current"}, "interpolation": "linear"},
                {"position": 0.5, "source": {"type": "current"}, "interpolation": "linear"}
            ]},
            "max_min": {"minimum": {"type": "value", "value": 0.0},
                "maximum": {"type": "value", "value": 1.0}, "function": "sinus"},
            "middle_amplitude": {"middle": {"type": "current"}, "amplitude": 0.5, "function": "sinus"},
            "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.0
        }],
        "phase": {"ordering": {"type": "selection"}, "offset_degrees": 0.0,
            "span_degrees": 0.0, "block_size": 1, "repeats": 1,
            "wings": false, "anchors_degrees": []},
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    }))
    .unwrap()
}

fn row(definition: &DynamicDefinition, link: Uuid, target: FixtureId) -> DynamicAddressValue {
    DynamicAddressValue {
        fixture_id: target,
        attribute: AttributeKey::intensity(),
        value: DynamicSemanticValue::DynamicOn {
            instance_link: link,
            dynamic: DynamicReference {
                dynamic_id: Some(definition.id),
                last_known_pool_number: definition.pool_number,
                embedded_fallback: DynamicDefinitionSnapshot {
                    definition: Arc::new(definition.clone()),
                },
            },
            lane_id: definition.lanes[0].id,
            overrides: DynamicInstanceOverrides {
                size: 1.0,
                speed_multiplier: Rational::ONE,
                phase_offset_degrees: 0.0,
            },
            timing: DynamicValueTiming::default(),
        },
        programmer_order: 1,
        changed_at_millis: 1_000,
    }
}

fn desk(
    definition: &DynamicDefinition,
    rows: Vec<DynamicAddressValue>,
) -> (OutputResource, ProgrammerRegistry, ProgrammerState) {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_500).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let mut state = programmers.start(SessionId::new());
    state.dynamic_values = Arc::new(rows);
    programmers.restore(state.clone());
    let output = publication_tests::output_with_programmers(programmers.clone());
    output
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    (output, programmers, state)
}

struct Sources;
impl ScalarSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        Some(0.35)
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
fn add_origins(output: &OutputResource) {
    let mut origins = DynamicSourceOrigins::default();
    let entry = light_engine::ContributionFamilyEntry::new(
        light_engine::ContributionSourceId::programmer(light_core::ProgrammerId::new()),
        light_core::ProgrammerEditStamp {
            changed_at: output.engine.application_time(),
            programmer_order: 1,
        },
        light_engine::ContributionFamilyFootprint::Whole,
        light_engine::ContributionFamilyRole::Authored,
    );
    origins
        .bind_static_evidence(
            crate::runtime::dynamic_source_origins::DynamicSourceBinding::StaticBaseline {
                target: FixtureId::new(),
                owner: light_core::programming::ProgrammingOwner::Focus,
            },
            &Arc::new(light_engine::ContributionFamilyEvidence::new(vec![entry])),
        )
        .unwrap();
    output.dynamic_source_origins.store(Arc::new(origins));
}
struct Before {
    snapshot: Arc<EngineSnapshot>,
    runtime: DynamicRuntimeSnapshot,
    sample: Option<DynamicSampleBoundary>,
    cold: ColdGenerationCursor,
    controls: DynamicControlCursor,
    input: Option<InputCaptureCursor>,
    origins: Arc<DynamicSourceOrigins>,
    playback: serde_json::Value,
}
impl Before {
    fn capture(output: &OutputResource) -> Self {
        add_origins(output);
        let snapshot = output.snapshot();
        let mut runtime = output.dynamics.lock();
        let (cold, controls) = output
            .dynamic_snapshot
            .begin_retained_history(&mut runtime, &snapshot, NonZeroUsize::new(8).unwrap())
            .unwrap();
        let saved = (runtime.snapshot(), runtime.committed_sample_boundary());
        drop(runtime);
        Self {
            snapshot,
            runtime: saved.0,
            sample: saved.1,
            cold,
            controls,
            input: output.dynamic_snapshot.input_capture_cursor(),
            origins: output.dynamic_source_origins.load_full(),
            playback: serde_json::to_value(output.playback_runtime()).unwrap(),
        }
    }
    fn assert_unchanged(&self, output: &OutputResource) {
        let mut runtime = output.dynamics.lock();
        assert_eq!(runtime.snapshot(), self.runtime);
        assert_eq!(runtime.committed_sample_boundary(), self.sample);
        assert_eq!(runtime.control_cursor(), Some(self.controls));
        assert_eq!(
            output
                .dynamic_snapshot
                .begin_retained_history(&mut runtime, &self.snapshot, NonZeroUsize::new(8).unwrap())
                .unwrap(),
            (self.cold, self.controls)
        );
        drop(runtime);
        assert_eq!(output.dynamic_snapshot.input_capture_cursor(), self.input);
        assert!(
            output
                .dynamic_snapshot
                .cold_generations_since(self.cold)
                .unwrap()
                .is_empty()
        );
        assert!(Arc::ptr_eq(&output.snapshot(), &self.snapshot));
        assert!(Arc::ptr_eq(
            &output.dynamic_source_origins.load_full(),
            &self.origins
        ));
        assert_eq!(
            serde_json::to_value(output.playback_runtime()).unwrap(),
            self.playback
        );
    }
}

fn prepare_empty_destination(
    output: &OutputResource,
    destination: EngineSnapshot,
) -> PreparedOutputSnapshot {
    output.prepare_snapshot(destination).unwrap()
}
fn empty_checkpoint() -> DynamicRuntimeSourceCheckpoint {
    DynamicRuntimeSourceCheckpoint {
        runtime: Default::default(),
        origins: None,
    }
}

#[test]
fn strict_owner_failure_and_prenormalized_restore_leave_live_and_lineage_untouched() {
    let definition = definition();
    let mut invalid = row(&definition, Uuid::new_v4(), FixtureId::new());
    let DynamicSemanticValue::DynamicOn { overrides, .. } = &mut invalid.value else {
        unreachable!()
    };
    overrides.size = -1.;
    let (output, _, _) = desk(&definition, vec![invalid]);
    let before = Before::capture(&output);
    let watermark = output.engine.playback_source_occurrence_watermark();
    let prepared = prepare_empty_destination(&output, (*output.snapshot()).clone());
    assert!(
        output
            .prepare_destination_activation(prepared, empty_checkpoint(), &[], None)
            .is_err()
    );
    before.assert_unchanged(&output);
    let plain = output
        .prepare_snapshot((*output.snapshot()).clone())
        .unwrap();
    let already_restored = output
        .prepare_snapshot_restore(plain, empty_checkpoint())
        .unwrap();
    assert!(
        output
            .prepare_destination_activation(already_restored, empty_checkpoint(), &[], None)
            .is_err()
    );
    before.assert_unchanged(&output);
    assert_eq!(
        output.engine.playback_source_occurrence_watermark(),
        watermark
    );
}

#[test]
fn exact_programmer_authority_changes_reject_before_watermark_or_publication() {
    for mutation in 0..5 {
        let definition = definition();
        let (output, registry, mut state) = desk(&definition, vec![]);
        let prepared = prepare_empty_destination(&output, (*output.snapshot()).clone());
        let token = output
            .prepare_destination_activation(prepared, empty_checkpoint(), &[], None)
            .unwrap();
        match mutation {
            0 => state.priority += 1,
            1 => state.dynamic_values = Arc::new((*state.dynamic_values).clone()),
            2 => state.preload_dynamic_active = Arc::new((*state.preload_dynamic_active).clone()),
            3 => state.id = light_core::ProgrammerId::new(),
            4 => {}
            _ => unreachable!(),
        }
        if mutation == 4 {
            registry.reset_all();
        } else {
            registry.restore(state);
        }
        let before = Before::capture(&output);
        let watermark = output.engine.playback_source_occurrence_watermark();
        let error = output
            .install_destination_activation(&playback(), token)
            .unwrap_err();
        assert!(error.to_string().contains("authority is stale"), "{error}");
        before.assert_unchanged(&output);
        assert_eq!(
            output.engine.playback_source_occurrence_watermark(),
            watermark
        );
    }
}

#[test]
fn foreign_output_and_changed_snapshot_reject_without_installing_any_part() {
    let definition = definition();
    let (output, _, _) = desk(&definition, vec![]);
    let prepared = prepare_empty_destination(&output, (*output.snapshot()).clone());
    let token = output
        .prepare_destination_activation(prepared, empty_checkpoint(), &[], None)
        .unwrap();
    let foreign = publication_tests::output();
    let before = Before::capture(&foreign);
    assert!(
        foreign
            .install_destination_activation(&playback(), token)
            .is_err()
    );
    before.assert_unchanged(&foreign);

    let prepared = prepare_empty_destination(&output, (*output.snapshot()).clone());
    let token = output
        .prepare_destination_activation(prepared, empty_checkpoint(), &[], None)
        .unwrap();
    let mut changed = (*output.snapshot()).clone();
    changed.revision += 1;
    output.replace_snapshot(changed).unwrap();
    let before = Before::capture(&output);
    assert!(
        output
            .install_destination_activation(&playback(), token)
            .unwrap_err()
            .to_string()
            .contains("snapshot is stale")
    );
    before.assert_unchanged(&output);
}

#[test]
fn destination_success_is_exact_unpinned_and_retains_surviving_current_history() {
    let original = definition();
    let (output, _, _) = desk(
        &original,
        vec![row(&original, Uuid::new_v4(), FixtureId::new())],
    );
    output.finalize_restored_owners(&playback()).unwrap();
    let saved = {
        let mut runtime = output.dynamics.lock();
        runtime.sample_all_addressed(
            1500,
            25,
            &[DynamicSpeedTransport {
                effective_bpm: 120.,
                phase_origin_millis: 0,
                phase_reference_millis: 1500,
                beat_phase: 3.,
                phase_advancing: true,
            }; 5],
            &Sources,
            None,
        );
        runtime.snapshot()
    };
    assert!(saved.instances[0].expression_tape.is_some());
    output.set_dynamic_definitions_pinned(true);
    let mut updated = original.clone();
    updated.revision += 1;
    updated.name = "Destination definition".into();
    let mut destination = (*output.snapshot()).clone();
    destination.revision += 1;
    destination.dynamics = vec![updated.clone()].into();
    let prepared = output.prepare_snapshot(destination).unwrap();
    let token = output
        .prepare_destination_activation(
            prepared,
            DynamicRuntimeSourceCheckpoint {
                runtime: saved.clone(),
                origins: None,
            },
            &[],
            None,
        )
        .unwrap();
    let expected_snapshot = token.engine.snapshot_arc();
    let expected = token.restored.runtime.snapshot();
    assert_eq!(expected.instances[0].definition, updated);
    assert_eq!(expected.instances[0].id, saved.instances[0].id);
    assert_eq!(
        expected.instances[0].started_at_millis,
        saved.instances[0].started_at_millis
    );
    assert_eq!(
        expected.instances[0].expression_tape,
        saved.instances[0].expression_tape
    );
    assert_eq!(
        expected.instances[0].last_sample_values,
        saved.instances[0].last_sample_values
    );
    let before = Before::capture(&output);
    before.assert_unchanged(&output);
    output
        .engine
        .reserve_playback_source_occurrence_watermark(901);
    output
        .install_destination_activation(&playback(), token)
        .unwrap();
    assert!(Arc::ptr_eq(&output.snapshot(), &expected_snapshot));
    assert!(output.dynamic_snapshot.matches(&expected_snapshot));
    assert_eq!(output.dynamic_runtime_snapshot(), expected);
    assert_eq!(output.engine.playback_source_occurrence_watermark(), 901);
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(before.cold)
            .is_err()
    );
    assert_ne!(
        output.dynamics.lock().control_cursor(),
        Some(before.controls)
    );
}

#[test]
fn missing_retained_group_is_passive_and_disconnected_authority_is_preserved() {
    let (output, registry, mut state) = desk(&definition(), vec![]);
    let mut fallback = definition();
    fallback.target_binding = light_dynamics::DynamicTargetBinding::LiveGroup {
        group_id: "missing-destination-group".into(),
    };
    state.dynamic_values = Arc::new(vec![row(&fallback, Uuid::new_v4(), FixtureId::new())]);
    registry.restore(state);
    let token = output
        .prepare_destination_activation(
            prepare_empty_destination(&output, (*output.snapshot()).clone()),
            empty_checkpoint(),
            &[],
            None,
        )
        .unwrap();
    assert!(token.restored.runtime.snapshot().instances.is_empty());
    // Connection status is intentionally outside retained authority.
    for session in registry.programmer_lifecycle().unwrap().connected_sessions {
        registry.disconnect(session.session_id);
    }
    output
        .install_destination_activation(&playback(), token)
        .unwrap();
    assert!(output.dynamic_runtime_snapshot().instances.is_empty());
}

#[test]
fn restored_playback_rebinding_and_removal_reconcile_before_any_live_install() {
    for removed in [false, true] {
        let mut definition = definition();
        definition.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
            targets: vec![FixtureId::new()],
        };
        let (output, _, _) = desk(&definition, vec![]);
        let assignment = light_playback::DynamicPlaybackAssignment {
            dynamic: DynamicReference {
                dynamic_id: Some(definition.id),
                last_known_pool_number: definition.pool_number,
                embedded_fallback: DynamicDefinitionSnapshot {
                    definition: Arc::new(definition.clone()),
                },
            },
            revision: 1,
            target_scope: None,
            fader_mode: light_playback::DynamicPlaybackFaderMode::SizeAndMaster,
            priority: 0,
            activation_override: None,
            resume_policy: light_playback::DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: false,
            auto_off_at_zero: false,
            auto_off_flash_release: false,
            auto_off_full_control: true,
        };
        let mut source = (*output.snapshot()).clone();
        source.playbacks = vec![
            serde_json::from_value(serde_json::json!({
                "number": 1, "name": "Destination Dynamic",
                "target": light_playback::PlaybackTarget::Dynamic { assignment }
            }))
            .unwrap(),
        ]
        .into();
        output.replace_snapshot(source).unwrap();
        output
            .engine
            .execute_playback(light_engine::EnginePlaybackCommand::Pool {
                number: 1,
                action: light_engine::PoolPlaybackAction::On,
            })
            .unwrap();
        output.finalize_restored_owners(&playback()).unwrap();
        let saved_playbacks = output.engine.active_dynamic_playbacks();
        assert_eq!(saved_playbacks.len(), 1);
        {
            let mut runtime = output.dynamics.lock();
            runtime.sample_all_addressed(
                1500,
                25,
                &[DynamicSpeedTransport {
                    effective_bpm: 120.,
                    phase_origin_millis: 0,
                    phase_reference_millis: 1500,
                    beat_phase: 3.,
                    phase_advancing: true,
                }; 5],
                &Sources,
                None,
            );
        }
        let checkpoint = output.dynamic_runtime_snapshot();
        assert_eq!(checkpoint.instances.len(), 1);
        let original_instance = checkpoint.instances[0].clone();
        assert!(original_instance.expression_tape.is_some());
        assert!(!original_instance.last_sample_values.is_empty());
        let mut destination = (*output.snapshot()).clone();
        destination.revision += 1;
        if removed {
            destination.playbacks = Arc::default();
        } else {
            let playback = &mut Arc::make_mut(&mut destination.playbacks)[0];
            playback.number = 7;
            let light_playback::PlaybackTarget::Dynamic { assignment } = &mut playback.target
            else {
                unreachable!()
            };
            assignment.priority = 37;
        }
        let prepared = output.prepare_snapshot(destination).unwrap();
        let before = Before::capture(&output);
        let token = output
            .prepare_destination_activation(
                prepared,
                DynamicRuntimeSourceCheckpoint {
                    runtime: checkpoint,
                    origins: None,
                },
                &saved_playbacks,
                Some(output.engine.application_time()),
            )
            .unwrap();
        let expected_rows = token.engine.dynamic_playbacks().to_vec();
        let expected_runtime = token.restored.runtime.snapshot();
        assert!(token.engine.playback_dynamics_paused());
        assert!(expected_runtime.global_paused);
        if removed {
            assert!(expected_rows.is_empty());
            assert!(expected_runtime.instances.is_empty());
        } else {
            assert_eq!(expected_rows.len(), 1);
            assert_eq!(expected_rows[0].playback_number, 7);
            assert_eq!(expected_runtime.instances.len(), 1);
            let retained = &expected_runtime.instances[0];
            assert_eq!(retained.id, original_instance.id);
            assert_eq!(original_instance.controllers[0].priority, 0);
            assert!(
                retained
                    .controllers
                    .iter()
                    .all(|controller| controller.priority == 37)
            );
            assert_eq!(
                retained.controllers[0].id,
                original_instance.controllers[0].id
            );
            assert_eq!(
                retained.started_at_millis,
                original_instance.started_at_millis
            );
            assert_eq!(retained.expression_tape, original_instance.expression_tape);
            assert_eq!(
                retained.last_sample_values,
                original_instance.last_sample_values
            );
            assert!(
                expected_runtime.instances[0]
                    .controllers
                    .iter()
                    .all(|controller| {
                        matches!(
                            controller.source,
                            light_dynamics::DynamicControllerSource::Playback {
                                playback_number: 7,
                                ..
                            }
                        )
                    })
            );
        }
        before.assert_unchanged(&output);
        assert_eq!(output.engine.active_dynamic_playbacks(), saved_playbacks);
        output
            .install_destination_activation(&playback(), token)
            .unwrap();
        assert_eq!(output.engine.active_dynamic_playbacks(), expected_rows);
        assert_eq!(output.dynamic_runtime_snapshot(), expected_runtime);
        assert!(output.engine.playback_dynamics().paused);
    }
}

#[test]
fn raw_legacy_checkpoint_is_normalized_from_the_same_restored_authority_without_losing_history() {
    let definition = definition();
    let link = Uuid::new_v4();
    let (output, registry, state) =
        desk(&definition, vec![row(&definition, link, FixtureId::new())]);
    output.finalize_restored_owners(&playback()).unwrap();
    {
        let mut runtime = output.dynamics.lock();
        runtime.sample_all_addressed(
            1500,
            25,
            &[DynamicSpeedTransport {
                effective_bpm: 120.,
                phase_origin_millis: 0,
                phase_reference_millis: 1500,
                beat_phase: 3.,
                phase_advancing: true,
            }; 5],
            &Sources,
            None,
        );
    }
    output.set_dynamic_runtime_paused(true);
    let original = output.dynamic_runtime_snapshot();
    assert_eq!(original.instances.len(), 1);
    let original_instance = &original.instances[0];
    assert!(original_instance.expression_tape.is_some());
    assert!(!original_instance.last_sample_values.is_empty());
    assert!(original_instance.paused_at_millis.is_some());
    let derived = light_dynamics::programmer_dynamic_controller_id(state.id, link);
    assert_eq!(original_instance.controllers[0].id, derived);
    let mut legacy = original.clone();
    let instance = &mut legacy.instances[0];
    instance.controllers[0].id = link;
    instance.controllers[0].source = light_dynamics::DynamicControllerSource::Programmer {
        programmer_id: state.id.0,
        instance_link: None,
    };
    for selection in &mut instance.lane_selections {
        selection.controller_id = link;
    }
    for transition in &mut instance.controller_transitions {
        transition.controller_id = link;
    }
    for value in instance
        .last_sample_values
        .iter_mut()
        .chain(&mut instance.synchronized_hold_values)
    {
        value.controller_id = link;
    }
    // Reproduce the old normalization window: link missing, then retained Undo restores it.
    let mut absent = state.clone();
    absent.dynamic_values = Arc::default();
    registry.restore(absent);
    assert_eq!(
        light_dynamics::normalize_legacy_programmer_controller_ids(&mut legacy, &[]).unwrap(),
        0
    );
    registry.restore(state.clone());
    let before = Before::capture(&output);
    let token = output
        .prepare_destination_activation(
            prepare_empty_destination(&output, (*output.snapshot()).clone()),
            DynamicRuntimeSourceCheckpoint {
                runtime: legacy,
                origins: None,
            },
            &[],
            Some(output.engine.application_time()),
        )
        .unwrap();
    let actual = token.restored.runtime.snapshot();
    assert_eq!(actual.instances.len(), 1);
    let retained = &actual.instances[0];
    assert_eq!(retained.id, original_instance.id);
    assert_eq!(retained.controllers[0].id, derived);
    assert!(matches!(retained.controllers[0].source,
        light_dynamics::DynamicControllerSource::Programmer { programmer_id, instance_link: Some(found) }
        if programmer_id == state.id.0 && found == link));
    assert_eq!(
        retained.started_at_millis,
        original_instance.started_at_millis
    );
    assert_eq!(
        retained.paused_at_millis,
        original_instance.paused_at_millis
    );
    assert_eq!(
        retained.phase_by_lane_target,
        original_instance.phase_by_lane_target
    );
    assert_eq!(
        retained.last_sample_values,
        original_instance.last_sample_values
    );
    assert_eq!(
        retained.synchronized_hold_values,
        original_instance.synchronized_hold_values
    );
    assert_eq!(retained.expression_tape, original_instance.expression_tape);
    before.assert_unchanged(&output);
    output
        .install_destination_activation(&playback(), token)
        .unwrap();
    assert_eq!(output.dynamic_runtime_snapshot(), actual);
}

#[test]
fn activation_retires_only_obsolete_authored_lookup_and_preserves_incoming_evidence() {
    use crate::runtime::dynamic_source_origins::{
        DynamicFixedSource, DynamicFixedStamp, DynamicProgrammerSourceLane, DynamicSequenceSource,
        DynamicSourceOrigin, DynamicStaticFootprint, DynamicStaticRole, DynamicStaticSource,
        DynamicStaticSourceEntry,
    };
    use light_core::programming::ProgrammingOwner;

    // Exercise MAX in both directions. The checkpoint's largest occurrence belongs to an
    // unbound historical record, not its current static binding.
    for later_live_watermark in [700, 1_200] {
        let definition = definition();
        let link = Uuid::new_v4();
        let target = FixtureId::new();
        let (output, registry, state) = desk(&definition, vec![row(&definition, link, target)]);
        output.finalize_restored_owners(&playback()).unwrap();
        let saved = output.dynamic_runtime_snapshot();
        assert_eq!(saved.instances.len(), 1);
        let authored = DynamicSourceBinding::Authored {
            instance_id: saved.instances[0].id,
            controller_id: light_dynamics::programmer_dynamic_controller_id(state.id, link),
            target,
            lane_id: definition.lanes[0].id,
        };
        let mut incoming = DynamicSourceOrigins::default();
        let authored_id = incoming
            .bind(
                authored,
                DynamicSourceOrigin::Programmer {
                    programmer_id: state.id,
                    lane: DynamicProgrammerSourceLane::Live,
                    instance_link: link,
                    changed_at_millis: 1_000,
                    programmer_order: 1,
                },
            )
            .unwrap();
        let fixed = DynamicSourceBinding::Fixed {
            source: DynamicFixedSource::Programmer {
                programmer_id: state.id,
                lane: DynamicProgrammerSourceLane::Live,
            },
            target,
            owner: ProgrammingOwner::Focus,
            component: None,
        };
        let fixed_id = incoming
            .bind(
                fixed,
                DynamicSourceOrigin::Fixed {
                    stamp: DynamicFixedStamp::Programmer {
                        changed_at_millis: 1_000,
                        programmer_order: 2,
                    },
                    priority: 100,
                    value: DynamicSemanticValue::Static {
                        value: light_core::AttributeValue::Normalized(0.4),
                        timing: Default::default(),
                    },
                },
            )
            .unwrap();
        let baseline = DynamicSourceBinding::StaticBaseline {
            target,
            owner: ProgrammingOwner::Focus,
        };
        let static_origin = |ordinal| DynamicSourceOrigin::StaticBaseline {
            sources: vec![DynamicStaticSourceEntry {
                source: DynamicStaticSource::Playback {
                    source: DynamicSequenceSource {
                        playback_number: Some(4),
                        playback_identity: Some(
                            light_playback::PlaybackIdentity::physical(4).unwrap(),
                        ),
                        cue_list_id: light_core::CueListId::new(),
                        temporary: false,
                    },
                },
                changed_at: output.engine.application_time(),
                programmer_order: 0,
                transition_ordinal: Some(ordinal),
                authored_cue_id: Some(Uuid::new_v4()),
                footprint: DynamicStaticFootprint::Whole,
                role: DynamicStaticRole::Authored,
                effective_fields: None,
            }],
        };
        let historical_id = incoming.bind(baseline, static_origin(900)).unwrap();
        let baseline_id = incoming.bind(baseline, static_origin(3)).unwrap();
        assert_ne!(historical_id, baseline_id);
        assert_eq!(incoming.playback_source_occurrence_watermark(), 900);
        let incoming_snapshot = incoming.snapshot();

        // The saved owner is no longer retained by the desk. It must be removed from the
        // destination, without deleting its immutable record or unrelated fixed/static data.
        registry.reset_all();
        let before = Before::capture(&output);
        let token = output
            .prepare_destination_activation(
                output
                    .prepare_snapshot((*output.snapshot()).clone())
                    .unwrap(),
                DynamicRuntimeSourceCheckpoint {
                    runtime: saved,
                    origins: Some(incoming_snapshot.clone()),
                },
                &[],
                None,
            )
            .unwrap();
        before.assert_unchanged(&output);
        assert!(token.restored.runtime.snapshot().instances.is_empty());
        assert_eq!(token.restored.origins.binding(&authored), None);
        assert_eq!(token.restored.origins.binding(&fixed), Some(fixed_id));
        assert_eq!(token.restored.origins.binding(&baseline), Some(baseline_id));
        assert_eq!(
            token.restored.origins.snapshot().records,
            incoming_snapshot.records
        );

        output
            .engine
            .reserve_playback_source_occurrence_watermark(later_live_watermark);
        output
            .install_destination_activation(&playback(), token)
            .unwrap();
        assert!(output.dynamic_runtime_snapshot().instances.is_empty());
        let installed = output.dynamic_source_origins.load_full();
        assert_eq!(installed.binding(&authored), None);
        assert_eq!(installed.binding(&fixed), Some(fixed_id));
        assert_eq!(installed.binding(&baseline), Some(baseline_id));
        for id in [authored_id, fixed_id, historical_id, baseline_id] {
            assert_eq!(installed.get(id), incoming.get(id));
        }
        assert_eq!(installed.snapshot().records, incoming_snapshot.records);
        assert_eq!(incoming.binding(&authored), Some(authored_id));
        assert_eq!(installed.playback_source_occurrence_watermark(), 900);
        assert_eq!(
            output.engine.playback_source_occurrence_watermark(),
            later_live_watermark.max(900)
        );
    }
}

#[test]
fn activation_metadata_failure_isolated_and_stale_authority_skips_metadata_commit() {
    let definition = definition();
    let (output, registry, mut state) = desk(&definition, vec![]);
    let token = output
        .prepare_destination_activation(
            output
                .prepare_snapshot((*output.snapshot()).clone())
                .unwrap(),
            empty_checkpoint(),
            &[],
            None,
        )
        .unwrap();
    let before = Before::capture(&output);
    let watermark = output.engine.playback_source_occurrence_watermark();
    let calls = std::cell::Cell::new(0);
    let error = output
        .install_destination_activation_with_commit(&playback(), token, || {
            calls.set(calls.get() + 1);
            Err::<(), _>(IntentError("injected metadata transaction failure".into()))
        })
        .unwrap_err();
    assert!(error.to_string().contains("metadata transaction failure"));
    assert_eq!(calls.get(), 1);
    before.assert_unchanged(&output);
    assert_eq!(
        output.engine.playback_source_occurrence_watermark(),
        watermark
    );

    let token = output
        .prepare_destination_activation(
            output
                .prepare_snapshot((*output.snapshot()).clone())
                .unwrap(),
            empty_checkpoint(),
            &[],
            None,
        )
        .unwrap();
    state.priority += 1;
    registry.restore(state);
    let before = Before::capture(&output);
    let error = output
        .install_destination_activation_with_commit(&playback(), token, || {
            calls.set(calls.get() + 1);
            Ok(42)
        })
        .unwrap_err();
    assert!(error.to_string().contains("authority is stale"));
    assert_eq!(calls.get(), 1);
    before.assert_unchanged(&output);
}

#[test]
fn activation_metadata_commit_runs_once_and_returns_result_with_exact_destination() {
    let definition = definition();
    let (output, _, _) = desk(&definition, vec![]);
    let mut destination = (*output.snapshot()).clone();
    destination.revision += 1;
    let token = output
        .prepare_destination_activation(
            output.prepare_snapshot(destination).unwrap(),
            empty_checkpoint(),
            &[],
            None,
        )
        .unwrap();
    let expected = token.engine.snapshot_arc();
    let calls = std::cell::Cell::new(0);
    let metadata = output
        .install_destination_activation_with_commit(&playback(), token, || {
            calls.set(calls.get() + 1);
            Ok(42)
        })
        .unwrap();
    assert_eq!(metadata, 42);
    assert_eq!(calls.get(), 1);
    assert!(Arc::ptr_eq(&output.snapshot(), &expected));
}
