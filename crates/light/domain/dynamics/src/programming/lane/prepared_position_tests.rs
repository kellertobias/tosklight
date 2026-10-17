use super::*;
use light_core::{AttributeValue, programming::PositionIntent};
use std::cell::RefCell;

struct Sources {
    calls: RefCell<Vec<&'static str>>,
    available: bool,
}
impl DynamicValueSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        self.calls.borrow_mut().push("current");
        self.available.then_some(DynamicValue::Scalar(0.3))
    }
    fn try_position_current_family(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, light_core::programming::TransitionError> {
        self.calls.borrow_mut().push("position");
        Ok(self
            .available
            .then(|| AttributeValue::Position(Arc::new(PositionIntent::angles(10., 20.)))))
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: uuid::Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        self.calls.borrow_mut().push("preset");
        None
    }
}
fn lane(angle: bool) -> CompiledProgrammingLane {
    let address = DynamicValueAddress {
        representation: if angle {
            DynamicFamilyRepresentation::Angles
        } else {
            DynamicFamilyRepresentation::Focus
        },
        component: Some(if angle {
            light_core::programming::ProgrammingComponent::Pan
        } else {
            light_core::programming::ProgrammingComponent::Focus
        }),
    };
    CompiledProgrammingLane::new(
        &DynamicLane {
            id: uuid::Uuid::from_u128(1),
            body: DynamicLaneBody::Programming(ProgrammingLaneBody {
                address,
                configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
                    minimum: DynamicValueSource::Current,
                    maximum: DynamicValueSource::Value {
                        value: DynamicValue::Scalar(0.9),
                    },
                    function: PeriodicFunction::Sinus,
                    size: 1.0,
                    pwm: PwmShape::default(),
                }),
            }),
            speed_multiplier: Rational {
                numerator: 7,
                denominator: 3,
            },
            width: 0.3,
            phase: None,
            random_group_id: None,
        },
        &[],
        None,
    )
    .unwrap()
}
fn context(sources: &Sources) -> ProgrammingEvaluationContext<'_> {
    ProgrammingEvaluationContext {
        instance_id: uuid::Uuid::from_u128(2),
        controller_id: uuid::Uuid::from_u128(3),
        authored_occurrence: None,
        target: FixtureId(uuid::Uuid::from_u128(4)),
        elapsed_millis: 411,
        cycle_duration_millis: 2400,
        phase_degrees: -93.5,
        random_envelope: None,
        sources,
    }
}
fn numeric(
    value: Result<AngleNumericSample, light_core::programming::TransitionError>,
) -> Result<serde_json::Value, String> {
    value
        .map(|value| match value {
            AngleNumericSample::Program(program) => serde_json::to_value(program).unwrap(),
            AngleNumericSample::Absent => serde_json::json!("absent"),
            AngleNumericSample::NotApplicable => serde_json::json!("not_applicable"),
        })
        .map_err(|error| error.to_string())
}
#[test]
fn prepared_entry_points_preserve_current_calls_and_size_error_precedence() {
    for angle in [false, true] {
        for available in [false, true] {
            let lane = lane(angle);
            let old = Sources {
                calls: RefCell::default(),
                available,
            };
            let new = Sources {
                calls: RefCell::default(),
                available,
            };
            let position = lane.prepare_lane_position(411, 2400).at(-93.5);
            for size in [f32::NAN, -1.0, 0.4, 1.0] {
                old.calls.borrow_mut().clear();
                new.calls.borrow_mut().clear();
                assert_eq!(
                    numeric(lane.pin_angle_numeric_with_operations(&context(&old), size, None)),
                    numeric(lane.pin_angle_numeric_at_position(
                        &context(&new),
                        size,
                        None,
                        Some(position)
                    ))
                );
                assert_eq!(*old.calls.borrow(), *new.calls.borrow());
            }
            old.calls.borrow_mut().clear();
            new.calls.borrow_mut().clear();
            let (mut old_cache, mut new_cache) = (None, None);
            assert_eq!(
                lane.sample_with_cache(context(&old), None, &mut old_cache)
                    .map_err(|e| e.to_string()),
                lane.sample_with_cache_at_position(
                    context(&new),
                    None,
                    &mut new_cache,
                    Some(position)
                )
                .map_err(|e| e.to_string())
            );
            assert_eq!(old_cache.is_some(), new_cache.is_some());
            assert_eq!(*old.calls.borrow(), *new.calls.borrow());
        }
    }
}
