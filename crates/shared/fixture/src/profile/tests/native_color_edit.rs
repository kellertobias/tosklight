use super::*;
use light_core::programming::{NativeColorEditModel, NativeColorRecipe, NativeColorSpread};

#[test]
fn direct_authoring_preserves_spreads_while_predicting_the_materialized_reference() {
    use light_core::programming::{
        ColorProgram, ComponentEdit, FamilyEditContext, NativeColorEdit, edit_family,
    };
    use std::sync::Arc;
    let p = additive();
    let recipe = native(&p, &[0, 0, 0]);
    let model = CompiledNativeColorEditModel::compile(&p, &recipe.source).unwrap();
    let mut value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        portable: model.predict(&recipe).unwrap(),
        recipe,
    }));
    let context = FamilyEditContext {
        native_model: Some(&model),
        ..Default::default()
    };
    let red = binding(&p.modes[0].channels[0]);
    let green = binding(&p.modes[0].channels[1]);
    for (binding, operation, expected_red, expected_green) in [
        (red, NativeColorEdit::Spread(vec![20, 200]), [20, 200], 0),
        (red, NativeColorEdit::Relative(10), [30, 210], 0),
        (green, NativeColorEdit::Set(128), [30, 210], 128),
    ] {
        value = edit_family(
            &value,
            &[ComponentEdit::Native { binding, operation }],
            &context,
        )
        .unwrap();
        let AttributeValue::ColorProgram(program) = &value else {
            unreachable!()
        };
        let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
            unreachable!()
        };
        assert_eq!(recipe.spreads.len(), 1);
        assert_eq!(recipe.spreads[0].points, expected_red);
        assert_eq!(recipe.channels[0].raw, expected_red[0]);
        assert_eq!(recipe.channels[1].raw, expected_green);
        let visible = portable.visible.unwrap();
        assert_eq!(visible.xyz.x, expected_red[0] as f32 / 255.);
        assert_eq!(visible.xyz.y, expected_green as f32 / 255.);
        assert!(
            model.predict(recipe).is_err(),
            "unresolved curves are not a sampled recipe"
        );
    }
}

fn additive() -> FixtureProfile {
    let mut p = additive_profile();
    p.modes[0].channels.truncate(3);
    let path = optical_path_mut(&mut p);
    path.controls.truncate(3);
    path.filters.clear();
    path.measurements.clear();
    let OpticalSource::Additive { emitters } = &mut path.source else {
        unreachable!()
    };
    for (e, xyz) in emitters.iter_mut().zip([
        Xyz {
            x: 1.,
            y: 0.,
            z: 0.,
        },
        Xyz {
            x: 0.,
            y: 1.,
            z: 0.,
        },
        Xyz {
            x: 0.,
            y: 0.,
            z: 1.,
        },
    ]) {
        e.xyz = Some(xyz);
        e.provenance = provenance();
    }
    p
}

fn native(p: &FixtureProfile, values: &[u32]) -> NativeColorRecipe {
    NativeColorRecipe {
        source: identity(p),
        channels: p.modes[0]
            .channels
            .iter()
            .zip(values)
            .map(|(c, raw)| NativeColorValue {
                channel_id: c.id,
                function_id: c.functions[0].id,
                raw: *raw,
            })
            .collect(),
        spreads: vec![],
    }
}

#[test]
fn exact_original_prediction_preserves_full_xyz_black_and_known_uv_zero() {
    let p = additive();
    let model = CompiledNativeColorEditModel::compile(&p, &identity(&p)).unwrap();
    let color = model.predict(&native(&p, &[255, 128, 64])).unwrap();
    let visible = color.visible.unwrap();
    assert_eq!(
        visible.xyz,
        Xyz {
            x: 1.,
            y: 128. / 255.,
            z: 64. / 255.
        }
    );
    assert_eq!(visible.relative_output, 1.);
    assert_eq!(color.quality, PhysicalDataQuality::Measured);
    assert_eq!(color.uv.unwrap().amount, 0.);
    assert_eq!(color.uv.unwrap().quality, PhysicalDataQuality::Estimated);
    assert!(color.limitations.is_empty());
    let black = model.predict(&native(&p, &[0, 0, 0])).unwrap();
    assert_eq!(
        black.visible.unwrap().xyz,
        Xyz {
            x: 0.,
            y: 0.,
            z: 0.
        }
    );
}

