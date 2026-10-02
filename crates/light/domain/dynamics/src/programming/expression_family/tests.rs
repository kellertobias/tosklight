use super::*;
use crate::{
    DynamicFamilyRepresentation, DynamicSampleExpression as E, DynamicTransitionReason,
    DynamicValueAddress,
};
use light_core::{NativeColorBinding, NativeColorValue, PhysicalDataQuality, Xyz};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};
use uuid::Uuid;

fn retained_history(expression: Arc<E>, steps: usize) -> Arc<E> {
    let mut tape = RetainedExpressionTape::from_roots(&[expression]).unwrap();
    let mut root = tape.roots()[0];
    for step in 0..steps {
        root = tape
            .append_resume(
                Some(root),
                Some(root),
                0.5,
                Uuid::from_u128(10_000 + step as u128),
            )
            .unwrap();
    }
    tape.replace_root(0, root).unwrap();
    Arc::new(E::Retained {
        tape: Arc::new(tape),
        root,
    })
}

#[test]
fn more_than_one_hundred_shared_history_steps_keep_point_dependencies_live() {
    let first = Uuid::from_u128(101);
    let second = Uuid::from_u128(102);
    let expression = retained_history(
        required(
            Some(leaf(ProgrammingOwner::Position, target(first))),
            Some(leaf(ProgrammingOwner::Position, target(second))),
            0.5,
            TransitionRequirement::LiveTargetPoints,
        ),
        128,
    );
    let compiled = CompiledProgrammingFamilyExpression::new(
        expression.clone(),
        ProgrammingOwner::Position,
        None,
        None,
    )
    .unwrap();
    assert!(matches!(compiled.expression(), E::Retained { .. }));
    let frame = CoherentFrame {
        point_x: RefCell::new(HashMap::from([(first, 0.0), (second, 10.0)])),
        calls: Cell::new(0),
    };
    assert_eq!(
        compiled.evaluate_optional(None, &frame).unwrap(),
        Some(angles(5.0, 0.0))
    );
    frame.point_x.borrow_mut().insert(second, 30.0);
    assert_eq!(
        compiled.evaluate_optional(None, &frame).unwrap(),
        Some(angles(15.0, 0.0))
    );
    assert_eq!(
        frame.calls.get(),
        2,
        "each shared dependency is evaluated once in each coherent frame"
    );

    let exact = required(
        Some(expression),
        Some(leaf(ProgrammingOwner::Position, angles(7.0, 3.0))),
        1.0,
        TransitionRequirement::LiveJointAngles,
    );
    let exact =
        CompiledProgrammingFamilyExpression::new(exact, ProgrammingOwner::Position, None, None)
            .unwrap();
    frame.point_x.borrow_mut().clear();
    assert_eq!(
        exact.evaluate_optional(None, &frame).unwrap(),
        Some(angles(7.0, 3.0))
    );
    assert_eq!(frame.calls.get(), 2);
}

