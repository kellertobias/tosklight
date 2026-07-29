use super::*;

#[test]
fn target_bound_sources_share_one_clock_and_controller_fallback_preserves_phase() {
    let target = FixtureId::new();
    let mut definition = definition(lane());
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![target],
    };
    let definition_id = definition.id;
    let first = controller(1, 1, false);
    let second = controller(2, 2, true);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let instance = runtime
        .start(start_request(
            definition_id,
            first.clone(),
            target,
            0,
            false,
        ))
        .unwrap();
    assert_eq!(
        runtime
            .start(start_request(
                definition_id,
                second.clone(),
                target,
                250,
                false,
            ))
            .unwrap(),
        instance
    );
    let sample = |runtime: &mut DynamicRuntime, at| {
        runtime
            .sample(instance, at, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()[0]
            .value
    };
    let held = sample(&mut runtime, 500);
    assert_eq!(held, sample(&mut runtime, 750));
    runtime
        .off_controller(instance, second.id, 750, 0, 0)
        .unwrap();
    assert_eq!(runtime.instance_count(), 1);
    assert_ne!(sample(&mut runtime, 1_000), held);
}

#[test]
fn controller_activation_and_release_mix_follow_authored_delay_and_fade() {
    let target = FixtureId::new();
    let definition = definition(lane());
    let definition_id = definition.id;
    let controller = controller(44, 1, false);
    let controller_id = controller.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let mut request = start_request(definition_id, controller, target, 1_000, false);
    request.activation_delay_millis = 100;
    request.activation_duration_millis = 400;
    let instance = runtime.start(request).unwrap();
    let mix = |runtime: &mut DynamicRuntime, at| {
        runtime
            .sample(instance, at, 1_000, 10, &Sources { current: 0.25 })
            .unwrap()[0]
            .activation_mix
    };
    assert_eq!(mix(&mut runtime, 1_050), 0.0);
    assert!((mix(&mut runtime, 1_300) - 0.5).abs() < f32::EPSILON);
    assert_eq!(mix(&mut runtime, 1_500), 1.0);

    assert!(
        !runtime
            .off_controller(instance, controller_id, 1_600, 100, 400)
            .unwrap()
    );
    assert_eq!(mix(&mut runtime, 1_650), 1.0);
    assert!((mix(&mut runtime, 1_900) - 0.5).abs() < f32::EPSILON);
    assert_eq!(mix(&mut runtime, 2_100), 0.0);
    assert!(
        runtime
            .sample_all(
                2_100,
                10,
                &[DynamicSpeedTransport {
                    effective_bpm: 120.0,
                    phase_origin_millis: 0,
                    phase_reference_millis: 2_100,
                    beat_phase: 0.2,
                    phase_advancing: true,
                }; 5],
                &Sources { current: 0.25 },
            )
            .is_empty()
    );
    assert_eq!(runtime.instance_count(), 0);
}

#[test]
fn releasing_singleton_controller_exposes_prior_controller_through_the_same_stack() {
    let target = FixtureId::new();
    let mut definition = definition(lane());
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![target],
    };
    let definition_id = definition.id;
    let lower = controller(45, 1, false);
    let upper = controller(46, 2, false);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let instance = runtime
        .start(start_request(
            definition_id,
            lower.clone(),
            target,
            0,
            false,
        ))
        .unwrap();
    runtime
        .start(start_request(
            definition_id,
            upper.clone(),
            target,
            0,
            false,
        ))
        .unwrap();
    runtime
        .off_controller(instance, upper.id, 100, 0, 400)
        .unwrap();

    let samples = runtime
        .sample(instance, 300, 1_000, 10, &Sources { current: 0.0 })
        .unwrap();
    let lower_sample = samples
        .iter()
        .find(|sample| sample.controller_id == lower.id)
        .unwrap();
    let upper_sample = samples
        .iter()
        .find(|sample| sample.controller_id == upper.id)
        .unwrap();
    assert_eq!(lower_sample.activation_mix, 1.0);
    assert!((upper_sample.activation_mix - 0.5).abs() < f32::EPSILON);
}

#[test]
fn targetless_cue_and_playback_starts_are_independent_but_programmer_toggle_can_reuse() {
    let target = FixtureId::new();
    let definition = definition(lane());
    let definition_id = definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();

    let independent_a = runtime
        .start(start_request(
            definition_id,
            controller(1, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let independent_b = runtime
        .start(start_request(
            definition_id,
            controller(2, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    assert_ne!(independent_a, independent_b);

    let programmer = controller(3, 1, false);
    let programmer_instance = runtime
        .start(start_request(
            definition_id,
            programmer.clone(),
            target,
            0,
            true,
        ))
        .unwrap();
    assert_eq!(
        runtime
            .start(start_request(definition_id, programmer, target, 10, true,))
            .unwrap(),
        programmer_instance
    );
}

#[test]
fn global_pause_freezes_existing_and_new_instances_without_inheriting_old_pause_time() {
    let first_target = FixtureId::new();
    let second_target = FixtureId::new();
    let definition = definition(lane());
    let definition_id = definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let first = runtime
        .start(start_request(
            definition_id,
            controller(10, 1, false),
            first_target,
            0,
            false,
        ))
        .unwrap();

    runtime.set_global_paused(true, 250);
    let held = runtime
        .sample(first, 250, 1_000, 10, &Sources { current: 0.0 })
        .unwrap()[0]
        .value;
    assert_eq!(
        runtime
            .sample(first, 750, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()[0]
            .value,
        held
    );

    let second = runtime
        .start(start_request(
            definition_id,
            controller(11, 1, false),
            second_target,
            500,
            false,
        ))
        .unwrap();
    assert_eq!(
        runtime
            .sample(second, 750, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()[0]
            .value,
        0.0
    );

    runtime.set_global_paused(false, 750);
    assert_eq!(
        runtime
            .sample(first, 750, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()[0]
            .value,
        held
    );
    assert!(
        runtime
            .sample(second, 1_000, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()[0]
            .value
            > 0.0
    );
}
