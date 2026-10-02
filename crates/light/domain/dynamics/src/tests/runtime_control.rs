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
            .legacy()
            .expect("scalar sample")
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
        .legacy()
        .expect("scalar sample")
        .value;
    assert_eq!(
        runtime
            .sample(first, 750, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()[0]
            .legacy()
            .expect("scalar sample")
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
            .legacy()
            .expect("scalar sample")
            .value,
        0.0
    );

    runtime.set_global_paused(false, 750);
    assert_eq!(
        runtime
            .sample(first, 750, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()[0]
            .legacy()
            .expect("scalar sample")
            .value,
        held
    );
    assert!(
        runtime
            .sample(second, 1_000, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()[0]
            .legacy()
            .expect("scalar sample")
            .value
            > 0.0
    );
}

mod frame_addresses {
    use super::*;
    use light_core::{FrameAddress, FrameAddressResolver};
    use std::cell::Cell;

    /// Numbers every pair it is asked about, and counts the asking.
    struct CountingAddresser {
        generation: u64,
        asked: Cell<usize>,
    }

    impl FrameAddressResolver for CountingAddresser {
        fn generation(&self) -> u64 {
            self.generation
        }

        fn frame_address(&self, _: FixtureId, _: &AttributeKey) -> Option<FrameAddress> {
            self.asked.set(self.asked.get() + 1);
            Some(FrameAddress {
                generation: self.generation,
                slot: 7,
            })
        }
    }

    /// A running Dynamic asks where its pairs live once, not once per tick, and asks again only
    /// when the patch generation moves on.
    #[test]
    fn addresses_are_resolved_once_per_generation() {
        let target = FixtureId::new();
        let dynamic = definition(lane());
        let definition_id = dynamic.id;
        let request = start_request(definition_id, controller(94, 1, false), target, 0, false);
        let mut runtime = DynamicRuntime::default();
        runtime.install_definitions([dynamic]).unwrap();
        runtime.start(request).unwrap();
        let addresser = CountingAddresser {
            generation: 5,
            asked: Cell::new(0),
        };
        let transport = DynamicSpeedTransport {
            effective_bpm: 60.0,
            phase_origin_millis: 0,
            phase_reference_millis: 0,
            beat_phase: 0.0,
            phase_advancing: true,
        };
        let sample = |runtime: &mut DynamicRuntime, now, addresser: &CountingAddresser| {
            runtime.sample_all_addressed(
                now,
                10,
                &[transport; 5],
                &Sources { current: 0.0 },
                Some(addresser),
            )
        };

        let first = sample(&mut runtime, 100, &addresser);
        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].address,
            Some(FrameAddress {
                generation: 5,
                slot: 7
            })
        );
        let asked_after_first = addresser.asked.get();
        assert!(asked_after_first >= 1);
        sample(&mut runtime, 200, &addresser);
        sample(&mut runtime, 300, &addresser);
        assert_eq!(
            addresser.asked.get(),
            asked_after_first,
            "later ticks reuse the remembered addresses"
        );

        let repatched = CountingAddresser {
            generation: 6,
            asked: Cell::new(0),
        };
        let after = sample(&mut runtime, 400, &repatched);
        assert_eq!(after[0].address.map(|address| address.generation), Some(6));
        assert!(
            repatched.asked.get() >= 1,
            "a new generation is asked again"
        );

        let unaddressed = runtime.sample_all(500, 10, &[transport; 5], &Sources { current: 0.0 });
        assert_eq!(
            unaddressed[0].address, None,
            "nobody asked, nothing is claimed"
        );
    }
}

fn sample_sync(runtime: &mut DynamicRuntime, now: u64) -> Vec<DynamicRuntimeSample> {
    let transport = DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: now,
        beat_phase: (now as f64 / 1000.0).rem_euclid(1.0),
        phase_advancing: true,
    };
    runtime.sample_all(now, 10, &[transport; 5], &Sources { current: 0.0 })
}

