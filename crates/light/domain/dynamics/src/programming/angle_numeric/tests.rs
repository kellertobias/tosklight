use super::*;
use crate::*;
use light_core::{AttributeValue, FixtureId, programming::PositionIntent};
use std::cell::Cell;
use uuid::Uuid;

fn pan() -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    }
}
fn constant(value: f32) -> DynamicValueSource {
    DynamicValueSource::Value {
        value: DynamicValue::Scalar(value),
    }
}
fn keyframes(size: f32) -> ProgrammingLaneConfiguration {
    ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
        points: vec![
            DynamicKeyframe {
                position: 0.,
                source: DynamicValueSource::Current,
                interpolation: ScalarInterpolation::Linear,
            },
            DynamicKeyframe {
                position: 0.5,
                source: constant(180.),
                interpolation: ScalarInterpolation::Linear,
            },
        ],
        size,
    })
}
fn lane(configuration: ProgrammingLaneConfiguration) -> DynamicLane {
    DynamicLane {
        id: Uuid::new_v4(),
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: pan(),
            configuration,
        }),
        speed_multiplier: Rational::ONE,
        width: 1.,
        phase: None,
        random_group_id: None,
    }
}
struct Sources {
    coherent: bool,
    value: f32,
    current_reads: Cell<usize>,
    family_reads: Cell<usize>,
}
impl Sources {
    fn new(coherent: bool, value: f32) -> Self {
        Self {
            coherent,
            value,
            current_reads: Cell::new(0),
            family_reads: Cell::new(0),
        }
    }
    fn context(&self) -> ProgrammingEvaluationContext<'_> {
        ProgrammingEvaluationContext {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            authored_occurrence: Some(DynamicSourceOccurrenceId::new(Uuid::from_u128(3)).unwrap()),
            target: FixtureId::new(),
            elapsed_millis: 250,
            cycle_duration_millis: 1000,
            phase_degrees: 0.,
            random_envelope: Some(0.3),
            sources: self,
        }
    }
}
impl DynamicValueSourceResolver for Sources {
    fn try_position_current_family(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.family_reads.set(self.family_reads.get() + 1);
        Ok(self
            .coherent
            .then(|| AttributeValue::Position(Arc::new(PositionIntent::angles(self.value, 20.)))))
    }
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        self.current_reads.set(self.current_reads.get() + 1);
        Some(DynamicValue::Scalar(self.value))
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
fn pinned(
    compiled: &CompiledProgrammingLane,
    sources: &Sources,
    size: f32,
) -> Arc<AngleNumericProgram> {
    let AngleNumericSample::Program(program) = compiled
        .pin_angle_numeric(&sources.context(), size)
        .unwrap()
    else {
        panic!("expected a pinned Angle program")
    };
    assert_eq!(
        sources.current_reads.get(),
        0,
        "pinning must not adopt root Current"
    );
    program
}
fn legacy_value(
    compiled: &mut CompiledProgrammingLane,
    sources: &Sources,
    size: f32,
) -> DynamicValue {
    let context = sources.context();
    let target = context.target;
    let expression = compiled.sample(context).unwrap().unwrap();
    let expression = compiled
        .apply_controller_size(expression, size, target, sources)
        .unwrap();
    let DynamicSampleExpression::Programming { value, .. } = expression else {
        panic!("scalar lane")
    };
    value
}

#[test]
fn keyframe_and_controller_sizes_are_pinned_separately_and_rebind_per_destination() {
    let mut compiled = CompiledProgrammingLane::new(&lane(keyframes(0.5)), &[], None).unwrap();
    let sources = Sources::new(true, 30.);
    let program = pinned(&compiled, &sources, 2.);
    assert!(program.uses_current());
    assert_eq!(
        program
            .nodes
            .iter()
            .filter(|node| matches!(node, AngleNumericNode::ScaleFrom { .. }))
            .count(),
        2
    );
    assert_eq!(
        program
            .nodes
            .iter()
            .filter(|node| matches!(node, AngleNumericNode::Current))
            .count(),
        1
    );
    let serialized = serde_json::to_string(&*program).unwrap();
    assert_eq!(
        serde_json::from_str::<AngleNumericProgram>(&serialized).unwrap(),
        *program
    );
    for current in [30., 300., -720.] {
        let expected = legacy_value(&mut compiled, &Sources::new(false, current), 2.);
        assert_eq!(
            program
                .evaluate(compiled.address(), Some(&DynamicValue::Scalar(current)))
                .unwrap(),
            expected
        );
    }
    assert_eq!(serde_json::to_string(&*program).unwrap(), serialized);
}

#[test]
fn max_min_middle_and_random_preserve_existing_math_without_resampling() {
    let random_id = Uuid::new_v4();
    let groups = [DynamicRandomGroup {
        id: random_id,
        seed: 7,
        range: DynamicRandomRange::Programming {
            low: DynamicValueSource::Current,
            high: constant(500.),
        },
        decision_interval_millis: 100,
        start_probability: 1.,
        mean_duration_millis: 200,
        duration_spread_millis: 0,
        attack_ratio: 0.2,
        decay_ratio: 0.2,
    }];
    let configs = [
        ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: DynamicValueSource::Current,
            maximum: constant(180.),
            function: PeriodicFunction::Sinus,
            size: 1.7,
            pwm: PwmShape::default(),
        }),
        ProgrammingLaneConfiguration::MiddleAmplitude(MiddleAmplitudeConfiguration {
            middle: DynamicValueSource::Current,
            amplitude: DynamicValue::Scalar(270.),
            function: PeriodicFunction::LinearUp,
            size: 1.5,
            pwm: PwmShape::default(),
            invert_waveform: true,
        }),
        ProgrammingLaneConfiguration::Random,
    ];
    for config in configs {
        let mut definition = lane(config);
        definition.random_group_id = Some(random_id);
        let mut compiled = CompiledProgrammingLane::new(&definition, &groups, None).unwrap();
        let program = pinned(&compiled, &Sources::new(true, 10.), 0.75);
        for current in [10., 720.] {
            assert_eq!(
                program
                    .evaluate(compiled.address(), Some(&DynamicValue::Scalar(current)))
                    .unwrap(),
                legacy_value(&mut compiled, &Sources::new(false, current), 0.75)
            );
        }
    }
}

