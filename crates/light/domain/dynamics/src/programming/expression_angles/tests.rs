use super::*;
use crate::DynamicPresetSourceBinding;
use crate::{
    CompiledDynamicValueAddress, CompiledProgrammingFamilyExpression, FamilyCompositionContext,
    FamilyCompositionScratch, FamilyExpressionOperation, FamilySample, FamilySampleRank,
    WholeFamilyExpressionFrameResolver, compose_dynamic_family,
};
use std::cell::Cell;
use uuid::Uuid;

struct Sources {
    pan: Option<f32>,
    tilt: Option<f32>,
    reads: Cell<usize>,
}
impl DynamicValueSourceResolver for Sources {
    fn current(&self, _: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.reads.set(self.reads.get() + 1);
        match address.component {
            Some(ProgrammingComponent::Pan) => self.pan,
            Some(ProgrammingComponent::Tilt) => self.tilt,
            _ => None,
        }
        .map(DynamicValue::Scalar)
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
fn angle(component: ProgrammingComponent, value: f32) -> Expression {
    Expression::Programming {
        address: address(component),
        value: DynamicValue::Scalar(value),
        occurrence: None,
        dependency_occurrence: None,
    }
}
fn current(component: ProgrammingComponent) -> Expression {
    Expression::AngleCurrent {
        address: address(component),
    }
}
fn resume(from: Option<Expression>, to: Option<Expression>, progress: f32, id: u128) -> Expression {
    Expression::Transition {
        from: from.map(Arc::new),
        to: to.map(Arc::new),
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(id),
        },
    }
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
fn whole(pan: f32, tilt: f32) -> Expression {
    Expression::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: None,
        }),
        value: DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::angles(
            pan, tilt,
        )))),
        occurrence: None,
        dependency_occurrence: None,
    }
}

fn assert_expression_eq(actual: &Expression, expected: Expression) {
    let actual = RetainedExpressionTape::from_roots(&[Arc::new(actual.clone())]).unwrap();
    let expected = RetainedExpressionTape::from_roots(&[Arc::new(expected)]).unwrap();
    let mut stack = vec![(actual.roots[0], expected.roots[0])];
    let mut compared = HashSet::default();
    while let Some((a, b)) = stack.pop() {
        if !compared.insert((a, b)) {
            continue;
        }
        match (&actual.nodes[a.0 as usize], &expected.nodes[b.0 as usize]) {
            (
                Node::Programming {
                    address: a,
                    value: av,
                    ..
                },
                Node::Programming {
                    address: b,
                    value: bv,
                    ..
                },
            ) => {
                assert_eq!(a, b);
                assert_eq!(av, bv);
            }
            (
                Node::Transition {
                    from: af,
                    to: at,
                    progress: ap,
                    reason: ar,
                },
                Node::Transition {
                    from: bf,
                    to: bt,
                    progress: bp,
                    reason: br,
                },
            ) => {
                assert_eq!(ap, bp);
                assert_eq!(ar, br);
                for (a, b) in [(*af, *bf), (*at, *bt)] {
                    assert_eq!(a.is_some(), b.is_some());
                    if let (Some(a), Some(b)) = (a, b) {
                        stack.push((a, b));
                    }
                }
            }
            (
                Node::Scale {
                    address: a,
                    base: ab,
                    value: av,
                    factor: af,
                    ..
                },
                Node::Scale {
                    address: b,
                    base: bb,
                    value: bv,
                    factor: bf,
                    ..
                },
            ) => {
                assert_eq!(a, b);
                assert_eq!(ab, bb);
                assert_eq!(af, bf);
                stack.push((*av, *bv));
            }
            (a, b) => assert_eq!(a, b),
        }
    }
}
struct NoGeometry;
impl WholeFamilyExpressionFrameResolver for NoGeometry {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        Err(TransitionError::Requires(requirement))
    }
}

