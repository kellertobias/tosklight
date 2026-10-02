use super::*;
use crate::{
    NativeColorBinding, NativeColorIdentity, NativeColorValue, OpeningConvention,
    PhysicalDataQuality, Xyz,
};
use std::sync::atomic::{AtomicUsize, Ordering};

type F = ProgrammingTraceField;
type S = ProgrammingFieldScope;

fn semantic(intent: ColorIntent) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
}

fn target(id: u128, offsets: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(id),
        },
        offsets,
    )))
}

fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}

fn zoom(degrees: f32, convention: OpeningConvention) -> AttributeValue {
    AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(degrees),
        convention,
    }))
}

fn compiled(from: AttributeValue, to: AttributeValue) -> CompiledProgrammingTransition {
    CompiledProgrammingTransition::new(from, to, None).unwrap()
}

#[test]
fn field_sets_are_canonical_on_construction_and_deserialization() {
    let scope = S::new([
        F::ColorWheel(2),
        F::WhiteBlend,
        F::ColorWheels,
        F::WhiteBlend,
    ]);
    assert_eq!(scope, S::new([F::ColorWheels, F::WhiteBlend]));
    assert!(scope.contains(F::ColorWheel(37)));
    let serialized = serde_json::to_value(&scope).unwrap();
    let mut redundant = serialized.as_array().unwrap().clone();
    redundant.reverse();
    redundant.push(serde_json::to_value(F::ColorWheel(2)).unwrap());
    redundant.push(serde_json::to_value(F::WhiteBlend).unwrap());
    let restored: S = serde_json::from_value(redundant.into()).unwrap();
    assert_eq!(scope, restored);
    assert_eq!(serialized, serde_json::to_value(restored).unwrap());
    assert!(scope.validate(ProgrammingOwner::Color).is_ok());
    assert!(scope.validate(ProgrammingOwner::Position).is_err());
    let invalid = S::new([F::NativeColorChannel(Uuid::nil())]);
    assert!(invalid.validate(ProgrammingOwner::Color).is_err());
    assert!(serde_json::from_value::<S>(serde_json::to_value(invalid).unwrap()).is_err());
    assert!(S::empty().validate(ProgrammingOwner::Color).is_ok());
}

#[test]
fn wheel_set_operations_preserve_specificity_and_reject_inexpressible_difference() {
    let all = S::new([F::ColorWheels, F::WhiteBlend]);
    let wheel = S::new([F::ColorWheel(7)]);
    assert_eq!(all.intersection(&wheel), wheel);
    assert_eq!(wheel.intersection(&all), wheel);
    assert_eq!(all.union(&wheel), all);
    assert!(all.difference(&wheel).is_err());
    assert_eq!(wheel.difference(&all).unwrap(), S::empty());
    assert_eq!(
        all.difference(&S::new([F::ColorWheels])).unwrap(),
        S::new([F::WhiteBlend])
    );
    assert_eq!(
        S::new([F::Pan, F::Tilt])
            .difference(&S::new([F::Pan]))
            .unwrap(),
        S::new([F::Tilt])
    );
}

#[test]
fn component_scopes_and_whole_scopes_distinguish_actual_representations() {
    assert_eq!(
        S::from_component(ProgrammingComponent::Color(ColorComponent::Amber)),
        S::new([F::ColorXyz, F::ColorRecipeAmber])
    );
    assert_eq!(
        S::from_component(ProgrammingComponent::Color(ColorComponent::Hue)),
        S::new([
            F::ColorXyz,
            F::ColorRecipeRed,
            F::ColorRecipeGreen,
            F::ColorRecipeBlue
        ])
    );
    let angle = S::for_value(ProgrammingOwner::Position, &angles(0.0, 0.0)).unwrap();
    assert_eq!(angle, S::new([F::Pan, F::Tilt]));
    let target = S::for_value(ProgrammingOwner::Position, &target(1, [0.0; 3])).unwrap();
    assert_eq!(
        target,
        S::new([F::TargetReference, F::TargetX, F::TargetY, F::TargetZ])
    );
    assert!(S::for_value(ProgrammingOwner::Color, &angles(0.0, 0.0)).is_err());
    for invalid in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(
            S::for_value(
                ProgrammingOwner::Focus,
                &AttributeValue::Normalized(invalid)
            )
            .is_err()
        );
    }
    let color = S::for_value(ProgrammingOwner::Color, &semantic(ColorIntent::default())).unwrap();
    assert!(color.contains(F::Allocation));
    assert!(color.contains(F::ColorWheel(5)));
    assert!(!color.contains(F::NativePrediction));
}

