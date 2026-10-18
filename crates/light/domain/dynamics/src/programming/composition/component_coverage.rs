//! Exact set membership for composition's finite semantic components. Bits never encode
//! ordering, rank, values, generations, native function identity or whole-family ownership.
use super::{ColorComponent, ProgrammingComponent};
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct ComponentCoverage {
    semantic: u32,
    // These variants carry dynamic identities and must retain their complete equality key.
    dynamic: HashSet<ProgrammingComponent>,
}

fn bit(component: ProgrammingComponent) -> Option<u32> {
    use ColorComponent as C;
    use ProgrammingComponent as P;
    let slot = match component {
        P::Color(color) => match color {
            C::Red => 0,
            C::Green => 1,
            C::Blue => 2,
            C::Amber => 3,
            C::Hue => 4,
            C::Saturation => 5,
            C::WhiteBlend => 6,
            C::Temperature => 7,
            C::Duv => 8,
            C::Uv => 9,
            C::RelativeOutput => 10,
        },
        P::Pan => 11,
        P::Tilt => 12,
        P::TargetReference => 13,
        P::TargetX => 14,
        P::TargetY => 15,
        P::TargetZ => 16,
        P::Focus => 17,
        P::Zoom => 18,
        P::ColorWheel(_) | P::NativeColor(_) => return None,
    };
    Some(1 << slot)
}

impl ComponentCoverage {
    pub(super) fn clear(&mut self) {
        self.semantic = 0;
        self.dynamic.clear();
    }

    pub(super) fn contains(&self, component: &ProgrammingComponent) -> bool {
        match bit(*component) {
            Some(bit) => self.semantic & bit != 0,
            None => self.dynamic.contains(component),
        }
    }

