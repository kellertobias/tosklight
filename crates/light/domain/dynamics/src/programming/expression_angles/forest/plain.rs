//! The Position forest of a controller whose lanes are all plain Angle leaves (TL-639 round 5,
//! compiled plans, design A).
//!
//! The general walk imports the lanes into one retained tape, derives each node's Position
//! metadata and walks the correlated root sets. For plain leaves — authored Angle or Target
//! components, numeric Angle programs without operation origins, Angle Current — that walk has
//! exactly one root set, no Resume transition and every root contributes Position, so it visits
//! the set once and answers one branch: each lane's leaf, in lane order, into [`BranchLeaves`].
//! This module builds that branch straight from the samples, with the same leaf calls, Current
//! reads and copies the walk makes (a numeric program is copied as the tape imports it).
//!
//! The leaves were validated by the controller's preparation (`DynamicSampleExpression::validate`
//! runs every node's own checks, and a program without operation origins imports with an empty
//! operation table, so the joint tape adds no check). Programs with operation origins still
//! import into the joint tape first, which checks their shared object table exactly as the walk
//! does, and the branch takes their programs from it. [`verify`] proves the shortcut: with plan
//! verification on, it rebuilds the forest through the general walk and compares them.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// Off unless asked for: the general walk reads Current again, which read-counting tests and
/// the frame budget both notice.
static VERIFY: AtomicBool = AtomicBool::new(false);

/// Build every planned Position forest through the general walk as well and assert they are
/// equal (the plan tests, and `LIGHT_VERIFY_PLANS=1` desk or benchmark runs).
pub fn set_plan_verification(enabled: bool) {
    VERIFY.store(enabled, Ordering::Relaxed);
}

