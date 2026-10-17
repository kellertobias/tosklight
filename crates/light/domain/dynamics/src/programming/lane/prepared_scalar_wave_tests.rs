use super::*;
use light_core::programming::{ColorComponent as C, ProgrammingComponent as P};
use std::cell::Cell;

struct Sources(Cell<usize>);
impl DynamicValueSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        self.0.set(self.0.get() + 1);
        Some(DynamicValue::Scalar(0.2))
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: uuid::Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        self.0.set(self.0.get() + 1);
        Some(DynamicValue::Scalar(0.3))
    }
}
fn lane(component: C) -> DynamicLane {
    DynamicLane {
        id: uuid::Uuid::from_u128(1),
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::SemanticColor {
                    basis: if component == C::Uv {
                        DynamicSemanticColorBasis::Retain
                    } else {
                        DynamicSemanticColorBasis::Recipe
                    },
                },
                component: Some(P::Color(component)),
            },
            configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
                minimum: DynamicValueSource::Value {
                    value: DynamicValue::Scalar(0.1),
                },
                maximum: DynamicValueSource::Value {
                    value: DynamicValue::Scalar(0.9),
                },
                function: PeriodicFunction::Cosinus,
                size: 1.,
                pwm: PwmShape::default(),
            }),
        }),
        speed_multiplier: Rational::ONE,
        width: 1.,
        phase: None,
        random_group_id: None,
    }
}
fn context(source: &Sources, elapsed: u64) -> ProgrammingEvaluationContext<'_> {
    ProgrammingEvaluationContext {
        instance_id: uuid::Uuid::from_u128(2),
        controller_id: uuid::Uuid::from_u128(3),
        authored_occurrence: Some(
            DynamicSourceOccurrenceId::new(uuid::Uuid::from_u128(elapsed as u128 + 100)).unwrap(),
        ),
        target: FixtureId(uuid::Uuid::from_u128(4)),
        elapsed_millis: elapsed,
        cycle_duration_millis: 2400,
        phase_degrees: -71.2,
        random_envelope: None,
        sources: source,
    }
}
#[test]
fn sealed_scalar_wave_admission_matches_generic_lane_values_and_source_calls() {
    for component in [C::Red, C::Green, C::Blue, C::Amber, C::WhiteBlend, C::Uv] {
        let compiled = CompiledProgrammingLane::new(&lane(component), &[], None).unwrap();
        assert!(compiled.prepared_scalar_wave.is_some());
        let mut reference = compiled.clone();
        reference.prepared_scalar_wave = None;
        for at in [0, 1, 411, u64::MAX] {
            let old = Sources(Cell::new(0));
            let new = Sources(Cell::new(0));
            let expected = reference
                .sample_with_cache(context(&old, at), None, &mut None)
                .unwrap();
            crate::DynamicRuntime::take_scalar_wave_samples();
            let actual = compiled
                .sample_with_cache(context(&new, at), None, &mut None)
                .unwrap();
            assert_eq!(crate::DynamicRuntime::take_scalar_wave_samples(), 1);
            assert_eq!(actual, expected);
            assert_eq!(old.0.get(), new.0.get());
            assert_eq!(new.0.get(), 0);
        }
    }
}
#[test]
fn cold_invalid_errors_and_nonliteral_fallback_are_unchanged() {
    let error = |lane: &DynamicLane| {
        CompiledProgrammingLane::new(lane, &[], None)
            .err()
            .unwrap()
            .to_string()
    };
    let mut invalid = lane(C::Red);
    let DynamicLaneBody::Programming(body) = &mut invalid.body else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::MaxMin(config) = &mut body.configuration else {
        unreachable!()
    };
    config.size = f32::NAN;
    config.minimum = DynamicValueSource::Value {
        value: DynamicValue::Scalar(-1.),
    };
    invalid.width = f32::NAN;
    assert!(error(&invalid).contains("numeric configuration requires a component and finite size"));
    let DynamicLaneBody::Programming(body) = &mut invalid.body else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::MaxMin(config) = &mut body.configuration else {
        unreachable!()
    };
    config.size = 1.;
    assert!(error(&invalid).contains("Dynamic scalar is outside its declared component domain"));
    let mut invalid = lane(C::Red);
    invalid.width = f32::NAN;
    assert!(error(&invalid).contains("Dynamic timing is invalid"));
    for source in [
        DynamicValueSource::Current,
        DynamicValueSource::Preset {
            preset_id: "test".into(),
            address: match lane(C::Red).body {
                DynamicLaneBody::Programming(body) => body.address,
                _ => unreachable!(),
            },
            last_valid_by_target: vec![],
            retained: None,
        },
    ] {
        let mut lane = lane(C::Red);
        let DynamicLaneBody::Programming(body) = &mut lane.body else {
            unreachable!()
        };
        let ProgrammingLaneConfiguration::MaxMin(config) = &mut body.configuration else {
            unreachable!()
        };
        config.minimum = source;
        assert!(
            CompiledProgrammingLane::new(&lane, &[], None)
                .unwrap()
                .prepared_scalar_wave
                .is_none()
        );
    }
}