#[test]
fn resolved_resume_competes_as_one_pair_and_applies_activation_once() {
    let samples = vec![
        sample(
            10,
            resume(
                Some(angle(ProgrammingComponent::Pan, 90.0)),
                Some(angle(ProgrammingComponent::Tilt, 60.0)),
                0.5,
                100,
            ),
        ),
        sample(
            11,
            resume(Some(current(ProgrammingComponent::Tilt)), None, 0.5, 100),
        ),
        sample(
            12,
            resume(None, Some(current(ProgrammingComponent::Pan)), 0.5, 100),
        ),
    ];
    let frame = sources();
    let source = bundle_position_sample_expressions(&samples, &frame)
        .unwrap()
        .position
        .unwrap();
    let base = AttributeValue::Position(Arc::new(PositionIntent::angles(25.0, -30.0)));
    let compiled = CompiledProgrammingFamilyExpression::new(
        Arc::new(source.expression),
        ProgrammingOwner::Position,
        Some(&base),
        None,
    )
    .unwrap();
    let resolved = compiled.evaluate(&base, &NoGeometry).unwrap();
    assert_eq!(
        resolved,
        AttributeValue::Position(Arc::new(PositionIntent::angles(57.5, 15.0)))
    );
    let sample = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Position, &resolved).unwrap(),
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Family(resolved),
        FamilySampleRank {
            priority: source.priority,
            changed_at_millis: source.activated_at_millis,
            changed_at_submillis_nanos: 0,
            stable_order: 0,
            identity: crate::FamilySampleIdentity::Dynamic {
                instance_id: source.instance_id,
                controller_id: source.controller_id,
                lane_id: source.lane_id,
            },
        },
        source.activation_mix,
    )
    .unwrap();
    let result = compose_dynamic_family(
        ProgrammingOwner::Position,
        &base,
        &[sample],
        &FamilyCompositionContext::default(),
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(
        result,
        AttributeValue::Position(Arc::new(PositionIntent::angles(38.0, -12.0)))
    );
}
fn sources() -> Sources {
    Sources {
        pan: Some(25.0),
        tilt: Some(-30.0),
        reads: Cell::new(0),
    }
}

#[test]
fn hot_axis_edit_bundles_each_resume_branch_and_reads_today_current() {
    let samples = vec![
        sample(
            10,
            resume(
                Some(angle(ProgrammingComponent::Pan, 90.0)),
                Some(angle(ProgrammingComponent::Tilt, 60.0)),
                0.5,
                100,
            ),
        ),
        sample(
            11,
            resume(Some(current(ProgrammingComponent::Tilt)), None, 0.5, 100),
        ),
        sample(
            12,
            resume(None, Some(current(ProgrammingComponent::Pan)), 0.5, 100),
        ),
    ];
    let mut frame = sources();
    for (pan, tilt) in [(25.0, -30.0), (40.0, -10.0)] {
        frame.pan = Some(pan);
        frame.tilt = Some(tilt);
        let bundled = bundle_position_sample_expressions(&samples, &frame).unwrap();
        assert!(bundled.remainder.is_empty());
        let position = bundled.position.unwrap();
        assert_expression_eq(
            &position.expression,
            resume(Some(whole(90.0, tilt)), Some(whole(pan, 60.0)), 0.5, 100),
        );
        assert_eq!(
            position.activation_mix, 0.4,
            "resume must not consume controller activation"
        );
        assert_eq!(position.lane_id, Uuid::from_u128(12));
    }
}

#[test]
fn missing_current_suppresses_only_incomplete_branch_and_exact_end_skips_it() {
    let frame = Sources {
        pan: None,
        tilt: Some(-20.0),
        reads: Cell::new(0),
    };
    let samples = |progress| {
        vec![
            sample(
                10,
                resume(
                    Some(angle(ProgrammingComponent::Pan, 90.0)),
                    Some(angle(ProgrammingComponent::Tilt, 60.0)),
                    progress,
                    100,
                ),
            ),
            sample(
                11,
                resume(
                    Some(current(ProgrammingComponent::Tilt)),
                    None,
                    progress,
                    100,
                ),
            ),
            sample(
                12,
                resume(
                    None,
                    Some(current(ProgrammingComponent::Pan)),
                    progress,
                    100,
                ),
            ),
        ]
    };
    let position = bundle_position_sample_expressions(&samples(0.5), &frame)
        .unwrap()
        .position
        .unwrap();
    assert_expression_eq(
        &position.expression,
        resume(Some(whole(90.0, -20.0)), None, 0.5, 100),
    );
    frame.reads.set(0);
    let position = bundle_position_sample_expressions(&samples(0.0), &frame)
        .unwrap()
        .position
        .unwrap();
    assert_expression_eq(&position.expression, whole(90.0, -20.0));
    assert_eq!(
        frame.reads.get(),
        1,
        "invisible new Current must not be queried"
    );
    assert!(
        bundle_position_sample_expressions(&samples(1.0), &frame)
            .unwrap()
            .position
            .is_none()
    );
}