    pub(super) fn insert(&mut self, component: ProgrammingComponent) -> bool {
        match bit(component) {
            Some(bit) => {
                let fresh = self.semantic & bit == 0;
                self.semantic |= bit;
                fresh
            }
            None => self.dynamic.insert(component),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use light_core::{NativeColorBinding, NativeColorIdentity};

    fn finite() -> [ProgrammingComponent; 19] {
        use ColorComponent as C;
        use ProgrammingComponent as P;
        [
            P::Color(C::Red),
            P::Color(C::Green),
            P::Color(C::Blue),
            P::Color(C::Amber),
            P::Color(C::Hue),
            P::Color(C::Saturation),
            P::Color(C::WhiteBlend),
            P::Color(C::Temperature),
            P::Color(C::Duv),
            P::Color(C::Uv),
            P::Color(C::RelativeOutput),
            P::Pan,
            P::Tilt,
            P::TargetReference,
            P::TargetX,
            P::TargetY,
            P::TargetZ,
            P::Focus,
            P::Zoom,
        ]
    }

    fn binding(channel: u128, function: u128) -> NativeColorBinding {
        NativeColorBinding {
            channel_id: Uuid::from_u128(channel),
            function_id: Uuid::from_u128(function),
        }
    }

    #[test]
    fn every_finite_component_has_one_distinct_bit_and_no_dynamic_entry() {
        let components = finite();
        let mut union = 0u32;
        for component in components {
            let bit = bit(component).unwrap();
            assert_eq!(bit.count_ones(), 1);
            assert_eq!(union & bit, 0, "component bits must never alias");
            union |= bit;
            let mut set = ComponentCoverage::default();
            assert!(set.insert(component));
            assert!(!set.insert(component));
            for other in components {
                assert_eq!(set.contains(&other), other == component);
            }
            assert!(
                set.dynamic.is_empty(),
                "finite components must never enter hashed storage"
            );
        }
        assert_eq!(union.count_ones(), 19);
    }

    #[test]
    fn dynamic_functions_and_wheel_numbers_keep_exact_identity_and_clear_with_bits() {
        let dynamic = [
            ProgrammingComponent::ColorWheel(0),
            ProgrammingComponent::ColorWheel(u16::MAX),
            ProgrammingComponent::NativeColor(binding(10, 20)),
            ProgrammingComponent::NativeColor(binding(10, 21)),
            ProgrammingComponent::NativeColor(binding(11, 20)),
        ];
        let mut set = ComponentCoverage::default();
        for component in dynamic {
            assert_eq!(bit(component), None);
            assert!(set.insert(component));
        }
        for component in finite() {
            assert!(set.insert(component));
        }
        assert_eq!(set.dynamic.len(), dynamic.len());
        for component in dynamic.into_iter().chain(finite()) {
            assert!(set.contains(&component));
        }
        set.clear();
        assert_eq!(set.semantic, 0);
        assert!(set.dynamic.is_empty());
        for component in dynamic.into_iter().chain(finite()) {
            assert!(!set.contains(&component));
        }
    }

    #[test]
    fn mixed_membership_operations_match_the_original_hashset() {
        let mut components = finite().to_vec();
        components.extend([
            ProgrammingComponent::ColorWheel(3),
            ProgrammingComponent::ColorWheel(4),
            ProgrammingComponent::NativeColor(binding(10, 20)),
            ProgrammingComponent::NativeColor(binding(10, 21)),
        ]);
        let mut actual = ComponentCoverage::default();
        let mut reference = HashSet::new();
        let mut seed = 775u64;
        for step in 0..1024 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let component = components[(seed as usize) % components.len()];
            if step % 37 == 0 {
                actual.clear();
                reference.clear();
            } else {
                assert_eq!(actual.insert(component), reference.insert(component));
            }
            for candidate in &components {
                assert_eq!(actual.contains(candidate), reference.contains(candidate));
            }
        }
    }

    // Retain the original reverse-walk algorithm as a test oracle. Its HashSet and the
    // production bit set must answer the same pruning decisions for real FamilySamples.
    fn reference_covered(
        samples: &[FamilySample],
        ordered: &[usize],
        winner: usize,
        orthogonal_pass: bool,
    ) -> Vec<bool> {
        let mut covered = vec![false; ordered.len()];
        let mut components = HashSet::new();
        let mut functions = HashMap::new();
        for (cursor, &index) in ordered.iter().enumerate().rev() {
            let sample = &samples[index];
            if !compatible(sample, &samples[winner])
                || orthogonal(sample.address.address()) != orthogonal_pass
            {
                continue;
            }
            let Some(component) = sample.address.address().component else {
                components.clear();
                functions.clear();
                continue;
            };
            if let ProgrammingComponent::NativeColor(binding) = component
                && !(native_fixed::is_native_fixed(sample) && sample.activation_mix < 1.0)
            {
                let selected = functions
                    .entry(binding.channel_id)
                    .or_insert(binding.function_id);
                if *selected != binding.function_id {
                    covered[cursor] = true;
                    continue;
                }
            }
            if components.contains(&component) {
                covered[cursor] = true;
            }
            if !sample.component_needs_underlay() {
                components.insert(component);
            }
        }
        covered
    }

    fn rank(order: u128) -> FamilySampleRank {
        FamilySampleRank {
            priority: 10,
            changed_at_millis: 100,
            changed_at_submillis_nanos: 0,
            stable_order: order,
            identity: crate::FamilySampleIdentity::Dynamic {
                instance_id: Uuid::from_u128(1000 + order),
                controller_id: Uuid::from_u128(2000 + order),
                lane_id: Uuid::from_u128(3000 + order),
            },
        }
    }

    fn color(component: Option<ColorComponent>, order: u128) -> FamilySample {
        let (basis, value) = match component {
            Some(ColorComponent::Uv) => {
                (DynamicSemanticColorBasis::Retain, DynamicValue::Scalar(0.4))
            }
            Some(_) => (DynamicSemanticColorBasis::Recipe, DynamicValue::Scalar(0.4)),
            None => (
                DynamicSemanticColorBasis::Whole,
                DynamicValue::Family(AttributeValue::ColorProgram(Arc::new(
                    ColorProgram::Semantic {
                        intent: ColorIntent::default(),
                    },
                ))),
            ),
        };
        FamilySample::new(
            Arc::new(
                CompiledDynamicValueAddress::new(
                    DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::SemanticColor { basis },
                        component: component.map(ProgrammingComponent::Color),
                    },
                    None,
                )
                .unwrap(),
            ),
            value,
            rank(order),
            1.0,
        )
        .unwrap()
    }

    fn compare(samples: &[FamilySample]) {
        let mut scratch = ComponentCompositionScratch::default();
        // Exercise scratch reuse, both passes, all winner choices and nonidentity ordering.
        for ordered in [
            (0..samples.len()).collect::<Vec<_>>(),
            (0..samples.len()).rev().collect(),
        ] {
            for winner in 0..samples.len() {
                for orthogonal_pass in [false, true] {
                    mark_covered_components(
                        samples,
                        &ordered,
                        winner,
                        orthogonal_pass,
                        &mut scratch,
                    );
                    assert_eq!(
                        scratch.covered,
                        reference_covered(samples, &ordered, winner, orthogonal_pass)
                    );
                }
            }
        }
    }

