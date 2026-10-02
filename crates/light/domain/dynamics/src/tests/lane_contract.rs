use super::*;
use light_core::programming::{ProgrammingComponent, TargetReference};

fn value(value: f32) -> DynamicValueSource {
    DynamicValueSource::Value {
        value: DynamicValue::Scalar(value),
    }
}

fn pan() -> DynamicLane {
    DynamicLane {
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: Some(ProgrammingComponent::Pan),
            },
            configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
                minimum: value(-720.0),
                maximum: value(720.0),
                function: PeriodicFunction::LinearUp,
                size: 1.0,
                pwm: PwmShape::default(),
            }),
        }),
        ..lane()
    }
}

#[test]
fn angle_pair_normalization_is_persisted_stable_and_prunes_only_untouched_partners() {
    let authored = pan();
    let mut original = definition(authored.clone());
    assert!(
        validate_definition(&original).is_err(),
        "uncompleted runtime definitions are invalid"
    );
    let raw = serde_json::to_value(&original).unwrap();
    let mut restored: DynamicDefinition = serde_json::from_value(raw).unwrap();
    assert_eq!(restored.lanes.len(), 2);
    assert!(restored.lanes[1].is_angle_current_passthrough());
    let partner_id = restored.lanes[1].id;
    validate_definition(&restored).unwrap();
    restored.revision += 1;
    restored.normalize_angle_pair();
    assert_eq!(restored.lanes[1].id, partner_id);
    let mut copied = restored.clone();
    copied.lanes[1].phase = Some(copied.phase.clone());
    copied.lanes[1].speed_multiplier = Rational {
        numerator: 2,
        denominator: 1,
    };
    copied.reidentify(Uuid::new_v4());
    assert_ne!(copied.lanes[1].id, partner_id);
    assert_eq!(copied.lanes[0].id, authored.id);
    copied.lanes.retain(|lane| lane.id != authored.id);
    copied.normalize_angle_pair();
    assert!(
        copied.lanes.is_empty(),
        "copy retains automatic partner lifecycle"
    );
    assert_eq!(
        serde_json::from_value::<DynamicDefinition>(serde_json::to_value(&restored).unwrap())
            .unwrap(),
        restored
    );

    // Deleting the generated partner recreates it; deleting the authored lane removes the pair.
    original = restored.clone();
    restored.lanes.retain(|lane| lane.id != partner_id);
    restored.normalize_angle_pair();
    assert_eq!(restored, original);
    restored.lanes.retain(|lane| lane.id != authored.id);
    restored.lanes.push(lane());
    restored.normalize_angle_pair();
    assert_eq!(restored.lanes.len(), 1);
    assert!(!restored.lanes[0].is_programming_angles());

    // An operator edit promotes the partner to an authored lane. Removing Pan retains it
    // and adds a Current Pan lane, instead of deleting the operator's Tilt animation.
    let mut edited_tilt = authored.clone();
    edited_tilt.id = partner_id;
    let DynamicLaneBody::Programming(body) = &mut edited_tilt.body else {
        panic!()
    };
    body.address.component = Some(ProgrammingComponent::Tilt);
    original.lanes = vec![edited_tilt.clone()];
    original.normalize_angle_pair();
    assert_eq!(original.lanes[0], edited_tilt);
    assert!(original.lanes[1].is_angle_current_passthrough());
    validate_definition(&original).unwrap();
}

#[test]
fn ordered_lane_document_round_trip_preserves_units_and_rejects_ambiguous_bodies() {
    let mut definition = definition(pan());
    definition.lanes.push(lane());
    definition.normalize_angle_pair();
    let saved = serde_json::to_value(&definition).unwrap();
    assert_eq!(
        saved["lanes"][0]["programming"]["configuration"]["configuration"]["minimum"]["value"]["value"],
        -720.0
    );
    assert!(saved["lanes"][0].get("attribute").is_none());
    assert_eq!(saved["lanes"][1]["attribute"], "intensity");
    assert_eq!(
        serde_json::from_value::<DynamicDefinition>(saved.clone()).unwrap(),
        definition
    );
    assert_eq!(definition.required_programming_contract(), 1);
    for extra in [serde_json::json!("pan"), serde_json::Value::Null] {
        let mut mixed = saved["lanes"][0].clone();
        mixed["attribute"] = extra;
        assert!(serde_json::from_value::<DynamicLane>(mixed).is_err());
    }
    let mut scalar = saved["lanes"][1].clone();
    scalar["programming"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<DynamicLane>(scalar).is_err());
    validate_definition(&definition).unwrap();
    definition.lanes[1].legacy_mut().unwrap().attribute = AttributeKey("pan".into());
    assert!(validate_definition(&definition).is_err());
}