#[test]
fn interrupted_resume_preserves_inner_pair_and_outer_membership() {
    let frame = sources();
    let samples = vec![
        sample(
            10,
            resume(
                Some(resume(
                    Some(angle(ProgrammingComponent::Pan, 90.0)),
                    Some(angle(ProgrammingComponent::Tilt, 60.0)),
                    0.5,
                    100,
                )),
                Some(angle(ProgrammingComponent::Pan, 180.0)),
                0.25,
                200,
            ),
        ),
        sample(
            11,
            resume(
                Some(resume(
                    Some(current(ProgrammingComponent::Tilt)),
                    None,
                    0.5,
                    100,
                )),
                Some(current(ProgrammingComponent::Tilt)),
                0.25,
                200,
            ),
        ),
        sample(
            12,
            resume(
                Some(resume(
                    None,
                    Some(current(ProgrammingComponent::Pan)),
                    0.5,
                    100,
                )),
                None,
                0.25,
                200,
            ),
        ),
    ];
    let position = bundle_position_sample_expressions(&samples, &frame)
        .unwrap()
        .position
        .unwrap();
    assert_expression_eq(
        &position.expression,
        resume(
            Some(resume(
                Some(whole(90.0, -30.0)),
                Some(whole(25.0, 60.0)),
                0.5,
                100,
            )),
            Some(whole(180.0, -30.0)),
            0.25,
            200,
        ),
    );
}

#[test]
fn cross_owner_resume_keeps_color_component_scope_and_independent_influence() {
    let color = Expression::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: super::super::DynamicSemanticColorBasis::Retain,
            },
            component: Some(ProgrammingComponent::Color(ColorComponent::Uv)),
        }),
        value: DynamicValue::Scalar(0.7),
        occurrence: None,
        dependency_occurrence: None,
    };
    let samples = vec![
        sample(
            10,
            resume(
                Some(angle(ProgrammingComponent::Pan, 90.0)),
                Some(color.clone()),
                0.25,
                100,
            ),
        ),
        sample(
            11,
            resume(Some(current(ProgrammingComponent::Tilt)), None, 0.25, 100),
        ),
    ];
    let bundle = bundle_position_sample_expressions(&samples, &sources()).unwrap();
    let Expression::Retained {
        tape: position_tape,
        ..
    } = &bundle.position.as_ref().unwrap().expression
    else {
        panic!("bundled Position must remain flat");
    };
    let Expression::Retained {
        tape: remainder_tape,
        ..
    } = &bundle.remainder[0].expression
    else {
        panic!("non-Position fragments must use the same flat storage");
    };
    assert!(Arc::ptr_eq(position_tape, remainder_tape));
    assert_expression_eq(
        &bundle.position.unwrap().expression,
        resume(Some(whole(90.0, -30.0)), None, 0.25, 100),
    );
    assert_eq!(bundle.remainder.len(), 1);
    assert_expression_eq(
        &bundle.remainder[0].expression,
        resume(None, Some(color), 0.25, 100),
    );
}

#[test]
fn mixed_legacy_and_typed_resume_visits_legacy_fragments_on_either_side() {
    let legacy = Expression::LegacyScalar {
        attribute: light_core::AttributeKey::intensity(),
        value: 0.8,
        occurrence: None,
        dependency_occurrence: None,
    };
    let typed = whole(90.0, 30.0);
    for (from, to, expected_weight) in
        [(legacy.clone(), typed.clone(), 0.75), (typed, legacy, 0.25)]
    {
        let expression = resume(Some(from), Some(to), 0.25, 100);
        let mut contributions = vec![];
        let only_legacy = expression.visit_legacy_contributions(|attribute, value, weight| {
            contributions.push((attribute.clone(), value, weight));
        });
        assert!(!only_legacy);
        assert_eq!(
            contributions,
            vec![(light_core::AttributeKey::intensity(), 0.8, expected_weight)]
        );
    }
}

#[test]
fn one_axis_cannot_borrow_partner_from_another_controller_or_inconsistent_frame() {
    let first = sample(10, angle(ProgrammingComponent::Pan, 90.0));
    assert!(
        bundle_position_sample_expressions(&[first.clone()], &sources())
            .unwrap()
            .position
            .is_none()
    );
    let mut second = sample(11, current(ProgrammingComponent::Tilt));
    second.controller_id = Uuid::from_u128(99);
    assert!(
        bundle_position_sample_expressions(&[first.clone(), second.clone()], &sources()).is_err()
    );
    second.controller_id = first.controller_id;
    second.activation_mix = 0.5;
    assert!(bundle_position_sample_expressions(&[first.clone(), second], &sources()).is_err());
    assert!(
        bundle_position_sample_expressions(
            &[
                first.clone(),
                sample(11, angle(ProgrammingComponent::Pan, 20.0))
            ],
            &sources()
        )
        .is_err()
    );
}

