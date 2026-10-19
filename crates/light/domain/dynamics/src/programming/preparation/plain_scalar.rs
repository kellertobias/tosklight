//! Direct lowering of selected plain semantic Color scalar leaves. The slots describe only
//! immutable semantic addresses/domains: no target, generation, Current, native model, value,
//! activation or provenance is cached. All other expression shapes use general preparation.
use super::*;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
#[repr(usize)]
enum Slot {
    RecipeRed,
    RecipeGreen,
    RecipeBlue,
    RecipeAmber,
    RecipeWhiteBlend,
    RetainUv,
}

impl Slot {
    fn address(self) -> DynamicValueAddress {
        use ColorComponent as C;
        let (basis, component) = match self {
            Self::RecipeRed => (DynamicSemanticColorBasis::Recipe, C::Red),
            Self::RecipeGreen => (DynamicSemanticColorBasis::Recipe, C::Green),
            Self::RecipeBlue => (DynamicSemanticColorBasis::Recipe, C::Blue),
            Self::RecipeAmber => (DynamicSemanticColorBasis::Recipe, C::Amber),
            Self::RecipeWhiteBlend => (DynamicSemanticColorBasis::Recipe, C::WhiteBlend),
            Self::RetainUv => (DynamicSemanticColorBasis::Retain, C::Uv),
        };
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor { basis },
            component: Some(ProgrammingComponent::Color(component)),
        }
    }

    fn compiled(self) -> Result<&'static Arc<CompiledDynamicValueAddress>, IntentError> {
        static ADDRESSES: [OnceLock<Result<Arc<CompiledDynamicValueAddress>, IntentError>>; 6] =
            [const { OnceLock::new() }; 6];
        ADDRESSES[self as usize]
            .get_or_init(|| CompiledDynamicValueAddress::new(self.address(), None).map(Arc::new))
            .as_ref()
            .map_err(Clone::clone)
    }
}

pub(super) struct Leaf<'a> {
    slot: Slot,
    value: f32,
    occurrence: Option<crate::DynamicSourceOccurrenceId>,
    dependency: Option<&'a crate::DynamicSourceDependency>,
}

/// Match the complete representation/component pair. In particular, semantic Retain is not
/// native Direct, and neither whole-family Color nor transparent retained wrappers match.
pub(super) fn leaf(expression: &DynamicSampleExpression) -> Option<Leaf<'_>> {
    let DynamicSampleExpression::Programming {
        address,
        value: DynamicValue::Scalar(value),
        occurrence,
        dependency_occurrence,
    } = expression
    else {
        return None;
    };
    use ColorComponent as C;
    use DynamicSemanticColorBasis as B;
    let DynamicFamilyRepresentation::SemanticColor { basis } = &address.representation else {
        return None;
    };
    let Some(ProgrammingComponent::Color(component)) = address.component else {
        return None;
    };
    let slot = match (*basis, component) {
        (B::Recipe, C::Red) => Slot::RecipeRed,
        (B::Recipe, C::Green) => Slot::RecipeGreen,
        (B::Recipe, C::Blue) => Slot::RecipeBlue,
        (B::Recipe, C::Amber) => Slot::RecipeAmber,
        (B::Recipe, C::WhiteBlend) => Slot::RecipeWhiteBlend,
        (B::Retain, C::Uv) => Slot::RetainUv,
        _ => return None,
    };
    Some(Leaf {
        slot,
        value: *value,
        occurrence: *occurrence,
        dependency: dependency_occurrence.as_ref(),
    })
}

