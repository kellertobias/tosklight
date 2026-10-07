use super::*;
use crate::{DynamicSemanticColorBasis as Basis, DynamicTransitionReason, DynamicValue};
use light_core::{
    NativeColorBinding, NativeColorIdentity,
    programming::{
        ColorComponent, IntentError, NativeColorComponentDescriptor, NativeColorRecipe,
        PortableColorEstimate, TargetReference,
    },
};
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

fn leaf(address: DynamicValueAddress, value: DynamicValue) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(address),
        value,
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn color(component: ColorComponent, value: f32) -> Arc<DynamicSampleExpression> {
    let basis = match component {
        ColorComponent::Red
        | ColorComponent::Green
        | ColorComponent::Blue
        | ColorComponent::Amber => Basis::Recipe,
        ColorComponent::Hue | ColorComponent::Saturation => Basis::HueSaturation,
        _ => Basis::Retain,
    };
    leaf(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor { basis },
            component: Some(ProgrammingComponent::Color(component)),
        },
        DynamicValue::Scalar(value),
    )
}

fn resume(
    from: Option<Arc<DynamicSampleExpression>>,
    to: Option<Arc<DynamicSampleExpression>>,
    progress: f32,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from,
        to,
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::new_v4(),
        },
    })
}

fn scalar(set: &CompiledComponentExpressionSet, component: ColorComponent, underlay: f32) -> f32 {
    let projection = set
        .components()
        .iter()
        .find(|value| {
            value.address().address().component == Some(ProgrammingComponent::Color(component))
        })
        .unwrap();
    let Some(DynamicValue::Scalar(value)) = projection
        .evaluate(Some(&DynamicValue::Scalar(underlay)))
        .unwrap()
    else {
        panic!("scalar component")
    };
    value
}

#[test]
fn uv_to_white_blend_projects_independent_masks_against_their_actual_underlays() {
    let expression = resume(
        Some(color(ColorComponent::Uv, 0.8)),
        Some(color(ColorComponent::WhiteBlend, 1.0)),
        0.25,
    );
    let set = CompiledComponentExpressionSet::new(expression.clone(), None).unwrap();
    assert!(Arc::ptr_eq(set.expression(), &expression));
    assert_eq!(set.components().len(), 2);
    assert!((scalar(&set, ColorComponent::Uv, 0.2) - 0.65).abs() < 0.00001);
    assert!((scalar(&set, ColorComponent::WhiteBlend, 0.1) - 0.325).abs() < 0.00001);
    for projection in set.components() {
        assert!(projection.needs_underlay());
        assert!(projection.address().address().component.is_some());
        let DynamicSampleExpression::Retained { tape, root } = projection.expression() else {
            panic!("flat retained projection")
        };
        let RetainedExpressionNode::Transition {
            reason, progress, ..
        } = tape.node(*root).unwrap()
        else {
            panic!("retained transition")
        };
        let DynamicSampleExpression::Transition {
            reason: original, ..
        } = expression.as_ref()
        else {
            panic!()
        };
        assert_eq!(reason, original);
        assert_eq!(*progress, 0.25);
    }
}

#[test]
fn recipe_components_and_retained_orthogonals_share_a_compatible_family() {
    let expression = resume(
        Some(resume(
            Some(color(ColorComponent::Red, 0.8)),
            Some(color(ColorComponent::Green, 0.6)),
            0.5,
        )),
        Some(color(ColorComponent::Uv, 1.0)),
        0.5,
    );
    let set = CompiledComponentExpressionSet::new(expression, None).unwrap();
    assert_eq!(set.components().len(), 3);
    assert!((scalar(&set, ColorComponent::Red, 0.0) - 0.2).abs() < 0.00001);
    assert!((scalar(&set, ColorComponent::Green, 0.0) - 0.15).abs() < 0.00001);
    assert!((scalar(&set, ColorComponent::Uv, 0.0) - 0.5).abs() < 0.00001);
}