#[test]
fn transfers_run_forward_and_reverse_without_changing_original_authorship_fields() {
    let transfer = ProgrammingFieldTransfer {
        identity: S::new([F::ColorXyz, F::WhiteBlend, F::ColorWheels]),
        remap: vec![
            (F::ColorXyz, F::ColorRecipeRed),
            (F::ColorXyz, F::ColorRecipeGreen),
            (F::ColorXyz, F::ColorRecipeBlue),
        ]
        .into(),
    };
    let original = S::from_component(ProgrammingComponent::Color(ColorComponent::Amber));
    assert_eq!(
        transfer.forward(&original),
        S::new([
            F::ColorXyz,
            F::ColorRecipeRed,
            F::ColorRecipeGreen,
            F::ColorRecipeBlue
        ])
    );
    assert_eq!(
        transfer.reverse(&S::new([F::ColorRecipeRed])),
        S::new([F::ColorXyz])
    );
    assert_eq!(transfer.reverse(&S::new([F::ColorRecipeAmber])), S::empty());
    assert_eq!(
        transfer.reverse(&S::new([F::ColorWheel(3)])),
        S::new([F::ColorWheel(3)])
    );
    assert_eq!(original, S::new([F::ColorXyz, F::ColorRecipeAmber]));
    let roundtrip: ProgrammingFieldTransfer =
        serde_json::from_str(&serde_json::to_string(&transfer).unwrap()).unwrap();
    assert_eq!(roundtrip, transfer);
}

#[test]
fn semantic_reconstruction_remaps_xyz_and_keeps_allocation_and_wheels_outgoing() {
    let mut a = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_base_component(&mut a, ColorComponent::Amber, 0.7)
        .unwrap();
    let mut b = ColorIntent::default();
    b.allocation = ColorAllocation::PreferWhite;
    b.white_blend = 1.0;
    let from = semantic(a);
    let to = semantic(b);
    let transition = compiled(from.clone(), to.clone());
    let (value, trace) = transition
        .sample_with_trace(ProgrammingOwner::Color, 0.5)
        .unwrap();
    assert_eq!(value, transition.sample(0.5).unwrap());
    let trace = trace.unwrap();
    let authored_amber = S::from_component(ProgrammingComponent::Color(ColorComponent::Amber));
    for transfer in [&trace.from, &trace.to] {
        assert_eq!(
            transfer.forward(&authored_amber),
            S::new([
                F::ColorXyz,
                F::ColorRecipeRed,
                F::ColorRecipeGreen,
                F::ColorRecipeBlue
            ])
        );
        assert!(transfer.reverse(&S::new([F::ColorRecipeAmber])).is_empty());
        assert!(transfer.identity.contains(F::WhiteBlend));
    }
    assert!(trace.from.identity.contains(F::Allocation));
    assert!(trace.from.identity.contains(F::ColorWheel(1)));
    assert!(!trace.to.identity.contains(F::Allocation));
    assert!(!trace.to.identity.contains(F::ColorWheel(1)));
    let repeated = transition
        .sample_trace(ProgrammingOwner::Color, 0.75)
        .unwrap();
    assert!(Arc::ptr_eq(&trace.from.remap, &repeated.from.remap));
    assert!(std::ptr::eq(
        trace.from.identity.fields(),
        repeated.from.identity.fields()
    ));
    assert_eq!(
        trace,
        interpolate_programming_trace(ProgrammingOwner::Color, &from, &to, 0.5).unwrap()
    );
}