struct Current;
impl DynamicValueSourceResolver for Current {
    fn current(&self, _: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        match address.component {
            Some(ProgrammingComponent::Pan) => Some(DynamicValue::Scalar(-90.0)),
            Some(ProgrammingComponent::Tilt) => Some(DynamicValue::Scalar(90.0)),
            _ => None,
        }
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
fn sample(
    lane: &mut CompiledProgrammingLane,
    elapsed_millis: u64,
    random_envelope: Option<f32>,
) -> DynamicValue {
    let expression = lane
        .sample(ProgrammingEvaluationContext {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            authored_occurrence: None,
            target: FixtureId(Uuid::from_u128(5)),
            elapsed_millis,
            cycle_duration_millis: 1000,
            phase_degrees: 0.0,
            random_envelope,
            sources: &Current,
        })
        .unwrap()
        .unwrap();
    let DynamicSampleExpression::Programming { value, .. } = expression else {
        panic!("expected resolved typed value");
    };
    value
}

#[test]
fn typed_waveforms_reuse_width_speed_and_preserve_signed_multiple_turns() {
    let mut lane = pan();
    let mut compiled = CompiledProgrammingLane::new(&lane, &[], None).unwrap();
    assert_eq!(
        sample(&mut compiled, 750, None),
        DynamicValue::Scalar(360.0)
    );
    lane.speed_multiplier = Rational {
        numerator: 2,
        denominator: 1,
    };
    let mut compiled = CompiledProgrammingLane::new(&lane, &[], None).unwrap();
    assert_eq!(
        sample(&mut compiled, 375, None),
        DynamicValue::Scalar(360.0)
    );
    lane.width = 0.5;
    let mut compiled = CompiledProgrammingLane::new(&lane, &[], None).unwrap();
    assert_eq!(
        sample(&mut compiled, 100, None),
        DynamicValue::Scalar(-720.0)
    );
    let DynamicLaneBody::Programming(body) = &mut lane.body else {
        panic!()
    };
    body.configuration =
        ProgrammingLaneConfiguration::MiddleAmplitude(MiddleAmplitudeConfiguration {
            middle: DynamicValueSource::Current,
            amplitude: DynamicValue::Scalar(720.0),
            function: PeriodicFunction::Cosinus,
            size: 2.0,
            pwm: PwmShape::default(),
            invert_waveform: false,
        });
    let mut compiled = CompiledProgrammingLane::new(&lane, &[], None).unwrap();
    assert_eq!(sample(&mut compiled, 0, None), DynamicValue::Scalar(1350.0));
}

#[test]
fn typed_random_group_keeps_shared_range_and_independent_current_addresses() {
    let id = Uuid::new_v4();
    let mut pan = pan();
    pan.random_group_id = Some(id);
    let DynamicLaneBody::Programming(body) = &mut pan.body else {
        panic!()
    };
    body.configuration = ProgrammingLaneConfiguration::Random;
    let mut tilt = pan.clone();
    tilt.id = Uuid::new_v4();
    let DynamicLaneBody::Programming(body) = &mut tilt.body else {
        panic!()
    };
    body.address.component = Some(ProgrammingComponent::Tilt);
    let group = DynamicRandomGroup {
        id,
        seed: 7,
        range: DynamicRandomRange::Programming {
            low: DynamicValueSource::Current,
            high: value(180.0),
        },
        decision_interval_millis: 250,
        start_probability: 0.4,
        mean_duration_millis: 500,
        duration_spread_millis: 100,
        attack_ratio: 0.1,
        decay_ratio: 0.1,
    };
    let mut definition = definition(pan.clone());
    definition.lanes.push(tilt.clone());
    definition.random_groups.push(group);
    validate_definition(&definition).unwrap();
    let mut a = CompiledProgrammingLane::new(&pan, &definition.random_groups, None).unwrap();
    let mut b = CompiledProgrammingLane::new(&tilt, &definition.random_groups, None).unwrap();
    assert_eq!(sample(&mut a, 25, Some(0.5)), DynamicValue::Scalar(45.0));
    assert_eq!(sample(&mut b, 25, Some(0.5)), DynamicValue::Scalar(135.0));
    definition.lanes.reverse();
    validate_definition(&definition).unwrap();
    definition.lanes[0].speed_multiplier.numerator = 2;
    assert!(validate_definition(&definition).is_err());
    definition.lanes[0].speed_multiplier.numerator = 1;
    let saved = serde_json::to_value(&definition.random_groups[0]).unwrap();
    assert!(saved.get("low").is_none());
    let mut mixed = saved.clone();
    mixed["low"] = serde_json::json!({"type":"value","value":0.0});
    assert!(serde_json::from_value::<DynamicRandomGroup>(mixed).is_err());
    assert_eq!(
        serde_json::from_value::<DynamicRandomGroup>(saved).unwrap(),
        definition.random_groups[0]
    );
}

#[test]
fn typed_definitions_reject_mixed_position_representations_and_whole_numeric_configs() {
    let mut definition = definition(pan());
    let mut target = pan();
    let DynamicLaneBody::Programming(body) = &mut target.body else {
        panic!()
    };
    body.address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Target {
            reference: Some(TargetReference::Origin),
        },
        component: Some(ProgrammingComponent::TargetX),
    };
    definition.lanes.push(target);
    assert!(validate_definition(&definition).is_err());
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        panic!()
    };
    body.address.component = None;
    assert!(body.validate().is_err());
}

