use super::*;

fn setup(dynamic: DynamicDefinition) -> (DynamicRuntime, Uuid, Uuid) {
    let id = dynamic.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let controller = controller(1701, 1, false);
    let controller_id = controller.id;
    let instance = runtime
        .start(start_request(id, controller, FixtureId::new(), 0, false))
        .unwrap();
    (runtime, instance, controller_id)
}

fn samples(runtime: &mut DynamicRuntime, at: u64) -> Vec<DynamicRuntimeSample> {
    let transport = DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: at,
        beat_phase: (at as f64 / 1_000.0).rem_euclid(1.0),
        phase_advancing: true,
    };
    runtime.sample_all(at, 10, &[transport; 5], &Sources { current: 0.2 })
}

fn assert_same_motion(muted: &mut DynamicRuntime, oracle: &mut DynamicRuntime, at: u64, mix: f32) {
    let actual = samples(muted, at);
    let mut expected = samples(oracle, at);
    assert!(!actual.is_empty(), "a muted source must still be sampled");
    for sample in &mut expected {
        sample.activation_mix *= mix;
    }
    assert_eq!(actual, expected);
    let mut checkpoint = muted.snapshot();
    for instance in &mut checkpoint.instances {
        for transition in &mut instance.controller_transitions {
            transition.output_gate = None;
        }
    }
    assert_eq!(
        checkpoint,
        oracle.snapshot(),
        "the mask must only affect output influence"
    );
}

fn random_definition() -> DynamicDefinition {
    let mut lane = lane();
    lane.legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    let group_id = Uuid::from_u128(1702);
    lane.random_group_id = Some(group_id);
    let mut dynamic = definition(lane);
    dynamic.random_groups.push(DynamicRandomGroup {
        id: group_id,
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
    dynamic
}

#[test]
fn completed_gate_preserves_motion_random_and_history_until_explicit_destructive_off() {
    for dynamic in [definition(lane()), random_definition()] {
        let (mut runtime, instance, controller_id) = setup(dynamic);
        samples(&mut runtime, 200);
        let mut oracle = runtime.fork_for_preview();
        assert!(
            runtime
                .set_controller_output_enabled(controller_id, false, 200, 100, 400)
                .unwrap()
        );
        for (at, mix) in [
            (250, 1.0),
            (500, 0.5),
            (700, 0.0),
            (1_300, 0.0),
            (2_100, 0.0),
        ] {
            assert_same_motion(&mut runtime, &mut oracle, at, mix);
        }
        assert_eq!(runtime.controller(controller_id).unwrap().0, instance);
        assert!(runtime.clear_controller_output_gate(controller_id).unwrap());
        assert!(!runtime.clear_controller_output_gate(controller_id).unwrap());
        assert_same_motion(&mut runtime, &mut oracle, 2_350, 1.0);
        assert!(
            runtime
                .off_controller(instance, controller_id, 2_400, 0, 0)
                .unwrap()
        );
        assert_eq!(runtime.instance_count(), 0);
    }
}

#[test]
fn gate_reversal_starts_from_current_influence_and_repeated_state_does_not_restart() {
    let (mut runtime, _, controller_id) = setup(definition(lane()));
    assert!(
        !runtime
            .set_controller_output_enabled(controller_id, true, 10, 40, 300)
            .unwrap()
    );
    assert!(!runtime.clear_controller_output_gate(controller_id).unwrap());
    runtime
        .set_controller_output_enabled(controller_id, false, 100, 100, 400)
        .unwrap();
    assert_eq!(samples(&mut runtime, 100)[0].activation_mix, 1.0);
    assert!(
        !runtime
            .set_controller_output_enabled(controller_id, false, 300, 100, 400)
            .unwrap()
    );
    assert_eq!(samples(&mut runtime, 400)[0].activation_mix, 0.5);
    runtime
        .set_controller_output_enabled(controller_id, true, 400, 0, 200)
        .unwrap();
    assert!(
        !runtime
            .set_controller_output_enabled(controller_id, true, 450, 0, 200)
            .unwrap()
    );
    assert_eq!(samples(&mut runtime, 500)[0].activation_mix, 0.75);
    assert_eq!(samples(&mut runtime, 600)[0].activation_mix, 1.0);
    assert!(
        !runtime
            .set_controller_output_enabled(controller_id, true, 900, 0, 200)
            .unwrap()
    );
    assert!(
        runtime
            .set_controller_output_enabled(controller_id, true, 900, 0, 0)
            .unwrap()
    );
    assert!(
        !runtime
            .set_controller_output_enabled(controller_id, true, 1_000, 0, 0)
            .unwrap()
    );
}

#[test]
fn output_gate_fades_on_output_clock_while_synchronized_motion_remains_paused_and_restorable() {
    let mut dynamic = definition(lane());
    dynamic.default_activation = ActivationPolicy::JoinSyncNow;
    dynamic.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    let (mut runtime, instance, controller_id) = setup(dynamic);
    samples(&mut runtime, 250);
    runtime
        .set_controller_paused(instance, controller_id, true, 250)
        .unwrap();
    samples(&mut runtime, 250);
    let mut oracle = runtime.fork_for_preview();
    runtime
        .set_controller_output_enabled(controller_id, false, 250, 0, 500)
        .unwrap();
    assert_same_motion(&mut runtime, &mut oracle, 500, 0.5);
    let serialized = serde_json::to_string(&runtime.snapshot()).unwrap();
    let mut restored = DynamicRuntime::default();
    restored
        .restore_snapshot(serde_json::from_str(&serialized).unwrap())
        .unwrap();
    assert_same_motion(&mut restored, &mut oracle, 1_500, 0.0);
    assert!(restored.snapshot().instances[0].synchronized_hold_captured);
    assert!(
        !restored.snapshot().instances[0]
            .synchronized_hold_values
            .is_empty()
    );
    restored
        .clear_controller_output_gate(controller_id)
        .unwrap();
    assert_same_motion(&mut restored, &mut oracle, 1_600, 1.0);
    for runtime in [&mut restored, &mut oracle] {
        runtime
            .set_controller_paused_with_resume(
                instance,
                controller_id,
                false,
                1_600,
                Some(ActivationPolicy::JoinSyncNow),
            )
            .unwrap();
    }
    assert_same_motion(&mut restored, &mut oracle, 1_700, 1.0);
}

#[test]
fn gate_and_sampling_rollback_together_and_identical_retry_commits_once() {
    let (mut runtime, _, controller_id) = setup(random_definition());
    samples(&mut runtime, 100);
    let before = runtime.snapshot();
    let mut scratch = DynamicOutputFrameScratch::default();
    let failed: Result<(), &'static str> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            // Exercise warm sampling before the cold control mutation, then more sampling.
            samples(runtime, 200);
            runtime
                .set_controller_output_enabled(controller_id, false, 200, 0, 100)
                .unwrap();
            samples(runtime, 800);
            Err("encoding failed")
        });
    assert_eq!(failed, Err("encoding failed"));
    assert_eq!(runtime.snapshot(), before);
    let mut oracle = runtime.fork_for_preview();
    let commit: Result<(), &'static str> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            samples(runtime, 200);
            runtime
                .set_controller_output_enabled(controller_id, false, 200, 0, 100)
                .unwrap();
            samples(runtime, 800);
            Ok(())
        });
    assert_eq!(commit, Ok(()));
    samples(&mut oracle, 200);
    oracle
        .set_controller_output_enabled(controller_id, false, 200, 0, 100)
        .unwrap();
    samples(&mut oracle, 800);
    assert_eq!(runtime.snapshot(), oracle.snapshot());
    let masked = runtime.snapshot();
    let failed: Result<(), &'static str> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.clear_controller_output_gate(controller_id).unwrap();
            samples(runtime, 1_000);
            Err("stale prepared frame")
        });
    assert!(failed.is_err());
    assert_eq!(runtime.snapshot(), masked);
}

