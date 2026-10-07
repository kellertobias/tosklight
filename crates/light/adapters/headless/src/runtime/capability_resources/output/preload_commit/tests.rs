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
use std::{num::NonZeroUsize, sync::mpsc};

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
    let cue_list = light_core::CueListId::new();
    output
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            cue_lists: vec![serde_json::from_value(serde_json::json!({
            "id":cue_list,"name":"GO candidate","priority":0,"mode":"sequence","looped":false,
            "cues":[light_playback::Cue::new("1".parse().unwrap())]
        })).unwrap()]
            .into(),
            playbacks: vec![serde_json::from_value(serde_json::json!({
            "number":1,"name":"GO candidate","target":{"type":"cue_list","cue_list_id":cue_list}
        })).unwrap()]
            .into(),
            ..Default::default()
        })
        .unwrap();
    (output, programmers, state)
}
fn batch(output: &OutputResource) -> light_engine::PreparedPlaybackBatch {
    output
        .prepare_playback_batch(
            &[light_engine::PlaybackBatchCommand {
                number: 1,
                page: None,
                action: light_engine::PlaybackBatchAction::Go,
                exclusion_zones: Arc::from([]),
                activation_origin: None,
            }],
            output.engine.application_time(),
            0,
        )
        .unwrap()
}
fn projection_before(
    output: &OutputResource,
) -> Vec<(
    light_application::PlaybackRuntimeIdentity,
    light_application::PlaybackRuntimeProjection,
)> {
    let identity = light_application::PlaybackRuntimeIdentity::Playback(1);
    vec![(
        identity.clone(),
        light_application::PlaybackRuntimeProjection {
            scope: light_application::PlaybackShowScope {
                show_id: Uuid::new_v4(),
                show_revision: output.snapshot().revision,
            },
            requested: identity,
            playback_number: Some(1),
            target: light_application::PlaybackTargetProjection::CueList {
                cue_list_id: output.snapshot().cue_lists[0].id,
                runtime: None,
            },
        },
    )]
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

#[test]
fn late_projection_rejection_keeps_surviving_pinned_owner_then_success_rebinds_without_resetting_history()
 {
    let original = definition();
    let (output, _, _) = desk(
        &original,
        vec![row(&original, Uuid::new_v4(), FixtureId::new())],
    );
    output.finalize_restored_owners(&playback()).unwrap();
    {
        let mut runtime = output.dynamics.lock();
        runtime.sample_all(
            1_500,
            25,
            &[DynamicSpeedTransport {
                effective_bpm: 120.,
                phase_origin_millis: 0,
                phase_reference_millis: 1_500,
                beat_phase: 3.,
                phase_advancing: true,
            }; 5],
            &Sources,
        );
    }
    output.set_dynamic_definitions_pinned(true);
    let mut latest = original.clone();
    latest.revision += 1;
    latest.name = "GO latest surviving definition".into();
    let mut snapshot = (*output.snapshot()).clone();
    snapshot.revision += 1;
    snapshot.dynamics = vec![latest.clone()].into();
    output.replace_snapshot(snapshot).unwrap();
    let before = Before::capture(&output);
    assert_eq!(before.runtime.instances[0].definition, original);
    assert!(before.runtime.instances[0].expression_tape.is_some());
    let mut invalid_projection = projection_before(&output);
    invalid_projection[0].1.scope.show_revision += 1;
    let mut called = false;
    let error = output
        .install_preload_playback_batch(
            batch(&output),
            output.engine.application_time(),
            |snapshot, playback, candidate| {
                called = true;
                assert_eq!(
                    candidate.instances[0].definition, latest,
                    "cold candidate unpins before projection validation"
                );
                crate::runtime::playback_service::prepared_runtime_projections(
                    snapshot,
                    &invalid_projection,
                    playback,
                    candidate,
                    25,
                )
            },
        )
        .unwrap_err();
    assert!(called);
    assert!(error.contains("mismatched authority"), "{error}");
    before.assert_unchanged(&output);
    let valid_projection = projection_before(&output);
    let (projections, changed) = output
        .install_preload_playback_batch(
            batch(&output),
            output.engine.application_time(),
            |snapshot, playback, candidate| {
                crate::runtime::playback_service::prepared_runtime_projections(
                    snapshot,
                    &valid_projection,
                    playback,
                    candidate,
                    25,
                )
            },
        )
        .unwrap();
    assert!(changed);
    assert!(projections[0].1.cue_list_runtime().unwrap().enabled);
    let after = output.dynamic_runtime_snapshot();
    assert_eq!(after.instances.len(), 1);
    let previous = &before.runtime.instances[0];
    let current = &after.instances[0];
    assert_eq!(current.definition, latest);
    assert_eq!(current.id, previous.id);
    assert_eq!(current.started_at_millis, previous.started_at_millis);
    assert_eq!(current.phase_by_lane_target, previous.phase_by_lane_target);
    assert_eq!(current.last_sample_values, previous.last_sample_values);
    assert_eq!(
        current.synchronized_hold_values,
        previous.synchronized_hold_values
    );
    assert_eq!(current.expression_tape, previous.expression_tape);
    assert_eq!(
        output.dynamics.lock().committed_sample_boundary(),
        before.sample
    );
    assert_ne!(output.dynamic_snapshot.input_capture_cursor(), before.input);
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(before.cold)
            .is_err()
    );
}

