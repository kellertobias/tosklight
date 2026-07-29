use super::*;

#[test]
fn synchronized_controller_pause_holds_output_and_crossfades_to_live_transport_on_resume() {
    let target = FixtureId::new();
    let mut dynamic = definition(lane());
    dynamic.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational {
            numerator: 4,
            denominator: 1,
        },
    };
    dynamic.default_activation = ActivationPolicy::JoinSyncNow;
    let definition_id = dynamic.id;
    let controller = controller(94, 1, false);
    let controller_id = controller.id;
    let mut request = start_request(definition_id, controller, target, 0, false);
    request.activation_duration_millis = 400;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let instance = runtime.start(request).unwrap();
    let transport_at = |phase_reference_millis| DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis,
        beat_phase: (phase_reference_millis as f64 / 1_000.0).rem_euclid(1.0),
        phase_advancing: true,
    };
    let sample = |runtime: &mut DynamicRuntime, now_millis| {
        runtime.sample_all(
            now_millis,
            10,
            &[transport_at(now_millis); 5],
            &Sources { current: 0.0 },
        )[0]
        .value
    };

    let held = sample(&mut runtime, 500);
    assert!((held - 0.25).abs() < 0.001);
    runtime
        .set_controller_paused(instance, controller_id, true, 500)
        .unwrap();
    assert!((sample(&mut runtime, 2_500) - held).abs() < 0.001);

    runtime
        .set_controller_paused_with_resume(
            instance,
            controller_id,
            false,
            2_500,
            Some(ActivationPolicy::JoinSyncNow),
        )
        .unwrap();
    assert!(
        (sample(&mut runtime, 2_700) - 0.45).abs() < 0.001,
        "halfway through resume the held 0.25 sample crossfades toward the live 0.65 sample"
    );
    assert!(
        (sample(&mut runtime, 2_900) - 0.55).abs() < 0.001,
        "after the resume transition the synchronized live sample is authoritative"
    );
}

#[test]
fn synchronized_global_pause_transition_survives_runtime_snapshot_restore() {
    let target = FixtureId::new();
    let mut dynamic = definition(lane());
    dynamic.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational {
            numerator: 4,
            denominator: 1,
        },
    };
    dynamic.default_activation = ActivationPolicy::JoinSyncNow;
    let definition_id = dynamic.id;
    let mut request = start_request(definition_id, controller(95, 1, false), target, 0, false);
    request.activation_duration_millis = 400;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    runtime.start(request).unwrap();
    let transport_at = |phase_reference_millis| DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis,
        beat_phase: (phase_reference_millis as f64 / 1_000.0).rem_euclid(1.0),
        phase_advancing: true,
    };
    let sample = |runtime: &mut DynamicRuntime, now_millis| {
        runtime.sample_all(
            now_millis,
            10,
            &[transport_at(now_millis); 5],
            &Sources { current: 0.0 },
        )[0]
        .value
    };

    assert!((sample(&mut runtime, 500) - 0.25).abs() < 0.001);
    runtime.set_global_paused(true, 500);
    assert!((sample(&mut runtime, 2_500) - 0.25).abs() < 0.001);
    let snapshot = runtime.snapshot();
    let mut restored = DynamicRuntime::default();
    restored.restore_snapshot(snapshot).unwrap();

    runtime.set_global_paused(false, 2_500);
    restored.set_global_paused(false, 2_500);
    assert!((sample(&mut runtime, 2_700) - 0.45).abs() < 0.001);
    assert!((sample(&mut restored, 2_700) - 0.45).abs() < 0.001);
    assert_eq!(restored.snapshot(), runtime.snapshot());

    let transition_snapshot = restored.snapshot();
    let mut restored_during_transition = DynamicRuntime::default();
    restored_during_transition
        .restore_snapshot(transition_snapshot)
        .unwrap();
    assert!((sample(&mut runtime, 2_800) - 0.5125).abs() < 0.001);
    assert!((sample(&mut restored_during_transition, 2_800) - 0.5125).abs() < 0.001);
}

#[test]
fn next_bar_boundary_waits_for_the_authoritative_four_beat_boundary() {
    let target = FixtureId::new();
    let mut definition = definition(lane());
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational {
            numerator: 4,
            denominator: 1,
        },
    };
    definition.default_activation = ActivationPolicy::NextBoundary;
    definition.activation_boundary = ActivationBoundary::Bar;
    let definition_id = definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    runtime
        .start(start_request(
            definition_id,
            controller(93, 1, false),
            target,
            1_250,
            false,
        ))
        .unwrap();
    let transport = DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: 1_500,
        beat_phase: 0.5,
        phase_advancing: true,
    };

    assert!(
        runtime
            .sample_all(1_500, 10, &[transport; 5], &Sources { current: 0.0 })
            .is_empty()
    );
    assert!(
        runtime
            .sample_all(
                3_999,
                10,
                &[DynamicSpeedTransport {
                    phase_reference_millis: 3_999,
                    beat_phase: 0.999,
                    ..transport
                }; 5],
                &Sources { current: 0.0 },
            )
            .is_empty()
    );
    let boundary = runtime.sample_all(
        4_000,
        10,
        &[DynamicSpeedTransport {
            phase_reference_millis: 4_000,
            beat_phase: 0.0,
            ..transport
        }; 5],
        &Sources { current: 0.0 },
    );
    assert_eq!(boundary[0].value, 0.0);
}
