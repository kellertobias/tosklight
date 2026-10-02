use super::*;
use crate::{
    DynamicSampleExpression as E, DynamicSemanticColorBasis, DynamicTransitionReason,
    DynamicValueAddress,
};
use light_core::programming::{
    ColorComponent, IntentError, NativeColorComponentDescriptor, NativeColorEditModel,
    NativeColorRecipe, PortableColorEstimate, ProgrammingComponent, TargetReference,
};
use light_core::{NativeColorBinding, NativeColorIdentity};
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

fn compiled(address: DynamicValueAddress) -> Arc<CompiledDynamicValueAddress> {
    Arc::new(CompiledDynamicValueAddress::new(address, None).unwrap())
}
fn leaf(address: &CompiledDynamicValueAddress, value: DynamicValue) -> Arc<E> {
    Arc::new(E::Programming {
        address: Arc::new(address.address().clone()),
        value,
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn resume(from: Option<Arc<E>>, to: Option<Arc<E>>, progress: f32) -> Arc<E> {
    Arc::new(E::Transition {
        from,
        to,
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::new_v4(),
        },
    })
}
fn kelvin() -> Arc<CompiledDynamicValueAddress> {
    compiled(DynamicValueAddress {
        representation: DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Retain,
        },
        component: Some(ProgrammingComponent::Color(ColorComponent::Temperature)),
    })
}
fn target_z() -> Arc<CompiledDynamicValueAddress> {
    compiled(DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Target {
            reference: Some(TargetReference::Origin),
        },
        component: Some(ProgrammingComponent::TargetZ),
    })
}

#[test]
fn reciprocal_kelvin_and_signed_target_metres_use_compiled_component_domains() {
    let temperature = kelvin();
    let expression = resume(
        Some(leaf(&temperature, DynamicValue::Scalar(2000.0))),
        Some(leaf(&temperature, DynamicValue::Scalar(10000.0))),
        0.5,
    );
    let compiled = CompiledComponentExpression::new(expression.clone(), temperature).unwrap();
    assert_eq!(compiled.expression(), expression.as_ref());
    assert!(compiled.participates());
    assert!(!compiled.needs_underlay());
    let Some(DynamicValue::Scalar(result)) = compiled.evaluate(None).unwrap() else {
        panic!()
    };
    assert!((result - 3333.3333).abs() < 0.001);

    let metres = target_z();
    let compiled = CompiledComponentExpression::new(
        resume(
            Some(leaf(&metres, DynamicValue::Scalar(-2.0))),
            Some(leaf(&metres, DynamicValue::Scalar(6.0))),
            0.5,
        ),
        metres.clone(),
    )
    .unwrap();
    assert_eq!(compiled.address().address(), metres.address());
    assert_eq!(
        compiled.evaluate(None).unwrap(),
        Some(DynamicValue::Scalar(2.0))
    );
}

#[test]
fn missing_endpoint_reads_only_the_eligible_underlay_and_exact_release_has_no_mask() {
    let address = target_z();
    let incoming = leaf(&address, DynamicValue::Scalar(10.0));
    let blended = CompiledComponentExpression::new(
        resume(None, Some(incoming.clone()), 0.25),
        address.clone(),
    )
    .unwrap();
    assert!(blended.participates());
    assert!(blended.needs_underlay());
    assert_eq!(
        blended.evaluate(Some(&DynamicValue::Scalar(2.0))).unwrap(),
        Some(DynamicValue::Scalar(4.0))
    );
    assert!(matches!(
        blended.evaluate(None),
        Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints
        ))
    ));
    assert!(blended.evaluate(Some(&DynamicValue::Native(5))).is_err());

    for expression in [
        resume(None, Some(incoming.clone()), 0.0),
        resume(Some(incoming.clone()), None, 1.0),
    ] {
        let compiled = CompiledComponentExpression::new(expression, address.clone()).unwrap();
        assert!(!compiled.participates());
        assert!(!compiled.needs_underlay());
        assert_eq!(compiled.evaluate(None).unwrap(), None);
    }
    let exact =
        CompiledComponentExpression::new(resume(None, Some(incoming), 1.0), address).unwrap();
    assert!(exact.participates());
    assert!(!exact.needs_underlay());
    assert_eq!(
        exact.evaluate(None).unwrap(),
        Some(DynamicValue::Scalar(10.0))
    );
}

