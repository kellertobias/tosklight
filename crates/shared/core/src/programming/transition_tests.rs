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

fn position(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn target(id: u128, offset: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(id),
        },
        offset,
    )))
}
fn color(intent: ColorIntent) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
}
fn semantic(value: &AttributeValue) -> &ColorIntent {
    let AttributeValue::ColorProgram(program) = value else {
        panic!()
    };
    let ColorProgram::Semantic { intent } = program.as_ref() else {
        panic!()
    };
    intent
}
fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
}

#[test]
fn physical_paths_keep_unwrapped_turns_and_live_target_references() {
    let transition =
        CompiledProgrammingTransition::new(position(0.0, -90.0), position(720.0, 90.0), None)
            .unwrap();
    assert_eq!(transition.sample(0.5).unwrap(), position(360.0, 0.0));
    let from = target(1, [-10.0, 2.0, 4.0]);
    let to = target(1, [10.0, -2.0, 6.0]);
    assert_eq!(
        interpolate_programming_value(&from, &to, 0.5).unwrap(),
        target(1, [0.0, 0.0, 5.0])
    );
    for (to, reason) in [
        (target(2, [0.0; 3]), TransitionRequirement::LiveTargetPoints),
        (position(0.0, 0.0), TransitionRequirement::LiveJointAngles),
    ] {
        let transition =
            CompiledProgrammingTransition::new(from.clone(), to.clone(), None).unwrap();
        assert_eq!(
            transition.sample(0.5),
            Err(TransitionError::Requires(reason))
        );
        assert_eq!(transition.endpoints(), (&from, &to));
        assert_eq!(transition.sample(1.0).unwrap(), to);
    }
}

#[test]
fn whole_color_uses_linear_xyz_reciprocal_kelvin_and_independent_uv() {
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
        kelvin: 10000.0,
        duv: 0.02,
    };
    b.white_blend = 1.0;
    b.relative_output = 4.0;
    b.uv.amount = 0.2;
    b.allocation = ColorAllocation::PreferWhite;
    let from = color(a);
    let to = color(b);
    let transition = CompiledProgrammingTransition::new(from.clone(), to.clone(), None).unwrap();
    let midpoint = transition.sample(0.5).unwrap();
    let intent = semantic(&midpoint);
    assert_eq!(
        intent.base_xyz,
        Xyz {
            x: 0.4,
            y: 0.2,
            z: 0.1
        }
    );
    assert!(intent.recipe.approximate);
    near(intent.white_target.kelvin, 3333.3333);
    assert_eq!(intent.white_target.duv, 0.0);
    assert_eq!(intent.white_blend, 0.5);
    assert_eq!(intent.relative_output, 2.0);
    near(intent.uv.amount, 0.5);
    assert_eq!(intent.allocation, ColorAllocation::PreserveRecipe);
    intent.validate().unwrap();
    let AttributeValue::ColorProgram(endpoint) = transition.sample(1.0).unwrap() else {
        panic!()
    };
    let AttributeValue::ColorProgram(original) = &to else {
        panic!()
    };
    assert!(Arc::ptr_eq(&endpoint, original));
    assert_eq!(transition.sample(-1.0).unwrap(), from);
    assert_eq!(transition.sample(2.0).unwrap(), to);
    assert!(transition.sample(f32::NAN).is_err());
}

#[test]
fn zoom_requires_consistent_angle_convention_and_focus_uses_real_endpoints() {
    let zoom = |degrees, convention| {
        AttributeValue::Zoom(Arc::new(ZoomIntent {
            opening_degrees: ScalarIntent::Value(degrees),
            convention,
        }))
    };
    let from = zoom(10.0, OpeningConvention::Beam);
    assert_eq!(
        interpolate_programming_value(&from, &zoom(60.0, OpeningConvention::Beam), 0.5).unwrap(),
        zoom(35.0, OpeningConvention::Beam)
    );
    assert_eq!(
        interpolate_programming_value(&from, &zoom(60.0, OpeningConvention::Field), 0.5),
        Err(TransitionError::Requires(
            TransitionRequirement::ZoomConvention
        ))
    );
    assert_eq!(
        interpolate_programming_value(
            &AttributeValue::Normalized(0.4),
            &AttributeValue::Normalized(0.8),
            0.5
        )
        .unwrap(),
        AttributeValue::Normalized(0.6)
    );
    assert_eq!(
        interpolate_programming_value(&position(-f32::MAX, 0.0), &position(f32::MAX, 0.0), 0.5)
            .unwrap(),
        position(0.0, 0.0)
    );
    let spread = AttributeValue::Position(Arc::new(PositionIntent::Angles {
        pan_degrees: ScalarIntent::Spread(vec![0.0, 720.0]),
        tilt_degrees: ScalarIntent::Value(0.0),
    }));
    assert!(matches!(
        CompiledProgrammingTransition::new(spread, position(0.0, 0.0), None),
        Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints
        ))
    ));
}

