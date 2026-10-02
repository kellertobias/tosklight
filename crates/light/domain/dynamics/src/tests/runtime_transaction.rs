use super::*;
use light_core::programming::ProgrammingComponent;

fn transports() -> [DynamicSpeedTransport; 5] {
    [DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: 0,
        beat_phase: 0.0,
        phase_advancing: true,
    }; 5]
}
fn sample_all(runtime: &mut DynamicRuntime, at: u64) -> Vec<DynamicRuntimeSample> {
    runtime.sample_all_addressed(at, 10, &transports(), &Sources { current: 0.0 }, None)
}
fn sorted(mut samples: Vec<DynamicRuntimeSample>) -> Vec<DynamicRuntimeSample> {
    samples.sort_by_key(|sample| {
        (
            sample.instance_id,
            sample.controller_id,
            sample.target.0,
            sample.lane_id,
        )
    });
    samples
}
fn random_definition() -> DynamicDefinition {
    let mut random = lane();
    random.legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    let group = Uuid::from_u128(801);
    random.random_group_id = Some(group);
    let mut definition = definition(random);
    definition.random_groups.push(DynamicRandomGroup {
        id: group,
        seed: 17,
        range: DynamicRandomRange::LegacyScalar {
            low: source(0.0),
            high: source(1.0),
        },
        decision_interval_millis: 100,
        start_probability: 1.0,
        mean_duration_millis: 250,
        duration_spread_millis: 30,
        attack_ratio: 0.1,
        decay_ratio: 0.5,
    });
    definition
}

#[test]
fn programmer_link_metadata_roundtrips_without_splitting_targetless_clock() {
    let dynamic = definition(lane());
    let id = dynamic.id;
    let target = FixtureId::new();
    let programmer_id = Uuid::from_u128(900);
    let first_link = Uuid::from_u128(901);
    let second_link = Uuid::from_u128(902);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic.clone()]).unwrap();
    let mut first = controller(901, 1, false);
    first.source = DynamicControllerSource::Programmer {
        programmer_id,
        instance_link: Some(first_link),
    };
    let instance = runtime
        .start(start_request(id, first, target, 100, true))
        .unwrap();
    let mut second = controller(902, 2, false);
    second.source = DynamicControllerSource::Programmer {
        programmer_id,
        instance_link: Some(second_link),
    };
    let reused = runtime
        .start(start_request(id, second, target, 250, true))
        .unwrap();
    assert_eq!(reused, instance);
    assert_eq!(runtime.instance_count(), 1);
    assert_eq!(runtime.snapshot().instances[0].started_at_millis, 100);

    let encoded = serde_json::to_value(runtime.snapshot()).unwrap();
    let decoded: DynamicRuntimeSnapshot = serde_json::from_value(encoded.clone()).unwrap();
    assert!(decoded.instances[0].controllers.iter().any(|controller| {
        matches!(&controller.source, DynamicControllerSource::Programmer {
            instance_link: Some(link), ..
        } if *link == first_link)
    }));
    assert!(decoded.instances[0].controllers.iter().any(|controller| {
        matches!(&controller.source, DynamicControllerSource::Programmer {
            instance_link: Some(link), ..
        } if *link == second_link)
    }));
    let mut restored = DynamicRuntime::default();
    restored.install_definitions([dynamic]).unwrap();
    restored.restore_snapshot(decoded).unwrap();
    assert_eq!(restored.snapshot().instances[0].started_at_millis, 100);

    let mut legacy = encoded;
    for controller in legacy["instances"][0]["controllers"]
        .as_array_mut()
        .unwrap()
    {
        controller["source"]
            .as_object_mut()
            .unwrap()
            .remove("instance_link");
    }
    let legacy: DynamicRuntimeSnapshot = serde_json::from_value(legacy).unwrap();
    assert!(legacy.instances[0].controllers.iter().all(|controller| {
        matches!(
            &controller.source,
            DynamicControllerSource::Programmer {
                instance_link: None,
                ..
            }
        )
    }));
}