#[test]
fn deep_native_history_preserves_full_width_values_uv_and_exact_pruning() {
    let model = native_model();
    let expression = retained_history(
        required(
            Some(leaf(ProgrammingOwner::Color, direct(&model, u32::MAX - 2))),
            Some(leaf(ProgrammingOwner::Color, direct(&model, u32::MAX))),
            0.5,
            TransitionRequirement::NativeColorModel,
        ),
        128,
    );
    assert!(matches!(
        CompiledProgrammingFamilyExpression::new(
            expression.clone(),
            ProgrammingOwner::Color,
            None,
            None
        ),
        Err(TransitionError::Requires(
            TransitionRequirement::NativeColorModel
        ))
    ));
    let compiled = CompiledProgrammingFamilyExpression::new(
        expression.clone(),
        ProgrammingOwner::Color,
        None,
        Some(&NativeModels(model.clone())),
    )
    .unwrap();
    let frame = CoherentFrame {
        point_x: RefCell::new(HashMap::new()),
        calls: Cell::new(0),
    };
    let Some(AttributeValue::ColorProgram(value)) =
        compiled.evaluate_optional(None, &frame).unwrap()
    else {
        panic!()
    };
    let ColorProgram::Direct { recipe, portable } = value.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].raw, u32::MAX - 1);
    assert_eq!(recipe.source, model.source);
    assert!(portable.visible.is_none());
    assert_eq!(portable.uv.unwrap().amount, 0.6);
    let released = CompiledProgrammingFamilyExpression::new(
        required(
            Some(expression),
            None,
            1.0,
            TransitionRequirement::NativeColorModel,
        ),
        ProgrammingOwner::Color,
        None,
        None,
    )
    .unwrap();
    assert_eq!(released.evaluate_optional(None, &frame).unwrap(), None);
    assert_eq!(frame.calls.get(), 0);
}

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target(point: Uuid) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point { point_id: point },
        [0.0; 3],
    )))
}
fn leaf(owner: ProgrammingOwner, value: AttributeValue) -> Arc<E> {
    let address = DynamicValueAddress::whole_family(owner, &value).unwrap();
    Arc::new(E::Programming {
        address: Arc::new(address),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn required(
    from: Option<Arc<E>>,
    to: Option<Arc<E>>,
    progress: f32,
    requirement: TransitionRequirement,
) -> Arc<E> {
    Arc::new(E::Transition {
        from,
        to,
        progress,
        reason: DynamicTransitionReason::Required { requirement },
    })
}

struct CoherentFrame {
    point_x: RefCell<HashMap<Uuid, f32>>,
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for CoherentFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        if requirement != TransitionRequirement::LiveTargetPoints {
            return Err(TransitionError::Requires(requirement));
        }
        let position_x = |value: &AttributeValue| -> f32 {
            let AttributeValue::Position(position) = value else {
                panic!()
            };
            let PositionIntent::Target {
                reference: TargetReference::Point { point_id },
                offset_metres,
            } = position.as_ref()
            else {
                panic!()
            };
            let ScalarIntent::Value(offset) = &offset_metres[0] else {
                panic!()
            };
            self.point_x.borrow()[point_id] + *offset
        };
        let FamilyExpressionOperation::Transition { progress } = operation else {
            panic!()
        };
        Ok(angles(
            position_x(from) + (position_x(to) - position_x(from)) * progress,
            0.0,
        ))
    }
}

#[test]
fn paused_retained_target_transition_re_reads_one_coherent_frame() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let expression = required(
        Some(leaf(ProgrammingOwner::Position, target(first))),
        Some(leaf(ProgrammingOwner::Position, target(second))),
        0.5,
        TransitionRequirement::LiveTargetPoints,
    );
    let frame = CoherentFrame {
        point_x: RefCell::new(HashMap::from([(first, 0.0), (second, 10.0)])),
        calls: Cell::new(0),
    };
    let compiled = CompiledProgrammingFamilyExpression::new(
        expression.clone(),
        ProgrammingOwner::Position,
        None,
        None,
    )
    .unwrap();
    assert_eq!(compiled.expression(), expression.as_ref());
    assert_eq!(
        compiled.evaluate_optional(None, &frame).unwrap(),
        Some(angles(5.0, 0.0))
    );
    frame.point_x.borrow_mut().insert(second, 30.0);
    assert_eq!(
        compiled.evaluate_optional(None, &frame).unwrap(),
        Some(angles(15.0, 0.0))
    );
    assert_eq!(frame.calls.get(), 2);
}

#[test]
fn scale_and_exact_endpoints_skip_irrelevant_live_dependencies() {
    let frame = CoherentFrame {
        point_x: RefCell::new(HashMap::new()),
        calls: Cell::new(0),
    };
    let base = angles(0.0, 0.0);
    let scaled = Arc::new(E::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &base).unwrap(),
        ),
        base: DynamicValue::Family(base.clone()),
        value: leaf(ProgrammingOwner::Position, angles(10.0, 20.0)),
        factor: 2.0,
        baseline_occurrence: None,
    });
    let compiled =
        CompiledProgrammingFamilyExpression::new(scaled, ProgrammingOwner::Position, None, None)
            .unwrap();
    assert_eq!(
        compiled.evaluate_optional(None, &frame).unwrap(),
        Some(angles(20.0, 40.0))
    );
    assert_eq!(frame.calls.get(), 0);

    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let live = required(
        Some(leaf(ProgrammingOwner::Position, target(first))),
        Some(leaf(ProgrammingOwner::Position, target(second))),
        0.5,
        TransitionRequirement::LiveTargetPoints,
    );
    for (progress, expected) in [(0.0, angles(1.0, 2.0)), (1.0, angles(3.0, 4.0))] {
        let expression = required(
            Some(leaf(ProgrammingOwner::Position, angles(1.0, 2.0))),
            Some(leaf(ProgrammingOwner::Position, angles(3.0, 4.0))),
            progress,
            TransitionRequirement::LiveTargetPoints,
        );
        let compiled = CompiledProgrammingFamilyExpression::new(
            expression,
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            compiled.evaluate_optional(None, &frame).unwrap(),
            Some(expected)
        );
    }
    let target_base = target(first);
    let exact_zero = Arc::new(E::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &target_base).unwrap(),
        ),
        base: DynamicValue::Family(target_base.clone()),
        value: live,
        factor: 0.0,
        baseline_occurrence: None,
    });
    // The nested moved-Point requirement is absent from this exact-zero operation.
    let compiled = CompiledProgrammingFamilyExpression::new(
        exact_zero,
        ProgrammingOwner::Position,
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        compiled.evaluate_optional(None, &frame).unwrap(),
        Some(target_base)
    );
    assert_eq!(frame.calls.get(), 0);

    let underlay_transition = required(
        None,
        Some(leaf(ProgrammingOwner::Position, angles(20.0, 0.0))),
        0.5,
        TransitionRequirement::LiveJointAngles,
    );
    let compiled = CompiledProgrammingFamilyExpression::new(
        underlay_transition,
        ProgrammingOwner::Position,
        Some(&angles(4.0, 0.0)),
        None,
    )
    .unwrap();
    assert_eq!(compiled.owner(), ProgrammingOwner::Position);
    assert!(compiled.participates());
    assert!(compiled.needs_underlay());
    assert!(matches!(
        compiled.evaluate_optional(None, &frame),
        Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints
        ))
    ));
    assert_eq!(
        compiled
            .evaluate_optional(Some(&angles(8.0, 0.0)), &frame)
            .unwrap(),
        Some(angles(14.0, 0.0))
    );
}

