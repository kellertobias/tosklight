//! Pure Direct capture/compatibility contracts with a scripted source model. Real optical
//! prediction is covered in light-fixture's native_color_edit tests.
use super::*;
use crate::{
    AttributeValue, NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality,
    Xyz,
};
use std::{cell::Cell, sync::Arc};
use uuid::Uuid;

struct Scripted {
    identity: NativeColorIdentity,
    controls: Vec<NativeColorComponentDescriptor>,
    visible: Option<PortableVisibleColor>,
    uv: Option<PortableUv>,
    predictions: Cell<usize>,
}

impl NativeColorEditModel for Scripted {
    fn source(&self) -> &NativeColorIdentity {
        &self.identity
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        self.controls.iter().copied().find(|c| c.binding == binding)
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        self.predictions.set(self.predictions.get() + 1);
        require(recipe.source == self.identity, "foreign source")?;
        require(recipe.spreads.is_empty(), "unresolved spreads")?;
        require(recipe.channels.len() == self.controls.len(), "incomplete")?;
        for value in &recipe.channels {
            let descriptor = self
                .descriptor(NativeColorBinding {
                    channel_id: value.channel_id,
                    function_id: value.function_id,
                })
                .ok_or_else(|| IntentError("foreign control".into()))?;
            require(
                (descriptor.raw_from..=descriptor.raw_to).contains(&value.raw),
                "outside function",
            )?;
        }
        Ok(PortableColorEstimate {
            model_revision: self.identity.model_revision,
            visible: self.visible,
            uv: self.uv,
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        })
    }
}

fn identity() -> NativeColorIdentity {
    NativeColorIdentity {
        profile_id: Uuid::from_u128(1),
        profile_revision: 1,
        profile_digest: "digest-1".into(),
        mode_id: Uuid::from_u128(2),
        head_id: Uuid::from_u128(3),
        path_id: Uuid::from_u128(4),
        model_revision: 7,
        native_layout_signature: "layout-a".into(),
    }
}

fn control(n: u128, raw_to: u32) -> NativeColorComponentDescriptor {
    NativeColorComponentDescriptor {
        binding: NativeColorBinding {
            channel_id: Uuid::from_u128(100 + n),
            function_id: Uuid::from_u128(200 + n),
        },
        raw_from: 0,
        raw_to,
        continuous: true,
    }
}

fn model(identity: NativeColorIdentity) -> Scripted {
    Scripted {
        identity,
        controls: vec![control(2, u32::MAX), control(1, 255)],
        visible: Some(PortableVisibleColor {
            xyz: Xyz {
                x: 0.25,
                y: 0.5,
                z: 0.125,
            },
            relative_output: 1.0,
        }),
        uv: Some(PortableUv {
            amount: 0.0,
            quality: PhysicalDataQuality::Estimated,
        }),
        predictions: Cell::new(0),
    }
}

fn value(descriptor: NativeColorComponentDescriptor, raw: u32) -> NativeColorValue {
    NativeColorValue {
        channel_id: descriptor.binding.channel_id,
        function_id: descriptor.binding.function_id,
        raw,
    }
}

fn observation(model: &Scripted, raws: [u32; 2]) -> NativeColorObservation {
    NativeColorObservation {
        source: model.identity.clone(),
        values: vec![
            value(model.controls[0], raws[0]),
            value(model.controls[1], raws[1]),
        ],
    }
}

#[test]
fn capture_is_one_prediction_of_the_exact_canonical_recipe_from_its_pinned_source() {
    let source = model(identity());
    let capture = capture_direct_color(&source, observation(&source, [u32::MAX - 1, 1])).unwrap();
    assert_eq!(source.predictions.get(), 1);
    let recipe = capture.recipe();
    assert_eq!(recipe.source, identity());
    assert!(recipe.spreads.is_empty());
    // Canonical channel order, exact full-width values.
    assert_eq!(recipe.channels[0].channel_id, Uuid::from_u128(101));
    assert_eq!(recipe.channels[0].raw, 1);
    assert_eq!(recipe.channels[1].raw, u32::MAX - 1);
    assert_eq!(capture.portable().visible, source.visible);
    assert_eq!(capture.portable().model_revision, 7);
    assert_eq!(
        capture.drive_limit(),
        NativeDriveLimit::Unknown,
        "a model without drive evidence never claims Within"
    );
    let reordered = capture_direct_color(&source, {
        let mut observed = observation(&source, [u32::MAX - 1, 1]);
        observed.values.reverse();
        observed
    })
    .unwrap();
    assert_eq!(reordered, capture);
    assert!(matches!(
        capture.clone().into_value(),
        AttributeValue::ColorProgram(program) if *program == *capture.program()
    ));
}