    #[test]
    fn reverse_pruning_matches_hashset_across_whole_barriers_and_retained_underlays() {
        let mut partial = color(Some(ColorComponent::Red), 2);
        partial.activation_mix = 0.5;
        let template = color(Some(ColorComponent::Uv), 7);
        let expression = DynamicSampleExpression::Transition {
            from: None,
            to: Some(Arc::new(DynamicSampleExpression::Programming {
                address: Arc::new(template.address.address().clone()),
                value: DynamicValue::Scalar(0.8),
                occurrence: None,
                dependency_occurrence: None,
            })),
            progress: 0.5,
            reason: crate::DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(700),
            },
        };
        let retained = FamilySample::retained_component(
            Arc::new(
                CompiledComponentExpression::new(
                    Arc::new(expression),
                    Arc::clone(template.address.as_ref()),
                )
                .unwrap(),
            ),
            template.rank,
            1.0,
        )
        .unwrap();
        assert!(retained.component_needs_underlay());
        let samples = [
            color(Some(ColorComponent::Red), 1),
            partial,
            color(None, 3),
            color(Some(ColorComponent::Red), 4),
            color(Some(ColorComponent::Green), 5),
            color(Some(ColorComponent::Uv), 6),
            retained,
        ];
        compare(&samples);
        let mut scratch = ComponentCompositionScratch::default();
        mark_covered_components(&samples[..2], &[0, 1], 1, false, &mut scratch);
        assert!(
            !scratch.covered[0],
            "partial components keep the lower underlay"
        );
        mark_covered_components(&samples[5..], &[0, 1], 1, true, &mut scratch);
        assert!(
            !scratch.covered[0],
            "retained missing endpoints keep the lower underlay"
        );
        mark_covered_components(
            &[samples[0].clone(), samples[2].clone(), samples[3].clone()],
            &[0, 1, 2],
            2,
            false,
            &mut scratch,
        );
        assert!(
            scratch.covered[0],
            "an incompatible whole Color basis is skipped before the barrier"
        );
        // Focus has one representation for both components and whole values. Unlike the
        // Whole/Recipe Color pair above, this whole value is compatible with the winner.
        let focus = |whole, order| {
            FamilySample::new(
                Arc::new(
                    CompiledDynamicValueAddress::new(
                        DynamicValueAddress {
                            representation: DynamicFamilyRepresentation::Focus,
                            component: if whole {
                                None
                            } else {
                                Some(ProgrammingComponent::Focus)
                            },
                        },
                        None,
                    )
                    .unwrap(),
                ),
                if whole {
                    DynamicValue::Family(AttributeValue::Normalized(0.4))
                } else {
                    DynamicValue::Scalar(0.4)
                },
                rank(order),
                1.0,
            )
            .unwrap()
        };
        let compatible_barrier = [focus(false, 1), focus(true, 2), focus(false, 3)];
        compare(&compatible_barrier);
        mark_covered_components(&compatible_barrier, &[0, 1, 2], 2, false, &mut scratch);
        assert!(
            !scratch.covered[0],
            "a compatible whole-family barrier resets component coverage"
        );
    }

    struct Model {
        source: NativeColorIdentity,
    }
    impl NativeColorEditModel for Model {
        fn source(&self) -> &NativeColorIdentity {
            &self.source
        }
        fn descriptor(
            &self,
            binding: NativeColorBinding,
        ) -> Option<NativeColorComponentDescriptor> {
            Some(NativeColorComponentDescriptor {
                binding,
                raw_from: 0,
                raw_to: u32::MAX,
                continuous: true,
            })
        }
        fn predict(&self, _: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
            unreachable!("coverage never predicts a native recipe")
        }
    }
    fn native(model: &Arc<Model>, channel: u128, function: u128, order: u128) -> FamilySample {
        FamilySample::new(
            Arc::new(
                CompiledDynamicValueAddress::new(
                    DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::DirectColor {
                            source: model.source.clone(),
                        },
                        component: Some(ProgrammingComponent::NativeColor(binding(
                            channel, function,
                        ))),
                    },
                    Some(model.clone()),
                )
                .unwrap(),
            ),
            DynamicValue::Native(40),
            rank(order),
            1.0,
        )
        .unwrap()
    }

    #[test]
    fn native_function_selection_and_partial_fixed_steps_match_the_hashset_oracle() {
        let model = Arc::new(Model {
            source: NativeColorIdentity {
                profile_id: Uuid::from_u128(101),
                profile_revision: 1,
                profile_digest: "coverage-test".into(),
                mode_id: Uuid::from_u128(102),
                head_id: Uuid::from_u128(103),
                path_id: Uuid::from_u128(104),
                model_revision: 1,
                native_layout_signature: "coverage-test".into(),
            },
        });
        let old = native(&model, 10, 20, 1);
        let mut fixed = native(&model, 10, 21, 2).into_fix_at();
        fixed.activation_mix = 0.5;
        let other = native(&model, 11, 20, 3);
        let mut scratch = ComponentCompositionScratch::default();
        let samples = [old.clone(), fixed.clone(), other.clone()];
        compare(&samples);
        mark_covered_components(&samples, &[0, 1, 2], 2, false, &mut scratch);
        assert!(
            !scratch.covered[0],
            "partial Fixed does not select its new function early"
        );
        fixed.activation_mix = 1.0;
        let samples = [old, fixed, other];
        compare(&samples);
        mark_covered_components(&samples, &[0, 1, 2], 2, false, &mut scratch);
        assert!(
            scratch.covered[0],
            "a completed Fixed function excludes the old function"
        );
    }
}
