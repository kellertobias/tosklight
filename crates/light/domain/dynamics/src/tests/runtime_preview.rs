use super::*;
use light_core::{FrameAddress, FrameAddressResolver, programming::ProgrammingComponent};

struct PreviewAddresser {
    generation: u64,
    calls: Cell<usize>,
}

impl FrameAddressResolver for PreviewAddresser {
    fn generation(&self) -> u64 {
        self.generation
    }

    fn frame_address(&self, _: FixtureId, _: &AttributeKey) -> Option<FrameAddress> {
        self.calls.set(self.calls.get() + 1);
        Some(FrameAddress {
            generation: self.generation,
            slot: 7,
        })
    }
}

fn transport() -> [DynamicSpeedTransport; 5] {
    [DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: 0,
        beat_phase: 0.0,
        phase_advancing: true,
    }; 5]
}

#[test]
fn preview_keeps_warm_addresses_and_phase_but_owns_controls_targets_and_off() {
    let targets = [FixtureId::new(), FixtureId::new()];
    let mut definition = definition(lane());
    definition.phase.span_degrees = 180.0;
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: targets.to_vec(),
    };
    let id = definition.id;
    let lane_id = definition.lanes[0].id;
    let c = controller(301, 1, false);
    let mut live = DynamicRuntime::default();
    live.install_definitions([definition]).unwrap();
    let mut request = start_request(id, c.clone(), targets[0], 0, false);
    request.target_scope.ordered_targets = targets.to_vec();
    let instance = live.start(request).unwrap();
    live.set_controller_lane_selection(
        instance,
        c.id,
        DynamicLaneSelection::Uniform {
            lanes: vec![lane_id],
        },
    )
    .unwrap();
    let addresses = PreviewAddresser {
        generation: 8,
        calls: Cell::new(0),
    };
    let sample = |runtime: &mut DynamicRuntime, at, addresses: &PreviewAddresser| {
        runtime.sample_all_addressed(
            at,
            10,
            &transport(),
            &Sources { current: 0.0 },
            Some(addresses),
        )
    };
    let frame = sample(&mut live, 125, &addresses);
    assert_eq!(frame.len(), 2);
    assert_ne!(
        frame[0].expression, frame[1].expression,
        "the fixture phases differ"
    );
    assert_eq!(addresses.calls.get(), 2);
    let before = live.snapshot();
    let mut preview = live.fork_for_preview();
    assert_eq!(preview.snapshot(), before);
    assert_eq!(sample(&mut preview, 125, &addresses), frame);
    assert_eq!(
        addresses.calls.get(),
        2,
        "fork reuses the already resolved frame addresses"
    );

    preview
        .update_controller(c.id, Some(0.3), Some(2.0), Some(90.0))
        .unwrap();
    preview
        .set_controller_lane_selection(
            instance,
            c.id,
            DynamicLaneSelection::Uniform { lanes: Vec::new() },
        )
        .unwrap();
    assert!(sample(&mut preview, 250, &addresses).is_empty());
    preview
        .set_controller_lane_selection(instance, c.id, DynamicLaneSelection::All)
        .unwrap();
    preview
        .reconcile_instance_targets(
            instance,
            DynamicTargetScope {
                ordered_targets: vec![targets[1]],
            },
            &HashMap::new(),
            None,
        )
        .unwrap();
    let repatched = PreviewAddresser {
        generation: 9,
        calls: Cell::new(0),
    };
    let changed = sample(&mut preview, 375, &repatched);
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].target, targets[1]);
    assert_eq!(repatched.calls.get(), 1);
    preview.set_global_paused(true, 375);
    sample(&mut preview, 750, &repatched);
    assert!(preview.off_controller(instance, c.id, 750, 0, 0).unwrap());
    assert_eq!(preview.instance_count(), 0);
    assert_eq!(live.snapshot(), before);
    assert_eq!(sample(&mut live, 125, &addresses), frame);
    assert_eq!(
        addresses.calls.get(),
        2,
        "preview repatching did not discard Live's cache"
    );

    // Off removed only the fork's bound-instance entry. Live still joins the original clock.
    assert_eq!(
        live.start(start_request(
            id,
            controller(302, 2, false),
            targets[0],
            125,
            false
        ))
        .unwrap(),
        instance
    );
}

