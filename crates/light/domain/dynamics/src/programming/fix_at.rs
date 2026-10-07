//! Typed mask payload. The recorded value is a complete intent, including native source
//! identity/portable estimate and independent UV. `address` selects the masked component;
//! it is not a sparse fixture-channel replacement for that recorded family.
use super::{
    CompiledDynamicValueAddress, DynamicValue, DynamicValueAddress, FamilySample, FamilySampleRank,
    address::ensure, extract_compatible_dynamic_value,
};
use light_core::{AttributeValue, programming::*};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingFamilyFixAt {
    pub address: DynamicValueAddress,
    pub family: AttributeValue,
}

impl ProgrammingFamilyFixAt {
    /// Capture the actual materialized representation without editing or adopting it. A caller
    /// changing representation must first perform that explicit edit with its frozen context.
    /// Native shape/binding checks run here; the pinned model is verified during compilation.
    pub fn from_family(
        owner: ProgrammingOwner,
        component: Option<ProgrammingComponent>,
        family: AttributeValue,
    ) -> Result<Self, IntentError> {
        use super::{DynamicFamilyRepresentation as R, DynamicSemanticColorBasis as B};
        let mut address = DynamicValueAddress::whole_family(owner, &family)?;
        if let Some(component) = component {
            address.component = Some(component);
            match (&family, component) {
                (
                    AttributeValue::Position(position),
                    ProgrammingComponent::TargetX
                    | ProgrammingComponent::TargetY
                    | ProgrammingComponent::TargetZ,
                ) => {
                    if let PositionIntent::Target { reference, .. } = position.as_ref() {
                        address.representation = R::Target {
                            reference: Some(*reference),
                        };
                    }
                }
                (AttributeValue::ColorProgram(program), ProgrammingComponent::Color(color))
                    if matches!(program.as_ref(), ColorProgram::Semantic { .. }) =>
                {
                    address.representation = R::SemanticColor {
                        basis: match color {
                            ColorComponent::Red
                            | ColorComponent::Green
                            | ColorComponent::Blue
                            | ColorComponent::Amber => B::Recipe,
                            ColorComponent::Hue | ColorComponent::Saturation => B::HueSaturation,
                            ColorComponent::WhiteBlend
                            | ColorComponent::Temperature
                            | ColorComponent::Duv
                            | ColorComponent::Uv
                            | ColorComponent::RelativeOutput => B::Retain,
                        },
                    };
                }
                _ => {}
            }
        }
        let mask = Self { address, family };
        mask.validate()?;
        Ok(mask)
    }

    pub fn required_programming_contract(&self) -> u16 {
        // A typed Focus mask still needs the new contract even though its complete payload
        // is the pre-existing Normalized AttributeValue.
        PROGRAMMING_CONTRACT_VERSION.max(self.family.required_programming_contract())
    }

    /// JSON/storage shape check; cold compilation additionally verifies a Direct source model.
    pub fn validate(&self) -> Result<(), IntentError> {
        self.address.validate()?;
        self.family
            .validate_programming_address(self.address.owner().key_ref())?;
        ensure(
            self.family.spread_control_points() == 0
                && !matches!(self.family, AttributeValue::GroupFamily(_)),
            "FixAT requires a complete materialized family",
        )?;
        ensure(
            self.address.matches_authored_source(&self.family),
            "FixAT component and stored family use different representations or native functions",
        )
    }

