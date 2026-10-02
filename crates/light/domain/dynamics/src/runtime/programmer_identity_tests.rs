use super::super::{
    DynamicHeldPayload, DynamicHeldSampleSnapshot, DynamicSynchronizedResumeTransitionSnapshot,
};
use super::*;
use crate::*;
use light_core::{AttributeKey, FixtureId};
use std::sync::Arc;

fn definition() -> DynamicDefinition {
    DynamicDefinition {
        id: Uuid::from_u128(91),
        pool_number: 1,
        revision: 1,
        name: "Checkpoint identity".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::Targetless,
        lanes: vec![DynamicLane {
            id: Uuid::from_u128(92),
            body: DynamicLaneBody::LegacyScalar(LegacyScalarLaneBody {
                attribute: AttributeKey::intensity(),
                mode: DynamicLaneMode::Keyframes,
                keyframes: KeyframeConfiguration {
                    points: vec![
                        DynamicKeyframe {
                            position: 0.0,
                            source: ScalarSource::Value { value: 0.0 },
                            interpolation: ScalarInterpolation::Linear,
                        },
                        DynamicKeyframe {
                            position: 0.5,
                            source: ScalarSource::Value { value: 1.0 },
                            interpolation: ScalarInterpolation::Linear,
                        },
                    ],
                    size: 1.0,
                },
                max_min: MaxMinConfiguration {
                    minimum: ScalarSource::Value { value: 0.0 },
                    maximum: ScalarSource::Value { value: 1.0 },
                    function: PeriodicFunction::Sinus,
                    size: 1.0,
                    pwm: PwmShape::default(),
                },
                middle_amplitude: MiddleAmplitudeConfiguration {
                    middle: ScalarSource::Current,
                    amplitude: 0.5,
                    function: PeriodicFunction::Sinus,
                    size: 1.0,
                    pwm: PwmShape::default(),
                    invert_waveform: false,
                },
            }),
            speed_multiplier: Rational::ONE,
            width: 1.0,
            phase: None,
            random_group_id: None,
        }],
        random_groups: vec![],
        phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: DynamicSpatialMappingOverride::default(),
        phase: PhaseDistribution {
            ordering: PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: vec![],
        },
        speed: DynamicSpeed::Fixed {
            duration_millis: 1_000,
        },
        overall_speed_multiplier: Rational::ONE,
        run_mode: DynamicRunMode::Loop,
        default_activation: ActivationPolicy::StartNow,
        activation_boundary: ActivationBoundary::Beat,
    }
}

fn instance(owner: ProgrammerId, link: Uuid, instance_id: u128) -> DynamicInstanceSnapshot {
    let target = FixtureId(Uuid::from_u128(93));
    let definition = definition();
    let lane_id = definition.lanes[0].id;
    let tape = Arc::new(
        RetainedExpressionTape::from_roots(&[Arc::new(DynamicSampleExpression::LegacyScalar {
            attribute: AttributeKey::intensity(),
            value: 0.375,
            occurrence: Some(DynamicSourceOccurrenceId::new(Uuid::from_u128(96)).unwrap()),
            dependency_occurrence: Some(crate::DynamicSourceDependency::unknown(Some(
                DynamicSourceOccurrenceId::new(Uuid::from_u128(97)).unwrap(),
            ))),
        })])
        .unwrap(),
    );
    let held = DynamicHeldSampleSnapshot {
        controller_id: link,
        target,
        lane_id,
        payload: DynamicHeldPayload::TapeRoot {
            tape_root: tape.roots[0],
        },
    };
    DynamicInstanceSnapshot {
        id: Uuid::from_u128(instance_id),
        definition,
        targets: vec![target],
        phase_by_target: vec![],
        phase_by_lane_target: vec![(lane_id, target, 0.25)],
        controllers: vec![DynamicController {
            id: link,
            source: DynamicControllerSource::Programmer {
                programmer_id: owner.0,
                instance_link: None,
            },
            priority: 2,
            activated_at_millis: 100,
            size: 0.75,
            speed_multiplier: 1.5,
            phase_offset_degrees: 45.0,
            paused: true,
        }],
        lane_selections: vec![DynamicControllerLaneSelection {
            controller_id: link,
            selection: DynamicLaneSelection::Uniform {
                lanes: vec![lane_id],
            },
        }],
        controller_transitions: vec![DynamicControllerTransitionSnapshot {
            controller_id: link,
            activation_started_at_millis: 100,
            activation_delay_millis: 50,
            activation_duration_millis: 1_000,
            release_started_at_millis: Some(500),
            release_delay_millis: 20,
            release_duration_millis: 2_000,
            output_gate: None,
        }],
        started_at_millis: 100,
        paused_at_millis: Some(300),
        paused_elapsed_millis: 40,
        activation_policy: ActivationPolicy::StartNow,
        pending_until_millis: Some(1_000),
        speed_paused_at_millis: Some(250),
        speed_paused_elapsed_millis: 30,
        random_streams: vec![DynamicRandomStreamSnapshot {
            group_id: Uuid::from_u128(94),
            target,
            last_elapsed_millis: 200,
            next_decision_index: 7,
            active: Some(DynamicRandomPulseSnapshot {
                started_at_millis: 180,
                duration_millis: 220,
            }),
        }],
        completed: false,
        synchronized_hold_elapsed_millis: Some(180),
        synchronized_hold_captured: true,
        last_synchronized_elapsed_millis: Some(180),
        synchronized_resume_transition: Some(DynamicSynchronizedResumeTransitionSnapshot {
            occurrence_id: Uuid::from_u128(95),
            started_at_millis: 300,
            duration_millis: 600,
            held_elapsed_millis: 180,
        }),
        last_sample_values: vec![held.clone()],
        synchronized_hold_values: vec![held],
        expression_tape: Some(tape),
        preset_source_values: vec![],
    }
}