#[test]
fn cancel_release_and_restamp_preserve_clock_and_rollback_cleanly() {
    let dynamic = definition(lane());
    let id = dynamic.id;
    let target = FixtureId::new();
    let controller_id = Uuid::from_u128(903);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let instance = runtime
        .start(start_request(
            id,
            controller(903, 1, false),
            target,
            100,
            true,
        ))
        .unwrap();
    sample_all(&mut runtime, 200);
    runtime
        .off_controller(instance, controller_id, 250, 10, 500)
        .unwrap();
    let before = runtime.snapshot();
    let mut scratch = DynamicOutputFrameScratch::default();
    let failed: Result<(), &'static str> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.cancel_controller_release(controller_id).unwrap();
            runtime
                .update_controller_rank(controller_id, 7, 300, 300)
                .unwrap();
            Err("retry")
        });
    assert_eq!(failed, Err("retry"));
    assert_eq!(runtime.snapshot(), before);

    runtime.cancel_controller_release(controller_id).unwrap();
    runtime
        .update_controller_rank(controller_id, 7, 300, 300)
        .unwrap();
    let after = runtime.snapshot();
    let instance = &after.instances[0];
    assert_eq!(
        instance.started_at_millis,
        before.instances[0].started_at_millis
    );
    assert_eq!(instance.controllers[0].priority, 7);
    assert_eq!(instance.controllers[0].activated_at_millis, 300);
    assert_eq!(
        instance.controller_transitions[0].activation_started_at_millis,
        before.instances[0].controller_transitions[0].activation_started_at_millis
    );
    assert_eq!(
        instance.controller_transitions[0].release_started_at_millis,
        None
    );
    assert_eq!(instance.controller_transitions[0].release_delay_millis, 0);
    assert_eq!(
        instance.controller_transitions[0].release_duration_millis,
        0
    );
    assert_eq!(instance.random_streams, before.instances[0].random_streams);
}

#[test]
fn restamping_a_shared_controller_reconciles_pause_at_observation_time() {
    let dynamic = definition(lane());
    let id = dynamic.id;
    let target = FixtureId::new();
    let programmer_id = Uuid::from_u128(940);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let mut live = controller(941, 2, false);
    live.source = DynamicControllerSource::Programmer {
        programmer_id,
        instance_link: Some(Uuid::from_u128(1)),
    };
    let instance = runtime
        .start(start_request(id, live, target, 100, true))
        .unwrap();
    let mut held = controller(942, 1, true);
    held.source = DynamicControllerSource::Programmer {
        programmer_id,
        instance_link: Some(Uuid::from_u128(2)),
    };
    assert_eq!(
        runtime
            .start(start_request(id, held, target, 100, true))
            .unwrap(),
        instance
    );
    assert_eq!(runtime.snapshot().instances[0].paused_at_millis, None);
    runtime
        .update_controller_rank(Uuid::from_u128(942), 3, 200, 500)
        .unwrap();
    assert_eq!(runtime.snapshot().instances[0].paused_at_millis, Some(500));
    runtime
        .update_controller_rank(Uuid::from_u128(941), 4, 300, 700)
        .unwrap();
    let state = &runtime.snapshot().instances[0];
    assert_eq!(state.paused_at_millis, None);
    assert_eq!(state.paused_elapsed_millis, 200);
    assert_eq!(state.started_at_millis, 100);
}