    /// Compile one capture in the original source context. The native model verifies the
    /// complete recipe even when only one native component is masked. Cache this result;
    /// mask influence/timing changes do not re-predict or rebuild the stored intent.
    pub fn compile(
        &self,
        native_model: Option<Arc<dyn NativeColorEditModel + Send + Sync>>,
        context: &FamilyEditContext<'_>,
        rank: FamilySampleRank,
        activation_mix: f32,
    ) -> Result<FamilySample, IntentError> {
        self.validate()?;
        let whole = CompiledDynamicValueAddress::new(
            DynamicValueAddress::whole_family(self.address.owner(), &self.family)?,
            native_model.clone(),
        )?;
        if self.address.component.is_none() {
            return FamilySample::new(
                Arc::new(whole),
                DynamicValue::Family(self.family.clone()),
                rank,
                activation_mix,
            )
            .map(FamilySample::into_fix_at);
        }
        whole.validate_source_value(&DynamicValue::Family(self.family.clone()))?;
        let component =
            CompiledDynamicValueAddress::new_fixed_component(self.address.clone(), native_model)?;
        let value = extract_compatible_dynamic_value(&self.family, &self.address, context)?
            .ok_or_else(|| {
                IntentError("FixAT has no compatible component in its stored family".into())
            })?;
        FamilySample::new(Arc::new(component), value, rank, activation_mix)
            .map(FamilySample::into_fix_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DynamicFamilyRepresentation as R, DynamicSemanticColorBasis as B, FamilyCompositionContext,
        FamilyCompositionScratch, compose_dynamic_family,
    };
    use uuid::Uuid;

    fn rank(order: u128) -> FamilySampleRank {
        FamilySampleRank {
            priority: 10,
            changed_at_millis: 100,
            changed_at_submillis_nanos: 0,
            stable_order: order,
            identity: crate::FamilySampleIdentity::Dynamic {
                instance_id: Uuid::from_u128(10 + order),
                controller_id: Uuid::from_u128(30 + order),
                lane_id: Uuid::from_u128(20 + order),
            },
        }
    }

    fn angles(pan: f32, tilt: f32) -> AttributeValue {
        AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
    }
    fn address(component: ProgrammingComponent) -> DynamicValueAddress {
        DynamicValueAddress {
            representation: R::Angles,
            component: Some(component),
        }
    }
    fn pair(pan: f32, tilt: f32, order: u128, mix: f32) -> FamilySample {
        FamilySample::new(
            Arc::new(
                CompiledDynamicValueAddress::new(
                    DynamicValueAddress {
                        representation: R::Angles,
                        component: None,
                    },
                    None,
                )
                .unwrap(),
            ),
            DynamicValue::Family(angles(pan, tilt)),
            rank(order),
            mix,
        )
        .unwrap()
    }

    #[test]
    fn from_family_retains_target_frame_and_requires_explicit_angle_adoption() {
        let reference = TargetReference::Point {
            point_id: Uuid::from_u128(10),
        };
        let family = AttributeValue::Position(Arc::new(PositionIntent::target(
            reference,
            [1.0, -2.0, 3.0],
        )));
        let mask = ProgrammingFamilyFixAt::from_family(
            ProgrammingOwner::Position,
            Some(ProgrammingComponent::TargetY),
            family.clone(),
        )
        .unwrap();
        assert_eq!(mask.family, family);
        assert_eq!(
            mask.address.representation,
            R::Target {
                reference: Some(reference)
            }
        );
        let whole =
            ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Position, None, family.clone())
                .unwrap();
        assert_eq!(whole.address.representation, R::Target { reference: None });
        assert_eq!(whole.family, family);
        assert!(
            ProgrammingFamilyFixAt::from_family(
                ProgrammingOwner::Position,
                Some(ProgrammingComponent::Pan),
                family,
            )
            .is_err()
        );
        assert!(
            ProgrammingFamilyFixAt::from_family(
                ProgrammingOwner::Position,
                Some(ProgrammingComponent::TargetX),
                angles(720.0, -15.0),
            )
            .is_err()
        );
    }