#[test]
fn interrupted_semantic_trace_retains_xyz_lineage_but_retires_previous_held_fields() {
    let a = semantic(ColorIntent::default());
    let mut b = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_base_component(&mut b, ColorComponent::Blue, 0.0)
        .unwrap();
    let first = compiled(a.clone(), semantic(b));
    let (middle, first_trace) = first
        .sample_with_trace(ProgrammingOwner::Color, 0.5)
        .unwrap();
    let first_trace = first_trace.unwrap();
    let original_a = first_trace
        .from
        .forward(&S::for_value(ProgrammingOwner::Color, &a).unwrap());
    let mut c = ColorIntent::default();
    c.allocation = ColorAllocation::PreferColoredEmitters;
    let second = compiled(semantic(c), middle);
    let second_trace = second.sample_trace(ProgrammingOwner::Color, 0.5).unwrap();
    let retained_a = second_trace.to.forward(&original_a);
    assert!(retained_a.contains(F::ColorXyz));
    assert!(retained_a.contains(F::ColorRecipeRed));
    assert!(!retained_a.contains(F::Allocation));
    assert!(!retained_a.contains(F::ColorWheels));
}

#[test]
fn equal_semantic_endpoints_keep_distinct_continuous_occurrences_and_recipe_identity() {
    let value = semantic(ColorIntent::default());
    let transition = compiled(value.clone(), value);
    for factor in [0.5, 2.0] {
        let trace = transition
            .scale_trace(ProgrammingOwner::Color, factor)
            .unwrap();
        for field in [
            F::ColorXyz,
            F::ColorRecipeRed,
            F::ColorRecipeAmber,
            F::WhiteBlend,
            F::Temperature,
            F::Duv,
            F::Uv,
            F::RelativeOutput,
        ] {
            assert!(trace.from.identity.contains(field));
            assert!(trace.to.identity.contains(field));
        }
        assert_eq!(trace.from.identity.contains(F::Allocation), factor < 1.0);
        assert_eq!(trace.to.identity.contains(F::ColorWheels), factor > 1.0);
        assert!(trace.from.remap.is_empty());
        assert!(trace.to.remap.is_empty());
    }
    let endpoint = transition
        .scale_trace(ProgrammingOwner::Color, 1.0)
        .unwrap();
    assert!(endpoint.from.identity.is_empty());
    assert!(endpoint.to.identity.contains(F::Allocation));
}

#[test]
fn physical_traces_preserve_same_reference_and_convention_while_blending_numbers() {
    let transition = compiled(target(1, [0.0; 3]), target(1, [1.0; 3]));
    for factor in [0.5, 2.0] {
        let trace = transition
            .scale_trace(ProgrammingOwner::Position, factor)
            .unwrap();
        assert_eq!(
            trace.from.identity,
            S::new([F::TargetReference, F::TargetX, F::TargetY, F::TargetZ])
        );
        assert_eq!(
            trace.to.identity,
            S::new([F::TargetX, F::TargetY, F::TargetZ])
        );
    }
    for (owner, transition, continuous, held) in [
        (
            ProgrammingOwner::Position,
            compiled(angles(0.0, 0.0), angles(720.0, 90.0)),
            S::new([F::Pan, F::Tilt]),
            S::empty(),
        ),
        (
            ProgrammingOwner::Focus,
            compiled(
                AttributeValue::Normalized(0.2),
                AttributeValue::Normalized(0.7),
            ),
            S::new([F::Focus]),
            S::empty(),
        ),
        (
            ProgrammingOwner::Zoom,
            compiled(
                zoom(10.0, OpeningConvention::Beam),
                zoom(60.0, OpeningConvention::Beam),
            ),
            S::new([F::Zoom]),
            S::new([F::ZoomConvention]),
        ),
    ] {
        for factor in [0.5, 2.0] {
            let trace = transition.scale_trace(owner, factor).unwrap();
            assert_eq!(trace.from.identity, continuous.union(&held));
            assert_eq!(trace.to.identity, continuous);
        }
    }
}