#[test]
fn preview_random_sampling_and_pause_do_not_advance_live_random_streams() {
    let mut random_lane = lane();
    random_lane.legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    let group = Uuid::from_u128(303);
    random_lane.random_group_id = Some(group);
    let mut definition = definition(random_lane);
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
    let id = definition.id;
    let target = FixtureId::new();
    let c = controller(303, 1, false);
    let mut live = DynamicRuntime::default();
    live.install_definitions([definition]).unwrap();
    let instance = live
        .start(start_request(id, c.clone(), target, 0, false))
        .unwrap();
    let sample = |runtime: &mut DynamicRuntime, at| {
        runtime
            .sample(instance, at, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
    };
    sample(&mut live, 150);
    assert!(!live.snapshot().instances[0].random_streams.is_empty());
    let mut preview = live.fork_for_preview();
    let mut pending = live.fork_for_pending_preview();
    for at in [150, 240, 400, 675] {
        assert_eq!(sample(&mut preview, at), sample(&mut live, at));
        assert_eq!(sample(&mut pending, at), sample(&mut live, at));
        assert_eq!(preview.snapshot(), live.snapshot());
        assert_eq!(pending.snapshot(), live.snapshot());
    }
    let before = live.snapshot();
    preview
        .set_controller_paused(instance, c.id, true, 675)
        .unwrap();
    sample(&mut preview, 2_000);
    preview
        .set_controller_paused(instance, c.id, false, 2_000)
        .unwrap();
    for at in [2_100, 3_000, 5_000] {
        sample(&mut preview, at);
    }
    assert_ne!(
        preview.snapshot().instances[0].random_streams,
        before.instances[0].random_streams
    );
    assert_eq!(live.snapshot(), before);
    preview.off_controller(instance, c.id, 5_000, 0, 0).unwrap();
    assert_eq!(live.snapshot(), before);
}

struct PreviewSources;
impl DynamicValueSourceResolver for PreviewSources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        Some(DynamicValue::Scalar(0.4))
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
    fn current_family_base(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Option<light_core::AttributeValue> {
        Some(light_core::AttributeValue::Normalized(0.4))
    }
}

fn focus_definition() -> DynamicDefinition {
    definition(DynamicLane {
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Focus,
                component: Some(ProgrammingComponent::Focus),
            },
            configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
                minimum: DynamicValueSource::Value {
                    value: DynamicValue::Scalar(0.0),
                },
                maximum: DynamicValueSource::Value {
                    value: DynamicValue::Scalar(1.0),
                },
                function: PeriodicFunction::LinearUp,
                size: 1.0,
                pwm: PwmShape::default(),
            }),
        }),
        ..lane()
    })
}

fn typed_sample(runtime: &mut DynamicRuntime, instance: Uuid, at: u64) -> DynamicRuntimeSample {
    runtime
        .sample_programming(
            instance,
            at,
            1_000,
            10,
            &Sources { current: 0.0 },
            &PreviewSources,
        )
        .unwrap()
        .remove(0)
}