#[test]
fn capture_rejects_non_original_models_and_malformed_observations_without_output() {
    let source = model(identity());
    let mut newer = identity();
    newer.profile_digest = "digest-2".into();
    let good = observation(&source, [5, 6]);
    let mut foreign = good.clone();
    foreign.source = newer.clone();
    assert!(capture_direct_color(&model(newer), good.clone()).is_err());
    assert!(capture_direct_color(&source, foreign).is_err());
    let mut incomplete = good.clone();
    incomplete.values.pop();
    let mut duplicate = good.clone();
    duplicate.values[1] = duplicate.values[0].clone();
    let mut outside = good.clone();
    outside.values[1].raw = 256;
    let mut invalid_identity = good;
    invalid_identity.source.profile_id = Uuid::nil();
    for bad in [incomplete, duplicate, outside, invalid_identity] {
        assert!(capture_direct_color(&source, bad).is_err());
    }
}

#[test]
fn compatibility_is_identity_and_layout_evidence_never_revision_digest_or_names() {
    let source = identity();
    let verdict = |change: fn(&mut NativeColorIdentity)| {
        let mut destination = identity();
        change(&mut destination);
        let destination = model(destination);
        direct_color_compatibility(&source, &DirectDestination::Verified(&destination))
    };
    // Calibration/revision-only changes keep exact replay eligibility.
    for change in [
        (|d: &mut NativeColorIdentity| d.profile_revision = 2) as fn(&mut NativeColorIdentity),
        |d| d.profile_digest = "digest-9".into(),
        |d| d.model_revision = 8,
    ] {
        assert_eq!(verdict(change), DirectCompatibility::Compatible);
    }
    for change in [
        (|d: &mut NativeColorIdentity| d.profile_id = Uuid::from_u128(9))
            as fn(&mut NativeColorIdentity),
        |d| d.mode_id = Uuid::from_u128(9),
        |d| d.head_id = Uuid::from_u128(9),
        |d| d.path_id = Uuid::from_u128(9),
    ] {
        assert_eq!(
            verdict(change),
            DirectCompatibility::Incompatible(DirectIncompatibility::DifferentSource)
        );
    }
    assert_eq!(
        verdict(|d| d.native_layout_signature = "layout-b".into()),
        DirectCompatibility::Incompatible(DirectIncompatibility::ChangedLayout)
    );
    assert_eq!(
        direct_color_compatibility(&source, &DirectDestination::NoNativeColor),
        DirectCompatibility::Incompatible(DirectIncompatibility::NoNativeColor)
    );
    assert_eq!(
        direct_color_compatibility(&source, &DirectDestination::Unverified("missing".into())),
        DirectCompatibility::Unknown("missing".into())
    );
}

fn estimate(visible: Option<[f32; 4]>, uv: Option<f32>) -> PortableColorEstimate {
    PortableColorEstimate {
        model_revision: 7,
        visible: visible.map(|[x, y, z, relative_output]| PortableVisibleColor {
            xyz: Xyz { x, y, z },
            relative_output,
        }),
        uv: uv.map(|amount| PortableUv {
            amount,
            quality: PhysicalDataQuality::Estimated,
        }),
        quality: PhysicalDataQuality::Estimated,
        limitations: vec!["recorded limitation".into()],
    }
}

fn direct(portable: PortableColorEstimate) -> ColorProgram {
    let source = model(identity());
    let recipe = NativeColorRecipe {
        source: identity(),
        channels: vec![value(source.controls[1], 0), value(source.controls[0], 0)],
        spreads: vec![],
    };
    ColorProgram::Direct { recipe, portable }
}

#[test]
fn black_dim_uv_only_and_partial_records_round_trip_and_fall_back_independently() {
    let cases = [
        // Known black with known UV off: fit black, never white.
        (Some([0.0, 0.0, 0.0, 1.0]), Some(0.0)),
        // Dim/partial output keeps its absolute XYZ; relative output stays unnormalized.
        (Some([0.125, 0.25, 0.0625, 1.0]), Some(0.0)),
        (Some([0.125, 0.25, 0.0625, 0.0]), None),
        // UV-only with unknown leakage: visible held, UV applied.
        (None, Some(0.5)),
        (None, None),
    ];
    for (visible, uv) in cases {
        let program = direct(estimate(visible, uv));
        let value = AttributeValue::ColorProgram(Arc::new(program.clone()));
        let json = serde_json::to_value(&value).unwrap();
        let restored: AttributeValue = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(restored, value, "exact round trip for {json}");
        let text = json.to_string();
        assert_eq!(text.contains("\"visible\":null"), visible.is_none());
        assert_eq!(text.contains("\"uv\":null"), uv.is_none());
        let ColorProgram::Direct { portable, .. } = &program else {
            unreachable!()
        };
        let fallback = DirectFallback::from_portable(portable);
        match (visible, fallback.visible) {
            (Some([x, y, z, relative]), VisibleFallback::Fit(fit)) => {
                assert_eq!(fit.xyz, Xyz { x, y, z });
                assert_eq!(fit.relative_output, relative);
            }
            (None, VisibleFallback::Hold) => {}
            other => panic!("visible knowledge changed: {other:?}"),
        }
        match (uv, fallback.uv) {
            (Some(amount), UvFallback::Apply(known)) => assert_eq!(known.amount, amount),
            (None, UvFallback::ParkOff) => {}
            other => panic!("UV knowledge changed: {other:?}"),
        }
        assert_eq!(fallback.limitations[0], "recorded limitation");
        let expected_notes = 1 + usize::from(visible.is_none()) + usize::from(uv.is_none());
        assert_eq!(fallback.limitations.len(), expected_notes);
    }
}