#[test]
fn trace_errors_keep_explicit_live_requirements_and_value_success_is_independent() {
    let transition = compiled(target(1, [0.0; 3]), target(2, [0.0; 3]));
    assert_eq!(
        transition.sample_trace(ProgrammingOwner::Position, 0.5),
        Err(TransitionError::Requires(
            TransitionRequirement::LiveTargetPoints
        ))
    );
    assert!(
        transition
            .sample_trace(ProgrammingOwner::Position, 1.0)
            .is_ok()
    );
    assert!(
        transition
            .sample_trace(ProgrammingOwner::Position, f32::NAN)
            .is_err()
    );
    let focus = compiled(
        AttributeValue::Normalized(0.0),
        AttributeValue::Normalized(1.0),
    );
    let (value, trace) = focus
        .sample_with_trace(ProgrammingOwner::Color, 0.5)
        .unwrap();
    assert_eq!(value, AttributeValue::Normalized(0.5));
    assert!(trace.is_none());
    // A mistaken first owner cannot poison later correct queries in the lazy plan cache.
    assert!(focus.sample_trace(ProgrammingOwner::Focus, 0.5).is_ok());
    let spread = AttributeValue::Position(Arc::new(PositionIntent::Angles {
        pan_degrees: ScalarIntent::Spread(vec![0.0, 1.0]),
        tilt_degrees: ScalarIntent::Value(0.0),
    }));
    assert_eq!(
        interpolate_programming_trace(ProgrammingOwner::Position, &spread, &angles(1.0, 0.0), 1.0),
        Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints
        ))
    );
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
        Some(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: 255,
            continuous: binding.channel_id != Uuid::from_u128(20),
        })
    }
    fn predict(&self, _: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        self.predictions.fetch_add(1, Ordering::Relaxed);
        Ok(PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
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
            profile_digest: "trace-model".into(),
            native_layout_signature: "three-channels".into(),
            model_revision: 1,
        },
        predictions: AtomicUsize::new(0),
    })
}
fn direct(model: &NativeModel, raw: u32, switched: bool) -> AttributeValue {
    let recipe = NativeColorRecipe {
        source: model.source.clone(),
        spreads: vec![],
        channels: [10, 20, 30]
            .into_iter()
            .map(|channel| NativeColorValue {
                channel_id: Uuid::from_u128(channel),
                function_id: Uuid::from_u128(
                    channel + if switched && channel == 30 { 2 } else { 1 },
                ),
                raw,
            })
            .collect(),
    };
    let portable = model.predict(&recipe).unwrap();
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct { recipe, portable }))
}

