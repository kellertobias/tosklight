use super::*;

fn native_definition(models: &NativeModels, high: u32) -> DynamicDefinition {
    definition(ramp(
        native_address(models, 1),
        value(DynamicValue::Native(0)),
        value(DynamicValue::Native(high)),
    ))
}

#[test]
fn rejected_preparation_never_publishes_verified_pins_or_definitions() {
    let models = models();
    let mut live = DynamicRuntime::with_native_color_models(1, models.clone());
    let current = definition(focus());
    live.install_definitions([current.clone()]).unwrap();
    let instance = live
        .start(start_request(
            current.id,
            controller(801, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    sampled(&mut live, instance, 250, &sources());
    let before = live.snapshot();
    let view = live.captured_native_color_models();
    assert!(
        live.prepare_definitions([native_definition(&models, u32::MAX)])
            .is_err()
    );
    assert_eq!(
        models.calls.load(Ordering::SeqCst),
        1,
        "failure occurs after verifying and staging the original model"
    );
    assert!(Arc::ptr_eq(&view, &live.captured_native_color_models()));
    assert_eq!(live.snapshot(), before);
    assert_eq!(
        live.instance_definition(instance).unwrap().as_ref(),
        &current
    );
}

fn random_definition() -> DynamicDefinition {
    let mut random = lane();
    random.legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    let group = Uuid::from_u128(802);
    random.random_group_id = Some(group);
    let mut result = definition(random);
    result.default_activation = ActivationPolicy::JoinSyncNow;
    result.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    result.random_groups.push(DynamicRandomGroup {
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
    result
}

fn sample_all(runtime: &mut DynamicRuntime, now: u64) -> Vec<DynamicRuntimeSample> {
    runtime
        .sample_all_programming_addressed(
            now,
            10,
            &[DynamicSpeedTransport {
                effective_bpm: 60.0,
                phase_origin_millis: 0,
                phase_reference_millis: now,
                beat_phase: (now as f64 / 1000.0).rem_euclid(1.0),
                phase_advancing: true,
            }; 5],
            &Sources { current: 0.0 },
            &sources(),
            None,
        )
        .unwrap()
}

#[test]
fn install_keeps_latest_clock_pause_controls_random_and_held_history() {
    let mut dynamic = random_definition();
    let mut live = DynamicRuntime::default();
    live.install_definitions([dynamic.clone()]).unwrap();
    let control = controller(802, 1, false);
    let instance = live
        .start(start_request(
            dynamic.id,
            control.clone(),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    sample_all(&mut live, 150);
    dynamic.revision += 1;
    dynamic.name = "Prepared while transport is still running".into();
    let prepared = live.prepare_definitions([dynamic.clone()]).unwrap();

    // These changes happen after preparation and must survive installing the token.
    sample_all(&mut live, 675);
    live.update_controller(control.id, Some(0.6), Some(1.5), Some(45.0))
        .unwrap();
    sample_all(&mut live, 700);
    live.set_global_paused(true, 700);
    let held = sample_all(&mut live, 750);
    let mut expected = live.snapshot();
    assert!(!expected.instances[0].random_streams.is_empty());
    assert!(!expected.instances[0].synchronized_hold_values.is_empty());
    expected.instances[0].definition = dynamic;
    live.install_prepared_definitions(prepared);
    assert_eq!(live.snapshot(), expected);
    assert_eq!(sample_all(&mut live, 900), held);
    assert_eq!(live.controller(control.id).unwrap().0, instance);
}

#[test]
fn installation_obeys_the_current_definition_pin_policy_in_both_directions() {
    let mut dynamic = definition(focus());
    let mut live = DynamicRuntime::default();
    live.install_definitions([dynamic.clone()]).unwrap();
    let instance = live
        .start(start_request(
            dynamic.id,
            controller(803, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let original = live.instance_definition(instance).unwrap().clone();
    dynamic.revision += 1;
    let prepared = live.prepare_definitions([dynamic.clone()]).unwrap();
    live.set_definitions_pinned(true);
    live.install_prepared_definitions(prepared);
    assert!(Arc::ptr_eq(
        live.instance_definition(instance).unwrap(),
        &original
    ));
    live.set_definitions_pinned(false);
    assert_eq!(live.instance_definition(instance).unwrap().revision, 2);

    live.set_definitions_pinned(true);
    dynamic.revision += 1;
    let prepared = live.prepare_definitions([dynamic]).unwrap();
    live.set_definitions_pinned(false);
    live.install_prepared_definitions(prepared);
    assert_eq!(live.instance_definition(instance).unwrap().revision, 3);
    assert_eq!(live.snapshot().instances[0].started_at_millis, 0);
}

struct ModelSet {
    models: Vec<Arc<NativeModel>>,
    missing: &'static str,
}

impl DynamicNativeModelResolver for ModelSet {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        self.resolve_capability(source)?.require_available()
    }

    fn resolve_capability(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<NativeColorModelCapability, IntentError> {
        Ok(
            match self.models.iter().find(|model| &model.source == source) {
                Some(model) => NativeColorModelCapability::Available(model.clone()),
                None => NativeColorModelCapability::Unavailable(NativeColorModelUnavailable {
                    source: source.clone(),
                    reason: NativeColorUnavailableReason::MissingRevision,
                    detail: self.missing.into(),
                }),
            },
        )
    }
}

fn distinct_models(profile: u128) -> Arc<NativeModels> {
    let mut result = models();
    Arc::get_mut(&mut Arc::get_mut(&mut result).unwrap().model)
        .unwrap()
        .source
        .profile_id = Uuid::from_u128(profile);
    result
}

#[test]
fn verified_pins_accumulate_without_replacing_newer_live_models_or_provider() {
    let old_a = models();
    let new_a = models();
    let b = distinct_models(802);
    let c = distinct_models(803);
    let old_provider: Arc<dyn DynamicNativeModelResolver> = Arc::new(ModelSet {
        models: vec![old_a.model.clone(), c.model.clone()],
        missing: "old provider",
    });
    let old_weak = Arc::downgrade(&old_provider);
    let mut live = DynamicRuntime::with_native_color_models(1, old_provider.clone());
    drop(old_provider);
    let prepared = live
        .prepare_definitions([native_definition(&old_a, 100), native_definition(&c, 100)])
        .unwrap();
    let new_provider: Arc<dyn DynamicNativeModelResolver> = Arc::new(ModelSet {
        models: vec![new_a.model.clone(), b.model.clone()],
        missing: "new provider",
    });
    live.refresh_native_color_models(new_provider.clone())
        .unwrap();
    assert!(
        old_weak.upgrade().is_none(),
        "a prepared token cannot retain the old provider"
    );
    live.install_definitions([native_definition(&new_a, 100), native_definition(&b, 100)])
        .unwrap();
    live.install_prepared_definitions(prepared);
    let captured = live.captured_native_color_models();
    for model in [&new_a.model, &b.model, &c.model] {
        let expected: Arc<dyn NativeColorEditModel + Send + Sync> = (*model).clone();
        assert!(Arc::ptr_eq(
            &captured.resolve(&model.source).unwrap(),
            &expected
        ));
    }
    let missing = distinct_models(804);
    assert!(
        matches!(captured.resolve_capability(&missing.model.source).unwrap(), NativeColorModelCapability::Unavailable(reason) if reason.detail == "new provider")
    );
    live.refresh_native_color_models(new_provider).unwrap();
    assert!(
        Arc::ptr_eq(&captured, &live.captured_native_color_models()),
        "unchanged fully verified install keeps the warm provider view"
    );
}

#[test]
fn suspended_prepared_lanes_recover_on_explicit_same_provider_refresh_without_clock_reset() {
    let models = models();
    let mut dynamic = native_definition(&models, 100);
    dynamic.lanes.push(focus());
    let mut live = DynamicRuntime::with_native_color_models(1, Arc::new(Unavailable));
    let prepared = live.prepare_definitions([dynamic.clone()]).unwrap();
    live.refresh_native_color_models(models.clone()).unwrap();
    live.install_prepared_definitions(prepared);
    let instance = live
        .start(start_request(
            dynamic.id,
            controller(804, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    assert_eq!(sampled(&mut live, instance, 250, &sources()).len(), 1);
    let before = live.snapshot();
    assert_eq!(live.unavailable_native_sources().len(), 1);
    live.refresh_native_color_models(models).unwrap();
    assert_eq!(live.snapshot(), before);
    assert!(live.unavailable_native_sources().is_empty());
    assert_eq!(sampled(&mut live, instance, 750, &sources()).len(), 2);
}