#[test]
fn preview_reuses_compiled_source_arcs_and_preserves_pinned_effective_definition() {
    let mut definition = focus_definition();
    let id = definition.id;
    let target = FixtureId::new();
    let mut live = DynamicRuntime::default();
    live.install_definitions([definition.clone()]).unwrap();
    let instance = live
        .start(start_request(
            id,
            controller(304, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let frame = typed_sample(&mut live, instance, 250);
    live.set_definitions_pinned(true);
    definition.revision += 1;
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::MaxMin(config) = &mut body.configuration else {
        unreachable!()
    };
    config.minimum = DynamicValueSource::Value {
        value: DynamicValue::Scalar(0.75),
    };
    config.maximum = DynamicValueSource::Value {
        value: DynamicValue::Scalar(0.75),
    };
    live.install_definitions([definition]).unwrap();
    let before = live.snapshot();
    let mut preview = live.fork_for_preview();
    let forked = typed_sample(&mut preview, instance, 250);
    assert_eq!(forked, frame);
    let (
        DynamicSampleExpression::Programming {
            address: live_address,
            ..
        },
        DynamicSampleExpression::Programming {
            address: preview_address,
            ..
        },
    ) = (&frame.expression, &forked.expression)
    else {
        panic!("Focus produces a compiled component leaf")
    };
    assert!(
        Arc::ptr_eq(live_address, preview_address),
        "fork must not recompile the source address"
    );
    preview.set_definitions_pinned(false);
    let changed = typed_sample(&mut preview, instance, 250);
    assert_eq!(
        changed.expression.programming_leaf().unwrap().1,
        &DynamicValue::Scalar(0.75)
    );
    assert_eq!(live.snapshot(), before);
    assert_eq!(typed_sample(&mut live, instance, 250), frame);
}

#[test]
fn preview_shares_immutable_held_history_but_owns_pause_and_resume_state() {
    let mut definition = focus_definition();
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: None,
        },
        configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
            points: [0.2, 0.8]
                .into_iter()
                .enumerate()
                .map(|(index, level)| DynamicKeyframe {
                    position: index as f32 * 0.5,
                    source: DynamicValueSource::Value {
                        value: DynamicValue::Family(light_core::AttributeValue::Normalized(level)),
                    },
                    interpolation: ScalarInterpolation::Linear,
                })
                .collect(),
            size: 1.0,
        }),
    });
    let target = FixtureId::new();
    let mut live = DynamicRuntime::default();
    live.install_definitions([definition.clone()]).unwrap();
    let mut c = controller(305, 1, false);
    c.size = 0.5;
    let mut request = start_request(definition.id, c, target, 0, false);
    request.activation_duration_millis = 1_000;
    let instance = live.start(request).unwrap();
    let sample = |runtime: &mut DynamicRuntime, now| {
        let synchronized = DynamicSpeedTransport {
            phase_reference_millis: now,
            beat_phase: (now as f64 / 1_000.0).rem_euclid(1.0),
            ..transport()[0]
        };
        let sampled = runtime
            .sample_all_programming_addressed(
                now,
                10,
                &[synchronized; 5],
                &Sources { current: 0.0 },
                &PreviewSources,
                None,
            )
            .unwrap()
            .remove(0);
        assert_eq!(sampled.instance_id, instance);
        sampled
    };
    sample(&mut live, 250);
    live.set_global_paused(true, 250);
    let held = sample(&mut live, 250);
    let before = live.snapshot();
    let mut preview = live.fork_for_preview();
    let forked = sample(&mut preview, 700);
    assert_eq!(forked.expression, held.expression);
    let (
        DynamicSampleExpression::Retained {
            tape: live_tape,
            root: live_root,
        },
        DynamicSampleExpression::Retained {
            tape: preview_tape,
            root: preview_root,
        },
    ) = (&held.expression, &forked.expression)
    else {
        panic!("paused source retains its immutable history tape")
    };
    assert!(Arc::ptr_eq(live_tape, preview_tape));
    assert_eq!(live_root, preview_root);
    preview.set_global_paused(false, 700);
    sample(&mut preview, 950);
    assert!(
        preview.snapshot().instances[0]
            .synchronized_resume_transition
            .is_some()
    );
    assert_eq!(live.snapshot(), before);
    assert_eq!(sample(&mut live, 950).expression, held.expression);
}
