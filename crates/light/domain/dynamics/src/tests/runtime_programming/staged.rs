use super::*;
mod requirements;

fn transports(at: u64) -> [DynamicSpeedTransport; 5] {
    [DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: at,
        beat_phase: (at as f64 / 1000.0).rem_euclid(1.0),
        phase_advancing: true,
    }; 5]
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

fn scalar_projection(
    samples: &[DynamicRuntimeSample],
) -> Vec<(Uuid, FixtureId, Uuid, AttributeKey, f32, f32)> {
    let mut values = Vec::new();
    for sample in samples {
        sample
            .expression
            .visit_legacy_contributions(|attribute, value, influence| {
                values.push((
                    sample.controller_id,
                    sample.target,
                    sample.lane_id,
                    attribute.clone(),
                    value,
                    influence * sample.activation_mix,
                ));
            });
    }
    values.sort_by(|a, b| (a.0, a.1.0, a.2, &a.3).cmp(&(b.0, b.1.0, b.2, &b.3)));
    values
}

fn stage_and_compare(
    runtime: &mut DynamicRuntime,
    at: u64,
    sources: &dyn DynamicValueSourceResolver,
) -> Vec<DynamicRuntimeSample> {
    let mut expected = runtime.fork_for_preview();
    let expected_samples = expected
        .sample_all_programming_addressed(
            at,
            10,
            &transports(at),
            &Sources { current: 0.3 },
            sources,
            None,
        )
        .unwrap();
    let mut transaction = DynamicOutputFrameScratch::default();
    let mut scratch = DynamicSamplingScratch::default();
    let result: Result<Vec<DynamicRuntimeSample>, DynamicRuntimeError> = runtime
        .with_output_frame_transaction(&mut transaction, |runtime| {
            runtime.sample_all_programming_staged(
                at,
                10,
                &transports(at),
                &Sources { current: 0.3 },
                sources,
                None,
                &mut scratch,
                |scalar, deferred| {
                    let completed = deferred.complete(sources)?;
                    assert_eq!(
                        scalar_projection(scalar),
                        scalar_projection(completed.samples())
                    );
                    let samples = completed.samples().to_vec();
                    Ok((completed, samples))
                },
            )
        });
    let samples = result.unwrap();
    assert_eq!(sorted(samples.clone()), sorted(expected_samples));
    assert_eq!(runtime.snapshot(), expected.snapshot());
    samples
}

fn focus_lane() -> DynamicLane {
    ramp(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        },
        DynamicValueSource::Current,
        value(DynamicValue::Scalar(0.8)),
    )
}

