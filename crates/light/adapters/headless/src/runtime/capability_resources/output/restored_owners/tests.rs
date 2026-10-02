use super::*;
use crate::runtime::dynamic_source_origins::{
    DynamicFixedSource, DynamicFixedStamp, DynamicProgrammerSourceLane, DynamicSourceOrigin,
    DynamicSourceOrigins,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, ProgrammerId, SessionId};
use light_dynamics::{
    DynamicAddressValue, DynamicController, DynamicControllerSource, DynamicDefinition,
    DynamicDefinitionSnapshot, DynamicInstanceOverrides, DynamicReference, DynamicSemanticValue,
    DynamicSpeedTransport, DynamicStartRequest, DynamicTargetScope, DynamicValueTiming, Rational,
    ScalarSource, ScalarSourceResolver,
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
    let output = super::super::publication_tests::output_with_programmers(programmers.clone());
    output
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    (output, programmers, state)
}

fn orphan(
    output: &OutputResource,
    definition: &DynamicDefinition,
) -> (
    DynamicSourceBinding,
    light_dynamics::DynamicSourceOccurrenceId,
) {
    let programmer_id = ProgrammerId::new();
    let link = Uuid::new_v4();
    let target = FixtureId::new();
    let controller_id = light_dynamics::programmer_dynamic_controller_id(programmer_id, link);
    let instance_id = output
        .dynamics
        .lock()
        .start(DynamicStartRequest {
            definition_id: definition.id,
            controller: DynamicController {
                id: controller_id,
                source: DynamicControllerSource::Programmer {
                    programmer_id: programmer_id.0,
                    instance_link: Some(link),
                },
                priority: 100,
                activated_at_millis: 1_000,
                size: 1.0,
                speed_multiplier: 1.0,
                phase_offset_degrees: 0.0,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: vec![target],
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 1_000,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    let binding = DynamicSourceBinding::Authored {
        instance_id,
        controller_id,
        target,
        lane_id: definition.lanes[0].id,
    };
    let mut origins = DynamicSourceOrigins::default();
    let occurrence = origins
        .bind(
            binding,
            DynamicSourceOrigin::Programmer {
                programmer_id,
                lane: DynamicProgrammerSourceLane::Live,
                instance_link: link,
                changed_at_millis: 1_000,
                programmer_order: 1,
            },
        )
        .unwrap();
    output.dynamic_source_origins.store(Arc::new(origins));
    (binding, occurrence)
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

#[test]
fn surviving_owner_keeps_instance_clock_expression_history_and_sample_boundary() {
    let mut definition = definition();
    // Exercise retained Current history, not merely an untouched empty runtime.
    for point in &mut definition.lanes[0].legacy_mut().unwrap().keyframes.points {
        point.source = ScalarSource::Current;
    }
    let target = FixtureId::new();
    let (output, _, _) = desk(&definition, vec![row(&definition, Uuid::new_v4(), target)]);
    let playback = playback();
    output.finalize_restored_owners(&playback).unwrap();
    let (before, sample) = {
        let mut runtime = output.dynamics.lock();
        runtime.sample_all_addressed(
            1_500,
            25,
            &[DynamicSpeedTransport {
                effective_bpm: 120.0,
                phase_origin_millis: 0,
                phase_reference_millis: 1_500,
                beat_phase: 3.0,
                phase_advancing: true,
            }; 5],
            &Sources,
            None,
        );
        (runtime.snapshot(), runtime.committed_sample_boundary())
    };
    assert_eq!(before.instances.len(), 1);
    assert!(before.instances[0].expression_tape.is_some());
    output.finalize_restored_owners(&playback).unwrap();
    let runtime = output.dynamics.lock();
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.committed_sample_boundary(), sample);
}

#[test]
fn orphan_lookup_is_retired_but_immutable_evidence_and_fixed_binding_survive() {
    let definition = definition();
    let (output, _, state) = desk(&definition, vec![]);
    let (authored, occurrence) = orphan(&output, &definition);
    let fixed = DynamicSourceBinding::Fixed {
        source: DynamicFixedSource::Programmer {
            programmer_id: state.id,
            lane: DynamicProgrammerSourceLane::Live,
        },
        target: FixtureId::new(),
        owner: light_core::programming::ProgrammingOwner::Focus,
        component: None,
    };
    let mut origins = (**output.dynamic_source_origins.load()).clone();
    let fixed_occurrence = origins
        .bind(
            fixed,
            DynamicSourceOrigin::Fixed {
                stamp: DynamicFixedStamp::Programmer {
                    changed_at_millis: 1_000,
                    programmer_order: 2,
                },
                priority: 100,
                value: DynamicSemanticValue::Static {
                    value: AttributeValue::Normalized(0.4),
                    timing: Default::default(),
                },
            },
        )
        .unwrap();
    let published = Arc::new(origins);
    output.dynamic_source_origins.store(published.clone());
    let snapshot = output.engine.snapshot();
    let (cold, _) = output
        .dynamic_snapshot
        .begin_retained_history(
            &mut output.dynamics.lock(),
            &snapshot,
            NonZeroUsize::new(8).unwrap(),
        )
        .unwrap();
    let input = output.dynamic_snapshot.input_capture_cursor();
    output.finalize_restored_owners(&playback()).unwrap();
    assert!(output.dynamics.lock().snapshot().instances.is_empty());
    let origins = output.dynamic_source_origins.load_full();
    assert_eq!(origins.binding(&authored), None);
    assert!(origins.get(occurrence).is_some());
    assert_eq!(origins.binding(&fixed), Some(fixed_occurrence));
    assert_eq!(published.binding(&authored), Some(occurrence));
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(cold)
            .is_err()
    );
    assert_ne!(output.dynamic_snapshot.input_capture_cursor(), input);
    assert!(Arc::ptr_eq(&snapshot, &output.engine.snapshot()));
}

#[test]
fn invalid_final_owner_discards_partial_reconciliation_and_keeps_all_cursors() {
    let definition = definition();
    let mut invalid = row(&definition, Uuid::new_v4(), FixtureId::new());
    let DynamicSemanticValue::DynamicOn { overrides, .. } = &mut invalid.value else {
        unreachable!()
    };
    overrides.size = -1.0;
    let (output, _, _) = desk(&definition, vec![invalid]);
    let (binding, occurrence) = orphan(&output, &definition);
    let snapshot = output.engine.snapshot();
    let mut runtime = output.dynamics.lock();
    let seed = output
        .dynamic_snapshot
        .begin_retained_history(&mut runtime, &snapshot, NonZeroUsize::new(8).unwrap())
        .unwrap();
    let before = runtime.snapshot();
    let sample = runtime.committed_sample_boundary();
    drop(runtime);
    let origins = output.dynamic_source_origins.load_full();
    let input = output.dynamic_snapshot.input_capture_cursor();
    assert!(output.finalize_restored_owners(&playback()).is_err());
    let mut runtime = output.dynamics.lock();
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.committed_sample_boundary(), sample);
    assert_eq!(runtime.control_cursor(), Some(seed.1));
    assert_eq!(
        output
            .dynamic_snapshot
            .begin_retained_history(&mut runtime, &snapshot, NonZeroUsize::new(8).unwrap())
            .unwrap(),
        seed
    );
    assert_eq!(output.dynamic_snapshot.input_capture_cursor(), input);
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(seed.0)
            .unwrap()
            .is_empty()
    );
    let after_origins = output.dynamic_source_origins.load_full();
    assert!(Arc::ptr_eq(&origins, &after_origins));
    assert_eq!(after_origins.binding(&binding), Some(occurrence));
    assert!(Arc::ptr_eq(&snapshot, &output.engine.snapshot()));
}

#[test]
fn missing_group_is_a_passive_success() {
    let (output, programmers, mut state) = desk(&definition(), vec![]);
    let mut definition = definition();
    definition.target_binding = light_dynamics::DynamicTargetBinding::LiveGroup {
        group_id: "not-in-this-show".into(),
    };
    // The active show cannot contain an unresolved declared Group reference, but an older
    // retained fallback can outlive both its show definition and its original Group.
    state.dynamic_values = Arc::new(vec![row(&definition, Uuid::new_v4(), FixtureId::new())]);
    programmers.restore(state);
    output.finalize_restored_owners(&playback()).unwrap();
    assert!(output.dynamics.lock().snapshot().instances.is_empty());
}