#[test]
fn exact_whole_release_and_nested_empty_branches_own_nothing() {
    let frame = CoherentFrame {
        point_x: RefCell::new(HashMap::new()),
        calls: Cell::new(0),
    };
    let value = leaf(ProgrammingOwner::Position, angles(90.0, 10.0));
    let left = required(
        None,
        Some(value.clone()),
        0.0,
        TransitionRequirement::LiveJointAngles,
    );
    let right = required(
        Some(value),
        None,
        1.0,
        TransitionRequirement::LiveJointAngles,
    );
    let expression = required(
        Some(left),
        Some(right),
        0.5,
        TransitionRequirement::LiveJointAngles,
    );
    let compiled = CompiledProgrammingFamilyExpression::new(
        expression.clone(),
        ProgrammingOwner::Position,
        None,
        None,
    )
    .unwrap();
    assert_eq!(compiled.expression(), expression.as_ref());
    assert!(!compiled.participates());
    assert!(!compiled.needs_underlay());
    assert_eq!(compiled.evaluate_optional(None, &frame).unwrap(), None);
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn interrupted_nested_resume_keeps_every_branch_and_rejects_component_tokens() {
    let frame = CoherentFrame {
        point_x: RefCell::new(HashMap::new()),
        calls: Cell::new(0),
    };
    let inner = Arc::new(E::Transition {
        from: Some(leaf(ProgrammingOwner::Position, angles(0.0, 0.0))),
        to: Some(leaf(ProgrammingOwner::Position, angles(10.0, 0.0))),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::new_v4(),
        },
    });
    let outer = Arc::new(E::Transition {
        from: Some(inner),
        to: Some(leaf(ProgrammingOwner::Position, angles(20.0, 0.0))),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::new_v4(),
        },
    });
    let compiled =
        CompiledProgrammingFamilyExpression::new(outer, ProgrammingOwner::Position, None, None)
            .unwrap();
    assert_eq!(
        compiled.evaluate_optional(None, &frame).unwrap(),
        Some(angles(12.5, 0.0))
    );

    let component = Arc::new(E::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        }),
        value: DynamicValue::Scalar(45.0),
        occurrence: None,
        dependency_occurrence: None,
    });
    assert!(matches!(
        CompiledProgrammingFamilyExpression::new(component, ProgrammingOwner::Position, None, None),
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners
        ))
    ));
    let current = Arc::new(E::AngleCurrent {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Tilt),
        }),
    });
    assert!(matches!(
        CompiledProgrammingFamilyExpression::new(current, ProgrammingOwner::Position, None, None),
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners
        ))
    ));
    let cross_owner = required(
        Some(leaf(ProgrammingOwner::Position, angles(0.0, 0.0))),
        Some(leaf(
            ProgrammingOwner::Color,
            AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                intent: ColorIntent::default(),
            })),
        )),
        0.5,
        TransitionRequirement::CompatibleOwners,
    );
    assert!(matches!(
        CompiledProgrammingFamilyExpression::new(
            cross_owner,
            ProgrammingOwner::Position,
            None,
            None
        ),
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners
        ))
    ));
}

