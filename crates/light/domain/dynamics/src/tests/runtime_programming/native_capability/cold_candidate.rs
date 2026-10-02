use super::*;

fn invalid_native(models: &NativeModels) -> DynamicLane {
    // This binding exists, so its model is pinned before the full-width value fails its
    // 16-bit function bounds. A structural-validation failure would miss the isolation bug.
    ramp(
        native_address(models, 1),
        value(DynamicValue::Native(u32::MAX - 1)),
        value(DynamicValue::Native(u32::MAX)),
    )
}

fn synchronized_sample(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    now: u64,
) -> Vec<DynamicRuntimeSample> {
    let transport = DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: now,
        beat_phase: (now as f64 / 1_000.0).rem_euclid(1.0),
        phase_advancing: true,
    };
    let samples = runtime
        .sample_all_programming_addressed(
            now,
            10,
            &[transport; 5],
            &Sources { current: 0.0 },
            &sources(),
            None,
        )
        .unwrap();
    assert!(samples.iter().all(|sample| sample.instance_id == instance));
    samples
}

fn paused_focus(
    provider: Arc<dyn DynamicNativeModelResolver>,
    native: Option<DynamicLane>,
) -> (
    DynamicRuntime,
    Uuid,
    DynamicDefinition,
    Vec<DynamicRuntimeSample>,
) {
    let mut definition = definition(focus());
    // StartNow pauses its elapsed clock and evaluates at that frozen time. This fixture
    // exercises immutable held history, which belongs to JoinSyncNow's pause policy.
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    definition.lanes.extend(native);
    let mut runtime = DynamicRuntime::with_native_color_models(1, provider);
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance = runtime
        .start(start_request(
            definition.id,
            controller(701, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    synchronized_sample(&mut runtime, instance, 250);
    runtime.set_global_paused(true, 250);
    let held = synchronized_sample(&mut runtime, instance, 300);
    assert!(!held.is_empty());
    assert!(
        !runtime.snapshot().instances[0]
            .synchronized_hold_values
            .is_empty()
    );
    (runtime, instance, definition, held)
}

#[test]
fn failed_compilation_with_unchanged_provider_cannot_publish_candidate_pins_to_live() {
    let models = models();
    let provider: Arc<dyn DynamicNativeModelResolver> = models.clone();
    let (mut live, instance, mut definition, held) = paused_focus(provider.clone(), None);
    let before = live.snapshot();
    let live_view = live.captured_native_color_models();
    let mut candidate = live.fork_for_cold_install();
    candidate.refresh_native_color_models(provider).unwrap();
    definition.lanes.push(invalid_native(&models));
    assert!(candidate.install_definitions([definition]).is_err());
    assert_eq!(
        models.calls.load(Ordering::SeqCst),
        1,
        "failure reached native model pinning"
    );
    assert!(
        Arc::ptr_eq(&live_view, &live.captured_native_color_models()),
        "failed candidate compilation must not invalidate Live's cached model view"
    );
    assert_eq!(live.snapshot(), before);
    assert_eq!(candidate.snapshot(), before);
    assert_eq!(synchronized_sample(&mut candidate, instance, 900), held);
    assert_eq!(synchronized_sample(&mut live, instance, 900), held);
    assert_eq!(
        live.snapshot().instances[0].synchronized_hold_values,
        before.instances[0].synchronized_hold_values
    );
}

#[test]
fn preview_fallback_model_discovery_is_isolated_on_success_and_failure() {
    for pending in [false, true] {
        for invalid in [false, true] {
            let models = models();
            let (mut live, _, _, _) = paused_focus(models.clone(), None);
            let before = live.snapshot();
            let live_view = live.captured_native_color_models();
            let mut preview = if pending {
                live.fork_for_pending_preview()
            } else {
                live.fork_for_preview()
            };
            let native = if invalid {
                invalid_native(&models)
            } else {
                ramp(
                    native_address(&models, 1),
                    value(DynamicValue::Native(0)),
                    value(DynamicValue::Native(255)),
                )
            };
            let mut fallback = definition(native);
            fallback.id = Uuid::new_v4();
            // Unlike registry preparation, fallback installation pins directly on this runtime.
            // This reaches the mutable cache shared by the old preview-fork implementation.
            let result = preview.install_fallback_definition(fallback);
            assert_eq!(result.is_err(), invalid);
            assert_eq!(models.calls.load(Ordering::SeqCst), 1);
            assert!(Arc::ptr_eq(
                &live_view,
                &live.captured_native_color_models()
            ));
            assert_eq!(live.snapshot(), before);
            live.refresh_native_color_models(Arc::new(Unavailable))
                .unwrap();
            preview
                .refresh_native_color_models(Arc::new(Unavailable))
                .unwrap();
            assert!(matches!(
                live.captured_native_color_models()
                    .resolve_capability(&models.model.source)
                    .unwrap(),
                NativeColorModelCapability::Unavailable(_)
            ));
            assert!(matches!(
                preview
                    .captured_native_color_models()
                    .resolve_capability(&models.model.source)
                    .unwrap(),
                NativeColorModelCapability::Available(_)
            ));
            assert_eq!(live.snapshot(), before);
        }
    }
}

#[test]
fn pending_preview_follows_registry_without_replacing_live_pins_or_held_history() {
    let (mut live, instance, mut definition, held) = paused_focus(Arc::new(Unavailable), None);
    live.set_definitions_pinned(true);
    let original = live.instance_definition(instance).unwrap().clone();
    definition.revision += 1;
    live.install_definitions([definition.clone()]).unwrap();
    let before = live.snapshot();
    let ordinary = live.fork_for_preview();
    assert!(Arc::ptr_eq(
        ordinary.instance_definition(instance).unwrap(),
        &original
    ));
    let mut pending = live.fork_for_pending_preview();
    assert_eq!(
        pending.instance_definition(instance).unwrap().revision,
        definition.revision
    );
    let mut expected = before.clone();
    expected.instances[0].definition = definition.clone();
    assert_eq!(
        pending.snapshot(),
        expected,
        "only the effective definition changes"
    );
    assert_eq!(synchronized_sample(&mut pending, instance, 900), held);
    definition.revision += 1;
    pending.install_definitions([definition.clone()]).unwrap();
    assert_eq!(
        pending.instance_definition(instance).unwrap().revision,
        definition.revision
    );
    pending.set_global_paused(false, 950);
    synchronized_sample(&mut pending, instance, 1000);
    assert_eq!(live.snapshot(), before);
    assert!(Arc::ptr_eq(
        live.instance_definition(instance).unwrap(),
        &original
    ));
}

#[test]
fn failed_provider_recovery_keeps_live_and_candidate_views_and_paused_history() {
    let models = models();
    let (mut live, instance, _, held) =
        paused_focus(Arc::new(Unavailable), Some(invalid_native(&models)));
    let before = live.snapshot();
    let live_view = live.captured_native_color_models();
    let mut candidate = live.fork_for_cold_install();
    let candidate_view = candidate.captured_native_color_models();
    assert!(
        candidate
            .refresh_native_color_models(models.clone())
            .is_err()
    );
    assert_eq!(models.calls.load(Ordering::SeqCst), 1);
    assert!(Arc::ptr_eq(
        &live_view,
        &live.captured_native_color_models()
    ));
    assert!(Arc::ptr_eq(
        &candidate_view,
        &candidate.captured_native_color_models()
    ));
    assert!(matches!(
        live_view.resolve_capability(&models.model.source).unwrap(),
        NativeColorModelCapability::Unavailable(_)
    ));
    assert_eq!(live.snapshot(), before);
    assert_eq!(candidate.snapshot(), before);
    assert_eq!(synchronized_sample(&mut candidate, instance, 900), held);
    assert_eq!(synchronized_sample(&mut live, instance, 900), held);
}

#[test]
fn cold_candidate_preserves_random_progress_and_definition_pin_policy() {
    let mut random = lane();
    random.legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    let group = Uuid::from_u128(702);
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
    let mut live = DynamicRuntime::default();
    live.install_definitions([definition.clone()]).unwrap();
    let instance = live
        .start(start_request(
            definition.id,
            controller(702, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let sample = |runtime: &mut DynamicRuntime, at| {
        runtime
            .sample(instance, at, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
    };
    sample(&mut live, 150);
    assert!(!live.snapshot().instances[0].random_streams.is_empty());
    live.set_definitions_pinned(true);
    let mut candidate = live.fork_for_cold_install();
    for at in [240, 400, 675] {
        assert_eq!(sample(&mut candidate, at), sample(&mut live, at));
        assert_eq!(candidate.snapshot(), live.snapshot());
    }
    let original = live.instance_definition(instance).unwrap().clone();
    definition.revision += 1;
    definition.random_groups[0].seed = 43;
    candidate.install_definitions([definition.clone()]).unwrap();
    assert!(
        Arc::ptr_eq(candidate.instance_definition(instance).unwrap(), &original),
        "running definition remains pinned during cold preparation"
    );
    let before = live.snapshot();
    candidate.set_definitions_pinned(false);
    assert_eq!(
        candidate.instance_definition(instance).unwrap().revision,
        definition.revision
    );
    sample(&mut candidate, 1_000);
    assert_eq!(
        live.snapshot(),
        before,
        "candidate unpin and sampling leave Live's policy and Random history unchanged"
    );
    live.install_definitions([definition]).unwrap();
    assert!(Arc::ptr_eq(
        live.instance_definition(instance).unwrap(),
        &original
    ));
}