#[test]
fn unavailable_optical_capacity_keeps_verified_native_edits_available() {
    use light_core::programming::{
        ColorProgram, ComponentEdit, FamilyEditContext, NativeColorEdit, edit_family,
    };
    use std::sync::Arc;
    let mut p = additive();
    let channels = recipe(&p.modes[0], 0);
    optical_path_mut(&mut p).measurements = (0..6000)
        .map(|index| {
            let mut recipe = channels.clone();
            recipe[0].raw = index % 256;
            recipe[1].raw = index / 256;
            ColorRecipeMeasurement {
                recipe,
                xyz: Xyz {
                    x: 1.,
                    y: 1.,
                    z: 1.,
                },
                provenance: provenance(),
            }
        })
        .collect();
    p.validate().unwrap();
    assert!(crate::forward::CompiledColorForward::compile(&p, p.modes[0].id, None).is_err());
    let recipe = native(&p, &[25, 0, 0]);
    let model = CompiledNativeColorEditModel::compile(&p, &recipe.source).unwrap();
    let portable = model.predict(&recipe).unwrap();
    assert_eq!(portable.visible, None);
    assert_eq!(portable.uv, None);
    assert!(!portable.limitations.is_empty());
    let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct { recipe, portable }));
    let changed = edit_family(
        &value,
        &[ComponentEdit::Native {
            binding: binding(&p.modes[0].channels[0]),
            operation: NativeColorEdit::Set(200),
        }],
        &FamilyEditContext {
            native_model: Some(&model),
            ..Default::default()
        },
    )
    .unwrap();
    let AttributeValue::ColorProgram(program) = changed else {
        unreachable!()
    };
    let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
        unreachable!()
    };
    assert_eq!(recipe.channels[0].raw, 200);
    assert_eq!(
        portable.visible, None,
        "a changed native recipe never reuses stale appearance"
    );
    assert!(portable.validate().is_ok());
}

#[test]
fn a_second_head_predicts_only_its_complete_control_path() {
    let mut p = additive();
    let mut head = p.modes[0].heads[0].clone();
    head.id = Uuid::new_v4();
    head.name = "Second".into();
    head.master_shared = false;
    let head_id = head.id;
    let mut channel = p.modes[0].channels[0].clone();
    channel.id = Uuid::new_v4();
    channel.head_id = head_id;
    channel.functions[0].id = Uuid::new_v4();
    let native = binding(&channel);
    let mut path = optical_path(&p).clone();
    path.id = Uuid::new_v4();
    path.head_id = head_id;
    path.controls = vec![channel.id];
    let OpticalSource::Additive { emitters } = &mut path.source else {
        unreachable!()
    };
    emitters.truncate(1);
    emitters[0].id = Uuid::new_v4();
    emitters[0].binding = native;
    emitters[0].xyz = Some(Xyz {
        x: 2.,
        y: 3.,
        z: 4.,
    });
    p.modes[0].heads.push(head);
    p.modes[0].channels.push(channel);
    p.modes[0].color_physical.as_mut().unwrap().paths.push(path);
    let source = p.native_color_identity(p.modes[0].id, head_id).unwrap();
    let model = CompiledNativeColorEditModel::compile(&p, &source).unwrap();
    let recipe = NativeColorRecipe {
        source,
        channels: vec![NativeColorValue {
            channel_id: native.channel_id,
            function_id: native.function_id,
            raw: 255,
        }],
        spreads: vec![],
    };
    assert_eq!(
        model.predict(&recipe).unwrap().visible.unwrap().xyz,
        Xyz {
            x: 2.,
            y: 3.,
            z: 4.
        }
    );
    assert!(model.descriptor(binding(&p.modes[0].channels[0])).is_none());
}

#[test]
fn original_identity_is_stricter_than_unchanged_native_layout() {
    let p = additive();
    let original = identity(&p);
    let model = CompiledNativeColorEditModel::compile(&p, &original).unwrap();
    let mut changed = p.clone();
    let OpticalSource::Additive { emitters } = &mut optical_path_mut(&mut changed).source else {
        unreachable!()
    };
    emitters[0].xyz = Some(Xyz {
        x: 0.2,
        y: 0.3,
        z: 0.4,
    });
    assert_eq!(
        identity(&changed).native_layout_signature,
        original.native_layout_signature
    );
    assert!(CompiledNativeColorEditModel::compile(&changed, &original).is_err());
    assert!(model.predict(&native(&changed, &[255, 0, 0])).is_err());
    assert_eq!(
        model
            .predict(&native(&p, &[255, 0, 0]))
            .unwrap()
            .visible
            .unwrap()
            .xyz
            .x,
        1.
    );
}

