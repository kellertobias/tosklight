use super::*;
use crate::{
    AttributeValue, NativeColorBinding, NativeColorIdentity, NativeColorValue, OpeningConvention,
    PhysicalDataQuality, Xyz,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use uuid::Uuid;

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target(point: u128, offset: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(point),
        },
        offset,
    )))
}
fn zoom(degrees: f32, convention: OpeningConvention) -> AttributeValue {
    AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(degrees),
        convention,
    }))
}
fn transition(from: AttributeValue, to: AttributeValue) -> CompiledProgrammingTransition {
    CompiledProgrammingTransition::new(from, to, None).unwrap()
}

#[test]
fn whole_size_extrapolates_unwrapped_angles_and_same_target_offsets() {
    let from = angles(0.0, -90.0);
    let to = angles(720.0, 90.0);
    let scaled = transition(from.clone(), to.clone());
    assert_eq!(scaled.scale(0.0).unwrap(), from);
    assert_eq!(scaled.scale(0.5).unwrap(), angles(360.0, 0.0));
    assert_eq!(scaled.scale(1.0).unwrap(), to);
    assert_eq!(scaled.scale(2.0).unwrap(), angles(1440.0, 270.0));
    assert!(scaled.scale(-0.1).is_err());
    assert!(scaled.scale(f32::NAN).is_err());
    assert!(
        transition(angles(-f32::MAX, 0.0), angles(f32::MAX, 0.0))
            .scale(2.0)
            .is_err()
    );

    let scaled = transition(target(1, [-10.0, 2.0, 4.0]), target(1, [10.0, -2.0, 6.0]));
    assert_eq!(scaled.scale(0.5).unwrap(), target(1, [0.0, 0.0, 5.0]));
    assert_eq!(scaled.scale(2.0).unwrap(), target(1, [30.0, -6.0, 8.0]));
    let unresolved = transition(target(1, [0.0; 3]), target(2, [0.0; 3]));
    assert_eq!(unresolved.scale(0.0).unwrap(), target(1, [0.0; 3]));
    assert_eq!(unresolved.scale(1.0).unwrap(), target(2, [0.0; 3]));
    assert_eq!(
        unresolved.scale(0.5),
        Err(TransitionError::Requires(
            TransitionRequirement::LiveTargetPoints
        )),
    );
    assert_eq!(
        transition(target(1, [0.0; 3]), angles(0.0, 0.0)).scale(2.0),
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles
        )),
    );
}

#[test]
fn whole_size_constrains_focus_and_zoom_without_changing_convention() {
    let focus = transition(
        AttributeValue::Normalized(0.4),
        AttributeValue::Normalized(0.8),
    );
    assert_eq!(focus.scale(0.5).unwrap(), AttributeValue::Normalized(0.6));
    assert_eq!(focus.scale(2.0).unwrap(), AttributeValue::Normalized(1.0));
    let beam = transition(
        zoom(10.0, OpeningConvention::Beam),
        zoom(60.0, OpeningConvention::Beam),
    );
    assert_eq!(
        beam.scale(0.5).unwrap(),
        zoom(35.0, OpeningConvention::Beam)
    );
    assert_eq!(
        beam.scale(2.0).unwrap(),
        zoom(110.0, OpeningConvention::Beam)
    );
    assert_eq!(
        beam.scale(10.0).unwrap(),
        zoom(180.0, OpeningConvention::Beam)
    );
    assert_eq!(
        transition(
            zoom(10.0, OpeningConvention::Beam),
            zoom(60.0, OpeningConvention::Field)
        )
        .scale(2.0),
        Err(TransitionError::Requires(
            TransitionRequirement::ZoomConvention
        )),
    );
}

#[test]
fn semantic_size_constrains_reciprocal_kelvin_before_inverse_and_keeps_uv_independent() {
    let mut a = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_coordinates(
            &mut a,
            Xyz {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        )
        .unwrap();
    a.white_target = WhiteTarget {
        kelvin: 2000.0,
        duv: -0.02,
    };
    a.relative_output = 0.0;
    a.uv.amount = 0.8;
    let mut b = a.clone();
    VirtualColorAuthoringV1
        .set_coordinates(
            &mut b,
            Xyz {
                x: 0.8,
                y: 0.4,
                z: 0.2,
            },
        )
        .unwrap();
    b.white_target = WhiteTarget {
        kelvin: 10_000.0,
        duv: 0.02,
    };
    b.white_blend = 1.0;
    b.relative_output = 4.0;
    b.uv.amount = 0.2;
    b.allocation = ColorAllocation::PreferWhite;
    b.wheel_constraints.push(ColorWheelConstraint {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(1),
            profile_revision: 1,
            profile_digest: "wheel-original".into(),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            model_revision: 1,
            native_layout_signature: "wheel-layout".into(),
        },
        value: NativeColorValue {
            channel_id: Uuid::from_u128(5),
            function_id: Uuid::from_u128(6),
            raw: 17,
        },
    });
    let from = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent: a }));
    let to = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent: b }));
    let scaled = transition(from.clone(), to.clone());
    let AttributeValue::ColorProgram(zero) = scaled.scale(0.0).unwrap() else {
        panic!()
    };
    let AttributeValue::ColorProgram(one) = scaled.scale(1.0).unwrap() else {
        panic!()
    };
    let AttributeValue::ColorProgram(original_from) = &from else {
        panic!()
    };
    let AttributeValue::ColorProgram(original_to) = &to else {
        panic!()
    };
    assert!(Arc::ptr_eq(&zero, original_from));
    assert!(Arc::ptr_eq(&one, original_to));
    let AttributeValue::ColorProgram(half) = scaled.scale(0.5).unwrap() else {
        panic!()
    };
    let ColorProgram::Semantic { intent: half } = half.as_ref() else {
        panic!()
    };
    assert!((half.white_target.kelvin - 3333.3333).abs() < 0.01);
    assert_eq!(half.white_target.duv, 0.0);
    assert!((half.uv.amount - 0.5).abs() < 0.00001);
    assert!(half.wheel_constraints.is_empty());
    assert_eq!(half.allocation, ColorAllocation::PreserveRecipe);
    let AttributeValue::ColorProgram(double) = scaled.scale(2.0).unwrap() else {
        panic!()
    };
    let ColorProgram::Semantic { intent: double } = double.as_ref() else {
        panic!()
    };
    assert_eq!(double.white_target.kelvin, 20_000.0);
    assert_eq!(double.white_target.duv, 0.03);
    assert_eq!(double.white_blend, 1.0);
    assert_eq!(double.uv.amount, 0.0);
    assert_eq!(double.relative_output, 8.0);
    assert_eq!(double.allocation, ColorAllocation::PreferWhite);
    assert_eq!(double.wheel_constraints.len(), 1);
    double.validate().unwrap();
}

