use super::*;
use crate::{NativeColorIdentity, NativeColorValue, PhysicalDataQuality, Xyz};
use uuid::Uuid;

fn identity() -> NativeColorIdentity {
    NativeColorIdentity {
        profile_id: Uuid::from_u128(1),
        profile_revision: 7,
        profile_digest: "profile-sha256".into(),
        mode_id: Uuid::from_u128(2),
        head_id: Uuid::from_u128(3),
        path_id: Uuid::from_u128(4),
        model_revision: 9,
        native_layout_signature: "verified-layout".into(),
    }
}
fn direct(visible: Option<PortableVisibleColor>, uv: Option<PortableUv>) -> ColorProgram {
    ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: identity(),
            spreads: vec![],
            channels: vec![NativeColorValue {
                channel_id: Uuid::from_u128(5),
                function_id: Uuid::from_u128(6),
                raw: u32::MAX - 1,
            }],
        },
        portable: PortableColorEstimate {
            model_revision: 9,
            visible,
            uv,
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        },
    }
}
#[test]
fn direct_color_round_trip_preserves_exact_32_bit_recipe_and_independent_unknowns() {
    let zero = Xyz {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let values = [
        direct(
            None,
            Some(PortableUv {
                amount: 0.7,
                quality: PhysicalDataQuality::Estimated,
            }),
        ),
        direct(
            Some(PortableVisibleColor {
                xyz: zero,
                relative_output: 0.0,
            }),
            None,
        ),
        direct(
            Some(PortableVisibleColor {
                xyz: zero,
                relative_output: 1.0,
            }),
            Some(PortableUv {
                amount: 0.0,
                quality: PhysicalDataQuality::Measured,
            }),
        ),
    ];
    for value in values {
        value.validate().unwrap();
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(encoded.contains("4294967294"));
        let restored: ColorProgram = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored, value);
    }
}
#[test]
fn white_blend_and_visible_envelope_do_not_redefine_uv_or_black() {
    let red = Xyz {
        x: 0.4,
        y: 0.2,
        z: 0.02,
    };
    let white = crate::color_intent::D65_WHITE;
    let mut intent = ColorIntent {
        base_xyz: red,
        recipe: VirtualColorRecipe {
            approximate: true,
            ..Default::default()
        },
        uv: UvIntent { amount: 0.8 },
        ..Default::default()
    };
    assert_eq!(intent.blend_visible(white).unwrap(), red);
    intent.white_blend = 0.5;
    assert_eq!(
        intent.blend_visible(white).unwrap(),
        Xyz {
            x: red.x + white.x,
            y: red.y + white.y,
            z: red.z + white.z
        }
    );
    intent.white_blend = 1.0;
    assert_eq!(intent.blend_visible(white).unwrap(), white);
    assert_eq!(intent.base_xyz, red);
    intent.relative_output = 0.0;
    assert_eq!(
        intent.blend_visible(white).unwrap(),
        Xyz {
            x: 0.0,
            y: 0.0,
            z: 0.0
        }
    );
    assert_eq!(intent.uv.amount, 0.8);
    intent.validate().unwrap();
    let mut json = serde_json::to_value(intent).unwrap();
    json.as_object_mut().unwrap().remove("uv");
    assert_eq!(
        serde_json::from_value::<ColorIntent>(json)
            .unwrap()
            .uv
            .amount,
        0.0
    );
}
#[test]
fn color_rejects_invalid_ranges_and_conflicting_base_spreads() {
    let mut intent = ColorIntent::default();
    intent.uv.amount = f32::NAN;
    assert!(intent.validate().is_err());
    intent.uv.amount = 0.0;
    intent.spreads = vec![
        ColorComponentSpread {
            component: ColorComponent::Red,
            points: vec![0.0, 1.0],
        },
        ColorComponentSpread {
            component: ColorComponent::Hue,
            points: vec![350.0, 10.0],
        },
    ];
    assert!(intent.validate().is_err());
    intent.spreads.remove(0);
    intent.spreads.push(ColorComponentSpread {
        component: ColorComponent::Uv,
        points: vec![1.0, 0.0],
    });
    intent.validate().unwrap();
    intent.spreads.push(intent.spreads[0].clone());
    assert!(intent.validate().is_err());
}
#[test]
fn angle_target_types_and_references_survive_round_trip_without_native_channels() {
    let point = Uuid::from_u128(77);
    for value in [
        PositionIntent::angles(-720.0, 270.0),
        PositionIntent::target(TargetReference::Point { point_id: point }, [1.0, -2.0, 3.0]),
    ] {
        value.validate().unwrap();
        let restored: PositionIntent =
            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(restored, value);
    }
    assert_eq!(
        PositionIntent::target(TargetReference::Point { point_id: point }, [0.0; 3])
            .referenced_point(),
        Some(point)
    );
    assert!(
        PositionIntent::target(
            TargetReference::Point {
                point_id: Uuid::nil()
            },
            [0.0; 3]
        )
        .validate()
        .is_err()
    );
}
#[test]
fn components_have_one_owner_without_absorbing_point_or_media_placement() {
    for component in [
        ProgrammingComponent::Pan,
        ProgrammingComponent::Tilt,
        ProgrammingComponent::TargetX,
        ProgrammingComponent::TargetReference,
    ] {
        assert_eq!(component.owner(), ProgrammingOwner::Position);
    }
    assert_eq!(
        ProgrammingComponent::Color(ColorComponent::Uv).owner(),
        ProgrammingOwner::Color
    );
    assert_ne!(
        ProgrammingComponent::Focus.owner(),
        ProgrammingComponent::Zoom.owner()
    );
    assert!(programming_component("position.pan", ProgrammingTargetRole::Point).is_none());
    assert!(programming_component("position.rotation", ProgrammingTargetRole::Media).is_none());
    assert!(programming_component("color.fake", ProgrammingTargetRole::LightHead).is_none());
    assert_eq!(
        ProgrammingComponent::Color(ColorComponent::Uv)
            .descriptor()
            .capability,
        AuthoringCapability::SemanticIntent
    );
    assert!(!ProgrammingComponent::TargetReference.descriptor().dynamics);
}