#[test]
fn validation_rejects_incomplete_duplicate_foreign_and_invalid_controls_without_poisoning_scratch()
{
    let p = additive();
    let good = native(&p, &[255, 128, 64]);
    let model = CompiledNativeColorEditModel::compile(&p, &good.source).unwrap();
    let expected = model.predict(&good).unwrap();
    let changes: [fn(&mut NativeColorRecipe); 6] = [
        |r| {
            r.channels.pop();
        },
        |r| {
            r.channels[1] = r.channels[0].clone();
        },
        |r| {
            r.channels[1].channel_id = Uuid::new_v4();
        },
        |r| {
            r.channels[1].function_id = Uuid::new_v4();
        },
        |r| {
            r.channels[1].raw = 256;
        },
        |r| {
            r.spreads.push(NativeColorSpread {
                binding: NativeColorBinding {
                    channel_id: r.channels[0].channel_id,
                    function_id: r.channels[0].function_id,
                },
                points: vec![0, 256],
            });
        },
    ];
    for change in changes {
        let mut invalid = good.clone();
        change(&mut invalid);
        assert!(model.predict(&invalid).is_err());
        assert_eq!(model.predict(&good).unwrap(), expected);
    }
    let mut reordered = good;
    reordered.channels.reverse();
    assert_eq!(model.predict(&reordered).unwrap(), expected);
}

#[test]
fn visible_measurement_does_not_invent_uv_for_unknown_source() {
    let mut p = serial_profile();
    optical_path_mut(&mut p).source = OpticalSource::Unknown;
    let model = CompiledNativeColorEditModel::compile(&p, &identity(&p)).unwrap();
    let measured = model.predict(&native(&p, &[128; 5])).unwrap();
    assert_eq!(
        measured.visible.unwrap().xyz,
        optical_path(&p).measurements[0].xyz
    );
    assert_eq!(measured.uv, None);
    assert_eq!(measured.limitations.len(), 1);
    let unknown = model.predict(&native(&p, &[0; 5])).unwrap();
    assert_eq!(unknown.visible, None);
    assert_eq!(unknown.uv, None);
    assert_eq!(unknown.limitations.len(), 2);
}

#[test]
fn uv_prediction_is_independent_of_visible_leakage_and_preserves_above_limit_drive() {
    let mut p = additive();
    let OpticalSource::Additive { emitters } = &mut optical_path_mut(&mut p).source else {
        unreachable!()
    };
    emitters[0].band = OpticalEmitterBand::Ultraviolet;
    emitters[0].xyz = None;
    emitters[0].maximum_level = 0.25;
    let model = CompiledNativeColorEditModel::compile(&p, &identity(&p)).unwrap();
    let unknown = model.predict(&native(&p, &[128, 0, 0])).unwrap();
    assert_eq!(unknown.visible, None);
    assert_eq!(unknown.uv.unwrap().amount, 128. / 255.);
    assert!(unknown.limitations.iter().any(|s| s.contains("maximum")));
    let OpticalSource::Additive { emitters } = &mut optical_path_mut(&mut p).source else {
        unreachable!()
    };
    emitters[0].xyz = Some(Xyz {
        x: 0.1,
        y: 0.2,
        z: 0.3,
    });
    let model = CompiledNativeColorEditModel::compile(&p, &identity(&p)).unwrap();
    let known = model.predict(&native(&p, &[255, 255, 0])).unwrap();
    assert_eq!(
        known.visible.unwrap().xyz,
        Xyz {
            x: 0.1,
            y: 1.2,
            z: 0.3
        }
    );
    assert_eq!(known.visible.unwrap().relative_output, 1.);
    assert_eq!(known.uv.unwrap().amount, 1.);
}

