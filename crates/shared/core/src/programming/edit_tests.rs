use super::*;
use crate::{
    AttributeValue, NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality,
};
use std::{cell::Cell, sync::Arc};
use uuid::Uuid;

fn scalar(component: ProgrammingComponent, value: f32) -> ComponentEdit {
    ComponentEdit::Scalar {
        component,
        operation: ScalarEdit::Set(ScalarIntent::Value(value)),
    }
}
#[test]
fn position_edits_preserve_unwrapped_pose_and_require_explicit_target_adoption() {
    let base = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(7),
        },
        [1.0, 2.0, 3.0],
    )));
    let original = base.clone();
    let pan = ComponentEdit::Scalar {
        component: ProgrammingComponent::Pan,
        operation: ScalarEdit::Relative(90.0),
    };
    assert!(edit_family(&base, &[pan.clone()], &Default::default()).is_err());
    let context = FamilyEditContext {
        solved_angles: Some(JointAngles {
            pan_degrees: 710.0,
            tilt_degrees: -90.0,
        }),
        ..Default::default()
    };
    let angles = edit_family(&base, &[pan], &context).unwrap();
    assert_eq!(
        angles,
        AttributeValue::Position(Arc::new(PositionIntent::angles(800.0, -90.0)))
    );
    assert_eq!(base, original);
    assert!(
        edit_family(
            &angles,
            &[scalar(ProgrammingComponent::TargetX, -3.0)],
            &context
        )
        .is_err()
    );
    let target = edit_family(
        &angles,
        &[
            scalar(ProgrammingComponent::TargetX, -3.0),
            ComponentEdit::Target {
                reference: TargetReference::Origin,
            },
        ],
        &context,
    )
    .unwrap();
    assert_eq!(
        target,
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [-3.0, 0.0, 0.0]
        )))
    );
    assert!(
        edit_family(
            &angles,
            &[
                scalar(ProgrammingComponent::Pan, 1.0),
                ComponentEdit::Target {
                    reference: TargetReference::Origin
                }
            ],
            &context
        )
        .is_err()
    );
    assert!(edit_family(&base, &[], &context).is_ok_and(|value| value == base));
}
#[test]
fn orthogonal_color_edits_preserve_base_recipe_uv_and_zero_output() {
    let intent = ColorIntent {
        relative_output: 0.0,
        uv: UvIntent { amount: 0.8 },
        ..Default::default()
    };
    let base = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: intent.clone(),
    }));
    let edited = edit_family(
        &base,
        &[
            scalar(ProgrammingComponent::Color(ColorComponent::WhiteBlend), 1.0),
            scalar(
                ProgrammingComponent::Color(ColorComponent::Temperature),
                2200.0,
            ),
            scalar(ProgrammingComponent::Color(ColorComponent::Duv), -0.02),
        ],
        &Default::default(),
    )
    .unwrap();
    let AttributeValue::ColorProgram(program) = edited else {
        panic!()
    };
    let ColorProgram::Semantic { intent: after } = program.as_ref() else {
        panic!()
    };
    assert_eq!(after.base_xyz, intent.base_xyz);
    assert_eq!(after.recipe, intent.recipe);
    assert_eq!(after.uv, intent.uv);
    assert_eq!(after.relative_output, 0.0);
    assert_eq!(after.white_blend, 1.0);
    assert_eq!(
        after.white_target,
        WhiteTarget {
            kelvin: 2200.0,
            duv: -0.02
        }
    );
    assert!(
        edit_family(
            &base,
            &[
                scalar(ProgrammingComponent::Color(ColorComponent::Red), 0.5),
                scalar(ProgrammingComponent::Color(ColorComponent::Hue), 200.0),
            ],
            &Default::default()
        )
        .is_err()
    );
    assert!(
        edit_family(
            &base,
            &[scalar(ProgrammingComponent::TargetReference, 0.0)],
            &Default::default()
        )
        .is_err()
    );
}
#[test]
fn physical_relative_spreads_keep_units_and_atomic_overflow_checks() {
    assert_eq!(
        ScalarEdit::Relative(90.0)
            .apply(
                &ScalarIntent::Spread(vec![-720.0, 720.0]),
                ScalarDomain::Finite
            )
            .unwrap(),
        ScalarIntent::Spread(vec![-630.0, 810.0])
    );
    assert_eq!(
        ScalarEdit::Relative(20.0)
            .apply(
                &ScalarIntent::Spread(vec![350.0, 10.0]),
                ScalarDomain::DEGREES
            )
            .unwrap(),
        ScalarIntent::Spread(vec![10.0, 30.0])
    );
    assert_eq!(
        ScalarEdit::Relative(-1000.0)
            .apply(&ScalarIntent::Value(2000.0), ScalarDomain::KELVIN)
            .unwrap(),
        ScalarIntent::Value(1000.0)
    );
    assert!(
        ScalarEdit::Relative(f32::MAX)
            .apply(&ScalarIntent::Value(f32::MAX), ScalarDomain::Finite)
            .is_err()
    );
    assert_eq!(
        ScalarEdit::Relative(f32::MAX)
            .apply(&ScalarIntent::Value(1.0), ScalarDomain::UNIT)
            .unwrap(),
        ScalarIntent::Value(1.0)
    );
}
struct NativeSource {
    identity: NativeColorIdentity,
    descriptor: NativeColorComponentDescriptor,
    predictions: Cell<usize>,
}
impl NativeColorEditModel for NativeSource {
    fn source(&self) -> &NativeColorIdentity {
        &self.identity
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding == self.descriptor.binding).then_some(self.descriptor)
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        self.predictions.set(self.predictions.get() + 1);
        Ok(PortableColorEstimate {
            model_revision: recipe.source.model_revision,
            visible: None,
            uv: Some(PortableUv {
                amount: 0.5,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
        })
    }
}
#[test]
fn native_edits_keep_low_bytes_and_predict_the_whole_recipe_once() {
    let source = NativeSource {
        identity: NativeColorIdentity {
            profile_id: Uuid::from_u128(1),
            profile_revision: 1,
            profile_digest: "digest".into(),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            model_revision: 9,
            native_layout_signature: "layout".into(),
        },
        descriptor: NativeColorComponentDescriptor {
            binding: NativeColorBinding {
                channel_id: Uuid::from_u128(5),
                function_id: Uuid::from_u128(6),
            },
            raw_from: u32::MAX - 5,
            raw_to: u32::MAX,
            continuous: true,
        },
        predictions: Cell::new(0),
    };
    let base = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: source.identity.clone(),
            channels: vec![NativeColorValue {
                channel_id: source.descriptor.binding.channel_id,
                function_id: source.descriptor.binding.function_id,
                raw: u32::MAX - 1,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 9,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
        },
    }));
    let edits = [ComponentEdit::Native {
        binding: source.descriptor.binding,
        operation: NativeColorEdit::Relative(-1),
    }];
    let context = FamilyEditContext {
        native_model: Some(&source),
        ..Default::default()
    };
    let changed = edit_family(&base, &edits, &context).unwrap();
    let AttributeValue::ColorProgram(value) = changed else {
        panic!()
    };
    let ColorProgram::Direct { recipe, portable } = value.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].raw, u32::MAX - 2);
    assert_eq!(portable.uv.unwrap().amount, 0.5);
    assert_eq!(source.predictions.get(), 1);
    assert!(
        edit_family(
            &base,
            &[ComponentEdit::Native {
                binding: source.descriptor.binding,
                operation: NativeColorEdit::Spread(vec![])
            }],
            &context
        )
        .is_err()
    );
    assert_eq!(source.predictions.get(), 1);
    assert!(
        edit_family(
            &base,
            &[
                edits[0].clone(),
                scalar(ProgrammingComponent::Color(ColorComponent::Uv), 0.0)
            ],
            &context
        )
        .is_err()
    );
}