#[test]
fn malformed_scalar_domains_never_panic_or_produce_nonfinite_values() {
    for domain in [
        ScalarDomain::Bounded {
            bounds: crate::AttributeBounds { min: 1.0, max: 0.0 },
        },
        ScalarDomain::Cyclic {
            bounds: crate::AttributeBounds { min: 0.0, max: 0.0 },
        },
    ] {
        assert!(domain.constrain(0.5).is_err());
        assert!(!domain.contains(0.5));
    }
}
#[test]
fn native_spreads_preserve_low_bytes_and_middle_rank_anchors() {
    let binding = crate::NativeColorBinding {
        channel_id: Uuid::from_u128(1),
        function_id: Uuid::from_u128(2),
    };
    let value = NativeColorSpread {
        binding,
        points: vec![u32::MAX - 4, u32::MAX],
    };
    value
        .validate(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: u32::MAX,
            continuous: true,
        })
        .unwrap();
    assert_eq!(
        value.resolve(5).unwrap(),
        ((u32::MAX - 4)..=u32::MAX).collect::<Vec<_>>()
    );
    assert_eq!(
        NativeColorSpread {
            binding,
            points: vec![100, 0, 100]
        }
        .resolve(6)
        .unwrap(),
        vec![100, 50, 0, 0, 50, 100]
    );
    assert!(
        value
            .validate(NativeColorComponentDescriptor {
                binding,
                raw_from: 0,
                raw_to: u32::MAX,
                continuous: false
            })
            .is_err()
    );
}

#[test]
fn large_valid_physical_values_do_not_overflow_spread_intermediates() {
    assert_eq!(
        ScalarIntent::Spread(vec![f32::MAX, f32::MAX]).resolve(3, false),
        vec![f32::MAX; 3]
    );
    assert_eq!(
        ScalarIntent::Spread(vec![-f32::MAX, f32::MAX]).resolve(3, false),
        vec![-f32::MAX, 0.0, f32::MAX]
    );
    let intent = ColorIntent {
        relative_output: f32::MAX,
        recipe: VirtualColorRecipe {
            approximate: true,
            ..Default::default()
        },
        base_xyz: Xyz {
            x: f32::MAX,
            y: 0.0,
            z: 0.0,
        },
        ..Default::default()
    };
    intent.validate().unwrap();
    assert!(
        intent
            .blend_visible(crate::color_intent::D65_WHITE)
            .is_err()
    );
}

#[test]
fn immutable_owners_keep_slots_compact_and_clone_without_copying_recipes() {
    use crate::{AttributeKey, AttributeValue};
    use std::sync::Arc;
    assert!(std::mem::size_of::<AttributeValue>() <= 32);
    let value = AttributeValue::ColorProgram(Arc::new(direct(None, None)));
    let copied = value.clone();
    let (AttributeValue::ColorProgram(left), AttributeValue::ColorProgram(right)) =
        (&value, &copied)
    else {
        panic!("Color owner");
    };
    assert!(Arc::ptr_eq(left, right));
    assert!(
        value
            .validate_programming_address(&AttributeKey::color())
            .is_ok()
    );
    assert!(
        value
            .validate_programming_address(&AttributeKey("pan".into()))
            .is_err()
    );
    assert_eq!(
        serde_json::from_str::<AttributeValue>(&serde_json::to_string(&value).unwrap()).unwrap(),
        value
    );
    let angles = AttributeValue::Position(Arc::new(PositionIntent::angles(720.0, -90.0)));
    assert!(
        angles
            .validate_programming_address(&ProgrammingOwner::Position.key())
            .is_ok()
    );
    assert!(
        angles
            .validate_programming_address(&AttributeKey::color())
            .is_err()
    );
}

#[test]
fn tagged_owners_reject_fields_from_the_other_representation() {
    let mut position = serde_json::to_value(PositionIntent::angles(0.0, 90.0)).unwrap();
    position["reference"] = serde_json::json!({ "kind": "origin" });
    assert!(serde_json::from_value::<PositionIntent>(position).is_err());
    let mut color = serde_json::to_value(ColorProgram::Semantic {
        intent: Default::default(),
    })
    .unwrap();
    color["portable"] = serde_json::json!({});
    assert!(serde_json::from_value::<ColorProgram>(color).is_err());
}

#[test]
fn shrinking_live_group_keeps_exact_native_spread_endpoints() {
    let spread = NativeColorSpread {
        binding: crate::NativeColorBinding {
            channel_id: Uuid::from_u128(1),
            function_id: Uuid::from_u128(2),
        },
        points: vec![u32::MAX, 3, u32::MAX - 2, 7],
    };
    assert_eq!(spread.resolve(2).unwrap(), vec![u32::MAX, 7]);
    assert_eq!(spread.resolve(3).unwrap(), vec![u32::MAX, 2147483648, 7]);
}