#[test]
fn paused_lane_hot_edit_retains_original_owner_then_resumes_against_each_underlay() {
    let mut source = definition(lane());
    source.default_activation = ActivationPolicy::JoinSyncNow;
    source.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    let id = source.id;
    let lane_id = source.lanes[0].id;
    let target = FixtureId::new();
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([source.clone()]).unwrap();
    let mut request = start_request(id, controller(41, 0, false), target, 0, false);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    sample_sync(&mut runtime, 250);
    runtime.set_global_paused(true, 250);
    let held = sample_sync(&mut runtime, 250)[0].expression.clone();
    source.lanes[0].legacy_mut().unwrap().attribute = AttributeKey("pan".into());
    runtime.install_definitions([source.clone()]).unwrap();
    assert_eq!(sample_sync(&mut runtime, 750)[0].expression, held);
    let serialized = serde_json::to_value(runtime.snapshot()).unwrap();
    let mut restored = DynamicRuntime::default();
    restored
        .restore_snapshot(serde_json::from_value(serialized.clone()).unwrap())
        .unwrap();
    assert_eq!(sample_sync(&mut restored, 750)[0].expression, held);
    runtime
        .set_controller_paused(instance, Uuid::from_u128(41), true, 750)
        .unwrap();
    runtime.set_global_paused(false, 750);
    runtime
        .set_controller_paused_with_resume(instance, Uuid::from_u128(41), false, 750, None)
        .unwrap();
    let resume = sample_sync(&mut runtime, 1250);
    let sample = resume
        .iter()
        .find(|sample| sample.lane_id == lane_id)
        .unwrap();
    let mut weights = HashMap::new();
    assert!(
        sample
            .expression
            .visit_legacy_contributions(|owner, _, weight| {
                weights.insert(owner.clone(), weight);
            })
    );
    assert_eq!(weights[&AttributeKey::intensity()], 0.5);
    assert_eq!(weights[&AttributeKey("pan".into())], 0.5);
    // A previously stored numeric hold is restored against its retained scalar definition.
    let mut legacy = serialized.clone();
    for field in ["last_sample_values", "synchronized_hold_values"] {
        for entry in legacy["instances"][0][field].as_array_mut().unwrap() {
            entry.as_object_mut().unwrap().remove("expression");
            entry.as_object_mut().unwrap().remove("tape_root");
            entry["value"] = serde_json::json!(0.75);
        }
    }
    DynamicRuntime::default()
        .restore_snapshot(serde_json::from_value(legacy).unwrap())
        .unwrap();
    let mut mixed = serialized.clone();
    mixed["instances"][0]["last_sample_values"][0]["value"] = serde_json::json!(0.5);
    assert!(serde_json::from_value::<DynamicRuntimeSnapshot>(mixed).is_err());
    // Returning the current definition to legacy does not hide typed held state.
    let mut typed_hold = serialized;
    typed_hold["instances"][0]["last_sample_values"][0]
        .as_object_mut()
        .unwrap()
        .remove("tape_root");
    typed_hold["instances"][0]["last_sample_values"][0]["expression"] = serde_json::json!({
        "kind":"programming", "address":{"representation":{"kind":"angles"},"component":{"kind":"pan"}},
        "value":{"kind":"scalar","value":720.0}
    });
    assert!(
        DynamicRuntime::with_programming_contract_support(0)
            .restore_snapshot(serde_json::from_value(typed_hold).unwrap())
            .is_err()
    );
}