#[test]
fn rejected_frame_restores_random_history_and_completed_release_then_retries_identically() {
    let random = random_definition();
    let regular = definition(lane());
    let ids = [random.id, regular.id];
    let target = FixtureId::new();
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([random, regular]).unwrap();
    runtime
        .start(start_request(
            ids[0],
            controller(801, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let release = runtime
        .start(start_request(
            ids[1],
            controller(802, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    sample_all(&mut runtime, 150);
    runtime
        .off_controller(release, Uuid::from_u128(802), 150, 0, 100)
        .unwrap();
    let before = runtime.snapshot();
    let mut expected = runtime.fork_for_preview();
    let samples = sorted(sample_all(&mut expected, 675));
    let mut scratch = DynamicOutputFrameScratch::default();
    for _ in 0..2 {
        let failed: Result<(), &'static str> =
            runtime.with_output_frame_transaction(&mut scratch, |runtime| {
                assert_eq!(sorted(sample_all(runtime, 675)), samples);
                assert_eq!(runtime.instance_count(), 1);
                Err("encoding failed")
            });
        assert_eq!(failed, Err("encoding failed"));
        assert_eq!(runtime.snapshot(), before);
    }
    let actual = runtime
        .with_output_frame_transaction(&mut scratch, |runtime| {
            Ok::<_, ()>(sorted(sample_all(runtime, 675)))
        })
        .unwrap();
    assert_eq!(actual, samples);
    assert_eq!(runtime.snapshot(), expected.snapshot());
}

#[test]
fn cold_reconciliation_and_mutation_after_sampling_restore_the_original_frame() {
    let mut original = definition(lane());
    let first = FixtureId::new();
    let second = FixtureId::new();
    original.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![first],
    };
    let id = original.id;
    let lane_id = original.lanes[0].id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([original]).unwrap();
    let instance = runtime
        .start(start_request(
            id,
            controller(803, 1, false),
            first,
            0,
            false,
        ))
        .unwrap();
    sample_all(&mut runtime, 125);
    let before = runtime.snapshot();
    let fallback = definition(lane());
    let fallback_id = fallback.id;
    let mut scratch = DynamicOutputFrameScratch::default();
    let failed: Result<(), u8> = runtime.with_output_frame_transaction(&mut scratch, |runtime| {
        sample_all(runtime, 250);
        runtime
            .update_controller(Uuid::from_u128(803), Some(0.3), Some(2.0), None)
            .unwrap();
        runtime
            .reconcile_instance_targets(
                instance,
                DynamicTargetScope {
                    ordered_targets: vec![second],
                },
                &HashMap::new(),
                None,
            )
            .unwrap();
        runtime
            .set_controller_lane_selection(
                instance,
                Uuid::from_u128(803),
                DynamicLaneSelection::Uniform {
                    lanes: vec![lane_id],
                },
            )
            .unwrap();
        runtime.set_global_paused(true, 250);
        sample_all(runtime, 500);
        runtime.install_fallback_definition(fallback).unwrap();
        runtime
            .start(start_request(
                fallback_id,
                controller(804, 2, false),
                second,
                250,
                false,
            ))
            .unwrap();
        runtime
            .off_controller(instance, Uuid::from_u128(803), 500, 0, 0)
            .unwrap();
        Err(7)
    });
    assert_eq!(failed, Err(7));
    assert_eq!(runtime.snapshot(), before);
    assert!(matches!(
        runtime.start(start_request(
            fallback_id,
            controller(804, 2, false),
            second,
            250,
            false
        )),
        Err(DynamicRuntimeError::MissingDefinition)
    ));
    assert_eq!(
        runtime
            .start(start_request(
                id,
                controller(805, 2, false),
                first,
                250,
                false
            ))
            .unwrap(),
        instance,
        "rollback restores the bound-instance index"
    );
}

struct TypedCurrent(f32);
impl DynamicValueSourceResolver for TypedCurrent {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        Some(DynamicValue::Scalar(self.0))
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}
fn angle_definition() -> DynamicDefinition {
    let mut result = definition(DynamicLane {
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: Some(ProgrammingComponent::Pan),
            },
            configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
                minimum: DynamicValueSource::Value {
                    value: DynamicValue::Scalar(90.0),
                },
                maximum: DynamicValueSource::Value {
                    value: DynamicValue::Scalar(90.0),
                },
                function: PeriodicFunction::LinearUp,
                size: 1.0,
                pwm: PwmShape::default(),
            }),
        }),
        ..lane()
    });
    result.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    result.default_activation = ActivationPolicy::JoinSyncNow;
    result
}
fn typed(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    at: u64,
    current: f32,
) -> Result<Vec<DynamicRuntimeSample>, DynamicRuntimeError> {
    runtime.sample_programming(
        instance,
        at,
        1000,
        10,
        &Sources { current: 0.0 },
        &TypedCurrent(current),
    )
}