#[test]
fn scalar_only_readers_and_unused_current_keep_the_legacy_path() {
    let mut compiled = CompiledProgrammingLane::new(&lane(keyframes(1.)), &[], None).unwrap();
    let legacy = Sources::new(false, 40.);
    assert!(matches!(
        compiled.pin_angle_numeric(&legacy.context(), 1.).unwrap(),
        AngleNumericSample::NotApplicable
    ));
    assert_eq!(legacy.current_reads.get(), 0);
    assert!(compiled.sample(legacy.context()).unwrap().is_some());
    assert_eq!(legacy.current_reads.get(), 1);

    // The first keyframe is not selected here and its Size pivot is inactive.
    let configuration = ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
        points: vec![
            DynamicKeyframe {
                position: 0.,
                source: DynamicValueSource::Current,
                interpolation: ScalarInterpolation::Linear,
            },
            DynamicKeyframe {
                position: 0.2,
                source: constant(20.),
                interpolation: ScalarInterpolation::Linear,
            },
            DynamicKeyframe {
                position: 0.8,
                source: constant(80.),
                interpolation: ScalarInterpolation::Linear,
            },
        ],
        size: 1.,
    });
    let compiled = CompiledProgrammingLane::new(&lane(configuration), &[], None).unwrap();
    let sources = Sources::new(true, 40.);
    assert!(matches!(
        compiled.pin_angle_numeric(&sources.context(), 1.).unwrap(),
        AngleNumericSample::NotApplicable
    ));
    assert_eq!(sources.family_reads.get(), 0);
    assert_eq!(sources.current_reads.get(), 0);
    let program = pinned(&compiled, &sources, 0.5);
    assert!(
        program.uses_current(),
        "controller Size requires a Current pivot even for constant endpoints"
    );
}