#[test]
fn descriptor_retains_u32_precision_and_wheel_functions_remain_discrete() {
    let mut p = additive();
    p.modes[0].channels.truncate(1);
    p.modes[0].splits[0].footprint = 4;
    let c = &mut p.modes[0].channels[0];
    c.resolution = ChannelResolution::U32;
    c.secondary_slots = vec![2, 3, 4];
    c.functions[0].dmx_to = u32::MAX;
    let path = optical_path_mut(&mut p);
    path.controls.truncate(1);
    let OpticalSource::Additive { emitters } = &mut path.source else {
        unreachable!()
    };
    emitters.truncate(1);
    let model = CompiledNativeColorEditModel::compile(&p, &identity(&p)).unwrap();
    let descriptor = model.descriptor(binding(&p.modes[0].channels[0])).unwrap();
    assert_eq!(descriptor.raw_to, u32::MAX);
    assert!(descriptor.continuous);
    assert!(
        model
            .predict(&native(&p, &[1]))
            .unwrap()
            .visible
            .unwrap()
            .xyz
            .x
            > 0.
    );
    assert_eq!(
        model
            .predict(&native(&p, &[u32::MAX]))
            .unwrap()
            .visible
            .unwrap()
            .xyz
            .x,
        1.
    );

    let mut wheel = serial_profile();
    wheel.modes[0].channels[4].functions[0].behavior = ChannelFunctionBehavior::Fixed {
        semantic_id: "open".into(),
        label: "Open".into(),
        raw_value: 0,
    };
    let model = CompiledNativeColorEditModel::compile(&wheel, &identity(&wheel)).unwrap();
    assert!(
        !model
            .descriptor(binding(&wheel.modes[0].channels[4]))
            .unwrap()
            .continuous
    );
}

mod direct_capture {
    //! TL-595: atomic Direct capture and verified replay eligibility on real source models.
    use super::*;
    use crate::direct_color_samples::{
        SAMPLE_EMITTER_XYZ, SAMPLE_RAW_MAXIMA, direct_color_samples, sample_identity,
        sample_observation,
    };
    use light_core::programming::{
        ColorProgram, DirectColorCapture, DirectCompatibility, DirectDestination,
        DirectIncompatibility, DirectReplay, NativeColorObservation, NativeDriveLimit, UvFallback,
        VisibleFallback, capture_direct_color, plan_direct_replay,
    };

    fn capture(p: &FixtureProfile, raws: [u32; 4]) -> DirectColorCapture {
        let model = CompiledNativeColorEditModel::compile(p, &sample_identity(p)).unwrap();
        capture_direct_color(&model, sample_observation(p, raws)).unwrap()
    }

    fn expected_xyz(raws: [u32; 4]) -> [f64; 3] {
        let mut xyz = [0.0; 3];
        for ((raw, max), emitter) in raws.iter().zip(SAMPLE_RAW_MAXIMA).zip(SAMPLE_EMITTER_XYZ) {
            let drive = f64::from(*raw) / f64::from(max);
            xyz[0] += drive * f64::from(emitter.x);
            xyz[1] += drive * f64::from(emitter.y);
            xyz[2] += drive * f64::from(emitter.z);
        }
        xyz
    }

    #[test]
    fn capture_keeps_exact_full_width_values_and_unnormalized_appearance_atomically() {
        let samples = direct_color_samples();
        for (raws, uv, limit) in [
            ([255, 65_535, 255, 0], 0.0, NativeDriveLimit::Within),
            ([128, 32_768, 1, 0], 0.0, NativeDriveLimit::Within),
            ([0, 0, 0, 0], 0.0, NativeDriveLimit::Within),
            ([0, 0, 0, u32::MAX / 4], 0.25, NativeDriveLimit::Within),
            // Above the model's 50% UV maximum: exact drive kept, status reported.
            (
                [0, 0, 0, u32::MAX],
                1.0,
                NativeDriveLimit::AboveModelMaximum,
            ),
        ] {
            let captured = capture(&samples.source, raws);
            let recipe = captured.recipe();
            assert_eq!(recipe.source, sample_identity(&samples.source));
            let observed = sample_observation(&samples.source, raws).values;
            for value in &observed {
                assert!(recipe.channels.contains(value), "exact value {value:?}");
            }
            assert_eq!(recipe.channels.len(), 4);
            let visible = captured
                .portable()
                .visible
                .expect("known leakage keeps visible");
            // Brightness and leakage live in XYZ once; relative output is never a normalizer.
            assert_eq!(visible.relative_output, 1.0);
            for (actual, expected) in [visible.xyz.x, visible.xyz.y, visible.xyz.z]
                .into_iter()
                .zip(expected_xyz(raws))
            {
                assert!((f64::from(actual) - expected).abs() < 1e-6, "{raws:?}");
            }
            let known_uv = captured.portable().uv.expect("single UV bank is portable");
            assert!((known_uv.amount - uv).abs() < 1e-6);
            assert_eq!(captured.drive_limit(), limit);
            assert_eq!(
                captured
                    .portable()
                    .limitations
                    .iter()
                    .any(|s| s.contains("maximum")),
                limit == NativeDriveLimit::AboveModelMaximum
            );
            let value = captured.clone().into_value();
            let restored: AttributeValue =
                serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
            assert_eq!(restored, value, "full-width values survive serialization");
        }
        let black = capture(&samples.source, [0; 4]);
        assert_eq!(
            black.portable().visible.unwrap().xyz,
            Xyz {
                x: 0.,
                y: 0.,
                z: 0.
            },
            "black stays known black, never white"
        );
        let full = capture(&samples.source, [255, 65_535, 255, 0]);
        assert_eq!(
            full.portable().visible.unwrap().xyz,
            Xyz {
                x: 0.875,
                y: 1.0625,
                z: 1.125
            }
        );
        let uv_only = capture(&samples.source, [0, 0, 0, u32::MAX]);
        assert_eq!(
            uv_only.portable().visible.unwrap().xyz,
            SAMPLE_EMITTER_XYZ[3],
            "known leakage counted exactly once"
        );
        let VisibleFallback::Fit(fit) = uv_only.fallback().visible else {
            panic!("known leakage is fit, not held")
        };
        assert_eq!(fit.xyz, SAMPLE_EMITTER_XYZ[3]);
    }