#[test]
fn actual_typed_failure_and_paused_current_preserve_the_last_committed_history() {
    let definition = angle_definition();
    let id = definition.id;
    let target = FixtureId::new();
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let mut request = start_request(id, controller(806, 1, false), target, 0, false);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    typed(&mut runtime, instance, 100, 20.0).unwrap();
    runtime.set_global_paused(true, 100);
    let before = runtime.snapshot();
    let mut scratch = DynamicOutputFrameScratch::default();
    let invalid = runtime.with_output_frame_transaction(&mut scratch, |runtime| {
        let pending = typed(runtime, instance, 200, f32::NAN).unwrap();
        prepare_dynamic_family_samples(
            &pending,
            &TypedCurrent(f32::NAN),
            None,
            &mut DynamicFamilyPreparationScratch::default(),
        )
        .map(|_| ())
    });
    assert!(invalid.is_err());
    assert_eq!(runtime.snapshot(), before);
    let mut expected = runtime.fork_for_preview();
    let expected_samples = sorted(typed(&mut expected, instance, 200, 40.0).unwrap());
    let failed: Result<(), ()> = runtime.with_output_frame_transaction(&mut scratch, |runtime| {
        assert_eq!(
            sorted(typed(runtime, instance, 200, 40.0).unwrap()),
            expected_samples
        );
        runtime.set_global_paused(false, 200);
        typed(runtime, instance, 500, 40.0).unwrap();
        Err(())
    });
    assert!(failed.is_err());
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(
        runtime
            .with_output_frame_transaction(&mut scratch, |runtime| typed(
                runtime, instance, 200, 40.0
            ))
            .map(sorted)
            .unwrap(),
        expected_samples
    );
    assert_eq!(runtime.snapshot(), expected.snapshot());
}

#[test]
fn panic_unwinds_the_output_transaction_and_reuses_its_workspace() {
    let definition = definition(lane());
    let id = definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    runtime
        .start(start_request(
            id,
            controller(807, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    sample_all(&mut runtime, 100);
    let before = runtime.snapshot();
    let mut scratch = DynamicOutputFrameScratch::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), ()> = runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            sample_all(runtime, 900);
            panic!("test calculation panic")
        });
    }));
    assert!(result.is_err());
    assert_eq!(runtime.snapshot(), before);
    runtime
        .with_output_frame_transaction(&mut scratch, |runtime| {
            Ok::<_, ()>(sample_all(runtime, 900))
        })
        .unwrap();
    assert_ne!(runtime.snapshot(), before);
}

#[test]
fn unchanged_controller_pause_reconciles_initial_instance_state_and_rolls_back() {
    let definition = angle_definition();
    let id = definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let instance = runtime
        .start(start_request(
            id,
            controller(808, 1, true),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let before = runtime.snapshot();
    assert!(before.instances[0].controllers[0].paused);
    assert_eq!(before.instances[0].paused_at_millis, None);
    let mut scratch = DynamicOutputFrameScratch::default();
    let failed: Result<(), ()> = runtime.with_output_frame_transaction(&mut scratch, |runtime| {
        runtime
            .set_controller_paused(instance, Uuid::from_u128(808), true, 25)
            .unwrap();
        assert_eq!(runtime.snapshot().instances[0].paused_at_millis, Some(25));
        typed(runtime, instance, 100, 15.0).unwrap();
        assert!(runtime.snapshot().instances[0].synchronized_hold_captured);
        Err(())
    });
    assert!(failed.is_err());
    assert_eq!(runtime.snapshot(), before);
    runtime
        .with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.set_controller_paused(instance, Uuid::from_u128(808), true, 25)?;
            typed(runtime, instance, 100, 15.0)
        })
        .unwrap();
    let paused = runtime.snapshot();
    runtime
        .set_controller_paused(instance, Uuid::from_u128(808), true, 500)
        .unwrap();
    assert_eq!(
        runtime.snapshot(),
        paused,
        "an already reconciled pause preserves its timestamp and held source membership"
    );
}