#[test]
fn no_op_pan_reset_preserves_target_but_explicit_angle_activation_takes_over() {
    let base = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [0.0; 3],
    )));
    let context = FamilyEditContext {
        solved_angles: Some(JointAngles {
            pan_degrees: 0.0,
            tilt_degrees: 90.0,
        }),
        ..Default::default()
    };
    assert_eq!(
        edit_family(&base, &[scalar(ProgrammingComponent::Pan, 0.0)], &context).unwrap(),
        base
    );
    assert_eq!(
        edit_family(&base, &[ComponentEdit::ActivateAngles], &context).unwrap(),
        AttributeValue::Position(Arc::new(PositionIntent::angles(0.0, 90.0)))
    );
}

#[test]
fn focus_and_zoom_edits_spreads_and_relative_steps_stay_in_their_own_owner_and_units() {
    let zoom = |opening_degrees, convention| {
        AttributeValue::Zoom(Arc::new(ZoomIntent {
            opening_degrees,
            convention,
        }))
    };
    let field = |degrees| {
        zoom(
            ScalarIntent::Value(degrees),
            crate::OpeningConvention::Field,
        )
    };
    let relative = |component, delta| ComponentEdit::Scalar {
        component,
        operation: ScalarEdit::Relative(delta),
    };
    let context = FamilyEditContext::default();
    // Zoom: degrees in, degrees out, convention preserved, bounded at 180°.
    let base = field(20.);
    assert_eq!(
        edit_family(&base, &[relative(ProgrammingComponent::Zoom, 5.)], &context).unwrap(),
        field(25.)
    );
    assert_eq!(
        edit_family(
            &base,
            &[relative(ProgrammingComponent::Zoom, 500.)],
            &context
        )
        .unwrap(),
        field(180.)
    );
    // Focus: normalized in, normalized out, bounded at 0..1; never a Zoom value.
    let focus = AttributeValue::Normalized(0.5);
    assert_eq!(
        edit_family(
            &focus,
            &[relative(ProgrammingComponent::Focus, 0.7)],
            &context
        )
        .unwrap(),
        AttributeValue::Normalized(1.)
    );
    // One transaction addresses one owner: Focus and Zoom are never edited together, and a
    // Focus edit cannot seed from, or rewrite, a Zoom value.
    assert!(
        edit_family(
            &base,
            &[
                relative(ProgrammingComponent::Zoom, 1.),
                relative(ProgrammingComponent::Focus, 0.1)
            ],
            &context
        )
        .is_err()
    );
    assert!(
        edit_family(
            &base,
            &[relative(ProgrammingComponent::Focus, 0.1)],
            &context
        )
        .is_err()
    );
    assert!(
        edit_family(
            &focus,
            &[relative(ProgrammingComponent::Zoom, 1.)],
            &context
        )
        .is_err()
    );

    // Spreads/Align: a Zoom spread materializes per rank in degrees with its convention; a
    // relative step shifts every point; a Focus spread materializes normalized values.
    let spread = edit_family(
        &base,
        &[scalar_spread(ProgrammingComponent::Zoom, vec![10., 40.])],
        &context,
    )
    .unwrap();
    let shifted = edit_family(
        &spread,
        &[relative(ProgrammingComponent::Zoom, 2.)],
        &context,
    )
    .unwrap();
    assert_eq!(
        shifted,
        zoom(
            ScalarIntent::Spread(vec![12., 42.]),
            crate::OpeningConvention::Field
        )
    );
    let ranks = compile_programming_spread(&spread, 4, &context).unwrap();
    for (rank, degrees) in [10., 20., 30., 40.].into_iter().enumerate() {
        assert_eq!(ranks.at_rank(rank), Some(&field(degrees)), "rank {rank}");
    }
    let focus_spread = edit_family(
        &focus,
        &[scalar_spread(ProgrammingComponent::Focus, vec![0., 1.])],
        &context,
    )
    .unwrap();
    assert_eq!(focus_spread, AttributeValue::Spread(vec![0., 1.]));
    let ranks = compile_programming_spread(&focus_spread, 3, &context).unwrap();
    assert_eq!(ranks.at_rank(1), Some(&AttributeValue::Normalized(0.5)));
}

fn scalar_spread(component: ProgrammingComponent, points: Vec<f32>) -> ComponentEdit {
    ComponentEdit::Scalar {
        component,
        operation: ScalarEdit::Set(ScalarIntent::Spread(points)),
    }
}