#[test]
fn deleted_paused_lane_retains_hold_and_releases_on_resume() {
    let mut source = definition(lane());
    source.default_activation = ActivationPolicy::JoinSyncNow;
    source.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    let id = source.id;
    let old_lane = source.lanes[0].id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([source.clone()]).unwrap();
    let mut request = start_request(id, controller(42, 0, false), FixtureId::new(), 0, false);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    sample_sync(&mut runtime, 250);
    runtime
        .set_controller_paused(instance, Uuid::from_u128(42), true, 250)
        .unwrap();
    source.lanes[0].id = Uuid::new_v4();
    source.lanes[0].legacy_mut().unwrap().attribute = AttributeKey("pan".into());
    runtime.install_definitions([source]).unwrap();
    let held = sample_sync(&mut runtime, 750);
    assert!(
        held.iter().any(|sample| sample.lane_id == old_lane
            && sample.legacy().unwrap().attribute.is_intensity())
    );
    runtime
        .set_controller_paused_with_resume(instance, Uuid::from_u128(42), false, 750, None)
        .unwrap();
    let resumed = sample_sync(&mut runtime, 1250);
    assert!(
        matches!(&resumed.iter().find(|sample| sample.lane_id == old_lane).unwrap().expression,
        DynamicSampleExpression::Transition { to:None, progress, .. } if *progress == 0.5)
    );
    assert!(
        sample_sync(&mut runtime, 1750)
            .iter()
            .all(|sample| sample.lane_id != old_lane)
    );
}

#[test]
fn explicit_lane_selection_survives_restore_and_does_not_activate_hot_added_lanes() {
    let targets = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let mut source = definition(lane());
    let first_lane = source.lanes[0].id;
    let second = lane();
    let second_lane = second.id;
    source.lanes.push(second);
    let mut request = start_request(source.id, controller(43, 0, false), targets[0], 0, false);
    request.target_scope.ordered_targets = targets[..2].to_vec();
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([source.clone()]).unwrap();
    let instance = runtime.start(request).unwrap();
    let selection = DynamicLaneSelection::PerTarget {
        targets: vec![
            DynamicTargetLanes {
                target: targets[0],
                lanes: vec![first_lane],
            },
            DynamicTargetLanes {
                target: targets[1],
                lanes: vec![second_lane],
            },
        ],
    };
    assert!(
        runtime
            .set_controller_lane_selection(instance, Uuid::from_u128(43), selection.clone())
            .unwrap()
    );
    assert!(
        !runtime
            .set_controller_lane_selection(instance, Uuid::from_u128(43), selection)
            .unwrap()
    );
    let sample = |runtime: &mut DynamicRuntime| {
        runtime
            .sample(instance, 250, 1000, 10, &Sources { current: 0.0 })
            .unwrap()
            .iter()
            .map(|value| (value.target, value.lane_id))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        sample(&mut runtime),
        vec![(targets[0], first_lane), (targets[1], second_lane)]
    );
    let snapshot =
        serde_json::from_value(serde_json::to_value(runtime.snapshot()).unwrap()).unwrap();
    let mut restored = DynamicRuntime::default();
    restored.restore_snapshot(snapshot).unwrap();
    assert_eq!(sample(&mut restored), sample(&mut runtime));
    source.lanes.push(lane());
    runtime.install_definitions([source.clone()]).unwrap();
    assert_eq!(sample(&mut runtime).len(), 2);
    runtime
        .set_controller_lane_selection(
            instance,
            Uuid::from_u128(43),
            DynamicLaneSelection::Uniform {
                lanes: vec![first_lane],
            },
        )
        .unwrap();
    runtime
        .reconcile_instance_targets(
            instance,
            DynamicTargetScope {
                ordered_targets: targets.to_vec(),
            },
            &HashMap::new(),
            None,
        )
        .unwrap();
    assert_eq!(
        sample(&mut runtime),
        targets.map(|target| (target, first_lane))
    );
    let snapshot = runtime.snapshot();
    assert!(
        DynamicRuntime::with_programming_contract_support(0)
            .restore_snapshot(snapshot)
            .is_err()
    );
    assert!(
        runtime
            .set_controller_lane_selection(
                instance,
                Uuid::from_u128(43),
                DynamicLaneSelection::Uniform {
                    lanes: vec![first_lane, first_lane]
                }
            )
            .is_err()
    );
    assert_eq!(sample(&mut runtime).len(), 3, "invalid selection is atomic");
}