#[test]
fn reciprocal_temperature_and_shortest_arc_hue_keep_their_own_interpolation() {
    let set = CompiledComponentExpressionSet::new(
        resume(
            Some(color(ColorComponent::Temperature, 2000.0)),
            Some(color(ColorComponent::Uv, 0.8)),
            0.5,
        ),
        None,
    )
    .unwrap();
    assert!((scalar(&set, ColorComponent::Temperature, 10000.0) - 3333.3333).abs() < 0.001);
    assert!((scalar(&set, ColorComponent::Uv, 0.2) - 0.5).abs() < 0.00001);
    let set = CompiledComponentExpressionSet::new(
        resume(
            Some(color(ColorComponent::Hue, 10.0)),
            Some(color(ColorComponent::Saturation, 1.0)),
            0.5,
        ),
        None,
    )
    .unwrap();
    assert!(scalar(&set, ColorComponent::Hue, 350.0).abs() < 0.00001);
    assert!((scalar(&set, ColorComponent::Saturation, 0.2) - 0.6).abs() < 0.00001);
}

#[test]
fn nested_exact_absence_removes_only_evaluation_footprints_and_keeps_authored_membership() {
    let released = resume(Some(color(ColorComponent::Uv, 0.8)), None, 1.0);
    let incoming = resume(None, Some(color(ColorComponent::WhiteBlend, 0.7)), 0.0);
    let expression = resume(Some(released), Some(incoming), 0.5);
    let set = CompiledComponentExpressionSet::new(expression.clone(), None).unwrap();
    assert!(set.components().is_empty());
    assert!(Arc::ptr_eq(set.expression(), &expression));

    let expression = resume(
        Some(color(ColorComponent::Uv, 0.8)),
        Some(color(ColorComponent::WhiteBlend, 0.7)),
        1.0,
    );
    let set = CompiledComponentExpressionSet::new(expression, None).unwrap();
    assert_eq!(set.components().len(), 1);
    assert_eq!(
        set.components()[0].address().address().component,
        Some(ProgrammingComponent::Color(ColorComponent::WhiteBlend))
    );
    assert!(!set.components()[0].needs_underlay());
    // Cold shape/domain validation still covers the original invisible endpoint.
    assert!(
        CompiledComponentExpressionSet::new(
            resume(Some(color(ColorComponent::Uv, 2.0)), None, 1.0),
            None
        )
        .is_err()
    );
}

struct Model {
    source: NativeColorIdentity,
    bindings: [NativeColorBinding; 2],
}
impl NativeColorEditModel for Model {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        self.bindings
            .contains(&binding)
            .then_some(NativeColorComponentDescriptor {
                binding,
                raw_from: 0,
                raw_to: u32::MAX,
                continuous: true,
            })
    }
    fn predict(&self, _: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        panic!("component projection must not predict complete Color")
    }
}
struct Models {
    model: Arc<Model>,
    calls: AtomicUsize,
}
impl DynamicNativeModelResolver for Models {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert_eq!(source, &self.model.source);
        Ok(self.model.clone())
    }
}
fn models() -> Models {
    Models {
        calls: AtomicUsize::new(0),
        model: Arc::new(Model {
            source: NativeColorIdentity {
                profile_id: Uuid::from_u128(1),
                profile_revision: 2,
                profile_digest: "original".into(),
                mode_id: Uuid::from_u128(2),
                head_id: Uuid::from_u128(3),
                path_id: Uuid::from_u128(4),
                model_revision: 1,
                native_layout_signature: "layout".into(),
            },
            bindings: [5, 6].map(|id| NativeColorBinding {
                channel_id: Uuid::from_u128(id),
                function_id: Uuid::from_u128(id + 10),
            }),
        }),
    }
}
fn native(model: &Model, index: usize, raw: u32) -> Arc<DynamicSampleExpression> {
    leaf(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: model.source.clone(),
            },
            component: Some(ProgrammingComponent::NativeColor(model.bindings[index])),
        },
        DynamicValue::Native(raw),
    )
}