    #[test]
    fn from_family_derives_semantic_bases_and_keeps_complete_uv_and_white_intent() {
        let family = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent {
                white_blend: 0.5,
                white_target: WhiteTarget {
                    kelvin: 2700.0,
                    duv: 0.004,
                },
                uv: UvIntent { amount: 0.8 },
                ..Default::default()
            },
        }));
        for (components, basis) in [
            (
                vec![
                    ColorComponent::Red,
                    ColorComponent::Green,
                    ColorComponent::Blue,
                    ColorComponent::Amber,
                ],
                B::Recipe,
            ),
            (
                vec![ColorComponent::Hue, ColorComponent::Saturation],
                B::HueSaturation,
            ),
            (
                vec![
                    ColorComponent::WhiteBlend,
                    ColorComponent::Temperature,
                    ColorComponent::Duv,
                    ColorComponent::Uv,
                    ColorComponent::RelativeOutput,
                ],
                B::Retain,
            ),
        ] {
            for component in components {
                let mask = ProgrammingFamilyFixAt::from_family(
                    ProgrammingOwner::Color,
                    Some(ProgrammingComponent::Color(component)),
                    family.clone(),
                )
                .unwrap();
                assert_eq!(mask.address.representation, R::SemanticColor { basis });
                assert_eq!(mask.family, family);
                let restored: ProgrammingFamilyFixAt =
                    serde_json::from_value(serde_json::to_value(&mask).unwrap()).unwrap();
                assert_eq!(restored, mask);
            }
        }
        let whole =
            ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Color, None, family).unwrap();
        assert_eq!(
            whole.address.representation,
            R::SemanticColor { basis: B::Whole }
        );
    }

    #[test]
    fn from_family_preserves_direct_source_exact_native_integers_and_unknown_visible_estimate() {
        use light_core::{
            NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality,
        };
        let source = NativeColorIdentity {
            profile_id: Uuid::from_u128(1),
            profile_revision: 3,
            profile_digest: "original-profile".into(),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            model_revision: 2,
            native_layout_signature: "rgb-wheel-uv".into(),
        };
        let binding = NativeColorBinding {
            channel_id: Uuid::from_u128(5),
            function_id: Uuid::from_u128(6),
        };
        let family = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
            recipe: NativeColorRecipe {
                source: source.clone(),
                channels: vec![
                    NativeColorValue {
                        channel_id: binding.channel_id,
                        function_id: binding.function_id,
                        raw: u32::MAX - 1,
                    },
                    NativeColorValue {
                        channel_id: Uuid::from_u128(7),
                        function_id: Uuid::from_u128(8),
                        raw: 17,
                    },
                ],
                spreads: vec![],
            },
            portable: PortableColorEstimate {
                model_revision: 2,
                visible: None,
                uv: Some(PortableUv {
                    amount: 0.8,
                    quality: PhysicalDataQuality::Estimated,
                }),
                quality: PhysicalDataQuality::Unknown,
                limitations: vec!["visible appearance unknown".into()],
            },
        }));
        let mask = ProgrammingFamilyFixAt::from_family(
            ProgrammingOwner::Color,
            Some(ProgrammingComponent::NativeColor(binding)),
            family.clone(),
        )
        .unwrap();
        assert_eq!(mask.address.representation, R::DirectColor { source });
        assert_eq!(mask.family, family);
        let restored: ProgrammingFamilyFixAt =
            serde_json::from_value(serde_json::to_value(&mask).unwrap()).unwrap();
        assert_eq!(restored, mask);
        for invalid in [
            NativeColorBinding {
                channel_id: Uuid::from_u128(9),
                ..binding
            },
            NativeColorBinding {
                function_id: Uuid::from_u128(9),
                ..binding
            },
        ] {
            assert!(
                ProgrammingFamilyFixAt::from_family(
                    ProgrammingOwner::Color,
                    Some(ProgrammingComponent::NativeColor(invalid)),
                    family.clone(),
                )
                .is_err()
            );
        }
        assert!(
            ProgrammingFamilyFixAt::from_family(
                ProgrammingOwner::Color,
                Some(ProgrammingComponent::Color(ColorComponent::Uv)),
                family,
            )
            .is_err()
        );
    }

    #[test]
    fn from_family_preserves_unwrapped_angles_focus_and_zoom_convention() {
        let zoom = AttributeValue::Zoom(Arc::new(ZoomIntent {
            opening_degrees: ScalarIntent::Value(42.0),
            convention: light_core::OpeningConvention::Field,
        }));
        for (owner, component, family, representation) in [
            (
                ProgrammingOwner::Position,
                ProgrammingComponent::Pan,
                angles(-720.0, 15.0),
                R::Angles,
            ),
            (
                ProgrammingOwner::Position,
                ProgrammingComponent::Tilt,
                angles(720.0, -15.0),
                R::Angles,
            ),
            (
                ProgrammingOwner::Focus,
                ProgrammingComponent::Focus,
                AttributeValue::Normalized(0.6),
                R::Focus,
            ),
            (
                ProgrammingOwner::Zoom,
                ProgrammingComponent::Zoom,
                zoom,
                R::Zoom {
                    convention: light_core::OpeningConvention::Field,
                },
            ),
        ] {
            let mask = ProgrammingFamilyFixAt::from_family(owner, Some(component), family.clone())
                .unwrap();
            assert_eq!(mask.address.representation, representation);
            assert_eq!(mask.family, family);
            assert_eq!(
                mask.required_programming_contract(),
                PROGRAMMING_CONTRACT_VERSION
            );
        }
    }

    #[test]
    fn from_family_rejects_sparse_spread_group_invalid_and_wrong_owner_values() {
        let spread = AttributeValue::Position(Arc::new(PositionIntent::Angles {
            pan_degrees: ScalarIntent::Spread(vec![0.0, 90.0]),
            tilt_degrees: ScalarIntent::Value(0.0),
        }));
        let group = AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
            owner: ProgrammingOwner::Position,
            template: angles(0.0, 0.0),
            members: Default::default(),
        }));
        for (owner, component, family) in [
            (
                ProgrammingOwner::Position,
                Some(ProgrammingComponent::Pan),
                AttributeValue::Normalized(0.5),
            ),
            (ProgrammingOwner::Position, None, spread),
            (ProgrammingOwner::Position, None, group),
            (ProgrammingOwner::Position, None, angles(f32::NAN, 0.0)),
            (ProgrammingOwner::Color, None, angles(0.0, 0.0)),
            (
                ProgrammingOwner::Position,
                Some(ProgrammingComponent::Focus),
                angles(0.0, 0.0),
            ),
            (
                ProgrammingOwner::Focus,
                None,
                AttributeValue::Normalized(1.1),
            ),
            (
                ProgrammingOwner::Focus,
                None,
                AttributeValue::Spread(vec![0.0, 1.0]),
            ),
        ] {
            assert!(ProgrammingFamilyFixAt::from_family(owner, component, family).is_err());
        }
    }

    #[test]
    fn component_mask_preserves_complete_capture_and_does_not_hold_unmasked_tilt() {
        let mask = ProgrammingFamilyFixAt {
            address: address(ProgrammingComponent::Pan),
            family: angles(720.0, 15.0),
        };
        let persisted = serde_json::to_string(&mask).unwrap();
        let restored: ProgrammingFamilyFixAt = serde_json::from_str(&persisted).unwrap();
        assert_eq!(restored, mask);
        let fixed = restored
            .compile(None, &FamilyEditContext::default(), rank(3), 1.0)
            .unwrap();
        let samples = [pair(0.0, -60.0, 1, 1.0), fixed];
        let mut scratch = FamilyCompositionScratch::default();
        assert_eq!(
            compose_dynamic_family(
                ProgrammingOwner::Position,
                &angles(0.0, 0.0),
                &samples,
                &FamilyCompositionContext::default(),
                &mut scratch
            )
            .unwrap(),
            angles(720.0, -60.0)
        );
        // Releasing the mask samples today's lane values, not the value at capture time.
        let running = [pair(45.0, -80.0, 1, 1.0)];
        assert_eq!(
            compose_dynamic_family(
                ProgrammingOwner::Position,
                &angles(0.0, 0.0),
                &running,
                &FamilyCompositionContext::default(),
                &mut scratch
            )
            .unwrap(),
            angles(45.0, -80.0)
        );
    }

    #[test]
    fn lower_ranked_mask_remains_the_underlay_of_a_partially_activated_dynamic() {
        let mask = ProgrammingFamilyFixAt {
            address: address(ProgrammingComponent::Pan),
            family: angles(80.0, 15.0),
        };
        let samples = [
            mask.compile(None, &FamilyEditContext::default(), rank(1), 1.0)
                .unwrap(),
            pair(20.0, 0.0, 2, 0.5),
        ];
        assert_eq!(
            compose_dynamic_family(
                ProgrammingOwner::Position,
                &angles(0.0, 0.0),
                &samples,
                &FamilyCompositionContext::default(),
                &mut FamilyCompositionScratch::default()
            )
            .unwrap(),
            angles(50.0, 0.0)
        );
    }

    #[test]
    fn color_mask_stores_uv_and_white_intent_and_rejects_cross_representation_payloads() {
        let intent = ColorIntent {
            white_blend: 0.5,
            uv: UvIntent { amount: 0.8 },
            ..Default::default()
        };
        let mask = ProgrammingFamilyFixAt {
            address: DynamicValueAddress {
                representation: R::SemanticColor { basis: B::Retain },
                component: Some(ProgrammingComponent::Color(ColorComponent::WhiteBlend)),
            },
            family: AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent })),
        };
        mask.validate().unwrap();
        let restored: ProgrammingFamilyFixAt =
            serde_json::from_value(serde_json::to_value(&mask).unwrap()).unwrap();
        assert_eq!(restored, mask);
        let sample = mask
            .compile(None, &FamilyEditContext::default(), rank(1), 1.0)
            .unwrap();
        assert_eq!(
            sample.materialized_value(),
            Some(&DynamicValue::Scalar(0.5))
        );
        let invalid = ProgrammingFamilyFixAt {
            address: address(ProgrammingComponent::Pan),
            family: mask.family,
        };
        assert!(invalid.validate().is_err());
        let target = ProgrammingFamilyFixAt {
            address: address(ProgrammingComponent::Pan),
            family: AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Origin,
                [0.0; 3],
            ))),
        };
        assert!(target.validate().is_err());
    }

    #[test]
    fn whole_mask_owns_both_angles_and_remains_underlay_for_higher_component_fades() {
        let mask = ProgrammingFamilyFixAt {
            address: DynamicValueAddress {
                representation: R::Angles,
                component: None,
            },
            family: angles(20.0, 30.0),
        };
        let fixed = mask
            .compile(None, &FamilyEditContext::default(), rank(3), 1.0)
            .unwrap();
        let mut samples = vec![pair(100.0, 200.0, 1, 1.0), fixed];
        let mut scratch = FamilyCompositionScratch::default();
        let run = |samples: &[FamilySample], scratch: &mut FamilyCompositionScratch| {
            compose_dynamic_family(
                ProgrammingOwner::Position,
                &angles(0.0, 0.0),
                samples,
                &FamilyCompositionContext::default(),
                scratch,
            )
            .unwrap()
        };
        assert_eq!(run(&samples, &mut scratch), angles(20.0, 30.0));
        samples.push(pair(80.0, 0.0, 4, 0.5));
        assert_eq!(run(&samples, &mut scratch), angles(50.0, 15.0));
        samples[1].activation_mix = 0.5;
        assert_eq!(run(&samples, &mut scratch), angles(70.0, 57.5));
        samples[1].activation_mix = 0.0;
        assert_eq!(run(&samples, &mut scratch), angles(90.0, 100.0));
    }

    #[test]
    fn whole_color_mask_cannot_be_bypassed_by_lower_ranked_orthogonal_lanes() {
        let semantic =
            |intent| AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }));
        let uv_address = DynamicValueAddress {
            representation: R::SemanticColor { basis: B::Retain },
            component: Some(ProgrammingComponent::Color(ColorComponent::Uv)),
        };
        let uv = |order, amount, mix| {
            FamilySample::new(
                Arc::new(CompiledDynamicValueAddress::new(uv_address.clone(), None).unwrap()),
                DynamicValue::Scalar(amount),
                rank(order),
                mix,
            )
            .unwrap()
        };
        let mask = ProgrammingFamilyFixAt {
            address: DynamicValueAddress {
                representation: R::SemanticColor { basis: B::Whole },
                component: None,
            },
            family: semantic(ColorIntent {
                uv: UvIntent { amount: 0.2 },
                white_blend: 0.8,
                ..Default::default()
            }),
        };
        let mut samples = vec![
            uv(1, 1.0, 1.0),
            mask.compile(None, &FamilyEditContext::default(), rank(2), 1.0)
                .unwrap(),
        ];
        let base = semantic(ColorIntent::default());
        let mut scratch = FamilyCompositionScratch::default();
        let result = compose_dynamic_family(
            ProgrammingOwner::Color,
            &base,
            &samples,
            &FamilyCompositionContext::default(),
            &mut scratch,
        )
        .unwrap();
        assert_eq!(result, mask.family);
        samples.push(uv(3, 0.8, 0.5));
        let AttributeValue::ColorProgram(result) = compose_dynamic_family(
            ProgrammingOwner::Color,
            &base,
            &samples,
            &FamilyCompositionContext::default(),
            &mut scratch,
        )
        .unwrap() else {
            panic!()
        };
        let ColorProgram::Semantic { intent } = result.as_ref() else {
            panic!()
        };
        assert_eq!(intent.uv.amount, 0.5);
        assert_eq!(intent.white_blend, 0.8);
    }

    #[test]
    fn full_mask_does_not_require_geometry_for_the_sources_it_completely_covers() {
        let target = AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [1.0; 3],
        )));
        let mask = ProgrammingFamilyFixAt {
            address: DynamicValueAddress {
                representation: R::Angles,
                component: None,
            },
            family: angles(10.0, 20.0),
        };
        let samples = [
            pair(720.0, 0.0, 1, 1.0),
            mask.compile(None, &FamilyEditContext::default(), rank(2), 1.0)
                .unwrap(),
        ];
        assert_eq!(
            compose_dynamic_family(
                ProgrammingOwner::Position,
                &target,
                &samples,
                &FamilyCompositionContext::default(),
                &mut FamilyCompositionScratch::default()
            )
            .unwrap(),
            mask.family
        );
    }

    #[test]
    fn partial_takeover_resolves_actual_composed_underlay_instead_of_reusing_old_pose() {
        let target = |x| {
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Origin,
                [x, 0.0, 0.0],
            )))
        };
        let base = target(1.0);
        let running_target = FamilySample::new(
            Arc::new(
                CompiledDynamicValueAddress::new(
                    DynamicValueAddress {
                        representation: R::Target { reference: None },
                        component: None,
                    },
                    None,
                )
                .unwrap(),
            ),
            DynamicValue::Family(target(2.0)),
            rank(1),
            1.0,
        )
        .unwrap();
        let mask = ProgrammingFamilyFixAt {
            address: DynamicValueAddress {
                representation: R::Angles,
                component: None,
            },
            family: angles(360.0, 30.0),
        };
        let samples = [
            running_target,
            mask.compile(None, &FamilyEditContext::default(), rank(2), 0.5)
                .unwrap(),
        ];
        let mut context = FamilyCompositionContext {
            edit: FamilyEditContext {
                solved_angles: Some(JointAngles {
                    pan_degrees: 720.0,
                    tilt_degrees: 80.0,
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut scratch = FamilyCompositionScratch::default();
        assert_eq!(
            compose_dynamic_family(
                ProgrammingOwner::Position,
                &base,
                &samples,
                &context,
                &mut scratch
            )
            .unwrap_err(),
            TransitionError::Requires(TransitionRequirement::LiveJointAngles)
        );
        let resolve = |actual: &AttributeValue, address: &DynamicValueAddress| {
            assert_eq!(actual, &target(2.0));
            assert_eq!(address.representation, R::Angles);
            Ok(angles(180.0, 10.0))
        };
        context.resolve_adoption = Some(&resolve);
        assert_eq!(
            compose_dynamic_family(
                ProgrammingOwner::Position,
                &base,
                &samples,
                &context,
                &mut scratch
            )
            .unwrap(),
            angles(270.0, 20.0)
        );
    }
}