struct NativeModel {
    source: NativeColorIdentity,
    binding: NativeColorBinding,
    predictions: AtomicUsize,
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding == self.binding).then_some(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: u32::MAX,
            continuous: true,
        })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        if recipe.source != self.source || recipe.channels.len() != 1 {
            return Err(IntentError("incomplete original native source".into()));
        }
        self.predictions.fetch_add(1, Ordering::Relaxed);
        Ok(PortableColorEstimate {
            model_revision: self.source.model_revision,
            visible: None,
            uv: Some(PortableUv {
                amount: 0.6,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec!["visible response unknown".into()],
        })
    }
}
struct NativeModels(Arc<NativeModel>);
impl DynamicNativeModelResolver for NativeModels {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        if source != &self.0.source {
            return Err(IntentError("pinned original source unavailable".into()));
        }
        Ok(self.0.clone())
    }
}
fn native_model() -> Arc<NativeModel> {
    Arc::new(NativeModel {
        source: NativeColorIdentity {
            profile_id: Uuid::new_v4(),
            profile_revision: 3,
            profile_digest: "pinned-original".into(),
            mode_id: Uuid::new_v4(),
            head_id: Uuid::new_v4(),
            path_id: Uuid::new_v4(),
            model_revision: 7,
            native_layout_signature: "u32-uv".into(),
        },
        binding: NativeColorBinding {
            channel_id: Uuid::new_v4(),
            function_id: Uuid::new_v4(),
        },
        predictions: AtomicUsize::new(0),
    })
}
fn direct(model: &NativeModel, raw: u32) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: model.source.clone(),
            channels: vec![NativeColorValue {
                channel_id: model.binding.channel_id,
                function_id: model.binding.function_id,
                raw,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: model.source.model_revision,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
        },
    }))
}
#[test]
fn pinned_native_tree_keeps_integer_low_bits_and_unknown_visible_with_known_uv() {
    let model = native_model();
    let from = direct(&model, 0);
    let to = direct(&model, u32::MAX);
    let expression = required(
        Some(leaf(ProgrammingOwner::Color, from.clone())),
        Some(leaf(ProgrammingOwner::Color, to.clone())),
        0.5,
        TransitionRequirement::NativeColorModel,
    );
    let frame = CoherentFrame {
        point_x: RefCell::new(HashMap::new()),
        calls: Cell::new(0),
    };
    let compiled = CompiledProgrammingFamilyExpression::new(
        expression,
        ProgrammingOwner::Color,
        None,
        Some(&NativeModels(model.clone())),
    )
    .unwrap();
    let Some(AttributeValue::ColorProgram(result)) =
        compiled.evaluate_optional(None, &frame).unwrap()
    else {
        panic!()
    };
    let ColorProgram::Direct { recipe, portable } = result.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].raw, 2_147_483_648);
    assert_eq!(recipe.source, model.source);
    assert_eq!(portable.visible, None);
    assert_eq!(portable.uv.unwrap().amount, 0.6);
    assert_eq!(model.predictions.load(Ordering::Relaxed), 3);
    assert_eq!(
        compiled
            .resolve_original_native_model(&model.source)
            .unwrap()
            .source(),
        &model.source
    );
    assert_eq!(frame.calls.get(), 0);

    let exact = required(
        Some(leaf(ProgrammingOwner::Color, to.clone())),
        Some(leaf(ProgrammingOwner::Color, from.clone())),
        0.0,
        TransitionRequirement::NativeColorModel,
    );
    let compiled = CompiledProgrammingFamilyExpression::new(
        exact,
        ProgrammingOwner::Color,
        None,
        Some(&NativeModels(model.clone())),
    )
    .unwrap();
    assert_eq!(compiled.evaluate_optional(None, &frame).unwrap(), Some(to));
    assert_eq!(model.predictions.load(Ordering::Relaxed), 4);
    let wrong = native_model();
    let failing = required(
        Some(leaf(ProgrammingOwner::Color, from.clone())),
        Some(leaf(ProgrammingOwner::Color, direct(&model, 100))),
        0.5,
        TransitionRequirement::NativeColorModel,
    );
    assert!(
        CompiledProgrammingFamilyExpression::new(
            failing,
            ProgrammingOwner::Color,
            None,
            Some(&NativeModels(wrong)),
        )
        .is_err()
    );
}