#[test]
fn unsupported_runtime_rejects_typed_install_fallback_and_restore_atomically() {
    let typed = definition(pan());
    let legacy = definition(lane());
    let mut source = DynamicRuntime::default();
    source.install_definitions([typed.clone()]).unwrap();
    source
        .start(start_request(
            typed.id,
            controller(1, 0, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let saved = source.snapshot();
    let mut runtime = DynamicRuntime::with_programming_contract_support(0);
    runtime.install_definitions([legacy.clone()]).unwrap();
    runtime
        .start(start_request(
            legacy.id,
            controller(2, 0, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let before = runtime.snapshot();
    assert!(
        runtime
            .install_definitions([typed.clone()])
            .unwrap_err()
            .to_string()
            .contains("programming contract 1")
    );
    assert!(runtime.install_fallback_definition(typed).is_err());
    assert!(runtime.restore_snapshot(saved).is_err());
    assert_eq!(runtime.snapshot(), before);
}

#[test]
fn live_target_keyframes_retain_both_expressions_and_reuse_cached_endpoints() {
    use light_core::{
        AttributeValue,
        programming::{PositionIntent, TransitionRequirement},
    };
    let point = Uuid::from_u128(123);
    let target = |reference| DynamicValueSource::Value {
        value: DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::target(
            reference,
            [1.0, 2.0, 3.0],
        )))),
    };
    let lane = DynamicLane {
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Target { reference: None },
                component: None,
            },
            configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                points: vec![
                    DynamicKeyframe {
                        position: 0.0,
                        source: target(TargetReference::Origin),
                        interpolation: ScalarInterpolation::Linear,
                    },
                    DynamicKeyframe {
                        position: 0.5,
                        source: target(TargetReference::Point { point_id: point }),
                        interpolation: ScalarInterpolation::Linear,
                    },
                ],
                size: 1.0,
            }),
        }),
        ..pan()
    };
    let mut compiled = CompiledProgrammingLane::new(&lane, &[], None).unwrap();
    let authored = DynamicSourceOccurrenceId::new(Uuid::from_u128(44)).unwrap();
    let replacement = DynamicSourceOccurrenceId::new(Uuid::from_u128(45)).unwrap();
    let mut at = |millis, authored_occurrence| {
        compiled
            .sample(ProgrammingEvaluationContext {
                instance_id: Uuid::from_u128(1),
                controller_id: Uuid::from_u128(2),
                authored_occurrence,
                target: FixtureId(Uuid::from_u128(5)),
                elapsed_millis: millis,
                cycle_duration_millis: 1000,
                phase_degrees: 0.0,
                random_envelope: None,
                sources: &Current,
            })
            .unwrap()
            .unwrap()
    };
    let mut first = at(125, Some(authored));
    let mut second = at(250, Some(authored));
    first.bind_fresh_authored_occurrence(Some(authored));
    second.bind_fresh_authored_occurrence(Some(authored));
    let DynamicSampleExpression::Transition {
        from: Some(from),
        to: Some(to),
        progress,
        reason,
    } = &first
    else {
        panic!("retain live transition");
    };
    assert_eq!(*progress, 0.25);
    assert_eq!(
        *reason,
        DynamicTransitionReason::Required {
            requirement: TransitionRequirement::LiveTargetPoints
        }
    );
    let DynamicSampleExpression::Transition {
        from: Some(next_from),
        to: Some(next_to),
        progress,
        ..
    } = &second
    else {
        panic!("retain live transition");
    };
    assert_eq!(*progress, 0.5);
    assert!(Arc::ptr_eq(from, next_from) && Arc::ptr_eq(to, next_to));
    let replaced = at(250, Some(replacement));
    let DynamicSampleExpression::Transition {
        from: Some(replaced_from),
        to: Some(replaced_to),
        ..
    } = &replaced
    else {
        panic!("retain replacement transition");
    };
    assert!(!Arc::ptr_eq(from, replaced_from) && !Arc::ptr_eq(to, replaced_to));
    assert!(matches!(
        from.as_ref(),
        DynamicSampleExpression::Programming {
            occurrence: Some(source),
            ..
        } if *source == authored
    ));
    assert!(matches!(
        replaced_from.as_ref(),
        DynamicSampleExpression::Programming {
            occurrence: Some(source),
            ..
        } if *source == replacement
    ));
    assert!(
        matches!(
            at(500, Some(replacement)),
            DynamicSampleExpression::Programming { .. }
        ),
        "an exact endpoint needs no conversion"
    );
    let resumed = DynamicSampleExpression::Transition {
        from: Some(Arc::new(first)),
        to: Some(Arc::new(second)),
        progress: 0.1,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::new_v4(),
        },
    };
    resumed.validate().unwrap();
    let saved = serde_json::to_value(&resumed).unwrap();
    assert!(saved.to_string().contains(&point.to_string()));
    let restored: DynamicSampleExpression = serde_json::from_value(saved).unwrap();
    assert_eq!(restored, resumed);
    assert_eq!(restored.required_programming_contract(), 1);
    assert!(
        DynamicSampleExpression::Transition {
            from: None,
            to: None,
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::new_v4()
            }
        }
        .validate()
        .is_err()
    );
}