    #[test]
    fn unknown_leakage_keeps_independent_uv_without_inventing_visible_white() {
        let samples = direct_color_samples();
        let uv_only = capture(&samples.unknown_leakage, [0, 0, 0, u32::MAX / 2]);
        assert_eq!(uv_only.portable().visible, None);
        assert!((uv_only.portable().uv.unwrap().amount - 0.5).abs() < 1e-6);
        let fallback = uv_only.fallback();
        assert_eq!(fallback.visible, VisibleFallback::Hold);
        assert!(matches!(fallback.uv, UvFallback::Apply(uv) if (uv.amount - 0.5).abs() < 1e-6));
        let json = serde_json::to_string(&uv_only.clone().into_value()).unwrap();
        assert!(json.contains("\"visible\":null"));
        let restored: AttributeValue = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, uv_only.into_value());
        // With UV off the unknown leakage does not contribute; RGB appearance stays known.
        let red = capture(&samples.unknown_leakage, [255, 0, 0, 0]);
        assert_eq!(red.portable().visible.unwrap().xyz, SAMPLE_EMITTER_XYZ[0]);
        assert_eq!(red.portable().uv.unwrap().amount, 0.0);
    }

    #[test]
    fn equal_uv_banks_are_portable_and_unequal_banks_remain_unknown() {
        let mut p = additive();
        let OpticalSource::Additive { emitters } = &mut optical_path_mut(&mut p).source else {
            unreachable!()
        };
        emitters[0].band = OpticalEmitterBand::Ultraviolet;
        emitters[1].band = OpticalEmitterBand::Ultraviolet;
        let model = CompiledNativeColorEditModel::compile(&p, &identity(&p)).unwrap();
        let observe = |raws: &[u32]| NativeColorObservation {
            source: identity(&p),
            values: native(&p, raws).channels,
        };
        let equal = capture_direct_color(&model, observe(&[128, 128, 0])).unwrap();
        assert_eq!(equal.portable().uv.unwrap().amount, 128. / 255.);
        let unequal = capture_direct_color(&model, observe(&[128, 64, 0])).unwrap();
        assert_eq!(
            unequal.portable().uv,
            None,
            "the maximum drive is diagnostic, not an aggregation"
        );
        assert_eq!(unequal.recipe().channels, {
            let mut values = native(&p, &[128, 64, 0]).channels;
            values.sort_by_key(|v| v.channel_id);
            values
        });
        let fallback = unequal.fallback();
        assert!(matches!(fallback.visible, VisibleFallback::Fit(_)));
        assert_eq!(fallback.uv, UvFallback::ParkOff);
    }

    #[test]
    fn unknown_macro_ranges_are_preserved_native_only_and_malformed_controls_fail() {
        let mut p = serial_profile();
        let wheel = &mut p.modes[0].channels[3];
        wheel.functions[0].dmx_to = 199;
        let mut rotate = wheel.functions[0].clone();
        rotate.id = Uuid::new_v4();
        rotate.name = "Rotate".into();
        rotate.dmx_from = 200;
        rotate.dmx_to = 255;
        wheel.functions.push(rotate.clone());
        p.validate().unwrap();
        let model = CompiledNativeColorEditModel::compile(&p, &identity(&p)).unwrap();
        let mut values = native(&p, &[128, 128, 128, 0, 128]).channels;
        values[3] = NativeColorValue {
            channel_id: p.modes[0].channels[3].id,
            function_id: rotate.id,
            raw: 220,
        };
        let observation = NativeColorObservation {
            source: identity(&p),
            values,
        };
        let captured = capture_direct_color(&model, observation.clone()).unwrap();
        assert_eq!(captured.portable().visible, None);
        assert_eq!(captured.portable().uv, None);
        assert!(captured.recipe().channels.contains(&observation.values[3]));
        assert_eq!(captured.fallback().visible, VisibleFallback::Hold);
        assert_eq!(captured.fallback().uv, UvFallback::ParkOff);
        let mut wrong_function = observation;
        wrong_function.values[3].raw = 100;
        assert!(capture_direct_color(&model, wrong_function).is_err());
    }

    #[test]
    fn replay_uses_verified_identity_and_layout_never_names_slots_or_new_calibration() {
        let samples = direct_color_samples();
        let captured = capture(&samples.source, [255, 40_000, 7, u32::MAX - 1]);
        let program = captured.program();
        let plan = |p: &FixtureProfile| {
            let model = CompiledNativeColorEditModel::compile(p, &sample_identity(p)).unwrap();
            plan_direct_replay(program, &DirectDestination::Verified(&model)).unwrap()
        };
        let slots = |p: &FixtureProfile| {
            let slots = p.modes[0].primary_slots().unwrap();
            p.modes[0]
                .channels
                .iter()
                .map(|c| (c.functions[0].name.clone(), slots[&c.id], c.resolution))
                .collect::<Vec<_>>()
        };
        // Revision 2 moves every DMX slot and recalibrates Red; exact replay stays eligible.
        assert_ne!(slots(&samples.compatible), slots(&samples.source));
        let DirectReplay::Exact { recipe } = plan(&samples.compatible) else {
            panic!("verified identical layout must replay exactly")
        };
        assert_eq!(&recipe, captured.recipe());
        let recalibrated = CompiledNativeColorEditModel::compile(
            &samples.compatible,
            &sample_identity(&samples.compatible),
        )
        .unwrap();
        let mut resourced = recipe.clone();
        resourced.source = sample_identity(&samples.compatible);
        assert_ne!(
            recalibrated.predict(&resourced).unwrap().visible,
            captured.portable().visible,
            "the saved estimate is not what a newer calibration predicts, and is kept"
        );
        // Same names, attributes, widths and slots, but an independent identity.
        assert_eq!(slots(&samples.lookalike), slots(&samples.source));
        for (profile, reason) in [
            (&samples.lookalike, DirectIncompatibility::DifferentSource),
            (
                &samples.changed_layout,
                DirectIncompatibility::ChangedLayout,
            ),
        ] {
            let DirectReplay::Fallback {
                compatibility,
                fallback,
            } = plan(profile)
            else {
                panic!("{reason:?} must not replay native values")
            };
            assert_eq!(compatibility, DirectCompatibility::Incompatible(reason));
            assert_eq!(
                fallback.visible,
                VisibleFallback::Fit(captured.portable().visible.unwrap())
            );
            assert!(matches!(fallback.uv, UvFallback::Apply(_)));
        }
        let DirectReplay::Fallback { compatibility, .. } = plan_direct_replay(
            program,
            &DirectDestination::Unverified("original model unavailable".into()),
        )
        .unwrap() else {
            panic!("unverified destination is unknown, not exact")
        };
        assert!(matches!(compatibility, DirectCompatibility::Unknown(_)));
        assert!(matches!(program, ColorProgram::Direct { .. }));
    }

    #[test]
    fn capture_requires_the_exact_original_even_when_the_layout_is_unchanged() {
        let samples = direct_color_samples();
        let newer = CompiledNativeColorEditModel::compile(
            &samples.compatible,
            &sample_identity(&samples.compatible),
        )
        .unwrap();
        assert!(
            capture_direct_color(&newer, sample_observation(&samples.source, [1, 2, 3, 4]))
                .is_err()
        );
        let lookalike = CompiledNativeColorEditModel::compile(
            &samples.lookalike,
            &sample_identity(&samples.lookalike),
        )
        .unwrap();
        let mut foreign = sample_observation(&samples.source, [1, 2, 3, 4]);
        foreign.source = sample_identity(&samples.lookalike);
        assert!(
            capture_direct_color(&lookalike, foreign).is_err(),
            "matching names cannot rebind another fixture's channel identities"
        );
    }
}