fn flat_angle_value(expression: &Expression) -> [f32; 2] {
    let Expression::Retained { tape, root } = expression else {
        panic!("Angle pairing must produce a flat retained expression");
    };
    let mut values: Vec<Option<[f32; 2]>> = Vec::with_capacity(tape.nodes.len());
    for node in &tape.nodes {
        let value = match node {
            Node::Programming {
                address,
                value: DynamicValue::Family(AttributeValue::Position(position)),
                ..
            } if address.component.is_none() => {
                let PositionIntent::Angles {
                    pan_degrees: ScalarIntent::Value(pan),
                    tilt_degrees: ScalarIntent::Value(tilt),
                } = position.as_ref()
                else {
                    panic!("expected resolved complete angles");
                };
                Some([*pan, *tilt])
            }
            Node::Transition {
                from: Some(from),
                to: Some(to),
                progress,
                ..
            } => {
                let from = values[from.0 as usize].unwrap();
                let to = values[to.0 as usize].unwrap();
                let p = f64::from(*progress);
                Some(std::array::from_fn(|axis| {
                    (f64::from(from[axis]) * (1.0 - p) + f64::from(to[axis]) * p) as f32
                }))
            }
            Node::AngleCurrent { .. } => {
                panic!("Current must be resolved from this frame before arbitration")
            }
            _ => None,
        };
        values.push(value);
    }
    values[root.0 as usize].unwrap()
}