#[cfg(test)]
thread_local! {
    /// Verification for one test thread only: other tests count their Current reads.
    static VERIFY_HERE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn verifying() -> bool {
    #[cfg(test)]
    if VERIFY_HERE.with(std::cell::Cell::get) {
        return true;
    }
    VERIFY.load(Ordering::Relaxed)
}

/// Whether a lane is a plain Angle leaf the direct build answers.
fn plain(expression: &Expression) -> bool {
    match expression {
        Expression::Programming { address, .. } => {
            address.owner() == ProgrammingOwner::Position
                && (address.component.is_some()
                    || matches!(
                        address.representation,
                        DynamicFamilyRepresentation::Target { reference: Some(_) }
                    ))
        }
        Expression::AngleNumeric { .. } => true,
        Expression::AngleCurrent { .. } => true,
        _ => false,
    }
}

/// The forest and its root, built directly; `None` when a lane is not a plain Angle leaf.
#[allow(clippy::type_complexity)]
pub(super) fn forest(
    samples: &[DynamicRuntimeSample],
    target: FixtureId,
    sources: &dyn DynamicValueSourceResolver,
) -> Option<Result<(Vec<PositionForestNode>, Option<usize>), TransitionError>> {
    if !samples.iter().all(|sample| plain(&sample.expression)) {
        return None;
    }
    let with_origins = samples.iter().any(|sample| {
        matches!(&sample.expression, Expression::AngleNumeric { program }
            if !program.operations.is_empty())
    });
    let forest = || {
        let tape = if with_origins {
            let expressions = samples
                .iter()
                .map(|sample| Arc::new(sample.expression.clone()))
                .collect::<Vec<_>>();
            Some(RetainedExpressionTape::from_roots(&expressions)?)
        } else {
            None
        };
        branch_of(samples, tape.as_ref(), target, sources).map(|node| match node {
            Some(node) => (vec![node], Some(0)),
            None => (Vec::new(), None),
        })
    };
    Some(forest())
}

/// The one branch, from the samples (or, with operation origins, the programs of `tape`, the
/// lanes' joint import).
fn branch_of(
    samples: &[DynamicRuntimeSample],
    tape: Option<&RetainedExpressionTape>,
    target: FixtureId,
    sources: &dyn DynamicValueSourceResolver,
) -> Result<Option<PositionForestNode>, TransitionError> {
    let mut current = [None, None, None];
    let mut captured_current = None;
    let mut reads = CurrentReads {
        target,
        sources,
        current: &mut current,
        captured_current: &mut captured_current,
    };
    let mut leaves = BranchLeaves::new();
    for (index, sample) in samples.iter().enumerate() {
        let lane_id = sample.lane_id;
        match &sample.expression {
            Expression::Programming {
                address,
                value,
                occurrence,
                dependency_occurrence,
            } => leaves.authored(lane_id, address, value, occurrence, dependency_occurrence)?,
            // The tape holds its own copy of an imported program, without operation origins.
            Expression::AngleNumeric { program } => match tape {
                Some(tape) => {
                    let Some(Node::AngleNumeric { program }) = tape.node(tape.roots[index]) else {
                        unreachable!("a numeric leaf imports as a numeric node")
                    };
                    leaves.angle_numeric(lane_id, program, &mut reads)?
                }
                None => leaves.angle_numeric(
                    lane_id,
                    &Arc::new(program.as_ref().clone()),
                    &mut reads,
                )?,
            },
            Expression::AngleCurrent { address } => {
                leaves.angle_current(lane_id, address, &mut reads)?
            }
            _ => unreachable!("plain Angle leaves only"),
        }
    }
    leaves.finish()
}

/// Rebuild the forest through the general walk and assert it equals the direct build.
pub(super) fn verify(
    samples: &[DynamicRuntimeSample],
    sources: &dyn DynamicValueSourceResolver,
    forest: &[PositionForestNode],
    root: Option<usize>,
) {
    let target = samples[0].target;
    let (general, general_root, ..) =
        general_forest(samples, target, sources).expect("the general Position walk succeeds");
    assert_eq!(root, general_root, "planned Position forest root");
    assert_eq!(forest.len(), general.len(), "planned Position forest size");
    for (planned, general) in forest.iter().zip(&general) {
        assert!(same_node(planned, general), "planned Position forest node");
    }
}

fn same_node(left: &PositionForestNode, right: &PositionForestNode) -> bool {
    match (left, right) {
        (PositionForestNode::AnglePair(left), PositionForestNode::AnglePair(right)) => left
            .axes
            .iter()
            .zip(&right.axes)
            .all(|(left, right)| same_axis(left, right)),
        (
            PositionForestNode::Whole {
                expression,
                lane_id,
                sources,
            },
            PositionForestNode::Whole {
                expression: other_expression,
                lane_id: other_lane,
                sources: other_sources,
            },
        ) => {
            expression == other_expression
                && lane_id == other_lane
                && sources.len() == other_sources.len()
                && sources
                    .iter()
                    .zip(other_sources.iter())
                    .all(|(left, right)| same_endpoint(left, right))
        }
        (PositionForestNode::Cohort(left), PositionForestNode::Cohort(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| same_endpoint(left, right))
        }
        _ => false,
    }
}

fn same_endpoint(left: &CoupledComponentEndpoint, right: &CoupledComponentEndpoint) -> bool {
    left.lane_id == right.lane_id
        && left.address.address() == right.address.address()
        && left.value == right.value
        && left.role == right.role
        && left.occurrence == right.occurrence
        && left.dependency_occurrence == right.dependency_occurrence
}

fn same_current(left: &CapturedPositionCurrent, right: &CapturedPositionCurrent) -> bool {
    left.value == right.value && left.occurrence == right.occurrence
}

fn same_axis(left: &PositionAngleAxis, right: &PositionAngleAxis) -> bool {
    match (left, right) {
        (PositionAngleAxis::Materialized(left), PositionAngleAxis::Materialized(right)) => {
            same_endpoint(left, right)
        }
        (
            PositionAngleAxis::Numeric {
                lane_id,
                address,
                program,
                original,
            },
            PositionAngleAxis::Numeric {
                lane_id: other_lane,
                address: other_address,
                program: other_program,
                original: other_original,
            },
        ) => {
            lane_id == other_lane
                && address.address() == other_address.address()
                && program == other_program
                && program.operations.is_empty() == other_program.operations.is_empty()
                && same_current(original, other_original)
        }
        (
            PositionAngleAxis::Current {
                lane_id,
                address,
                original,
            },
            PositionAngleAxis::Current {
                lane_id: other_lane,
                address: other_address,
                original: other_original,
            },
        ) => {
            lane_id == other_lane
                && address.address() == other_address.address()
                && same_current(original, other_original)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AngleNumericNode, AngleNumericProgram, DynamicPresetSourceBinding};

    /// Captured Position Current of one fixture, counting reads.
    struct Current {
        pan: f32,
        tilt: f32,
    }

    impl DynamicValueSourceResolver for Current {
        fn try_position_current_family(
            &self,
            _: FixtureId,
            _: &DynamicValueAddress,
        ) -> Result<Option<AttributeValue>, TransitionError> {
            Ok(Some(AttributeValue::Position(Arc::new(
                PositionIntent::angles(self.pan, self.tilt),
            ))))
        }

        fn current(&self, _: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
            match address.component {
                Some(ProgrammingComponent::Pan) => Some(DynamicValue::Scalar(self.pan)),
                Some(ProgrammingComponent::Tilt) => Some(DynamicValue::Scalar(self.tilt)),
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

    fn address(component: ProgrammingComponent) -> Arc<DynamicValueAddress> {
        Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(component),
        })
    }

    fn sample(lane: u128, expression: Expression) -> DynamicRuntimeSample {
        DynamicRuntimeSample {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            target: FixtureId(Uuid::from_u128(3)),
            lane_id: Uuid::from_u128(lane),
            expression,
            priority: 3,
            activated_at_millis: 100,
            activation_mix: 0.4,
            address: None,
        }
    }

    fn authored(component: ProgrammingComponent, value: f32) -> Expression {
        Expression::Programming {
            address: address(component),
            value: DynamicValue::Scalar(value),
            occurrence: None,
            dependency_occurrence: None,
        }
    }

    fn numeric(component: ProgrammingComponent, offset: f32) -> Expression {
        Expression::AngleNumeric {
            program: Arc::new(AngleNumericProgram {
                address: address(component).as_ref().clone(),
                occurrence: None,
                nodes: vec![
                    AngleNumericNode::Current,
                    AngleNumericNode::Materialized {
                        value: DynamicValue::Scalar(offset),
                        dependency_occurrence: None,
                    },
                    AngleNumericNode::Transition {
                        from: 0,
                        to: 1,
                        progress: 0.25,
                    },
                ],
                root: 2,
                operations: Default::default(),
            }),
        }
    }

    /// Bundle with verification on this thread: the planned forest must equal the general walk's.
    fn bundle_verified(samples: &[DynamicRuntimeSample]) -> PositionComponentForestBundle {
        VERIFY_HERE.with(|verify| verify.set(true));
        let bundled = bundle_position_component_forest(
            samples,
            &Current {
                pan: 12.0,
                tilt: -7.5,
            },
        );
        VERIFY_HERE.with(|verify| verify.set(false));
        bundled.unwrap()
    }

    #[test]
    fn planned_forests_equal_the_general_walk_for_every_plain_shape() {
        use ProgrammingComponent::{Pan, Tilt};
        let shapes: Vec<Vec<DynamicRuntimeSample>> = vec![
            vec![
                sample(10, authored(Pan, 30.0)),
                sample(11, authored(Tilt, 40.0)),
            ],
            vec![
                sample(11, authored(Tilt, -40.0)),
                sample(10, authored(Pan, 3.0)),
            ],
            vec![
                sample(10, numeric(Pan, 5.0)),
                sample(11, numeric(Tilt, -5.0)),
            ],
            vec![
                sample(10, numeric(Pan, 5.0)),
                sample(
                    11,
                    Expression::AngleCurrent {
                        address: address(Tilt),
                    },
                ),
            ],
            vec![
                sample(10, authored(Pan, 30.0)),
                sample(
                    11,
                    Expression::AngleCurrent {
                        address: address(Tilt),
                    },
                ),
            ],
            // One axis alone forms no pair: no Position, and no remainder.
            vec![sample(10, authored(Pan, 30.0))],
        ];
        for samples in shapes {
            assert!(
                forest(
                    &samples,
                    samples[0].target,
                    &Current {
                        pan: 0.0,
                        tilt: 0.0
                    }
                )
                .is_some()
            );
            let bundled = bundle_verified(&samples);
            assert!(bundled.remainder.is_empty());
            assert_eq!(bundled.position.is_some(), samples.len() == 2);
        }
    }

    #[test]
    fn a_transition_or_a_whole_leaf_takes_the_general_walk() {
        let resume = Expression::Transition {
            from: Some(Arc::new(authored(ProgrammingComponent::Pan, 1.0))),
            to: None,
            progress: 0.5,
            reason: crate::DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(9),
            },
        };
        let samples = [sample(10, resume)];
        assert!(
            forest(
                &samples,
                samples[0].target,
                &Current {
                    pan: 0.0,
                    tilt: 0.0
                }
            )
            .is_none()
        );
        let whole = Expression::Programming {
            address: Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: None,
            }),
            value: DynamicValue::Family(AttributeValue::Position(Arc::new(
                PositionIntent::angles(1.0, 2.0),
            ))),
            occurrence: None,
            dependency_occurrence: None,
        };
        let samples = [sample(10, whole)];
        assert!(
            forest(
                &samples,
                samples[0].target,
                &Current {
                    pan: 0.0,
                    tilt: 0.0
                }
            )
            .is_none()
        );
    }
}