fn snapshot(instances: Vec<DynamicInstanceSnapshot>) -> DynamicRuntimeSnapshot {
    DynamicRuntimeSnapshot {
        global_paused: true,
        instances,
    }
}

#[test]
fn verified_legacy_identity_rekeys_every_row_without_changing_clock_history_or_tape() {
    let owner = ProgrammerId(Uuid::from_u128(1));
    let link = Uuid::from_u128(2);
    let new_id = programmer_dynamic_controller_id(owner, link);
    let mut stored = snapshot(vec![instance(owner, link, 3)]);
    let original_tape = stored.instances[0].expression_tape.clone().unwrap();
    let mut expected = stored.clone();
    let expected_instance = &mut expected.instances[0];
    expected_instance.controllers[0].id = new_id;
    expected_instance.controllers[0].source = DynamicControllerSource::Programmer {
        programmer_id: owner.0,
        instance_link: Some(link),
    };
    expected_instance.lane_selections[0].controller_id = new_id;
    expected_instance.controller_transitions[0].controller_id = new_id;
    expected_instance.last_sample_values[0].controller_id = new_id;
    expected_instance.synchronized_hold_values[0].controller_id = new_id;

    assert_eq!(
        normalize_legacy_programmer_controller_ids(&mut stored, &[(owner, link)]),
        Ok(1)
    );
    assert_eq!(stored, expected);
    assert!(Arc::ptr_eq(
        stored.instances[0].expression_tape.as_ref().unwrap(),
        &original_tape
    ));
    assert_eq!(
        normalize_legacy_programmer_controller_ids(&mut stored, &[(owner, link)]),
        Ok(0)
    );
    let mut runtime = DynamicRuntime::default();
    runtime.restore_snapshot(stored).unwrap();
    assert_eq!(runtime.controller(new_id).unwrap().0, Uuid::from_u128(3));
    assert!(runtime.controller(link).is_none());
    let restored = runtime.snapshot();
    assert_eq!(
        restored.instances[0].random_streams,
        expected.instances[0].random_streams
    );
    assert_eq!(restored.instances[0].started_at_millis, 100);
    assert_eq!(
        restored.instances[0].controller_transitions,
        expected.instances[0].controller_transitions
    );
}