#[test]
fn staged_and_immediate_sampling_match_with_unchanged_geometry_and_pause_resume() {
    let target = FixtureId::new();
    let mut definition = definition(lane());
    definition.lanes.push(focus_lane());
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let control = controller(101, 1, false);
    let mut request = start_request(definition.id, control.clone(), target, 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    let sources = TypedSources {
        current: Some(DynamicValue::Scalar(0.4)),
        preset: None,
        calls: Cell::new(0),
    };
    stage_and_compare(&mut runtime, 250, &sources);
    runtime
        .set_controller_paused(instance, control.id, true, 250)
        .unwrap();
    stage_and_compare(&mut runtime, 600, &sources);
    runtime
        .set_controller_paused_with_resume(
            instance,
            control.id,
            false,
            700,
            Some(ActivationPolicy::JoinSyncNow),
        )
        .unwrap();
    stage_and_compare(&mut runtime, 900, &sources);
    stage_and_compare(&mut runtime, 1700, &sources);
}

struct FinalGeometrySources {
    ready: Cell<bool>,
    pan: Cell<f32>,
    reads: Cell<usize>,
}
impl DynamicValueSourceResolver for FinalGeometrySources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        assert!(
            self.ready.get(),
            "numeric typed Current must wait for final Point geometry"
        );
        self.reads.set(self.reads.get() + 1);
        Some(DynamicValue::Scalar(self.pan.get()))
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

#[test]
fn scalar_point_stage_precedes_numeric_current_and_history_uses_final_geometry() {
    let mut point = lane();
    let scalar = point.legacy_mut().unwrap();
    scalar.attribute = AttributeKey("point.position.x".into());
    for point in &mut scalar.keyframes.points {
        point.source = ScalarSource::Current;
    }
    let pan = ramp(
        pan_address(),
        DynamicValueSource::Current,
        DynamicValueSource::Current,
    );
    let pan_id = pan.id;
    let mut definition = definition(point);
    definition.lanes.push(pan);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
        .start(start_request(
            definition.id,
            controller(102, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let sources = FinalGeometrySources {
        ready: Cell::new(false),
        pan: Cell::new(-99.0),
        reads: Cell::new(0),
    };
    let mut transaction = DynamicOutputFrameScratch::default();
    let mut scratch = DynamicSamplingScratch::default();
    let callbacks = Cell::new(0);
    let result: Result<(), DynamicRuntimeError> =
        runtime.with_output_frame_transaction(&mut transaction, |runtime| {
            runtime.sample_all_programming_staged(
                250,
                10,
                &transports(250),
                &Sources { current: 0.3 },
                &sources,
                None,
                &mut scratch,
                |scalar, deferred| {
                    callbacks.set(callbacks.get() + 1);
                    assert_eq!(sources.reads.get(), 0);
                    let scalar = scalar_projection(scalar);
                    assert_eq!(scalar.len(), 1);
                    assert_eq!(
                        scalar[0].4, 0.3,
                        "legacy Current keeps its immutable static source"
                    );
                    sources.pan.set(scalar[0].4 * 180.0);
                    sources.ready.set(true);
                    let completed = deferred.complete(&sources)?;
                    let pan = completed
                        .samples()
                        .iter()
                        .find(|sample| sample.lane_id == pan_id)
                        .unwrap();
                    assert_eq!(typed_value(pan), &DynamicValue::Scalar(sources.pan.get()));
                    Ok((completed, ()))
                },
            )
        });
    result.unwrap();
    assert_eq!(callbacks.get(), 1);
    assert!(sources.reads.get() > 0);
    let snapshot = runtime.snapshot();
    let retained = snapshot.instances[0].expression_tape.as_ref().unwrap();
    assert!(retained.nodes.iter().any(|node| matches!(node, RetainedExpressionNode::Programming { value: DynamicValue::Scalar(value), .. } if *value == sources.pan.get())));
}

#[test]
fn callback_and_typed_failures_rollback_and_retry_like_immediate_sampling() {
    let mut definition = definition(lane());
    definition.lanes.push(focus_lane());
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
        .start(start_request(
            definition.id,
            controller(103, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let before = runtime.snapshot();
    let sources = TypedSources {
        current: Some(DynamicValue::Scalar(f32::NAN)),
        preset: None,
        calls: Cell::new(0),
    };
    let mut transaction = DynamicOutputFrameScratch::default();
    let mut scratch = DynamicSamplingScratch::default();
    for finish in [false, true] {
        let result: Result<(), DynamicRuntimeError> =
            runtime.with_output_frame_transaction(&mut transaction, |runtime| {
                runtime.sample_all_programming_staged(
                    250,
                    10,
                    &transports(250),
                    &Sources { current: 0.3 },
                    &sources,
                    None,
                    &mut scratch,
                    |_, deferred| {
                        if !finish {
                            return Err(DynamicRuntimeError::InvalidSample(
                                "geometry failed".into(),
                            ));
                        }
                        let completed = deferred.complete(&sources)?;
                        Ok((completed, ()))
                    },
                )
            });
        assert!(result.is_err());
        assert_eq!(runtime.snapshot(), before);
    }
    let sources = TypedSources {
        current: Some(DynamicValue::Scalar(0.4)),
        preset: None,
        calls: Cell::new(0),
    };
    stage_and_compare(&mut runtime, 250, &sources);
}

fn point_lane(attribute: &str, value: f32) -> DynamicLane {
    let mut point = lane();
    let body = point.legacy_mut().unwrap();
    body.attribute = AttributeKey(attribute.into());
    for point in &mut body.keyframes.points {
        point.source = source(value);
    }
    point
}

fn assert_point_weights(samples: &[DynamicRuntimeSample], expected: &[(&str, f32, f32)]) {
    let actual = scalar_projection(samples);
    assert_eq!(actual.len(), expected.len());
    for (attribute, value, weight) in expected {
        let row = actual
            .iter()
            .find(|row| row.3 == AttributeKey((*attribute).into()))
            .unwrap();
        assert!((row.4 - value).abs() < 0.00001);
        assert!(
            (row.5 - weight).abs() < 0.00001,
            "{attribute}: {} != {weight}",
            row.5
        );
    }
}

#[test]
fn mixed_interrupted_and_deleted_histories_project_the_same_scalar_paths_when_typed_current_is_missing()
 {
    for available in [false, true] {
        let mut definition = definition(point_lane("point.position.x", 0.6));
        let lane_id = definition.lanes[0].id;
        let mut runtime = DynamicRuntime::default();
        runtime.install_definitions([definition.clone()]).unwrap();
        let control = controller(110, 1, false);
        let mut request = start_request(definition.id, control.clone(), FixtureId::new(), 0, false);
        request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
        request.activation_duration_millis = 1000;
        let instance = runtime.start(request).unwrap();
        let sources = TypedSources {
            current: available.then_some(DynamicValue::Scalar(0.4)),
            preset: None,
            calls: Cell::new(0),
        };
        assert_point_weights(
            &stage_and_compare(&mut runtime, 2000, &sources),
            &[("point.position.x", 0.6, 1.0)],
        );
        runtime
            .set_controller_paused(instance, control.id, true, 2000)
            .unwrap();
        let mut focus = focus_lane();
        focus.id = lane_id;
        definition.lanes = vec![focus];
        definition.revision += 1;
        runtime.install_definitions([definition.clone()]).unwrap();
        assert_point_weights(
            &stage_and_compare(&mut runtime, 2200, &sources),
            &[("point.position.x", 0.6, 1.0)],
        );
        assert_eq!(
            sources.calls.get(),
            0,
            "a paused legacy hold never evaluates its edited typed lane"
        );
        runtime
            .set_controller_paused_with_resume(
                instance,
                control.id,
                false,
                2300,
                Some(ActivationPolicy::JoinSyncNow),
            )
            .unwrap();
        assert_point_weights(
            &stage_and_compare(&mut runtime, 2550, &sources),
            &[("point.position.x", 0.6, 0.75)],
        );

        runtime
            .set_controller_paused(instance, control.id, true, 2550)
            .unwrap();
        let mut incoming = point_lane("point.position.y", 0.9);
        incoming.id = lane_id;
        definition.lanes = vec![incoming];
        definition.revision += 1;
        runtime.install_definitions([definition.clone()]).unwrap();
        runtime
            .set_controller_paused_with_resume(
                instance,
                control.id,
                false,
                2600,
                Some(ActivationPolicy::JoinSyncNow),
            )
            .unwrap();
        let nested = stage_and_compare(&mut runtime, 2850, &sources);
        assert_point_weights(
            &nested,
            &[
                ("point.position.x", 0.6, 0.5625),
                ("point.position.y", 0.9, 0.25),
            ],
        );

        runtime
            .set_controller_paused(instance, control.id, true, 2850)
            .unwrap();
        definition.lanes = vec![focus_lane()]; // The retained Point/Focus forest is now deleted.
        definition.revision += 1;
        runtime.install_definitions([definition.clone()]).unwrap();
        assert_point_weights(
            &stage_and_compare(&mut runtime, 3000, &sources),
            &[
                ("point.position.x", 0.6, 0.5625),
                ("point.position.y", 0.9, 0.25),
            ],
        );
        let snapshot =
            serde_json::from_value(serde_json::to_value(runtime.snapshot()).unwrap()).unwrap();
        runtime.restore_snapshot(snapshot).unwrap();
        runtime
            .set_controller_paused_with_resume(
                instance,
                control.id,
                false,
                3100,
                Some(ActivationPolicy::JoinSyncNow),
            )
            .unwrap();
        assert_point_weights(
            &stage_and_compare(&mut runtime, 3350, &sources),
            &[
                ("point.position.x", 0.6, 0.421875),
                ("point.position.y", 0.9, 0.1875),
            ],
        );
        let complete = stage_and_compare(&mut runtime, 4100, &sources);
        assert!(complete.iter().all(|sample| sample.lane_id != lane_id));
        assert!(scalar_projection(&complete).is_empty());
    }
}

#[test]
fn typed_to_legacy_resume_emits_only_the_incoming_scalar_influence() {
    let mut definition = definition(focus_lane());
    let lane_id = definition.lanes[0].id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let control = controller(111, 1, false);
    let mut request = start_request(definition.id, control.clone(), FixtureId::new(), 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    let sources = TypedSources {
        current: Some(DynamicValue::Scalar(0.4)),
        preset: None,
        calls: Cell::new(0),
    };
    assert!(scalar_projection(&stage_and_compare(&mut runtime, 2000, &sources)).is_empty());
    runtime
        .set_controller_paused(instance, control.id, true, 2000)
        .unwrap();
    let mut point = point_lane("point.position.z", 0.7);
    point.id = lane_id;
    definition.lanes = vec![point];
    definition.revision += 1;
    runtime.install_definitions([definition]).unwrap();
    assert!(scalar_projection(&stage_and_compare(&mut runtime, 2200, &sources)).is_empty());
    runtime
        .set_controller_paused_with_resume(
            instance,
            control.id,
            false,
            2300,
            Some(ActivationPolicy::JoinSyncNow),
        )
        .unwrap();
    assert_point_weights(
        &stage_and_compare(&mut runtime, 2550, &sources),
        &[("point.position.z", 0.7, 0.25)],
    );
}

fn random_definition() -> (DynamicDefinition, Vec<Uuid>, Uuid) {
    let group_id = Uuid::from_u128(1200);
    let mut first = lane();
    first.legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    first.random_group_id = Some(group_id);
    let mut second = first.clone();
    second.id = Uuid::from_u128(1201);
    second.legacy_mut().unwrap().attribute = AttributeKey("point.position.x".into());
    let mut skipped = first.clone();
    skipped.id = Uuid::from_u128(1202);
    skipped.random_group_id = Some(Uuid::from_u128(1203));
    let focus = focus_lane();
    let selected = vec![first.id, second.id, focus.id];
    let mut definition = definition(first);
    definition.lanes.extend([second, skipped, focus]);
    let group = DynamicRandomGroup {
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
    };
    definition.random_groups.push(group.clone());
    definition.random_groups.push(DynamicRandomGroup {
        id: Uuid::from_u128(1203),
        ..group
    });
    (definition, selected, group_id)
}

#[test]
fn random_reached_groups_are_shared_and_failures_at_each_stage_retry_without_advancing() {
    let (definition, selected, group_id) = random_definition();
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let control = controller(112, 1, false);
    let mut request = start_request(definition.id, control.clone(), FixtureId::new(), 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    let instance = runtime.start(request).unwrap();
    runtime
        .set_controller_lane_selection(
            instance,
            control.id,
            DynamicLaneSelection::Uniform { lanes: selected },
        )
        .unwrap();
    let sources = TypedSources {
        current: Some(DynamicValue::Scalar(0.4)),
        preset: None,
        calls: Cell::new(0),
    };
    let invalid = TypedSources {
        current: Some(DynamicValue::Scalar(f32::NAN)),
        preset: None,
        calls: Cell::new(0),
    };
    let before = runtime.snapshot();
    let mut scratch = DynamicSamplingScratch::default();
    let mut transaction = DynamicOutputFrameScratch::default();
    for failure_stage in 0..3 {
        let result: Result<(), DynamicRuntimeError> =
            runtime.with_output_frame_transaction(&mut transaction, |runtime| {
                runtime.sample_all_programming_staged(
                    675,
                    10,
                    &transports(675),
                    &Sources { current: 0.3 },
                    &sources,
                    None,
                    &mut scratch,
                    |scalar, deferred| {
                        assert_eq!(scalar.len(), 2);
                        let values = scalar_projection(scalar);
                        assert_eq!(
                            values[0].4, values[1].4,
                            "shared group members use one envelope"
                        );
                        if failure_stage == 0 {
                            return Err(DynamicRuntimeError::InvalidSample(
                                "Point resolution failed".into(),
                            ));
                        }
                        let _completed = deferred.complete(if failure_stage == 1 {
                            &invalid
                        } else {
                            &sources
                        })?;
                        Err(DynamicRuntimeError::InvalidSample(
                            "final encoding failed".into(),
                        ))
                    },
                )
            });
        assert!(result.is_err());
        assert_eq!(runtime.snapshot(), before);
    }
    stage_and_compare(&mut runtime, 675, &sources);
    let streams = runtime.snapshot().instances[0].random_streams.clone();
    assert_eq!(streams.len(), 1, "the unselected group never creates state");
    assert_eq!(streams[0].group_id, group_id);
    assert_eq!(streams[0].last_elapsed_millis, 675);
    assert_eq!(streams[0].next_decision_index, 7);
    runtime
        .set_controller_paused(instance, control.id, true, 675)
        .unwrap();
    stage_and_compare(&mut runtime, 900, &sources);
    assert_eq!(
        runtime.snapshot().instances[0].random_streams,
        streams,
        "held samples do not advance Random"
    );
}

#[test]
fn staged_release_and_one_shot_completion_rollback_until_the_complete_frame_succeeds() {
    let mut one_shot = definition(lane());
    one_shot.run_mode = DynamicRunMode::OneShot;
    let regular = definition(lane());
    let mut runtime = DynamicRuntime::default();
    runtime
        .install_definitions([one_shot.clone(), regular.clone()])
        .unwrap();
    let target = FixtureId::new();
    let shot = runtime
        .start(start_request(
            one_shot.id,
            controller(113, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let released = runtime
        .start(start_request(
            regular.id,
            controller(114, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let sources = TypedSources {
        current: None,
        preset: None,
        calls: Cell::new(0),
    };
    stage_and_compare(&mut runtime, 100, &sources);
    runtime
        .off_controller(released, Uuid::from_u128(114), 100, 0, 100)
        .unwrap();
    let before = runtime.snapshot();
    let mut transaction = DynamicOutputFrameScratch::default();
    let mut scratch = DynamicSamplingScratch::default();
    let result: Result<(), DynamicRuntimeError> =
        runtime.with_output_frame_transaction(&mut transaction, |runtime| {
            runtime.sample_all_programming_staged(
                1200,
                10,
                &transports(1200),
                &Sources { current: 0.3 },
                &sources,
                None,
                &mut scratch,
                |scalar, deferred| {
                    assert!(scalar.is_empty());
                    let completed = deferred.complete(&sources)?;
                    assert!(completed.samples().is_empty());
                    Err(DynamicRuntimeError::InvalidSample("encoding failed".into()))
                },
            )
        });
    assert!(result.is_err());
    assert_eq!(runtime.snapshot(), before);
    assert!(stage_and_compare(&mut runtime, 1200, &sources).is_empty());
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.instances.len(), 1);
    assert_eq!(snapshot.instances[0].id, shot);
    assert!(snapshot.instances[0].completed);
}

#[test]
fn staged_sampling_without_an_output_transaction_returns_an_error_without_invoking_callback() {
    let mut runtime = DynamicRuntime::default();
    let sources = TypedSources {
        current: None,
        preset: None,
        calls: Cell::new(0),
    };
    let mut scratch = DynamicSamplingScratch::default();
    let called = Cell::new(false);
    let result: Result<(), DynamicRuntimeError> = runtime.sample_all_programming_staged(
        0,
        10,
        &transports(0),
        &Sources { current: 0.3 },
        &sources,
        None,
        &mut scratch,
        |_, deferred| {
            called.set(true);
            Ok((deferred.complete(&sources)?, ()))
        },
    );
    assert!(matches!(result, Err(DynamicRuntimeError::InvalidSample(_))));
    assert!(!called.get());
}