#[test]
fn compiled_native_trace_uses_channel_continuity_and_prediction_inputs_once() {
    let model = native_model();
    let transition = CompiledProgrammingTransition::new(
        direct(&model, 20, false),
        direct(&model, 100, true),
        Some(model.clone()),
    )
    .unwrap();
    model.predictions.store(0, Ordering::Relaxed);
    let (_, trace) = transition
        .sample_with_trace(ProgrammingOwner::Color, 0.5)
        .unwrap();
    let trace = trace.unwrap();
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);
    let all_channels = [10, 20, 30].map(|id| F::NativeColorChannel(Uuid::from_u128(id)));
    assert_eq!(
        trace.from.identity,
        S::new(all_channels.into_iter().chain([F::NativeColorIdentity]))
    );
    assert_eq!(trace.to.identity, S::new([all_channels[0]]));
    let prediction = S::new([F::NativePrediction]);
    assert_eq!(trace.from.reverse(&prediction), trace.from.identity);
    assert_eq!(trace.to.reverse(&prediction), S::new([all_channels[0]]));
    assert!(!trace.from.identity.contains(F::NativePrediction));
    assert!(!trace.to.identity.contains(F::NativePrediction));
    let (_, scaled) = transition
        .scale_with_trace(ProgrammingOwner::Color, 2.0)
        .unwrap();
    let scaled = scaled.unwrap();
    assert_eq!(model.predictions.load(Ordering::Relaxed), 2);
    assert_eq!(
        scaled.from.identity,
        S::new([F::NativeColorIdentity, all_channels[0]])
    );
    assert_eq!(scaled.to.identity, S::new(all_channels));
    assert_eq!(scaled.to.reverse(&prediction), S::new(all_channels));
    let (_, exact) = transition
        .scale_with_trace(ProgrammingOwner::Color, 1.0)
        .unwrap();
    let exact = exact.unwrap();
    assert_eq!(model.predictions.load(Ordering::Relaxed), 2);
    assert!(exact.from.identity.is_empty());
    assert!(exact.to.identity.contains(F::NativePrediction));
    assert!(exact.to.remap.is_empty());
}

#[test]
fn equal_native_optimization_preserves_occurrences_and_stored_prediction_lineage() {
    let model = native_model();
    let value = direct(&model, 20, false);
    let unknown = compiled(value.clone(), value.clone());
    let (sample, trace) = unknown
        .sample_with_trace(ProgrammingOwner::Color, 0.5)
        .unwrap();
    assert_eq!(sample, value);
    assert!(trace.is_none());
    assert_eq!(
        unknown.sample_trace(ProgrammingOwner::Color, 0.5),
        Err(TransitionError::Requires(
            TransitionRequirement::NativeColorModel
        ))
    );
    let transition =
        CompiledProgrammingTransition::new(value.clone(), value, Some(model.clone())).unwrap();
    model.predictions.store(0, Ordering::Relaxed);
    let (_, trace) = transition
        .sample_with_trace(ProgrammingOwner::Color, 0.5)
        .unwrap();
    let trace = trace.unwrap();
    assert_eq!(model.predictions.load(Ordering::Relaxed), 0);
    for transfer in [&trace.from, &trace.to] {
        assert!(
            transfer
                .identity
                .contains(F::NativeColorChannel(Uuid::from_u128(10)))
        );
        assert!(transfer.identity.contains(F::NativePrediction));
        assert!(transfer.remap.is_empty());
    }
    assert!(
        !trace
            .to
            .identity
            .contains(F::NativeColorChannel(Uuid::from_u128(20)))
    );
}

#[test]
fn trace_endpoint_selection_matches_progress_and_scale_evaluators() {
    let from = angles(0.0, 0.0);
    let to = angles(1.0, 1.0);
    let transition = compiled(from, to);
    for progress in [-1.0, 0.0] {
        let trace = transition
            .sample_trace(ProgrammingOwner::Position, progress)
            .unwrap();
        assert_eq!(trace.from.identity, S::new([F::Pan, F::Tilt]));
        assert!(trace.to.identity.is_empty());
    }
    for progress in [1.0, 2.0] {
        let trace = transition
            .sample_trace(ProgrammingOwner::Position, progress)
            .unwrap();
        assert!(trace.from.identity.is_empty());
        assert_eq!(trace.to.identity, S::new([F::Pan, F::Tilt]));
    }
    assert!(
        transition
            .scale_trace(ProgrammingOwner::Position, -1.0)
            .is_err()
    );
    let legacy = compiled(
        AttributeValue::ColorXyz(Xyz {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }),
        AttributeValue::ColorXyz(Xyz {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        }),
    );
    assert!(
        !legacy
            .sample_trace(ProgrammingOwner::Color, 0.5)
            .unwrap()
            .to
            .identity
            .is_empty()
    );
    assert!(
        legacy
            .scale_trace(ProgrammingOwner::Color, 0.5)
            .unwrap()
            .to
            .identity
            .is_empty()
    );
}
