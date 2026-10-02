use super::*;

fn installed(definition: &DynamicDefinition) -> DynamicRuntime {
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
}

fn random_definition() -> DynamicDefinition {
    let mut random = lane();
    random.legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    let group = Uuid::new_v4();
    random.random_group_id = Some(group);
    let mut definition = definition(random);
    definition.random_groups.push(DynamicRandomGroup {
        id: group,
        seed: 17,
        range: DynamicRandomRange::LegacyScalar {
            low: source(0.),
            high: source(1.),
        },
        decision_interval_millis: 100,
        start_probability: 1.,
        mean_duration_millis: 250,
        duration_spread_millis: 30,
        attack_ratio: 0.1,
        decay_ratio: 0.5,
    });
    definition
}

#[test]
fn replayed_new_start_preserves_identity_original_timing_and_random_sequence() {
    let definition = random_definition();
    let mut live = installed(&definition);
    let mut pending = live.fork_for_pending_preview();
    let mut request = start_request(
        definition.id,
        controller(1001, 1, false),
        FixtureId::new(),
        100,
        false,
    );
    request.activation_delay_millis = 50;
    request.activation_duration_millis = 500;
    let id = live.start(request.clone()).unwrap();
    assert_eq!(
        pending.start_with_instance_identity(request, id).unwrap(),
        id
    );
    assert_eq!(pending.snapshot(), live.snapshot());
    for at in [150, 240, 400, 675, 1000, 1900] {
        let sample = |runtime: &mut DynamicRuntime| {
            runtime
                .sample(id, at, 1000, 10, &Sources { current: 0. })
                .unwrap()
        };
        assert_eq!(sample(&mut pending), sample(&mut live));
        assert_eq!(pending.snapshot(), live.snapshot());
    }
    assert!(!pending.snapshot().instances[0].random_streams.is_empty());
}

#[test]
fn replay_selects_the_recorded_targetless_clock_among_multiple_eligible_instances() {
    let definition = definition(lane());
    let mut runtime = installed(&definition);
    let target = FixtureId::new();
    let first = controller(1002, 1, false);
    let mut second = controller(1003, 1, false);
    second.source = first.source.clone();
    let a = runtime
        .start(start_request(
            definition.id,
            first.clone(),
            target,
            10,
            false,
        ))
        .unwrap();
    let b = runtime
        .start(start_request(definition.id, second, target, 20, false))
        .unwrap();
    assert_ne!(a, b);
    let before = runtime.snapshot();
    for chosen in [a, b] {
        let mut branch = runtime.fork_for_pending_preview();
        let mut joining = controller(1004, 2, false);
        joining.source = first.source.clone();
        let request = start_request(definition.id, joining.clone(), target, 40, true);
        assert_eq!(
            branch
                .start_with_instance_identity(request, chosen)
                .unwrap(),
            chosen
        );
        assert_eq!(branch.controller(joining.id).unwrap().0, chosen);
        let other = if chosen == a { b } else { a };
        assert_eq!(
            branch
                .snapshot()
                .instances
                .iter()
                .find(|instance| instance.id == other),
            before
                .instances
                .iter()
                .find(|instance| instance.id == other)
        );
    }
    assert_eq!(runtime.snapshot(), before);
}

#[test]
fn replay_identity_conflicts_are_atomic_and_do_not_replace_bound_or_targetless_clocks() {
    let target = FixtureId::new();
    for bound in [false, true] {
        let mut definition = definition(lane());
        if bound {
            definition.target_binding = DynamicTargetBinding::FrozenTargets {
                targets: vec![target],
            };
        }
        let mut runtime = installed(&definition);
        let request = start_request(
            definition.id,
            controller(1005, 1, false),
            target,
            10,
            !bound,
        );
        let id = runtime.start(request.clone()).unwrap();
        let before = runtime.snapshot();
        for wrong in [Uuid::nil(), Uuid::new_v4()] {
            assert!(matches!(
                runtime.start_with_instance_identity(request.clone(), wrong),
                Err(DynamicRuntimeError::InvalidReplay(_))
            ));
            assert_eq!(runtime.snapshot(), before);
        }
        let mut independent = request.clone();
        independent.reuse_matching_targetless = false;
        if !bound {
            assert!(matches!(
                runtime.start_with_instance_identity(independent.clone(), id),
                Err(DynamicRuntimeError::InvalidReplay(_))
            ));
            assert_eq!(runtime.snapshot(), before);
            let separate = Uuid::new_v4();
            assert_eq!(
                runtime
                    .start_with_instance_identity(independent, separate)
                    .unwrap(),
                separate
            );
        } else {
            assert_eq!(
                runtime.start(request).unwrap(),
                id,
                "conflict must not damage the bound index"
            );
        }
    }
}