#[test]
fn recorded_selection_preserves_legacy_activation_and_freezes_ids_at_typed_boundary() {
    let target = FixtureId::new();
    let mut legacy = definition(lane());
    legacy.lanes.push(lane());
    let old_ids = legacy.lanes.iter().map(|lane| lane.id).collect::<Vec<_>>();
    let mut reference = DynamicReference {
        dynamic_id: Some(legacy.id),
        last_known_pool_number: 1,
        embedded_fallback: DynamicDefinitionSnapshot {
            definition: Arc::new(legacy.clone()),
        },
    };
    let sparse_old = [(target, old_ids[0])];
    assert_eq!(
        DynamicLaneSelection::for_recorded_values(&reference, &legacy, &sparse_old),
        DynamicLaneSelection::All
    );
    let mut modern = legacy.clone();
    modern.lanes.push(pan());
    let new_lane = modern.lanes[2].id;
    let DynamicLaneSelection::PerTarget { targets } =
        DynamicLaneSelection::for_recorded_values(&reference, &modern, &sparse_old)
    else {
        panic!("explicit legacy expansion");
    };
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].lanes.len(), 2);
    assert!(old_ids.iter().all(|id| targets[0].lanes.contains(id)));
    assert!(!targets[0].lanes.contains(&new_lane));
    reference.embedded_fallback.definition = Arc::new(modern.clone());
    modern.target_binding = DynamicTargetBinding::LiveGroup {
        group_id: "front".into(),
    };
    assert_eq!(
        DynamicLaneSelection::for_recorded_values(
            &reference,
            &modern,
            &[(target, new_lane), (target, new_lane)]
        ),
        DynamicLaneSelection::Uniform {
            lanes: vec![new_lane]
        }
    );
}