#[test]
fn import_rejects_contradictory_records_but_keeps_unverified_identity_as_data() {
    let program = direct(estimate(Some([0.1, 0.2, 0.3, 1.0]), Some(0.0)));
    let value = AttributeValue::ColorProgram(Arc::new(program));
    let json = serde_json::to_value(&value).unwrap();
    let pointer = |json: &mut serde_json::Value, path: &str, replacement: serde_json::Value| {
        *json.pointer_mut(path).expect("tampered path exists") = replacement;
    };
    let root = if json.pointer("/value/recipe").is_some() {
        "/value"
    } else {
        ""
    };
    for (path, replacement) in [
        ("/portable/model_revision", serde_json::json!(8)),
        ("/portable/visible/xyz/y", serde_json::json!(-0.5)),
        ("/portable/uv/amount", serde_json::json!(1.5)),
        ("/recipe/source/profile_id", serde_json::json!(Uuid::nil())),
    ] {
        let mut tampered = json.clone();
        pointer(&mut tampered, &format!("{root}{path}"), replacement);
        assert!(
            serde_json::from_value::<AttributeValue>(tampered).is_err(),
            "{path} must be rejected on import"
        );
    }
    // A syntactically valid but different layout signature loads as data; replay then treats it
    // as an incompatible layout instead of trusting slot numbers or names.
    let mut foreign = json;
    pointer(
        &mut foreign,
        &format!("{root}/recipe/source/native_layout_signature"),
        serde_json::json!("layout-b"),
    );
    let AttributeValue::ColorProgram(program) =
        serde_json::from_value::<AttributeValue>(foreign).unwrap()
    else {
        unreachable!()
    };
    let destination = model(identity());
    let DirectReplay::Fallback { compatibility, .. } =
        plan_direct_replay(&program, &DirectDestination::Verified(&destination)).unwrap()
    else {
        panic!("changed layout must not replay exactly")
    };
    assert_eq!(
        compatibility,
        DirectCompatibility::Incompatible(DirectIncompatibility::ChangedLayout)
    );
}

#[test]
fn exact_replay_keeps_the_recorded_recipe_and_invalid_compatible_records_are_errors() {
    let program = direct(estimate(None, Some(0.5)));
    let mut calibrated = identity();
    calibrated.model_revision = 8;
    calibrated.profile_digest = "digest-2".into();
    let destination = model(calibrated);
    let DirectReplay::Exact { recipe } =
        plan_direct_replay(&program, &DirectDestination::Verified(&destination)).unwrap()
    else {
        panic!("compatible destination must replay exactly")
    };
    let ColorProgram::Direct {
        recipe: recorded, ..
    } = &program
    else {
        unreachable!()
    };
    assert_eq!(&recipe, recorded, "the pinned source is not rewritten");
    assert_eq!(destination.predictions.get(), 1, "one completeness proof");

    let mut spread = program.clone();
    let ColorProgram::Direct { recipe, .. } = &mut spread else {
        unreachable!()
    };
    recipe.spreads.push(NativeColorSpread {
        binding: destination.controls[1].binding,
        points: vec![0, 300],
    });
    assert!(plan_direct_replay(&spread, &DirectDestination::Verified(&destination)).is_err());
    let mut incomplete = program.clone();
    let ColorProgram::Direct { recipe, .. } = &mut incomplete else {
        unreachable!()
    };
    recipe.channels.pop();
    assert!(plan_direct_replay(&incomplete, &DirectDestination::Verified(&destination)).is_err());
    assert!(
        plan_direct_replay(
            &ColorProgram::Semantic {
                intent: Default::default()
            },
            &DirectDestination::NoNativeColor
        )
        .is_err()
    );
}