#[test]
fn hundreds_of_alternating_axis_interruptions_survive_tape_roundtrip_and_live_current_changes() {
    const INTERRUPTIONS: usize = 160;
    let mut tape = RetainedExpressionTape::from_roots(&[
        Arc::new(angle(ProgrammingComponent::Pan, 90.0)),
        Arc::new(current(ProgrammingComponent::Tilt)),
        Arc::new(current(ProgrammingComponent::Pan)),
    ])
    .unwrap();
    let current_tilt = tape.roots[1];
    let current_pan = tape.roots[2];
    let mut roots = [Some(tape.roots[0]), Some(current_tilt), None];
    for interruption in 1..=INTERRUPTIONS {
        let pan = interruption % 2 == 0;
        let new_axis = append(
            &mut tape,
            Node::Programming {
                address: (*address(if pan {
                    ProgrammingComponent::Pan
                } else {
                    ProgrammingComponent::Tilt
                }))
                .clone(),
                value: DynamicValue::Scalar(90.0 + interruption as f32),
                occurrence: None,
                dependency_occurrence: None,
            },
        )
        .unwrap();
        let incoming = [
            Some(new_axis),
            pan.then_some(current_tilt),
            (!pan).then_some(current_pan),
        ];
        for lane in 0..roots.len() {
            roots[lane] = Some(
                tape.append_resume(
                    roots[lane],
                    incoming[lane],
                    0.25,
                    Uuid::from_u128(1000 + interruption as u128),
                )
                .unwrap(),
            );
        }
    }
    tape.roots = roots.into_iter().map(Option::unwrap).collect();
    tape.validate().unwrap();
    let restored: RetainedExpressionTape =
        serde_json::from_slice(&serde_json::to_vec(&tape).unwrap()).unwrap();
    assert_eq!(
        restored, tape,
        "snapshot transport must preserve flat membership references"
    );
    let tape = Arc::new(restored);
    let samples = tape
        .roots
        .iter()
        .enumerate()
        .map(|(lane, root)| {
            sample(
                10 + lane as u128,
                Expression::Retained {
                    tape: tape.clone(),
                    root: *root,
                },
            )
        })
        .collect::<Vec<_>>();
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            for (pan, tilt) in [(25.0, -30.0), (720.0, 35.0), (-450.0, -80.0)] {
                let frame = Sources {
                    pan: Some(pan),
                    tilt: Some(tilt),
                    reads: Cell::new(0),
                };
                let bundled = bundle_position_sample_expressions(&samples, &frame).unwrap();
                assert!(bundled.remainder.is_empty());
                let position = bundled.position.unwrap();
                let mut expected = [90.0, tilt];
                for interruption in 1..=INTERRUPTIONS {
                    let authored = 90.0 + interruption as f32;
                    let incoming = if interruption % 2 == 0 {
                        [authored, tilt]
                    } else {
                        [pan, authored]
                    };
                    expected = std::array::from_fn(|axis| {
                        (f64::from(expected[axis]) * 0.75 + f64::from(incoming[axis]) * 0.25) as f32
                    });
                }
                assert_eq!(flat_angle_value(&position.expression), expected);
                assert_eq!(
                    frame.reads.get(),
                    2,
                    "all occurrences must share this frame's two Current values"
                );
                assert_eq!(position.lane_id, Uuid::from_u128(12));
                assert_eq!(position.activation_mix, 0.4);
                let Expression::Retained { tape, .. } = &position.expression else {
                    unreachable!()
                };
                assert!(
                    tape.nodes.len() <= INTERRUPTIONS * 2 + 1,
                    "pairing must retain a linear history rather than expanding every branch"
                );
                tape.validate().unwrap();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn ordinary_and_retained_roots_share_resume_pairing_and_consistency_checks() {
    let original = resume(
        Some(angle(ProgrammingComponent::Pan, 90.0)),
        Some(angle(ProgrammingComponent::Tilt, 60.0)),
        0.5,
        100,
    );
    let tape = Arc::new(RetainedExpressionTape::from_roots(&[Arc::new(original)]).unwrap());
    let mut samples = vec![
        sample(
            10,
            Expression::Retained {
                tape: tape.clone(),
                root: tape.roots[0],
            },
        ),
        sample(
            11,
            resume(Some(current(ProgrammingComponent::Tilt)), None, 0.5, 100),
        ),
        sample(
            12,
            resume(None, Some(current(ProgrammingComponent::Pan)), 0.5, 100),
        ),
    ];
    let position = bundle_position_sample_expressions(&samples, &sources())
        .unwrap()
        .position
        .unwrap();
    assert_expression_eq(
        &position.expression,
        resume(Some(whole(90.0, -30.0)), Some(whole(25.0, 60.0)), 0.5, 100),
    );
    samples[2].expression = resume(None, Some(current(ProgrammingComponent::Pan)), 0.25, 100);
    assert!(
        bundle_position_sample_expressions(&samples, &sources()).is_err(),
        "one retained occurrence cannot disagree with an ordinary sibling's progress"
    );
    samples[2].expression = resume(None, Some(current(ProgrammingComponent::Pan)), 0.5, 101);
    assert!(
        bundle_position_sample_expressions(&samples, &sources()).is_err(),
        "different outer occurrences cannot borrow each other's axes"
    );
}

#[test]
fn whole_target_crosses_retained_angle_branch_but_target_components_cannot_supply_an_axis() {
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [1.0, 2.0, 3.0],
    )));
    let target_expression = Expression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &target).unwrap(),
        ),
        value: DynamicValue::Family(target),
        occurrence: None,
        dependency_occurrence: None,
    };
    let tape = Arc::new(
        RetainedExpressionTape::from_roots(&[
            Arc::new(resume(
                Some(angle(ProgrammingComponent::Pan, 720.0)),
                Some(target_expression.clone()),
                0.5,
                100,
            )),
            Arc::new(resume(
                Some(current(ProgrammingComponent::Tilt)),
                None,
                0.5,
                100,
            )),
        ])
        .unwrap(),
    );
    let samples = tape
        .roots
        .iter()
        .enumerate()
        .map(|(lane, root)| {
            sample(
                10 + lane as u128,
                Expression::Retained {
                    tape: tape.clone(),
                    root: *root,
                },
            )
        })
        .collect::<Vec<_>>();
    let position = bundle_position_sample_expressions(&samples, &sources())
        .unwrap()
        .position
        .unwrap();
    assert_expression_eq(
        &position.expression,
        resume(
            Some(whole(720.0, -30.0)),
            Some(target_expression.clone()),
            0.5,
            100,
        ),
    );
    assert!(
        bundle_position_sample_expressions(
            &[
                sample(10, target_expression),
                sample(11, angle(ProgrammingComponent::Pan, 90.0)),
            ],
            &sources()
        )
        .is_err(),
        "whole Target and Angle components cannot coexist in one branch"
    );
    let target_x = Expression::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Origin),
            },
            component: Some(ProgrammingComponent::TargetX),
        }),
        value: DynamicValue::Scalar(2.0),
        occurrence: None,
        dependency_occurrence: None,
    };
    assert!(
        bundle_position_sample_expressions(
            &[
                sample(10, target_x),
                sample(11, current(ProgrammingComponent::Tilt)),
            ],
            &sources()
        )
        .is_err(),
        "Target components require their own component compositor"
    );
}