#[test]
fn missing_preset_and_random_envelope_are_absent_without_scalar_current_fallback() {
    let configuration = ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
        minimum: DynamicValueSource::Current,
        maximum: DynamicValueSource::Preset {
            preset_id: "1".into(),
            address: pan(),
            last_valid_by_target: vec![],
            retained: None,
        },
        function: PeriodicFunction::LinearUp,
        size: 1.,
        pwm: PwmShape::default(),
    });
    let compiled = CompiledProgrammingLane::new(&lane(configuration), &[], None).unwrap();
    let sources = Sources::new(true, 40.);
    assert!(matches!(
        compiled.pin_angle_numeric(&sources.context(), 1.).unwrap(),
        AngleNumericSample::Absent
    ));
    assert_eq!(sources.current_reads.get(), 0);
    let group = DynamicRandomGroup {
        id: Uuid::new_v4(),
        seed: 0,
        range: DynamicRandomRange::Programming {
            low: DynamicValueSource::Current,
            high: constant(50.),
        },
        decision_interval_millis: 100,
        start_probability: 1.,
        mean_duration_millis: 200,
        duration_spread_millis: 0,
        attack_ratio: 0.2,
        decay_ratio: 0.2,
    };
    let mut definition = lane(ProgrammingLaneConfiguration::Random);
    definition.random_group_id = Some(group.id);
    let compiled = CompiledProgrammingLane::new(&definition, &[group], None).unwrap();
    let mut context = sources.context();
    context.random_envelope = None;
    assert!(matches!(
        compiled.pin_angle_numeric(&context, 1.).unwrap(),
        AngleNumericSample::Absent
    ));
    assert_eq!(sources.current_reads.get(), 0);
}

#[test]
fn validation_rejects_malformed_or_unbounded_graphs_and_requires_current() {
    let compiled = CompiledProgrammingLane::new(&lane(keyframes(1.)), &[], None).unwrap();
    let valid = pinned(&compiled, &Sources::new(true, 20.), 1.);
    assert!(matches!(
        valid.evaluate(compiled.address(), None),
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles
        ))
    ));
    let mut bad = (*valid).clone();
    bad.root = 99;
    assert!(bad.validate().is_err());
    let mut bad = (*valid).clone();
    bad.nodes.push(AngleNumericNode::Current);
    assert!(bad.validate().is_err(), "unreachable node");
    let mut bad = (*valid).clone();
    bad.nodes[0] = AngleNumericNode::ScaleFrom {
        pivot: 0,
        value: 0,
        factor: 1.,
    };
    assert!(bad.validate().is_err(), "self edge");
    let mut bad = (*valid).clone();
    bad.nodes[2] = AngleNumericNode::Transition {
        from: 0,
        to: 1,
        progress: f32::NAN,
    };
    assert!(bad.validate().is_err());
    let mut bad = (*valid).clone();
    bad.nodes = vec![AngleNumericNode::Current; ANGLE_NUMERIC_MAX_NODES + 1];
    assert!(bad.validate().is_err());
    let mut bad = (*valid).clone();
    bad.address.component = Some(ProgrammingComponent::TargetX);
    assert!(bad.validate().is_err());
    let mut bad = (*valid).clone();
    bad.nodes[2] = AngleNumericNode::Around {
        middle: 0,
        amplitude: DynamicValue::Scalar(-1.),
        amount: 1.,
    };
    assert!(bad.validate().is_err());
    let mut bad = (*valid).clone();
    bad.nodes[2] = AngleNumericNode::Around {
        middle: 0,
        amplitude: DynamicValue::Scalar(1.),
        amount: f64::INFINITY,
    };
    assert!(bad.validate().is_err());
    let tilt = CompiledDynamicValueAddress::new(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Tilt),
        },
        None,
    )
    .unwrap();
    assert!(
        valid
            .evaluate(&tilt, Some(&DynamicValue::Scalar(0.)))
            .is_err()
    );
}
