use super::*;
use crate::{
    CompiledProgrammingFamilyExpression, FamilyExpressionOperation, RetainedExpressionNode,
    RetainedNodeId, WholeFamilyExpressionFrameResolver,
};
use light_core::programming::{
    ProgrammingOwner, ScalarIntent, TransitionError, TransitionRequirement,
};
use std::collections::HashSet;

struct UnavailableFrame(Cell<usize>);
impl WholeFamilyExpressionFrameResolver for UnavailableFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.0.set(self.0.get() + 1);
        Err(TransitionError::Requires(requirement))
    }
}

struct History {
    runtime: DynamicRuntime,
    definition: DynamicDefinition,
    controller: DynamicController,
    target: FixtureId,
    instance: Uuid,
    lane: Uuid,
    sources: TypedSources,
    at: u64,
    // An independent affine calculation: constant + coefficient * today's Current.
    affine: [[f64; 2]; 2],
}

impl History {
    fn new() -> Self {
        let target = FixtureId(Uuid::from_u128(8001));
        let mut pan = ramp(
            pan_address(),
            value(DynamicValue::Scalar(90.0)),
            value(DynamicValue::Scalar(90.0)),
        );
        pan.id = Uuid::from_u128(8002);
        let lane = pan.id;
        let mut definition = definition(pan);
        definition.id = Uuid::from_u128(8003);
        let mut runtime = DynamicRuntime::default();
        runtime.install_definitions([definition.clone()]).unwrap();
        let controller = controller(1, 7, false);
        let mut request = start_request(definition.id, controller.clone(), target, 0, false);
        request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
        request.activation_duration_millis = 1000;
        let instance = runtime.start(request).unwrap();
        runtime
            .set_controller_lane_selection(
                instance,
                controller.id,
                DynamicLaneSelection::Uniform { lanes: vec![lane] },
            )
            .unwrap();
        let mut result = Self {
            runtime,
            definition,
            controller,
            target,
            instance,
            lane,
            sources: TypedSources {
                current: Some(DynamicValue::Scalar(10.0)),
                preset: None,
                calls: Cell::new(0),
            },
            at: 1200,
            affine: [[90.0, 0.0], [0.0, 1.0]],
        };
        result.check(result.at);
        result
    }