#[test]
fn interrupted_nested_resume_retains_original_tree_and_prunes_exact_branches() {
    let address = target_z();
    let inner = resume(
        Some(leaf(&address, DynamicValue::Scalar(0.0))),
        Some(leaf(&address, DynamicValue::Scalar(10.0))),
        0.5,
    );
    let expression = resume(
        Some(inner),
        Some(leaf(&address, DynamicValue::Scalar(20.0))),
        0.5,
    );
    let compiled = CompiledComponentExpression::new(expression.clone(), address.clone()).unwrap();
    assert_eq!(compiled.expression(), expression.as_ref());
    assert_eq!(
        compiled.evaluate(None).unwrap(),
        Some(DynamicValue::Scalar(12.5))
    );

    let exact = resume(
        Some(leaf(&address, DynamicValue::Scalar(4.0))),
        Some(resume(
            None,
            Some(leaf(&address, DynamicValue::Scalar(100.0))),
            0.5,
        )),
        0.0,
    );
    let compiled = CompiledComponentExpression::new(exact, address).unwrap();
    assert!(!compiled.needs_underlay());
    assert_eq!(
        compiled.evaluate(None).unwrap(),
        Some(DynamicValue::Scalar(4.0))
    );
}

#[test]
fn nested_absent_endpoints_release_the_footprint_even_at_an_interior_progress() {
    let address = target_z();
    let complete_release = resume(Some(leaf(&address, DynamicValue::Scalar(8.0))), None, 1.0);
    let interrupted = resume(Some(complete_release), None, 0.5);
    let compiled = CompiledComponentExpression::new(interrupted, address).unwrap();
    assert!(!compiled.participates());
    assert!(!compiled.needs_underlay());
    assert_eq!(compiled.evaluate(None).unwrap(), None);
}

struct NativeModel {
    source: NativeColorIdentity,
    descriptor: NativeColorComponentDescriptor,
    predictions: AtomicUsize,
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding == self.descriptor.binding).then_some(self.descriptor)
    }
    fn predict(&self, _: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        self.predictions.fetch_add(1, Ordering::Relaxed);
        panic!("a native component interpolation must not predict whole Color")
    }
}
fn native() -> (Arc<CompiledDynamicValueAddress>, Arc<NativeModel>) {
    let model = Arc::new(NativeModel {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(1),
            profile_revision: 2,
            profile_digest: "original-profile".into(),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            model_revision: 1,
            native_layout_signature: "native-u32".into(),
        },
        descriptor: NativeColorComponentDescriptor {
            binding: NativeColorBinding {
                channel_id: Uuid::from_u128(5),
                function_id: Uuid::from_u128(6),
            },
            raw_from: 0,
            raw_to: u32::MAX,
            continuous: true,
        },
        predictions: AtomicUsize::new(0),
    });
    let address = Arc::new(
        CompiledDynamicValueAddress::new(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::DirectColor {
                    source: model.source.clone(),
                },
                component: Some(ProgrammingComponent::NativeColor(model.descriptor.binding)),
            },
            Some(model.clone()),
        )
        .unwrap(),
    );
    (address, model)
}

#[test]
fn native_u32_interpolation_preserves_low_bits_without_predicting_color() {
    let (address, model) = native();
    let compiled = CompiledComponentExpression::new(
        resume(
            Some(leaf(&address, DynamicValue::Native(0))),
            Some(leaf(&address, DynamicValue::Native(u32::MAX))),
            0.5,
        ),
        address,
    )
    .unwrap();
    assert_eq!(
        compiled.evaluate(None).unwrap(),
        Some(DynamicValue::Native(2_147_483_648))
    );
    assert_eq!(model.predictions.load(Ordering::Relaxed), 0);
}