#[test]
fn invalid_gate_snapshot_is_rejected_atomically_and_absent_legacy_field_defaults_open() {
    let (mut runtime, _, controller_id) = setup(definition(lane()));
    runtime
        .set_controller_output_enabled(controller_id, false, 10, 20, 300)
        .unwrap();
    samples(&mut runtime, 100);
    let before = runtime.snapshot();
    for (from, to) in [
        (f32::NAN, 0.0),
        (-0.1, 0.0),
        (1.1, 1.0),
        (0.3, 0.5),
        (0.2, f32::INFINITY),
    ] {
        let mut invalid = before.clone();
        let gate = invalid.instances[0].controller_transitions[0]
            .output_gate
            .as_mut()
            .unwrap();
        gate.from = from;
        gate.to = to;
        assert!(matches!(
            runtime.restore_snapshot(invalid),
            Err(DynamicRuntimeError::InvalidSnapshot(_))
        ));
        assert_eq!(runtime.snapshot(), before);
    }
    let mut invalid_owner = before.clone();
    let mut orphan = invalid_owner.instances[0].controller_transitions[0];
    orphan.controller_id = Uuid::from_u128(1709);
    invalid_owner.instances[0]
        .controller_transitions
        .push(orphan);
    assert!(runtime.restore_snapshot(invalid_owner).is_err());
    assert_eq!(runtime.snapshot(), before);

    let mut legacy = serde_json::to_value(&before).unwrap();
    legacy["instances"][0]["controller_transitions"][0]
        .as_object_mut()
        .unwrap()
        .remove("output_gate");
    runtime
        .restore_snapshot(serde_json::from_value(legacy).unwrap())
        .unwrap();
    assert!(
        runtime.snapshot().instances[0].controller_transitions[0]
            .output_gate
            .is_none()
    );
    assert_eq!(samples(&mut runtime, 1_000)[0].activation_mix, 1.0);
}

#[test]
fn gate_multiplies_activation_and_does_not_convert_destructive_release_to_retention() {
    let dynamic = definition(lane());
    let id = dynamic.id;
    let controller_id = Uuid::from_u128(1710);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let mut request = start_request(id, controller(1710, 1, false), FixtureId::new(), 0, false);
    request.activation_duration_millis = 1_000;
    let instance = runtime.start(request).unwrap();
    runtime
        .set_controller_output_enabled(controller_id, false, 0, 0, 1_000)
        .unwrap();
    assert_eq!(samples(&mut runtime, 500)[0].activation_mix, 0.25);
    runtime
        .off_controller(instance, controller_id, 500, 0, 100)
        .unwrap();
    assert!(samples(&mut runtime, 700).is_empty());
    assert_eq!(runtime.instance_count(), 0);
}
