use super::*;
use crate::{DynamicRuntimeSample, DynamicSourceOccurrenceId, FamilyCompositionSample};
use light_core::FixtureId;
use std::cell::RefCell;
use uuid::Uuid;

fn general<T>(f: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            GENERAL_ONLY.with(|v| v.set(self.0));
        }
    }
    let _reset = Reset(GENERAL_ONLY.with(|v| v.replace(true)));
    f()
}
fn inputs(pan: f32, tilt: f32) -> Vec<DynamicRuntimeSample> {
    [ProgrammingComponent::Pan, ProgrammingComponent::Tilt]
        .into_iter()
        .zip([pan, tilt])
        .enumerate()
        .map(|(i, (component, value))| DynamicRuntimeSample {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            target: FixtureId(Uuid::from_u128(3)),
            lane_id: Uuid::from_u128(10 + i as u128),
            expression: DynamicSampleExpression::Programming {
                address: Arc::new(DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component: Some(component),
                }),
                value: DynamicValue::Scalar(value),
                occurrence: Some(
                    DynamicSourceOccurrenceId::new(Uuid::from_u128(30 + i as u128)).unwrap(),
                ),
                dependency_occurrence: None,
            },
            priority: 100,
            activated_at_millis: 1000,
            activation_mix: 1.,
            address: None,
        })
        .collect()
}
struct NoSources;
impl crate::DynamicValueSourceResolver for NoSources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        panic!("plain authored pair cannot query Current")
    }
    fn preset(
        &self,
        _: &crate::DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        panic!("plain authored pair cannot query Presets")
    }
}
fn bundle(samples: &[DynamicRuntimeSample]) -> Result<FamilyCompositionSample, TransitionError> {
    crate::bundle_position_component_forest(samples, &NoSources)?
        .position
        .ok_or(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
}
fn compiled(sample: &FamilyCompositionSample) -> &Arc<CompiledCoupledExpression> {
    let FamilyCompositionSample::CoupledExpression { expression, .. } = sample else {
        panic!("retain coupled expression")
    };
    expression
}
fn compare_sources(a: &[CoupledComponentEndpoint], b: &[CoupledComponentEndpoint]) {
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        assert_eq!(a.lane_id, b.lane_id);
        assert_eq!(a.address.address(), b.address.address());
        assert_eq!(a.value, b.value);
        if let (DynamicValue::Scalar(a), DynamicValue::Scalar(b)) = (&a.value, &b.value) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        assert_eq!(a.role, b.role);
        assert_eq!(a.occurrence, b.occurrence);
        assert_eq!(a.dependency_occurrence, b.dependency_occurrence);
    }
}
fn compare(a: &CompiledCoupledExpression, b: &CompiledCoupledExpression) {
    assert_eq!(a.owner, b.owner);
    assert_eq!(a.root, b.root);
    assert_eq!(a.position_node_origins, b.position_node_origins);
    assert_eq!(a.node_roles, b.node_roles);
    assert_eq!(a.base_underlay, b.base_underlay);
    assert_eq!(a.roles, b.roles);
    assert_eq!(a.base_endpoint_nodes, b.base_endpoint_nodes);
    assert!(
        a.orthogonals.is_empty()
            && b.orthogonals.is_empty()
            && a.models.is_empty()
            && b.models.is_empty()
    );
    assert_eq!(a.nodes.len(), 2);
    assert_eq!(b.nodes.len(), 2);
    assert!(matches!(a.nodes[0], Node::Underlay) && matches!(b.nodes[0], Node::Underlay));
    let leaf = |node: &Node| match node {
        Node::Leaf {
            address,
            value,
            role,
            occurrence,
            dependency_occurrence,
        } => format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}",
            address.address(),
            value,
            role,
            occurrence,
            dependency_occurrence
        ),
        _ => panic!("one leaf"),
    };
    assert_eq!(leaf(&a.nodes[1]), leaf(&b.nodes[1]));
    assert_eq!(a.leaf_sources.len(), b.leaf_sources.len());
    for (x, y) in a.leaf_sources.iter().zip(&b.leaf_sources) {
        compare_sources(x, y);
    }
    let (Some(x), Some(y)) = (&a.position_forest_lineage, &b.position_forest_lineage) else {
        panic!("lineage")
    };
    assert_eq!(x.root, y.root);
    assert_eq!(x.compiled_nodes, y.compiled_nodes);
    assert_eq!(x.nodes.len(), 1);
    assert_eq!(y.nodes.len(), 1);
    assert!(x.cohort_members.is_empty() && y.cohort_members.is_empty());
    let (
        PositionForestNode::Whole {
            expression: xe,
            lane_id: xl,
            sources: xs,
        },
        PositionForestNode::Whole {
            expression: ye,
            lane_id: yl,
            sources: ys,
        },
    ) = (&x.nodes[0], &y.nodes[0])
    else {
        panic!("original whole")
    };
    assert_eq!(xe, ye);
    assert_eq!(xl, yl);
    compare_sources(xs, ys);
    assert!(Arc::ptr_eq(xs, &a.leaf_sources[1]) && Arc::ptr_eq(ys, &b.leaf_sources[1]));
    assert_eq!(x.wholes.len(), 1);
    assert_eq!(y.wholes.len(), 1);
    let (x, y) = (&x.wholes[0], &y.wholes[0]);
    assert_eq!(x.forest_node, y.forest_node);
    assert_eq!(x.lane_id, y.lane_id);
    assert_eq!(x.tape, y.tape);
    assert_eq!(x.tape_to_compiled, y.tape_to_compiled);
    assert_eq!(a.base_endpoints.len(), b.base_endpoints.len());
    for (x, y) in a.base_endpoints.iter().zip(&b.base_endpoints) {
        let (xa, xv) = x.materialized().unwrap();
        let (ya, yv) = y.materialized().unwrap();
        assert_eq!(xa.address(), ya.address());
        assert_eq!(xv, yv);
    }
    let (CoupledRetainedSources::PositionForest(x), CoupledRetainedSources::PositionForest(y)) =
        (&a.retained, &b.retained)
    else {
        panic!("original samples")
    };
    assert_eq!(x, y);
    let x = a.position_operation_provenance().unwrap().unwrap();
    let y = b.position_operation_provenance().unwrap().unwrap();
    assert_eq!(x.len(), y.len());
    for ((a, x), (b, y)) in x.iter().zip(&y) {
        assert_eq!(a, b);
        assert!(x.handles().is_empty() && y.handles().is_empty());
        assert_eq!(x.unattributed_operations(), y.unattributed_operations());
    }
}
#[derive(Default)]
struct Frame {
    reads: RefCell<Vec<String>>,
    require: bool,
}
impl CoupledExpressionContext for Frame {
    fn materialize_base(
        &self,
        endpoint: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
    ) -> Result<AttributeValue, TransitionError> {
        self.reads.borrow_mut().push(format!(
            "base:{:?}",
            endpoint.map(|(a, v)| (a.address(), v))
        ));
        if self.require {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        match endpoint {
            Some((_, DynamicValue::Family(value))) => Ok(value.clone()),
            _ => panic!("complete authored endpoint"),
        }
    }
    fn orthogonal_underlay(
        &self,
        _: ColorComponent,
        _: &AttributeValue,
    ) -> Result<f32, TransitionError> {
        panic!("no color")
    }
}
impl WholeFamilyExpressionFrameResolver for Frame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.reads
            .borrow_mut()
            .push(format!("resolve:{requirement:?}"));
        Err(TransitionError::Requires(requirement))
    }
}
#[derive(Default)]
struct Observer(Vec<String>);
impl CoupledEvaluationObserver for Observer {
    fn evaluated(
        &mut self,
        node: usize,
        step: CoupledEvaluationStep<'_>,
        value: &AttributeValue,
    ) -> Result<(), TransitionError> {
        let CoupledEvaluationStep::Leaf {
            address,
            value: source,
            sources,
            role,
            occurrence,
            dependency_occurrence,
        } = step
        else {
            panic!("plain leaf observation")
        };
        self.0.push(format!("{node}|{:?}|{source:?}|{role:?}|{occurrence:?}|{dependency_occurrence:?}|{value:?}|{:?}",address.address(),sources.iter().map(|s|(s.lane_id,s.address.address(),&s.value,s.role,s.occurrence,&s.dependency_occurrence)).collect::<Vec<_>>()));
        Ok(())
    }
}
fn evaluate(a: &CompiledCoupledExpression, require: bool) -> (String, Vec<String>, Vec<String>) {
    let frame = Frame {
        require,
        ..Default::default()
    };
    let mut observer = Observer::default();
    let value = a.evaluate_base_observed(&frame, &frame, Some(&mut observer));
    (format!("{value:?}"), frame.reads.into_inner(), observer.0)
}
#[test]
fn actual_plain_pan_tilt_uses_fixed_slots_and_matches_frozen_general_graph_and_full_observation() {
    for (pan, tilt) in [
        (30., -50.),
        (-0., 0.),
        (0., -0.),
        (-720., 270.),
        (f32::MAX, -f32::MAX),
    ] {
        for reverse in [false, true] {
            let mut samples = inputs(pan, tilt);
            if reverse {
                samples.reverse();
            }
            COUNTS.with(|n| n.set((0, 0)));
            let fast = bundle(&samples).unwrap();
            assert_eq!(
                COUNTS.with(std::cell::Cell::get),
                (1, 0),
                "fast path must actually run"
            );
            let slow = general(|| bundle(&samples)).unwrap();
            assert_eq!(COUNTS.with(std::cell::Cell::get), (1, 1));
            compare(compiled(&fast), compiled(&slow));
            for require in [false, true] {
                assert_eq!(
                    evaluate(compiled(&fast), require),
                    evaluate(compiled(&slow), require)
                );
            }
            let FamilyCompositionSample::CoupledExpression {
                rank: a,
                activation_mix: x,
                ..
            } = fast
            else {
                unreachable!()
            };
            let FamilyCompositionSample::CoupledExpression {
                rank: b,
                activation_mix: y,
                ..
            } = slow
            else {
                unreachable!()
            };
            assert_eq!(a, b);
            assert_eq!(x, y);
        }
    }
}
#[test]
fn admission_checks_original_pair_identity_source_order_and_exact_plain_forest() {
    let samples = inputs(30., 20.);
    let prepared = bundle(&samples).unwrap();
    let compiled = compiled(&prepared);
    let forest = &compiled.position_forest_lineage.as_ref().unwrap().nodes;
    assert!(admit(&samples, forest, 0).is_some());
    for edit in 0..10 {
        let mut changed = samples.clone();
        match edit {
            0 => changed[1].instance_id = Uuid::from_u128(99),
            1 => changed[1].controller_id = Uuid::from_u128(99),
            2 => changed[1].target = FixtureId(Uuid::from_u128(99)),
            3 => changed[1].priority += 1,
            4 => changed[1].activated_at_millis += 1,
            5 => changed[1].activation_mix = 0.5,
            6 => changed[1].lane_id = changed[0].lane_id,
            7 => changed.reverse(),
            8 => {
                let DynamicSampleExpression::Programming {
                    dependency_occurrence,
                    ..
                } = &mut changed[0].expression
                else {
                    unreachable!()
                };
                *dependency_occurrence = Some(crate::DynamicSourceDependency::unknown(None));
            }
            _ => {
                let value = Arc::new(changed[0].expression.clone());
                changed[0].expression = DynamicSampleExpression::Operation {
                    origin: None,
                    value,
                };
            }
        }
        assert!(admit(&changed, forest, 0).is_none(), "edit {edit}");
    }
    assert!(admit(&samples, forest, 1).is_none());
    assert!(admit(&samples, &[], 0).is_none());
    let mut more = forest.to_vec();
    more.push(PositionForestNode::Transition {
        from: Some(0),
        to: None,
        progress: 0.5,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::LiveJointAngles,
        },
    });
    assert!(admit(&samples, &more, 1).is_none());
}
#[test]
fn malformed_and_unavailable_inputs_keep_error_precedence_and_leave_old_sources_unchanged() {
    let samples = inputs(30., 20.);
    let good = bundle(&samples).unwrap();
    let baseline = evaluate(compiled(&good), false);
    for edit in 0..5 {
        let mut changed = samples.clone();
        match edit {
            0 => {
                let DynamicSampleExpression::Programming { value, .. } = &mut changed[0].expression
                else {
                    unreachable!()
                };
                *value = DynamicValue::Scalar(f32::NAN);
            }
            1 => changed[1].lane_id = changed[0].lane_id,
            2 => changed[1].priority += 1,
            3 => {
                let DynamicSampleExpression::Programming { address, .. } =
                    &mut changed[0].expression
                else {
                    unreachable!()
                };
                Arc::make_mut(address).component = Some(ProgrammingComponent::Tilt);
            }
            _ => {
                changed[1].expression = DynamicSampleExpression::AngleCurrent {
                    address: Arc::new(DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::Angles,
                        component: Some(ProgrammingComponent::Tilt),
                    }),
                }
            }
        }
        // Current fallback intentionally reports the original query through the same reader.
        if edit == 4 {
            assert!(
                admit(
                    &changed,
                    &compiled(&good)
                        .position_forest_lineage
                        .as_ref()
                        .unwrap()
                        .nodes,
                    0
                )
                .is_none()
            );
            continue;
        }
        let fast = bundle(&changed).err().map(|e| format!("{e:?}"));
        let slow = general(|| bundle(&changed)).err().map(|e| format!("{e:?}"));
        assert!(fast.is_some());
        assert_eq!(fast, slow);
        assert_eq!(evaluate(compiled(&good), false), baseline);
    }
    let mut edited = samples.clone();
    let DynamicSampleExpression::Programming { value, .. } = &mut edited[0].expression else {
        unreachable!()
    };
    *value = DynamicValue::Scalar(90.);
    let updated = bundle(&edited).unwrap();
    let expected = general(|| bundle(&edited)).unwrap();
    compare(compiled(&updated), compiled(&expected));
    assert_ne!(evaluate(compiled(&updated), false), baseline);
    assert_eq!(evaluate(compiled(&good), false), baseline);
    // The invalid attempt installed no retained state: the next original compilation recovers.
    compare(compiled(&bundle(&samples).unwrap()), compiled(&good));
}
#[test]
fn one_two_and_four_workers_keep_per_target_graph_values_provenance_and_observation_order() {
    for workers in [1, 2, 4] {
        let inputs = (0..32)
            .map(|i| {
                let mut s = inputs(i as f32, -(i as f32));
                for sample in &mut s {
                    sample.target = FixtureId(Uuid::from_u128(100 + i));
                    sample.activation_mix = 0.75;
                }
                s
            })
            .collect::<Vec<_>>();
        let run = |generic| {
            std::thread::scope(|scope| {
                let handles = (0..workers)
                    .map(|worker| {
                        let inputs = &inputs;
                        scope.spawn(move || {
                            (worker..inputs.len())
                                .step_by(workers)
                                .map(|i| {
                                    let sample = if generic {
                                        general(|| bundle(&inputs[i]))
                                    } else {
                                        bundle(&inputs[i])
                                    }
                                    .unwrap();
                                    (i, sample)
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect::<Vec<_>>();
                let mut result = handles
                    .into_iter()
                    .flat_map(|h| h.join().unwrap())
                    .collect::<Vec<_>>();
                result.sort_by_key(|r| r.0);
                result
            })
        };
        let fast = run(false);
        let slow = run(true);
        for ((i, a), (j, b)) in fast.iter().zip(&slow) {
            assert_eq!(i, j);
            compare(compiled(a), compiled(b));
            assert_eq!(evaluate(compiled(a), false), evaluate(compiled(b), false));
        }
    }
}

#[derive(Default)]
struct Reads {
    events: RefCell<Vec<String>>,
    missing: bool,
}
impl crate::DynamicValueSourceResolver for Reads {
    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.events
            .borrow_mut()
            .push(format!("current:{target:?}:{address:?}"));
        if self.missing {
            return None;
        }
        match address.component {
            Some(ProgrammingComponent::Pan) => Some(DynamicValue::Scalar(70.)),
            Some(ProgrammingComponent::Tilt) => Some(DynamicValue::Scalar(80.)),
            _ => None,
        }
    }
    fn try_position_current_family(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.events
            .borrow_mut()
            .push(format!("family:{target:?}:{address:?}"));
        Ok((!self.missing)
            .then(|| AttributeValue::Position(Arc::new(PositionIntent::angles(70., 80.)))))
    }
    fn preset(
        &self,
        _: &crate::DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        self.events.borrow_mut().push("preset".into());
        None
    }
}
fn prepared_observation(
    samples: &[DynamicRuntimeSample],
    sources: &Reads,
    scratch: &mut crate::DynamicFamilyPreparationScratch,
) -> Vec<String> {
    let prepared = crate::prepare_dynamic_family_samples(samples, sources, None, scratch);
    let prepared = match prepared {
        Ok(p) => p,
        Err(e) => return vec![format!("error:{e:?}")],
    };
    let mut output = vec![
        format!("requirements:{:?}", prepared.requirements),
        format!("legacy:{:?}", prepared.legacy),
    ];
    for group in prepared.families {
        let mut trace = crate::RetainedFamilyCompositionScratch::default();
        let frame = Frame::default();
        let result = crate::compose_retained_dynamic_family_traced(
            group.owner,
            &AttributeValue::Position(Arc::new(PositionIntent::angles(7., 8.))),
            &group.samples,
            &crate::FamilyCompositionContext::default(),
            &frame,
            &mut trace,
        );
        output.push(format!("result:{result:?}"));
        output.push(format!("frame:{:?}", frame.reads.borrow()));
        if let Some(root) = trace.family_trace().root() {
            for component in [ProgrammingComponent::Pan, ProgrammingComponent::Tilt] {
                let scope = ProgrammingFieldScope::from_component(component);
                output.push(format!(
                    "trace:{:?}",
                    trace.family_trace().query_fields_with_base(root, &scope)
                ));
            }
        }
    }
    output
}
#[test]
fn full_preparation_traces_requirements_and_current_reader_order_match_on_actual_fallbacks() {
    let plain = inputs(30., 20.);
    let mut cases = vec![plain.clone()];
    let mut current = plain.clone();
    current[1].expression = DynamicSampleExpression::AngleCurrent {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Tilt),
        }),
    };
    cases.push(current);
    let mut interrupted = plain.clone();
    for sample in &mut interrupted {
        let to = Arc::new(sample.expression.clone());
        let mut from = sample.expression.clone();
        let DynamicSampleExpression::Programming { value, .. } = &mut from else {
            unreachable!()
        };
        *value = DynamicValue::Scalar(-10.);
        sample.expression = DynamicSampleExpression::Transition {
            from: Some(Arc::new(from)),
            to: Some(to),
            progress: 0.4,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(50),
            },
        };
    }
    cases.push(interrupted);
    let mut annotated = plain.clone();
    for sample in &mut annotated {
        sample.expression = DynamicSampleExpression::Operation {
            origin: None,
            value: Arc::new(sample.expression.clone()),
        };
    }
    cases.push(annotated);
    let mut target = plain.clone();
    for (sample, component) in target
        .iter_mut()
        .zip([ProgrammingComponent::TargetX, ProgrammingComponent::TargetY])
    {
        sample.expression = DynamicSampleExpression::Programming {
            address: Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Target {
                    reference: Some(TargetReference::Origin),
                },
                component: Some(component),
            }),
            value: DynamicValue::Scalar(1.),
            occurrence: None,
            dependency_occurrence: None,
        };
    }
    cases.push(target);
    cases.push(vec![plain[0].clone()]);
    for activation in [0., 0.25, 0.75] {
        let mut changed = plain.clone();
        for sample in &mut changed {
            sample.activation_mix = activation;
            sample.priority = -12;
            sample.activated_at_millis = 2345;
        }
        cases.push(changed);
    }
    for (case, samples) in cases.iter().enumerate() {
        for missing in [false, true] {
            let a = Reads {
                missing,
                ..Default::default()
            };
            let b = Reads {
                missing,
                ..Default::default()
            };
            let fast = prepared_observation(samples, &a, &mut Default::default());
            let slow = general(|| prepared_observation(samples, &b, &mut Default::default()));
            assert_eq!(fast, slow, "case {case}, missing {missing}");
            assert_eq!(*a.events.borrow(), *b.events.borrow());
            if case == 1 {
                assert!(
                    !a.events.borrow().is_empty(),
                    "must actually execute Current fallback"
                );
            }
        }
    }
}
#[test]
fn preparation_scratch_recovers_after_failed_edit_without_changing_retained_trace() {
    let original = inputs(-0., 20.);
    let sources = Reads::default();
    let mut fast = crate::DynamicFamilyPreparationScratch::default();
    let mut slow = crate::DynamicFamilyPreparationScratch::default();
    let baseline = prepared_observation(&original, &sources, &mut fast);
    let expected = general(|| prepared_observation(&original, &sources, &mut slow));
    assert_eq!(baseline, expected);
    let held = bundle(&original).unwrap();
    let held_before = evaluate(compiled(&held), false);
    let mut changed = original.clone();
    let DynamicSampleExpression::Programming { address, value, .. } = &mut changed[0].expression
    else {
        unreachable!()
    };
    Arc::make_mut(address).component = Some(ProgrammingComponent::Tilt);
    *value = DynamicValue::Scalar(f32::INFINITY);
    let failure = prepared_observation(&changed, &sources, &mut fast);
    assert!(failure[0].starts_with("error:"));
    assert_eq!(
        failure,
        general(|| prepared_observation(&changed, &sources, &mut slow))
    );
    assert_eq!(
        prepared_observation(&original, &sources, &mut fast),
        general(|| prepared_observation(&original, &sources, &mut slow))
    );
    assert_eq!(evaluate(compiled(&held), false), held_before);
}