#[test]
fn invalid_owner_preflight_preserves_playback_origins_and_retained_lineage_before_callback() {
    let definition = definition();
    let mut invalid = row(&definition, Uuid::new_v4(), FixtureId::new());
    let DynamicSemanticValue::DynamicOn { overrides, .. } = &mut invalid.value else {
        unreachable!()
    };
    overrides.size = -1.;
    let (output, _, _) = desk(&definition, vec![invalid]);
    let before = Before::capture(&output);
    let mut called = false;
    let rejected: Result<((), bool), String> = output.install_preload_playback_batch(
        batch(&output),
        output.engine.application_time(),
        |_, _, _| {
            called = true;
            Ok(())
        },
    );
    assert!(rejected.is_err());
    assert!(!called);
    before.assert_unchanged(&output);
}

#[test]
fn invalid_preset_owner_target_rejects_before_callback_without_installing_playback() {
    use light_core::programming::{PositionIntent, ProgrammingComponent, ScalarIntent};
    use light_dynamics::{
        DynamicFamilyRepresentation, DynamicLaneBody, DynamicPresetGroupTemplate,
        DynamicPresetTemplate, DynamicValueAddress, DynamicValueSource, MaxMinConfiguration,
        PeriodicFunction, ProgrammingLaneBody, ProgrammingLaneConfiguration, PwmShape,
    };
    let mut definition = definition();
    let target = FixtureId(Uuid::nil()); // Rejected by strict lane ownership before Preset compilation.
    definition.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
        targets: vec![target],
    };
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    };
    let preset = DynamicValueSource::Preset {
        preset_id: "3.1".into(),
        address: address.clone(),
        last_valid_by_target: vec![],
        retained: Some(Arc::new(DynamicPresetTemplate {
            groups: vec![DynamicPresetGroupTemplate {
                group_id: "front".into(),
                value: light_core::AttributeValue::Position(Arc::new(PositionIntent::Angles {
                    pan_degrees: ScalarIntent::Spread(vec![0., 1.]),
                    tilt_degrees: ScalarIntent::Value(0.),
                })),
            }],
            ..Default::default()
        })),
    };
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address,
        configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: preset.clone(),
            maximum: preset,
            function: PeriodicFunction::LinearUp,
            size: 1.,
            pwm: PwmShape::default(),
        }),
    });
    // Use the normal authoring normalization: an authored Pan lane retains a real Tilt
    // Current partner. The resulting definition and owner are valid before materialization.
    definition.normalize_angle_pair();
    assert_eq!(definition.lanes.len(), 2);
    assert!(
        definition
            .lanes
            .iter()
            .any(|lane| lane.is_angle_current_passthrough())
    );
    let mut owner = row(&definition, Uuid::new_v4(), target);
    owner.attribute = AttributeKey("pan".into());
    let (output, _, _) = desk(&definition, vec![owner]);
    let before = Before::capture(&output);
    let mut called = false;
    let rejected: Result<((), bool), String> = output.install_preload_playback_batch(
        batch(&output),
        output.engine.application_time(),
        |_, _, _| {
            called = true;
            Ok(())
        },
    );
    let error = rejected.unwrap_err();
    assert!(
        error.contains("LaneSelection") && error.contains("invalid or duplicate targets"),
        "{error}"
    );
    assert!(!called);
    before.assert_unchanged(&output);
}

#[test]
fn callback_holds_dynamics_until_infallible_commit_and_blocked_reader_observes_the_complete_pair() {
    let definition = definition();
    let (output, _, _) = desk(
        &definition,
        vec![row(&definition, Uuid::new_v4(), FixtureId::new())],
    );
    let prepared = batch(&output);
    let at = output.engine.application_time();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (committed_tx, committed_rx) = mpsc::channel();
    let writer = output.clone();
    let writing = std::thread::spawn(move || {
        let result =
            writer.install_preload_playback_batch(prepared, at, |_, playback, candidate| {
                assert!(
                    playback
                        .runtime_status_at(light_playback::PlaybackIdentity::physical(1).unwrap())
                        .unwrap()
                        .playback
                        .enabled
                );
                assert_eq!(candidate.instances.len(), 1);
                entered_tx.send(()).unwrap();
                release_rx
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(|error| error.to_string())?;
                Ok(candidate.clone())
            });
        committed_tx.send(result).unwrap();
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let (contended_tx, contended_rx) = mpsc::channel();
    let (observed_tx, observed_rx) = mpsc::channel();
    let reader = output.clone();
    let reading = std::thread::spawn(move || {
        // Direct try-lock evidence establishes contention; absence of a timed response alone
        // would allow a reader that never reached the lock to make this test pass falsely.
        let contended = reader.dynamics.try_lock().is_none();
        contended_tx.send(contended).unwrap();
        let runtime = reader.dynamics.lock().output_projection_snapshot();
        let playback = reader.playback_runtime();
        observed_tx.send((runtime, playback)).unwrap();
    });
    let contended = contended_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    release_tx.send(()).unwrap();
    let (candidate, _) = committed_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let (runtime, playback) = observed_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    writing.join().unwrap();
    reading.join().unwrap();
    assert!(
        contended,
        "the actual shared Dynamics mutex must remain held inside final preparation"
    );
    assert_eq!(runtime, candidate);
    assert!(
        playback
            .iter()
            .any(|status| status.playback_number == Some(1) && status.enabled)
    );
}