    fn check(&mut self, at: u64) {
        let samples = sampled(&mut self.runtime, self.instance, at, &self.sources);
        assert!(
            (2..=3).contains(&samples.len()),
            "only the selected axis and its two historical partners may participate"
        );
        let mut lanes = HashSet::new();
        for sample in &samples {
            assert_eq!(sample.instance_id, self.instance);
            assert_eq!(sample.controller_id, self.controller.id);
            assert_eq!(sample.target, self.target);
            assert_eq!(sample.priority, self.controller.priority);
            assert!(lanes.insert(sample.lane_id));
            assert!(
                self.allowed_lane(sample.lane_id),
                "an unrelated source must never supply Current"
            );
            assert_eq!(
                sample.activation_mix, 1.0,
                "the history fade is separate from controller activation"
            );
        }
        let bundled = bundle_position_sample_expressions(&samples, &self.sources).unwrap();
        assert!(bundled.remainder.is_empty());
        let position = bundled.position.unwrap();
        assert_eq!(position.lane_id, *lanes.iter().max().unwrap());
        let compiled = CompiledProgrammingFamilyExpression::new(
            Arc::new(position.expression),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap();
        let frame = UnavailableFrame(Cell::new(0));
        let Some(AttributeValue::Position(value)) =
            compiled.evaluate_optional(None, &frame).unwrap()
        else {
            panic!("complete Angle history must evaluate without an underlay");
        };
        let PositionIntent::Angles {
            pan_degrees: ScalarIntent::Value(pan),
            tilt_degrees: ScalarIntent::Value(tilt),
        } = value.as_ref()
        else {
            panic!("expected complete authored Angle output");
        };
        let Some(DynamicValue::Scalar(current)) = self.sources.current else {
            unreachable!()
        };
        for (axis, actual) in [*pan, *tilt].into_iter().enumerate() {
            let expected = self.affine[axis][0] + self.affine[axis][1] * f64::from(current);
            assert!(
                (f64::from(actual) - expected).abs() < 0.0002,
                "axis {axis} at {at}: expected affine {expected} with Current {current}, got {actual}"
            );
        }
        assert_eq!(
            frame.0.get(),
            0,
            "Angle histories need no geometry or fixture fitting"
        );
    }

    fn allowed_lane(&self, lane: Uuid) -> bool {
        lane == self.lane
            || [ProgrammingComponent::Pan, ProgrammingComponent::Tilt]
                .into_iter()
                .any(|axis| self.definition.is_automatic_angle_partner_id(lane, axis))
    }

    fn interrupt(&mut self, step: usize) {
        self.runtime
            .set_controller_paused(self.instance, self.controller.id, true, self.at)
            .unwrap();
        let pan = step % 2 == 0;
        let mut address = pan_address();
        address.component = Some(if pan {
            ProgrammingComponent::Pan
        } else {
            ProgrammingComponent::Tilt
        });
        let authored = if pan { 90.0 } else { 60.0 };
        let mut replacement = ramp(
            address,
            value(DynamicValue::Scalar(authored)),
            value(DynamicValue::Scalar(authored)),
        );
        replacement.id = self.lane;
        self.definition.lanes = vec![replacement];
        self.definition.revision += 1;
        self.runtime
            .install_definitions([self.definition.clone()])
            .unwrap();
        self.check(self.at + 1); // The hot edit must not change the held source membership.
        self.runtime
            .set_controller_paused_with_resume(
                self.instance,
                self.controller.id,
                false,
                self.at + 10,
                Some(ActivationPolicy::JoinSyncNow),
            )
            .unwrap();
        let incoming = if pan {
            [[90.0, 0.0], [0.0, 1.0]]
        } else {
            [[0.0, 1.0], [60.0, 0.0]]
        };
        for (axis, coefficients) in self.affine.iter_mut().enumerate() {
            for (term, coefficient) in coefficients.iter_mut().enumerate() {
                *coefficient = *coefficient * 0.75 + incoming[axis][term] * 0.25;
            }
        }
        self.at += 260;
        self.check(self.at); // 250 ms of the 1000 ms resume: the next pause interrupts here.
    }

    fn checkpoint(&self, steps: usize) -> DynamicRuntimeSnapshot {
        let snapshot = self.runtime.snapshot();
        assert_eq!(snapshot.instances.len(), 1);
        let instance = &snapshot.instances[0];
        assert_eq!(instance.id, self.instance);
        assert_eq!(instance.targets, vec![self.target]);
        assert_eq!(
            instance.lane_selections,
            vec![DynamicControllerLaneSelection {
                controller_id: self.controller.id,
                selection: DynamicLaneSelection::Uniform {
                    lanes: vec![self.lane]
                },
            }]
        );
        let resume = instance.synchronized_resume_transition.unwrap();
        assert_eq!(resume.duration_millis, 1000);
        assert_eq!(self.at - resume.started_at_millis, 250);
        let tape = instance
            .expression_tape
            .as_ref()
            .expect("one shared expression tape per instance");
        tape.validate().unwrap();
        let row_count = instance.last_sample_values.len() + instance.synchronized_hold_values.len();
        assert_eq!(tape.roots.len(), row_count);
        for rows in [
            &instance.last_sample_values,
            &instance.synchronized_hold_values,
        ] {
            let mut keys = HashSet::new();
            assert!((2..=3).contains(&rows.len()));
            for row in rows {
                assert_eq!(row.controller_id, self.controller.id);
                assert_eq!(row.target, self.target);
                assert!(self.allowed_lane(row.lane_id));
                assert!(keys.insert((row.controller_id, row.target, row.lane_id)));
                let crate::runtime::DynamicHeldPayload::TapeRoot { tape_root } = row.payload else {
                    panic!(
                        "new checkpoints must store keyed root IDs instead of nested expressions"
                    );
                };
                assert!(tape.roots.contains(&tape_root));
                assert!(tape.node(tape_root).is_some());
            }
        }
        assert!(
            tape.nodes.len() <= 16 * (steps + 1) + 16,
            "history node growth must be linear: {} nodes after {steps} interruptions",
            tape.nodes.len()
        );
        let json = serde_json::to_string(&snapshot).unwrap();
        assert_eq!(json.matches("\"expression_tape\"").count(), 1);
        assert!(
            !json.contains("\"expression\":"),
            "rows must not duplicate nested expression trees"
        );
        assert!(
            json.len() <= 8192 + 1400 * tape.nodes.len(),
            "checkpoint shape must remain bounded per node"
        );
        snapshot
    }
}

#[test]
fn repeated_runtime_axis_hot_edits_keep_affine_current_live_through_flat_snapshot_roundtrips() {
    let mut history = History::new();
    for step in 1..=128 {
        history.interrupt(step);
        let snapshot = history.checkpoint(step);
        if step % 7 == 0 || step == 128 {
            let bytes = serde_json::to_vec(&snapshot).unwrap();
            let restored = serde_json::from_slice(&bytes).unwrap();
            history.runtime.restore_snapshot(restored).unwrap();
            history.check(history.at);
            history.sources.current = Some(DynamicValue::Scalar(if step % 2 == 0 {
                -75.0
            } else {
                140.0
            }));
            history.check(history.at);
            // Sampling the same timestamp may replace frame values, but cannot append history.
            let refreshed = history.checkpoint(step);
            assert_eq!(
                refreshed.instances[0]
                    .expression_tape
                    .as_ref()
                    .unwrap()
                    .nodes
                    .len(),
                snapshot.instances[0]
                    .expression_tape
                    .as_ref()
                    .unwrap()
                    .nodes
                    .len()
            );
        }
    }
}

#[test]
fn malformed_history_roots_or_tapes_reject_atomically_without_losing_the_live_pair() {
    let mut history = History::new();
    for step in 1..=9 {
        history.interrupt(step);
    }
    let valid = history.checkpoint(9);
    for corruption in 0..3 {
        let mut forged = valid.clone();
        match corruption {
            0 => {
                forged.instances[0].last_sample_values[0].payload =
                    crate::runtime::DynamicHeldPayload::TapeRoot {
                        tape_root: RetainedNodeId(u32::MAX),
                    }
            }
            1 => forged.instances[0].expression_tape = None,
            2 => {
                let tape = Arc::make_mut(forged.instances[0].expression_tape.as_mut().unwrap());
                tape.nodes[0] = RetainedExpressionNode::Transition {
                    from: Some(RetainedNodeId(0)),
                    to: None,
                    progress: 0.25,
                    reason: DynamicTransitionReason::Resume {
                        occurrence_id: Uuid::from_u128(9900),
                    },
                };
            }
            _ => unreachable!(),
        }
        assert!(
            history.runtime.restore_snapshot(forged).is_err(),
            "malformed checkpoint {corruption} must fail"
        );
        assert_eq!(
            history.runtime.snapshot(),
            valid,
            "failed restore must preserve all prior runtime state"
        );
    }
    history.sources.current = Some(DynamicValue::Scalar(-120.0));
    history.check(history.at);
}

#[test]
fn whole_target_runtime_history_keeps_point_references_live_after_many_interrupted_restores() {
    use std::cell::RefCell;

    // A deterministic linear stand-in for the caller's coherent geometry frame. This test
    // proves retained dependency liveness; physical inverse kinematics has separate coverage.
    struct PointFrame {
        points: RefCell<HashMap<Uuid, [f32; 3]>>,
        calls: Cell<usize>,
    }
    impl PointFrame {
        fn joints(&self, value: &AttributeValue) -> [f32; 2] {
            let AttributeValue::Position(position) = value else {
                panic!("Position endpoint");
            };
            match position.as_ref() {
                PositionIntent::Angles {
                    pan_degrees: ScalarIntent::Value(pan),
                    tilt_degrees: ScalarIntent::Value(tilt),
                } => [*pan, *tilt],
                PositionIntent::Target {
                    reference: TargetReference::Point { point_id },
                    offset_metres,
                } => {
                    let point = self.points.borrow()[point_id];
                    let world: [f32; 3] = std::array::from_fn(|axis| {
                        let ScalarIntent::Value(offset) = offset_metres[axis] else {
                            panic!("materialized offset");
                        };
                        point[axis] + offset
                    });
                    [world[0] + 2.0 * world[2], world[1] - world[2]]
                }
                _ => panic!("complete materialized Point or Angle endpoint"),
            }
        }
    }
    impl WholeFamilyExpressionFrameResolver for PointFrame {
        fn resolve(
            &self,
            requirement: TransitionRequirement,
            from: &AttributeValue,
            to: &AttributeValue,
            operation: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            assert!(matches!(
                requirement,
                TransitionRequirement::LiveTargetPoints | TransitionRequirement::LiveJointAngles
            ));
            self.calls.set(self.calls.get() + 1);
            let FamilyExpressionOperation::Transition { progress } = operation else {
                panic!("resume transition");
            };
            let from = self.joints(from);
            let to = self.joints(to);
            let p = f64::from(progress);
            let joints: [f32; 2] = std::array::from_fn(|axis| {
                (f64::from(from[axis]) * (1.0 - p) + f64::from(to[axis]) * p) as f32
            });
            Ok(AttributeValue::Position(Arc::new(PositionIntent::angles(
                joints[0], joints[1],
            ))))
        }
    }

    const STEPS: usize = 96;
    let fixture = FixtureId(Uuid::from_u128(8100));
    let points = [Uuid::from_u128(8101), Uuid::from_u128(8102)];
    let offsets = [[1.0, 2.0, 3.0], [-4.0, 5.0, -6.0]];
    let targets: [AttributeValue; 2] = std::array::from_fn(|index| {
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Point {
                point_id: points[index],
            },
            offsets[index],
        )))
    });
    let authored_lane = Uuid::from_u128(8103);
    let make_lane = |index: usize| {
        let source = value(DynamicValue::Family(targets[index].clone()));
        DynamicLane {
            id: authored_lane,
            body: DynamicLaneBody::Programming(ProgrammingLaneBody {
                address: DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Target { reference: None },
                    component: None,
                },
                configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                    points: vec![
                        DynamicKeyframe {
                            position: 0.0,
                            source: source.clone(),
                            interpolation: ScalarInterpolation::Linear,
                        },
                        DynamicKeyframe {
                            position: 0.5,
                            source,
                            interpolation: ScalarInterpolation::Linear,
                        },
                    ],
                    size: 1.0,
                }),
            }),
            ..lane()
        }
    };
    let mut definition = definition(make_lane(0));
    definition.id = Uuid::from_u128(8104);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let controller = controller(2, 8, false);
    let mut request = start_request(definition.id, controller.clone(), fixture, 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    runtime
        .set_controller_lane_selection(
            instance,
            controller.id,
            DynamicLaneSelection::Uniform {
                lanes: vec![authored_lane],
            },
        )
        .unwrap();
    let sources = TypedSources {
        current: None,
        preset: None,
        calls: Cell::new(0),
    };
    let mut at = 1200;
    assert_eq!(sampled(&mut runtime, instance, at, &sources).len(), 1);
    let mut weights = [1.0_f64, 0.0];
    for step in 1..=STEPS {
        runtime
            .set_controller_paused(instance, controller.id, true, at)
            .unwrap();
        definition.lanes = vec![make_lane(step % 2)];
        definition.revision += 1;
        runtime.install_definitions([definition.clone()]).unwrap();
        runtime
            .set_controller_paused_with_resume(
                instance,
                controller.id,
                false,
                at + 10,
                Some(ActivationPolicy::JoinSyncNow),
            )
            .unwrap();
        at += 260;
        let samples = sampled(&mut runtime, instance, at, &sources);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].controller_id, controller.id);
        assert_eq!(samples[0].target, fixture);
        assert_eq!(samples[0].lane_id, authored_lane);
        for (index, weight) in weights.iter_mut().enumerate() {
            *weight = *weight * 0.75 + if index == step % 2 { 0.25 } else { 0.0 };
        }
        if step % 8 == 0 {
            let snapshot = runtime.snapshot();
            let tape = snapshot.instances[0].expression_tape.as_ref().unwrap();
            assert!(
                tape.nodes.len() <= 6 * (step + 1),
                "whole Target history must remain linear"
            );
            let bytes = serde_json::to_vec(&snapshot).unwrap();
            let mut restored = DynamicRuntime::default();
            restored
                .restore_snapshot(serde_json::from_slice(&bytes).unwrap())
                .unwrap();
            runtime = restored;
        }
    }
    let snapshot = runtime.snapshot();
    let stored = &snapshot.instances[0];
    assert_eq!(stored.targets, vec![fixture]);
    assert_eq!(
        stored.lane_selections,
        vec![DynamicControllerLaneSelection {
            controller_id: controller.id,
            selection: DynamicLaneSelection::Uniform {
                lanes: vec![authored_lane]
            },
        }]
    );
    let mut observed_points = HashSet::new();
    for node in &stored.expression_tape.as_ref().unwrap().nodes {
        if let RetainedExpressionNode::Programming { address, value, .. } = node {
            assert!(address.component.is_none());
            let DynamicValue::Family(AttributeValue::Position(position)) = value else {
                panic!("stored Position intent");
            };
            let PositionIntent::Target {
                reference: TargetReference::Point { point_id },
                offset_metres,
            } = position.as_ref()
            else {
                panic!("snapshot must retain Point intent, never resolved angles");
            };
            let index = points.iter().position(|point| point == point_id).unwrap();
            assert_eq!(*offset_metres, offsets[index].map(ScalarIntent::Value));
            observed_points.insert(*point_id);
        }
    }
    assert_eq!(observed_points, HashSet::from(points));
    let samples = sampled(&mut runtime, instance, at, &sources);
    let bundled = bundle_position_sample_expressions(&samples, &sources).unwrap();
    assert!(bundled.remainder.is_empty());
    let compiled = CompiledProgrammingFamilyExpression::new(
        Arc::new(bundled.position.unwrap().expression),
        ProgrammingOwner::Position,
        None,
        None,
    )
    .unwrap();
    let unavailable = UnavailableFrame(Cell::new(0));
    assert!(matches!(
        compiled.evaluate_optional(None, &unavailable),
        Err(TransitionError::Requires(
            TransitionRequirement::LiveTargetPoints
        ))
    ));
    assert!(
        unavailable.0.get() > 0,
        "the restored history still requires its live Points"
    );
    let frame = PointFrame {
        points: RefCell::new(HashMap::from([
            (points[0], [10.0, 20.0, 30.0]),
            (points[1], [-10.0, 40.0, 50.0]),
        ])),
        calls: Cell::new(0),
    };
    let mut resolved = Vec::new();
    for moved in [false, true] {
        if moved {
            frame
                .points
                .borrow_mut()
                .insert(points[1], [80.0, -30.0, 15.0]);
        }
        let result = compiled.evaluate_optional(None, &frame).unwrap().unwrap();
        let actual = frame.joints(&result);
        let endpoints = targets.each_ref().map(|target| frame.joints(target));
        for axis in 0..2 {
            let expected = f64::from(endpoints[0][axis]) * weights[0]
                + f64::from(endpoints[1][axis]) * weights[1];
            assert!(
                (f64::from(actual[axis]) - expected).abs() < 0.0002,
                "restored axis {axis} must use today's Points: expected {expected}, got {}",
                actual[axis]
            );
        }
        resolved.push(actual);
    }
    assert_ne!(
        resolved[0], resolved[1],
        "moving a referenced Point must change the same compiled history"
    );
    assert!(
        frame.calls.get() >= STEPS * 2,
        "each evaluation must resolve the live retained dependencies again"
    );
}