#[test]
fn unmatched_legacy_and_explicit_sources_are_not_guessed_or_relabelled() {
    let owner = ProgrammerId(Uuid::from_u128(1));
    let link = Uuid::from_u128(2);
    let mut explicit = instance(owner, link, 4);
    explicit.controllers[0].source = DynamicControllerSource::Programmer {
        programmer_id: owner.0,
        instance_link: Some(Uuid::from_u128(6)),
    };
    let mut stored = snapshot(vec![instance(owner, link, 3), explicit]);
    let original = stored.clone();
    let wrong_owner = ProgrammerId(Uuid::from_u128(7));
    assert_eq!(
        normalize_legacy_programmer_controller_ids(
            &mut stored,
            &[(wrong_owner, link), (owner, Uuid::from_u128(6))]
        ),
        Ok(0)
    );
    assert_eq!(stored, original);
}

#[test]
fn collision_found_after_an_earlier_valid_mapping_rolls_back_the_entire_snapshot() {
    let owner = ProgrammerId(Uuid::from_u128(1));
    let first = Uuid::from_u128(2);
    let second = Uuid::from_u128(3);
    let collision = programmer_dynamic_controller_id(owner, second);
    let mut occupied = instance(owner, collision, 13);
    occupied.controllers[0].source = DynamicControllerSource::physical_playback(1);
    let mut stored = snapshot(vec![
        instance(owner, first, 11),
        instance(owner, second, 12),
        occupied,
    ]);
    let original = stored.clone();
    assert!(
        normalize_legacy_programmer_controller_ids(&mut stored, &[(owner, first), (owner, second)])
            .is_err()
    );
    assert_eq!(stored, original);
}

#[test]
fn existing_explicit_claim_of_the_same_source_rejects_even_with_a_different_id() {
    let owner = ProgrammerId(Uuid::from_u128(1));
    let link = Uuid::from_u128(2);
    let mut explicit = instance(owner, Uuid::from_u128(8), 4);
    explicit.controllers[0].source = DynamicControllerSource::Programmer {
        programmer_id: owner.0,
        instance_link: Some(link),
    };
    let mut stored = snapshot(vec![instance(owner, link, 3), explicit]);
    let original = stored.clone();
    assert!(normalize_legacy_programmer_controller_ids(&mut stored, &[(owner, link)]).is_err());
    assert_eq!(stored, original);
}

#[test]
fn shared_imported_link_from_distinct_owners_keeps_both_existing_motion_instances() {
    let first_owner = ProgrammerId(Uuid::from_u128(1));
    let second_owner = ProgrammerId(Uuid::from_u128(2));
    let link = Uuid::from_u128(3);
    let mut stored = snapshot(vec![
        instance(first_owner, link, 4),
        instance(second_owner, link, 5),
    ]);
    assert_eq!(
        normalize_legacy_programmer_controller_ids(
            &mut stored,
            &[(first_owner, link), (second_owner, link)]
        ),
        Ok(2)
    );
    assert_eq!(stored.instances[0].id, Uuid::from_u128(4));
    assert_eq!(stored.instances[1].id, Uuid::from_u128(5));
    let first_id = programmer_dynamic_controller_id(first_owner, link);
    let second_id = programmer_dynamic_controller_id(second_owner, link);
    assert_ne!(first_id, second_id);
    assert_eq!(
        stored.instances[0].last_sample_values[0].controller_id,
        first_id
    );
    assert_eq!(
        stored.instances[1].last_sample_values[0].controller_id,
        second_id
    );
    let mut runtime = DynamicRuntime::default();
    runtime.restore_snapshot(stored).unwrap();
    assert_eq!(runtime.controller(first_id).unwrap().0, Uuid::from_u128(4));
    assert_eq!(runtime.controller(second_id).unwrap().0, Uuid::from_u128(5));
}

#[test]
fn ambiguous_old_key_within_one_instance_is_rejected_before_rekeying_rows() {
    let first_owner = ProgrammerId(Uuid::from_u128(1));
    let second_owner = ProgrammerId(Uuid::from_u128(2));
    let link = Uuid::from_u128(3);
    let mut first = instance(first_owner, link, 4);
    first
        .controllers
        .push(instance(second_owner, link, 5).controllers.remove(0));
    let mut stored = snapshot(vec![first]);
    let original = stored.clone();
    assert!(
        normalize_legacy_programmer_controller_ids(
            &mut stored,
            &[(first_owner, link), (second_owner, link)]
        )
        .is_err()
    );
    assert_eq!(stored, original);
}