#[test]
fn replayed_join_keeps_branch_local_current_history_when_it_changes_the_paused_winner() {
    let target = FixtureId::new();
    let mut current = lane();
    for point in &mut current.legacy_mut().unwrap().keyframes.points {
        point.source = ScalarSource::Current;
    }
    let mut definition = definition(current);
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![target],
    };
    let mut live = installed(&definition);
    let id = live
        .start(start_request(
            definition.id,
            controller(1006, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let mut pending = live.fork_for_pending_preview();
    let transports = [DynamicSpeedTransport {
        effective_bpm: 60.,
        phase_origin_millis: 0,
        phase_reference_millis: 100,
        beat_phase: 0.1,
        phase_advancing: true,
    }; 5];
    live.sample_all_addressed(100, 10, &transports, &Sources { current: 0.2 }, None);
    pending.sample_all_addressed(100, 10, &transports, &Sources { current: 0.8 }, None);
    let mut oracle = pending.fork_for_preview();
    let request = start_request(definition.id, controller(1007, 2, true), target, 150, false);
    assert_eq!(live.start(request.clone()).unwrap(), id);
    assert_eq!(oracle.start(request.clone()).unwrap(), id);
    assert_eq!(
        pending.start_with_instance_identity(request, id).unwrap(),
        id
    );
    assert_eq!(pending.snapshot(), oracle.snapshot());
    for (runtime, expected) in [(&live, 0.2), (&pending, 0.8)] {
        let snapshot = runtime.snapshot();
        let instance = &snapshot.instances[0];
        let DynamicHeldPayload::TapeRoot { tape_root } =
            instance.synchronized_hold_values[0].payload
        else {
            panic!("paused Current retains a source graph");
        };
        let expression = DynamicSampleExpression::Retained {
            tape: Arc::clone(instance.expression_tape.as_ref().unwrap()),
            root: tape_root,
        };
        let mut held = Vec::new();
        expression.visit_legacy_contributions(|_, value, _| held.push(value));
        assert!(!held.is_empty());
        assert!(
            held.iter().all(|value| (*value - expected).abs() < 1e-6),
            "Pause must retain the branch's own Current: {held:?}"
        );
    }
}

#[test]
fn replayed_restart_uses_branch_completion_and_rolls_back_with_its_original_history() {
    let target = FixtureId::new();
    let mut definition = definition(lane());
    definition.run_mode = DynamicRunMode::OneShot;
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![target],
    };
    let mut live = installed(&definition);
    let original = start_request(definition.id, controller(1008, 1, false), target, 0, false);
    let id = live.start(original.clone()).unwrap();
    let mut pending = live.fork_for_pending_preview();
    pending
        .sample(id, 1100, 1000, 10, &Sources { current: 0. })
        .unwrap();
    assert!(pending.snapshot().instances[0].completed);
    assert!(!live.snapshot().instances[0].completed);
    let before = pending.snapshot();
    let boundary = pending.committed_sample_boundary();
    let mut request = original;
    request.now_millis = 1200;
    let mut scratch = DynamicOutputFrameScratch::default();
    let rejected: Result<(), &'static str> =
        pending.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime
                .start_with_instance_identity(request.clone(), id)
                .unwrap();
            assert!(!runtime.snapshot().instances[0].completed);
            Err("later replay step failed")
        });
    assert!(rejected.is_err());
    assert_eq!(pending.snapshot(), before);
    assert_eq!(pending.committed_sample_boundary(), boundary);
    let mut oracle = pending.fork_for_preview();
    oracle.start(request.clone()).unwrap();
    pending.start_with_instance_identity(request, id).unwrap();
    assert_eq!(pending.snapshot(), oracle.snapshot());
    assert_eq!(pending.snapshot().instances[0].started_at_millis, 1200);
}

#[test]
fn failed_replayed_new_start_releases_its_identity_for_an_exact_retry() {
    let target = FixtureId::new();
    let mut definition = random_definition();
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![target],
    };
    let mut runtime = installed(&definition);
    let request = start_request(
        definition.id,
        controller(1009, 1, false),
        target,
        100,
        false,
    );
    let id = Uuid::new_v4();
    let before = runtime.snapshot();
    let mut scratch = DynamicOutputFrameScratch::default();
    let rejected: Result<(), &'static str> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime
                .start_with_instance_identity(request.clone(), id)
                .unwrap();
            runtime
                .sample(id, 500, 1000, 10, &Sources { current: 0. })
                .unwrap();
            Err("later replay step failed")
        });
    assert!(rejected.is_err());
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.committed_sample_boundary(), None);
    assert_eq!(
        runtime.start_with_instance_identity(request, id).unwrap(),
        id
    );
    assert_eq!(runtime.instance_count(), 1);
}