impl Leaf<'_> {
    /// The unchanged source constructor checks each current rank, numeric value and influence.
    /// Original dependency validation is retained at the controller's expression boundary.
    pub(super) fn materialize(
        self,
        rank: FamilySampleRank,
        activation: f32,
        provenance: &mut super::super::composition::LeafProvenance<'_>,
    ) -> Result<FamilySample, IntentError> {
        Ok(FamilySample::with_address(
            std::borrow::Cow::Borrowed(self.slot.compiled()?),
            DynamicValue::Scalar(self.value),
            rank,
            activation,
        )?
        .with_interned_leaf_provenance(self.occurrence, self.dependency, provenance))
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{FourChunks, Sources, color, compose, sample, semantic};
    use super::*;
    use crate::DynamicSampleExpression as E;

    fn wrapped(expression: &E) -> E {
        E::Operation {
            origin: None,
            value: Arc::new(expression.clone()),
        }
    }

    fn selected(component: ColorComponent, value: f32) -> E {
        let mut expression = color(component, value);
        if component == ColorComponent::WhiteBlend {
            let E::Programming { address, .. } = &mut expression else {
                unreachable!()
            };
            Arc::make_mut(address).representation = DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Recipe,
            };
        }
        expression
    }

    fn all_components() -> [ColorComponent; 6] {
        [
            ColorComponent::Red,
            ColorComponent::Green,
            ColorComponent::Blue,
            ColorComponent::Amber,
            ColorComponent::WhiteBlend,
            ColorComponent::Uv,
        ]
    }

    fn observed(prepared: &PreparedDynamicFamilySamples<'_>) -> (Vec<AttributeValue>, Vec<String>) {
        let base = semantic(ColorIntent::default());
        let values = prepared
            .families
            .iter()
            .map(|group| compose(group, &base).unwrap())
            .collect();
        let trace = prepared
            .families
            .iter()
            .map(|group| {
                let mut scratch = RetainedFamilyCompositionScratch::default();
                compose_retained_dynamic_family_traced(
                    group.owner,
                    &base,
                    &group.samples,
                    &FamilyCompositionContext {
                        edit: FamilyEditContext {
                            color_model: Some(&VirtualColorAuthoringV1),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    &super::super::tests::Frame,
                    &mut scratch,
                )
                .unwrap();
                let trace = scratch.family_trace();
                let root = trace.root().unwrap();
                all_components()
                    .into_iter()
                    .map(|component| {
                        let fields = ProgrammingFieldScope::from_component(
                            ProgrammingComponent::Color(component),
                        );
                        format!("{:?}", trace.query_fields(root, &fields))
                    })
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();
        (values, trace)
    }

    #[test]
    fn changing_plain_color_values_use_slots_without_expression_cache_entries() {
        // Fail-first: with the prepare_sample hook disabled, these ordinary leaves populate
        // scratch.cache. This asserts the structural removal, not a timing improvement.
        let mut scratch = DynamicFamilyPreparationScratch::default();
        let mut first = None;
        for (index, value) in [0.2, 0.8, 0.4].into_iter().enumerate() {
            let mut input = sample(10, selected(ColorComponent::Red, value));
            input.priority += index as i16;
            input.activation_mix = 1.0 - index as f32 * 0.2;
            {
                let prepared = prepare_dynamic_family_samples(
                    &[input.clone()],
                    &Sources::default(),
                    None,
                    &mut scratch,
                )
                .unwrap();
                let FamilyCompositionSample::Known(known) = &prepared.families[0].samples[0] else {
                    panic!()
                };
                assert_eq!(
                    known.materialized_value(),
                    Some(&DynamicValue::Scalar(value))
                );
                assert_eq!(known.rank, rank(&input));
                assert_eq!(known.activation_mix, input.activation_mix);
                let address = known.address() as *const _;
                if let Some(previous) = first {
                    assert_eq!(address, previous);
                } else {
                    first = Some(address);
                }
            }
            assert!(
                scratch.cache.is_empty(),
                "plain changing values must not retain compiled trees"
            );
        }
    }

    #[test]
    fn scalar_slots_match_general_lowering_with_fresh_provenance_and_parallel_order() {
        let mut fast = DynamicFamilyPreparationScratch::default();
        let mut reference = DynamicFamilyPreparationScratch::default();
        let mut parallel = DynamicFamilyPreparationScratch::default();
        for (frame, value) in [0.2, 0.8, 0.4].into_iter().enumerate() {
            let inputs = all_components()
                .into_iter()
                .enumerate()
                .map(|(index, component)| {
                    let mut expression = selected(component, value);
                    let E::Programming {
                        address,
                        occurrence,
                        dependency_occurrence,
                        ..
                    } = &mut expression
                    else {
                        unreachable!()
                    };
                    *occurrence = Some(
                        crate::DynamicSourceOccurrenceId::new(Uuid::from_u128(100 + frame as u128))
                            .unwrap(),
                    );
                    *dependency_occurrence = Some(crate::DynamicSourceDependency::compatible(
                        Some(
                            crate::DynamicSourceOccurrenceId::new(Uuid::from_u128(
                                200 + frame as u128,
                            ))
                            .unwrap(),
                        ),
                        address,
                    ));
                    let mut input = sample(10 + index as u128, expression);
                    input.target = FixtureId(Uuid::from_u128(300 + index as u128));
                    input.priority += frame as i16;
                    input.activation_mix = 1.0 - frame as f32 * 0.2;
                    input
                })
                .collect::<Vec<_>>();
            let refs = inputs
                .iter()
                .cloned()
                .map(|mut sample| {
                    sample.expression = wrapped(&sample.expression);
                    sample
                })
                .collect::<Vec<_>>();
            let expected =
                prepare_dynamic_family_samples(&refs, &Sources::default(), None, &mut reference)
                    .unwrap();
            let actual =
                prepare_dynamic_family_samples(&inputs, &Sources::default(), None, &mut fast)
                    .unwrap();
            let worker = prepare_dynamic_family_samples_in_parallel(
                &inputs,
                &[],
                &Sources::default(),
                None,
                &mut parallel,
                &mut FourChunks,
            )
            .unwrap();
            assert_eq!(actual.requirements, expected.requirements);
            assert_eq!(worker.requirements, expected.requirements);
            assert_eq!(observed(&actual), observed(&expected));
            assert_eq!(observed(&worker), observed(&expected));
            assert_eq!(
                actual
                    .families
                    .iter()
                    .map(|g| (g.target, g.owner))
                    .collect::<Vec<_>>(),
                expected
                    .families
                    .iter()
                    .map(|g| (g.target, g.owner))
                    .collect::<Vec<_>>()
            );
            assert!(fast.cache.is_empty() && parallel.cache.is_empty());
            assert!(
                !reference.cache.is_empty(),
                "the reference must actually traverse general lowering"
            );
        }
    }

    #[test]
    fn hot_edits_into_and_out_of_selected_slots_preserve_general_fallback() {
        let leaf = selected(ColorComponent::Red, 0.2);
        let tape = RetainedExpressionTape::from_roots(&[Arc::new(leaf.clone())]).unwrap();
        let retained = E::Retained {
            root: tape.roots()[0],
            tape: Arc::new(tape),
        };
        let general = super::super::tests::resume(
            Some(leaf.clone()),
            Some(selected(ColorComponent::Blue, 0.8)),
            0.4,
        );
        let inputs = [
            leaf,
            color(ColorComponent::Hue, 45.0),
            retained,
            general,
            selected(ColorComponent::Blue, 0.6),
        ];
        let mut fast = DynamicFamilyPreparationScratch::default();
        let mut reference = DynamicFamilyPreparationScratch::default();
        for (index, expression) in inputs.into_iter().enumerate() {
            let input = sample(10, expression);
            let reference_input = sample(10, wrapped(&input.expression));
            let expected = prepare_dynamic_family_samples(
                &[reference_input],
                &Sources::default(),
                None,
                &mut reference,
            )
            .unwrap();
            let actual =
                prepare_dynamic_family_samples(&[input], &Sources::default(), None, &mut fast)
                    .unwrap();
            assert_eq!(observed(&actual), observed(&expected));
            assert_eq!(fast.cache.is_empty(), index == 0 || index == 4);
        }
    }

    #[test]
    fn sampling_requirements_suppress_slot_emission_without_caching() {
        let input = sample(10, selected(ColorComponent::Red, 0.4));
        let requirement = DynamicFamilyPreparationRequirement {
            target: input.target,
            owner: ProgrammingOwner::Color,
            rank: rank(&input),
            reason: DynamicFamilyPreparationRequirementReason::Transition(
                TransitionRequirement::CompatibleOwners,
            ),
        };
        let mut scratch = DynamicFamilyPreparationScratch::default();
        let prepared = prepare_dynamic_family_samples_with_requirements(
            &[input],
            &[requirement.clone()],
            &Sources::default(),
            None,
            &mut scratch,
        )
        .unwrap();
        assert!(prepared.families.is_empty());
        assert_eq!(prepared.requirements, &[requirement]);
        assert!(scratch.cache.is_empty());
    }

    #[test]
    fn invalid_current_values_and_influence_still_fail_like_general_lowering() {
        let mut cases = [f32::NAN, f32::INFINITY, -0.1, 1.1]
            .into_iter()
            .map(|value| sample(10, selected(ColorComponent::Red, value)))
            .collect::<Vec<_>>();
        let mut bad_activation = sample(10, selected(ColorComponent::Red, 0.4));
        bad_activation.activation_mix = f32::NAN;
        cases.push(bad_activation);
        let mut bad_dependency = sample(10, selected(ColorComponent::Red, 0.4));
        if let E::Programming {
            dependency_occurrence,
            ..
        } = &mut bad_dependency.expression
        {
            *dependency_occurrence = Some(crate::DynamicSourceDependency::mapped(
                None,
                ProgrammingFieldTransfer {
                    identity: ProgrammingFieldScope::from_component(ProgrammingComponent::Focus),
                    remap: Arc::from([]),
                },
            ));
        }
        cases.push(bad_dependency);
        for input in cases {
            let mut slow = input.clone();
            slow.expression = wrapped(&slow.expression);
            let error = |input| {
                prepare_dynamic_family_samples(
                    &[input],
                    &Sources::default(),
                    None,
                    &mut DynamicFamilyPreparationScratch::default(),
                )
                .map(|_| ())
                .map_err(|error| format!("{error:?}"))
            };
            assert!(error(input.clone()).is_err());
            assert_eq!(error(input), error(slow));
        }
    }

    #[test]
    fn unrelated_expression_shapes_never_enter_scalar_color_slots() {
        let direct = E::Programming {
            address: Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Focus,
                component: Some(ProgrammingComponent::Focus),
            }),
            value: DynamicValue::Scalar(0.5),
            occurrence: None,
            dependency_occurrence: None,
        };
        assert!(leaf(&direct).is_none());
        assert!(leaf(&wrapped(&selected(ColorComponent::Red, 0.5))).is_none());
        assert!(leaf(&super::super::tests::angle(ProgrammingComponent::Pan, 30.0)).is_none());
        let native = E::Programming {
            address: Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::DirectColor {
                    source: light_core::NativeColorIdentity {
                        profile_id: Uuid::from_u128(101),
                        profile_revision: 2,
                        profile_digest: "original".into(),
                        mode_id: Uuid::from_u128(102),
                        head_id: Uuid::from_u128(103),
                        path_id: Uuid::from_u128(104),
                        model_revision: 1,
                        native_layout_signature: "original-layout".into(),
                    },
                },
                component: Some(ProgrammingComponent::NativeColor(
                    light_core::NativeColorBinding {
                        channel_id: Uuid::from_u128(105),
                        function_id: Uuid::from_u128(106),
                    },
                )),
            }),
            value: DynamicValue::Native(u32::MAX - 2),
            occurrence: None,
            dependency_occurrence: None,
        };
        assert!(leaf(&native).is_none());
        let retain_white = color(ColorComponent::WhiteBlend, 0.5);
        assert!(
            leaf(&retain_white).is_none(),
            "a different valid basis must keep its general path"
        );
        let mut recipe_uv = selected(ColorComponent::Uv, 0.5);
        if let E::Programming { address, .. } = &mut recipe_uv {
            Arc::make_mut(address).representation = DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Recipe,
            };
        }
        assert!(leaf(&recipe_uv).is_none());
        let whole =
            super::super::tests::whole(ProgrammingOwner::Color, semantic(ColorIntent::default()));
        assert!(leaf(&whole).is_none());
    }
    #[test]
    fn new_scratch_and_source_identities_reuse_only_the_immutable_address() {
        let mut previous_address = None;
        for generation in 0..2 {
            let mut scratch = DynamicFamilyPreparationScratch::default();
            let mut input = sample(
                30 + generation,
                selected(ColorComponent::Green, 0.2 + generation as f32 * 0.4),
            );
            input.instance_id = Uuid::from_u128(500 + generation);
            input.controller_id = Uuid::from_u128(600 + generation);
            input.target = FixtureId(Uuid::from_u128(700 + generation));
            let prepared = prepare_dynamic_family_samples(
                &[input.clone()],
                &Sources::default(),
                None,
                &mut scratch,
            )
            .unwrap();
            assert_eq!(prepared.families[0].target, input.target);
            let FamilyCompositionSample::Known(known) = &prepared.families[0].samples[0] else {
                panic!()
            };
            assert_eq!(known.rank, rank(&input));
            assert_eq!(
                known.materialized_value(),
                Some(&DynamicValue::Scalar(0.2 + generation as f32 * 0.4))
            );
            let pointer = known.address() as *const _;
            if let Some(previous) = previous_address {
                assert_eq!(pointer, previous);
            }
            previous_address = Some(pointer);
        }
    }
}