#[test]
fn surviving_direct_leaf_is_verified_once_and_pruned_direct_needs_no_model() {
    let model = native_model();
    let value = direct(&model, u32::MAX - 1);
    let frame = CoherentFrame {
        point_x: RefCell::new(HashMap::new()),
        calls: Cell::new(0),
    };
    let expression = leaf(ProgrammingOwner::Color, value.clone());
    assert!(matches!(
        CompiledProgrammingFamilyExpression::new(
            expression.clone(),
            ProgrammingOwner::Color,
            None,
            None,
        ),
        Err(TransitionError::Requires(
            TransitionRequirement::NativeColorModel
        ))
    ));
    let compiled = CompiledProgrammingFamilyExpression::new(
        expression,
        ProgrammingOwner::Color,
        None,
        Some(&NativeModels(model.clone())),
    )
    .unwrap();
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);
    for _ in 0..2 {
        assert_eq!(
            compiled.evaluate_optional(None, &frame).unwrap(),
            Some(value.clone())
        );
    }
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);

    let release = required(
        Some(leaf(ProgrammingOwner::Color, value.clone())),
        None,
        1.0,
        TransitionRequirement::NativeColorModel,
    );
    let compiled = CompiledProgrammingFamilyExpression::new(
        release,
        ProgrammingOwner::Color,
        Some(&value),
        None,
    )
    .unwrap();
    assert!(!compiled.participates());
    assert_eq!(compiled.evaluate_optional(None, &frame).unwrap(), None);
}

#[test]
fn frame_returned_direct_is_verified_against_pinned_original_source() {
    struct ColorFrame(AttributeValue);
    impl WholeFamilyExpressionFrameResolver for ColorFrame {
        fn resolve(
            &self,
            requirement: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            assert_eq!(requirement, TransitionRequirement::ColorAppearance);
            Ok(self.0.clone())
        }
    }
    let model = native_model();
    let semantic = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    let direct_leaf = direct(&model, 100);
    let expression = required(
        Some(leaf(ProgrammingOwner::Color, semantic.clone())),
        Some(leaf(ProgrammingOwner::Color, direct_leaf)),
        0.5,
        TransitionRequirement::ColorAppearance,
    );
    let compiled = CompiledProgrammingFamilyExpression::new(
        expression,
        ProgrammingOwner::Color,
        None,
        Some(&NativeModels(model.clone())),
    )
    .unwrap();
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);
    let resolved = direct(&model, 200);
    assert_eq!(
        compiled
            .evaluate_optional(None, &ColorFrame(resolved.clone()))
            .unwrap(),
        Some(resolved.clone())
    );
    assert_eq!(model.predictions.load(Ordering::Relaxed), 2);
    assert_eq!(
        compiled
            .transition(&semantic, &resolved, 0.0, &ColorFrame(resolved.clone()))
            .unwrap(),
        semantic
    );

    let foreign = native_model();
    let foreign_value = direct(&foreign, 300);
    assert!(matches!(
        compiled.evaluate_optional(None, &ColorFrame(foreign_value)),
        Err(TransitionError::Requires(
            TransitionRequirement::NativeColorModel
        ))
    ));
}

#[test]
fn semantic_activation_can_use_portable_direct_underlay_without_its_source_model() {
    struct SemanticFrame(AttributeValue);
    impl WholeFamilyExpressionFrameResolver for SemanticFrame {
        fn resolve(
            &self,
            requirement: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            assert_eq!(requirement, TransitionRequirement::ColorAppearance);
            Ok(self.0.clone())
        }
    }
    let model = native_model();
    let mut native = direct(&model, 17);
    let AttributeValue::ColorProgram(color) = &mut native else {
        panic!()
    };
    let ColorProgram::Direct { portable, .. } = Arc::make_mut(color) else {
        panic!()
    };
    portable.visible = Some(PortableVisibleColor {
        xyz: Xyz {
            x: 0.2,
            y: 0.3,
            z: 0.4,
        },
        relative_output: 0.5,
    });
    portable.quality = PhysicalDataQuality::Estimated;

    let semantic = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    let compiled = CompiledProgrammingFamilyExpression::new(
        leaf(ProgrammingOwner::Color, semantic.clone()),
        ProgrammingOwner::Color,
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        compiled
            .transition(&native, &semantic, 0.5, &SemanticFrame(semantic.clone()))
            .unwrap(),
        semantic
    );
}