#[test]
fn native_components_preserve_u32_low_bits_and_resolve_the_original_model_once() {
    let models = models();
    let expression = resume(
        Some(native(&models.model, 0, u32::MAX - 2)),
        Some(native(&models.model, 1, u32::MAX - 1)),
        0.5,
    );
    let set = CompiledComponentExpressionSet::new(expression, Some(&models)).unwrap();
    assert_eq!(models.calls.load(Ordering::Relaxed), 1);
    for (index, underlay, expected) in [
        (0, u32::MAX - 4, u32::MAX - 3),
        (1, u32::MAX - 3, u32::MAX - 2),
    ] {
        let projection = set
            .components()
            .iter()
            .find(|value| {
                value.address().address().component
                    == Some(ProgrammingComponent::NativeColor(
                        models.model.bindings[index],
                    ))
            })
            .unwrap();
        assert_eq!(
            projection
                .evaluate(Some(&DynamicValue::Native(underlay)))
                .unwrap(),
            Some(DynamicValue::Native(expected))
        );
    }
    assert_eq!(models.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn fully_pruned_native_endpoints_do_not_require_an_available_original_model() {
    let models = models();
    let expression = resume(Some(native(&models.model, 0, 123)), None, 1.0);
    let set = CompiledComponentExpressionSet::new(expression.clone(), None).unwrap();
    assert!(set.components().is_empty());
    assert!(Arc::ptr_eq(set.expression(), &expression));
    assert!(
        CompiledComponentExpressionSet::new(expression, Some(&models))
            .unwrap()
            .components()
            .is_empty()
    );
    assert_eq!(models.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn competing_bases_frames_sources_and_native_functions_require_a_coupled_operation() {
    let origin_x = leaf(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Origin),
            },
            component: Some(ProgrammingComponent::TargetX),
        },
        DynamicValue::Scalar(1.0),
    );
    let other_y = leaf(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Point {
                    point_id: Uuid::from_u128(1),
                }),
            },
            component: Some(ProgrammingComponent::TargetY),
        },
        DynamicValue::Scalar(2.0),
    );
    let uv_recipe = leaf(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: Basis::Recipe,
            },
            component: Some(ProgrammingComponent::Color(ColorComponent::Uv)),
        },
        DynamicValue::Scalar(0.5),
    );
    let models = models();
    let mut other_source = models.model.source.clone();
    other_source.profile_revision += 1;
    let different_source = leaf(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: other_source,
            },
            component: Some(ProgrammingComponent::NativeColor(models.model.bindings[1])),
        },
        DynamicValue::Native(1),
    );
    let different_function = leaf(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: models.model.source.clone(),
            },
            component: Some(ProgrammingComponent::NativeColor(NativeColorBinding {
                channel_id: models.model.bindings[0].channel_id,
                function_id: Uuid::from_u128(99),
            })),
        },
        DynamicValue::Native(1),
    );
    for expression in [
        resume(
            Some(color(ColorComponent::Red, 0.5)),
            Some(color(ColorComponent::Hue, 90.0)),
            0.5,
        ),
        resume(
            Some(color(ColorComponent::Uv, 0.5)),
            Some(resume(
                Some(color(ColorComponent::Red, 0.5)),
                Some(color(ColorComponent::Hue, 90.0)),
                0.5,
            )),
            0.5,
        ),
        resume(Some(origin_x), Some(other_y), 0.5),
        resume(Some(color(ColorComponent::Uv, 0.5)), Some(uv_recipe), 0.5),
        resume(
            Some(native(&models.model, 0, 1)),
            Some(different_source),
            0.5,
        ),
        resume(
            Some(native(&models.model, 0, 1)),
            Some(different_function),
            0.5,
        ),
    ] {
        assert!(matches!(
            CompiledComponentExpressionSet::new(expression, Some(&models)),
            Err(TransitionError::Requires(
                TransitionRequirement::CompatibleOwners
            ))
        ));
    }
    assert_eq!(models.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn long_retained_history_projects_flat_component_roots_with_original_identity() {
    let expression = resume(
        Some(color(ColorComponent::Uv, 0.8)),
        Some(color(ColorComponent::WhiteBlend, 0.6)),
        0.5,
    );
    let mut tape = RetainedExpressionTape::from_roots(&[expression]).unwrap();
    let mut root = tape.roots()[0];
    for ordinal in 1..=128 {
        root = tape
            .append_resume(Some(root), Some(root), 0.5, Uuid::from_u128(ordinal))
            .unwrap();
    }
    tape.replace_root(0, root).unwrap();
    let retained = Arc::new(DynamicSampleExpression::Retained {
        tape: Arc::new(tape),
        root,
    });
    let set = CompiledComponentExpressionSet::new(retained.clone(), None).unwrap();
    assert!(Arc::ptr_eq(set.expression(), &retained));
    assert_eq!(set.components().len(), 2);
    assert!((scalar(&set, ColorComponent::Uv, 0.2) - 0.5).abs() < 0.00001);
    assert!((scalar(&set, ColorComponent::WhiteBlend, 0.1) - 0.35).abs() < 0.00001);
    for component in set.components() {
        let DynamicSampleExpression::Retained { tape, root } = component.expression() else {
            panic!("projection must remain a flat retained tape")
        };
        assert_eq!(tape.roots(), &[*root]);
        assert!(tape.nodes.len() > 100);
    }
}