#[test]
fn mixed_addresses_unsupported_nodes_and_cold_out_of_domain_values_fail() {
    let address = target_z();
    let other = kelvin();
    let whole_focus = compiled(DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Focus,
        component: None,
    });
    for expression in [
        resume(
            Some(leaf(&address, DynamicValue::Scalar(1.0))),
            Some(leaf(&other, DynamicValue::Scalar(3000.0))),
            0.5,
        ),
        Arc::new(E::LegacyScalar {
            attribute: light_core::AttributeKey("focus".into()),
            value: 0.5,
            occurrence: None,
            dependency_occurrence: None,
        }),
        Arc::new(E::Scale {
            address: Arc::new(whole_focus.address().clone()),
            base: DynamicValue::Family(light_core::AttributeValue::Normalized(0.0)),
            value: leaf(
                &whole_focus,
                DynamicValue::Family(light_core::AttributeValue::Normalized(0.5)),
            ),
            factor: 2.0,
            baseline_occurrence: None,
        }),
    ] {
        assert!(matches!(
            CompiledComponentExpression::new(expression, address.clone()),
            Err(TransitionError::Requires(
                TransitionRequirement::CompatibleOwners
            ))
        ));
    }
    assert!(matches!(
        CompiledComponentExpression::new(
            leaf(&other, DynamicValue::Scalar(3000.0)),
            address.clone()
        ),
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners
        ))
    ));
    assert!(
        CompiledComponentExpression::new(leaf(&address, DynamicValue::Native(5)), address.clone())
            .is_err()
    );

    let angles = compiled(DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    });
    assert!(matches!(
        CompiledComponentExpression::new(leaf(&angles, DynamicValue::Scalar(30.0)), angles),
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners
        ))
    ));
}

#[test]
fn flat_retained_history_evaluates_more_than_one_hundred_resumes_without_tree_expansion() {
    let address = target_z();
    let source = leaf(&address, DynamicValue::Scalar(6.0));
    let mut tape = RetainedExpressionTape::from_roots(&[source]).unwrap();
    let mut root = tape.roots()[0];
    for ordinal in 1..=128 {
        let next = RetainedNodeId(tape.nodes.len() as u32);
        tape.nodes.push(RetainedExpressionNode::Programming {
            address: address.address().clone(),
            value: DynamicValue::Scalar(6.0),
            occurrence: None,
            dependency_occurrence: None,
        });
        root = tape
            .append_resume(Some(root), Some(next), 0.5, Uuid::from_u128(ordinal))
            .unwrap();
    }
    tape.replace_root(0, root).unwrap();
    let source = Arc::new(E::Retained {
        tape: Arc::new(tape),
        root,
    });
    let compiled = CompiledComponentExpression::new(source.clone(), address).unwrap();
    assert_eq!(compiled.expression(), source.as_ref());
    assert!(compiled.participates());
    assert!(!compiled.needs_underlay());
    assert_eq!(
        compiled.evaluate(None).unwrap(),
        Some(DynamicValue::Scalar(6.0))
    );
}

#[test]
fn flat_exact_branch_ignores_hidden_incompatible_model_but_keeps_original_history() {
    let address = target_z();
    let hidden = kelvin();
    let mut tape = RetainedExpressionTape::empty();
    tape.nodes.push(RetainedExpressionNode::Programming {
        address: address.address().clone(),
        value: DynamicValue::Scalar(8.0),
        occurrence: None,
        dependency_occurrence: None,
    });
    tape.nodes.push(RetainedExpressionNode::Programming {
        address: hidden.address().clone(),
        value: DynamicValue::Scalar(3000.0),
        occurrence: None,
        dependency_occurrence: None,
    });
    let mut root = RetainedNodeId(0);
    for ordinal in 1..=120 {
        root = tape
            .append_resume(
                Some(root),
                Some(RetainedNodeId(1)),
                0.0,
                Uuid::from_u128(ordinal),
            )
            .unwrap();
    }
    tape.push_root(root).unwrap();
    let retained = Arc::new(E::Retained {
        tape: Arc::new(tape),
        root,
    });
    let compiled = CompiledComponentExpression::new(retained.clone(), address).unwrap();
    assert_eq!(compiled.expression(), retained.as_ref());
    assert_eq!(
        compiled.evaluate(None).unwrap(),
        Some(DynamicValue::Scalar(8.0))
    );
}