struct NativeModel {
    source: NativeColorIdentity,
    predictions: AtomicUsize,
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        let continuous = binding.channel_id == Uuid::from_u128(10);
        [10, 20]
            .contains(&binding.channel_id.as_u128())
            .then_some(NativeColorComponentDescriptor {
                binding,
                raw_from: if continuous { u32::MAX } else { 0 },
                raw_to: if continuous { 0 } else { 255 },
                continuous,
            })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        self.predictions.fetch_add(1, Ordering::Relaxed);
        // Deliberately nonlinear prediction proves the varying recipe is evaluated, not its saved
        // endpoint estimates interpolated. UV is unknown, and must stay distinct from known zero.
        let drive = f64::from(
            recipe
                .channels
                .iter()
                .find(|v| v.channel_id == Uuid::from_u128(10))
                .unwrap()
                .raw,
        ) / f64::from(u32::MAX);
        Ok(PortableColorEstimate {
            model_revision: self.source.model_revision,
            visible: Some(PortableVisibleColor {
                xyz: Xyz {
                    x: (drive * drive) as f32,
                    y: 0.0,
                    z: 0.0,
                },
                relative_output: 1.0,
            }),
            uv: None,
            quality: PhysicalDataQuality::Estimated,
            limitations: vec!["UV source unknown".into()],
        })
    }
}
fn native_model() -> Arc<NativeModel> {
    Arc::new(NativeModel {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(1),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            profile_revision: 1,
            profile_digest: "pinned-digest".into(),
            native_layout_signature: "two-channel-layout".into(),
            model_revision: 1,
        },
        predictions: AtomicUsize::new(0),
    })
}
fn direct(model: &NativeModel, level: u32, wheel: u32, reversed: bool) -> AttributeValue {
    let mut channels = vec![
        NativeColorValue {
            channel_id: Uuid::from_u128(10),
            function_id: Uuid::from_u128(11),
            raw: level,
        },
        NativeColorValue {
            channel_id: Uuid::from_u128(20),
            function_id: Uuid::from_u128(21),
            raw: wheel,
        },
    ];
    if reversed {
        channels.reverse();
    }
    let recipe = NativeColorRecipe {
        source: model.source.clone(),
        channels,
        spreads: vec![],
    };
    let portable = model.predict(&recipe).unwrap();
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct { recipe, portable }))
}
#[test]
fn direct_transition_preserves_u32_low_bytes_channel_identity_and_discrete_slots() {
    let model = native_model();
    let from = direct(&model, u32::MAX - 4, 17, false);
    let to = direct(&model, u32::MAX, 220, true);
    let transition =
        CompiledProgrammingTransition::new(from.clone(), to.clone(), Some(model.clone())).unwrap();
    model.predictions.store(0, Ordering::Relaxed);
    let AttributeValue::ColorProgram(midpoint) = transition.sample(0.5).unwrap() else {
        panic!()
    };
    let ColorProgram::Direct { recipe, portable } = midpoint.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].raw, u32::MAX - 2);
    assert_eq!(recipe.channels[1].raw, 17);
    assert_eq!(portable.uv, None);
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);
    assert_eq!(transition.sample(1.0).unwrap(), to);
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);
    let reverse = CompiledProgrammingTransition::new(to, from, Some(model)).unwrap();
    let AttributeValue::ColorProgram(midpoint) = reverse.sample(0.5).unwrap() else {
        panic!()
    };
    let ColorProgram::Direct { recipe, .. } = midpoint.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[1].raw, u32::MAX - 2);
    assert_eq!(recipe.channels[0].raw, 220);
}
#[test]
fn direct_transition_predicts_once_and_never_treats_a_layout_signature_as_model_identity() {
    let model = native_model();
    let from = direct(&model, 0, 0, false);
    let to = direct(&model, u32::MAX, 255, false);
    assert_eq!(
        interpolate_programming_value(&from, &to, 0.5),
        Err(TransitionError::Requires(
            TransitionRequirement::NativeColorModel
        ))
    );
    let transition =
        CompiledProgrammingTransition::new(from.clone(), to.clone(), Some(model.clone())).unwrap();
    let AttributeValue::ColorProgram(midpoint) = transition.sample(0.5).unwrap() else {
        panic!()
    };
    let ColorProgram::Direct { portable, .. } = midpoint.as_ref() else {
        panic!()
    };
    near(portable.visible.unwrap().xyz.x, 0.25);
    let mut wrong_model = native_model();
    Arc::get_mut(&mut wrong_model)
        .unwrap()
        .source
        .profile_digest = "different-digest".into();
    assert!(
        CompiledProgrammingTransition::new(from.clone(), to, Some(wrong_model.clone())).is_err()
    );
    let other_source = direct(&wrong_model, u32::MAX, 255, false);
    assert_eq!(
        interpolate_programming_value(&from, &other_source, 0.5),
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance
        ))
    );
    assert_eq!(
        interpolate_programming_value(&from, &color(ColorIntent::default()), 0.5),
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance
        ))
    );
    let invalid = direct(&model, 0, 256, false);
    assert!(CompiledProgrammingTransition::new(from, invalid, Some(model)).is_err());
}

#[test]
fn native_fade_rounding_is_exact_near_half_steps_in_both_directions() {
    let model = native_model();
    for (from, to, progress, expected) in [
        (u32::MAX, 0, 0.4999999403953552_f32, 2147483903),
        (0, u32::MAX, 0.4999999403953552_f32, 2147483392),
        (u32::MAX, 0, 0.5, 2147483648),
        (0, u32::MAX, 0.5, 2147483648),
        (u32::MAX, 0, f32::from_bits(1), u32::MAX),
    ] {
        let transition = CompiledProgrammingTransition::new(
            direct(&model, from, 0, false),
            direct(&model, to, 0, false),
            Some(model.clone()),
        )
        .unwrap();
        let AttributeValue::ColorProgram(sample) = transition.sample(progress).unwrap() else {
            panic!()
        };
        let ColorProgram::Direct { recipe, .. } = sample.as_ref() else {
            panic!()
        };
        assert_eq!(recipe.channels[0].raw, expected);
    }
}