struct NativeModel {
    source: NativeColorIdentity,
    upper: u32,
    predictions: AtomicUsize,
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        let channel = binding.channel_id.as_u128();
        ([10, 20].contains(&channel)).then_some(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: if channel == 10 { self.upper } else { 255 },
            continuous: channel == 10,
        })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        require(
            recipe.source == self.source,
            "native prediction source changed",
        )?;
        self.predictions.fetch_add(1, Ordering::Relaxed);
        Ok(PortableColorEstimate {
            model_revision: self.source.model_revision,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec!["appearance unknown".into()],
        })
    }
}
fn native_model(upper: u32) -> Arc<NativeModel> {
    Arc::new(NativeModel {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(11),
            profile_revision: 3,
            profile_digest: "original-source".into(),
            mode_id: Uuid::from_u128(12),
            head_id: Uuid::from_u128(13),
            path_id: Uuid::from_u128(14),
            model_revision: 5,
            native_layout_signature: "u32-and-wheel".into(),
        },
        upper,
        predictions: AtomicUsize::new(0),
    })
}
fn direct(model: &NativeModel, drive: u32, wheel: u32) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: model.source.clone(),
            channels: vec![
                NativeColorValue {
                    channel_id: Uuid::from_u128(10),
                    function_id: Uuid::from_u128(11),
                    raw: drive,
                },
                NativeColorValue {
                    channel_id: Uuid::from_u128(20),
                    function_id: Uuid::from_u128(21),
                    raw: wheel,
                },
            ],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 5,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec!["stored source unknown".into()],
        },
    }))
}
#[test]
fn native_size_preserves_low_integer_bits_discrete_wheel_and_unknown_appearance() {
    let model = native_model(u32::MAX);
    let from = direct(&model, u32::MAX - 4, 17);
    let to = direct(&model, u32::MAX, 220);
    let scaled =
        CompiledProgrammingTransition::new(from.clone(), to.clone(), Some(model.clone())).unwrap();
    assert_eq!(scaled.scale(0.0).unwrap(), from);
    assert_eq!(scaled.scale(1.0).unwrap(), to);
    assert_eq!(model.predictions.load(Ordering::Relaxed), 0);
    let AttributeValue::ColorProgram(half) = scaled.scale(0.5).unwrap() else {
        panic!()
    };
    let ColorProgram::Direct { recipe, portable } = half.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].raw, u32::MAX - 2);
    assert_eq!(recipe.channels[1].raw, 17);
    assert_eq!(portable.visible, None);
    assert_eq!(portable.uv, None);
    let AttributeValue::ColorProgram(double) = scaled.scale(2.0).unwrap() else {
        panic!()
    };
    let ColorProgram::Direct { recipe, .. } = double.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].raw, u32::MAX);
    assert_eq!(recipe.channels[1].raw, 220);
    assert_eq!(model.predictions.load(Ordering::Relaxed), 2);

    let bounded = native_model(200);
    let scaled = CompiledProgrammingTransition::new(
        direct(&bounded, 120, 17),
        direct(&bounded, 150, 220),
        Some(bounded),
    )
    .unwrap();
    let AttributeValue::ColorProgram(double) = scaled.scale(10.0).unwrap() else {
        panic!()
    };
    let ColorProgram::Direct { recipe, .. } = double.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].raw, 200);
}

#[test]
fn native_size_requires_pinned_model_and_rejects_other_source_identity() {
    let model = native_model(u32::MAX);
    let from = direct(&model, 0, 0);
    let to = direct(&model, u32::MAX, 255);
    assert_eq!(
        transition(from.clone(), to.clone()).scale(0.5),
        Err(TransitionError::Requires(
            TransitionRequirement::NativeColorModel
        ))
    );
    let mut wrong = native_model(u32::MAX);
    Arc::get_mut(&mut wrong).unwrap().source.profile_digest = "different-source".into();
    assert!(CompiledProgrammingTransition::new(from.clone(), to, Some(wrong.clone())).is_err());
    assert_eq!(
        transition(from.clone(), direct(&wrong, 100, 10)).scale(2.0),
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance
        ))
    );
    assert_eq!(
        transition(
            from,
            AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                intent: ColorIntent::default(),
            }))
        )
        .scale(0.5),
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance
        ))
    );
}
