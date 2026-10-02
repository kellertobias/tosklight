use super::*;

fn provenance() -> OpticalProvenance {
    OpticalProvenance {
        quality: PhysicalDataQuality::Measured,
        source: Some("Synthetic test measurement; not a fixture calibration".into()),
        revision: 4,
    }
}

fn spectrum(value: f32) -> Vec<SpectrumSample> {
    vec![
        SpectrumSample {
            wavelength_nm: 380.0,
            value,
        },
        SpectrumSample {
            wavelength_nm: 780.0,
            value,
        },
    ]
}

fn binding(channel: &FixtureChannel) -> NativeColorBinding {
    NativeColorBinding {
        channel_id: channel.id,
        function_id: channel.functions[0].id,
    }
}

fn optical_path(profile: &FixtureProfile) -> &HeadOpticalPath {
    &profile.modes[0].color_physical.as_ref().unwrap().paths[0]
}

fn optical_path_mut(profile: &mut FixtureProfile) -> &mut HeadOpticalPath {
    &mut profile.modes[0].color_physical.as_mut().unwrap().paths[0]
}

fn recipe(mode: &FixtureMode, raw: u32) -> Vec<NativeColorValue> {
    mode.channels
        .iter()
        .map(|channel| NativeColorValue {
            channel_id: channel.id,
            function_id: channel.functions[0].id,
            raw,
        })
        .collect()
}

/// A complete package-sized profile: three serial CMY filters followed by two wheels.
/// The synthetic spectra are deliberately simple; these tests assert data contracts,
/// not physical color fitting.
fn serial_profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Serial CMY and two color wheels".into();
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    let attributes = [
        (
            "color.cyan",
            "color.red",
            CanonicalTransform::InvertNormalized,
        ),
        (
            "color.magenta",
            "color.green",
            CanonicalTransform::InvertNormalized,
        ),
        (
            "color.yellow",
            "color.blue",
            CanonicalTransform::InvertNormalized,
        ),
        (
            "color.wheel.1",
            "color.wheel.1",
            CanonicalTransform::Identity,
        ),
        (
            "color.wheel.2",
            "color.wheel.2",
            CanonicalTransform::Identity,
        ),
    ];
    mode.channels = attributes
        .iter()
        .map(|(fixture_attribute, attribute, transform)| {
            let mut channel = channel(head_id, ChannelResolution::U8, vec![]);
            channel.fixture_attribute = AttributeKey((*fixture_attribute).into());
            channel.attribute = AttributeKey((*attribute).into());
            channel.canonical_transform = *transform;
            channel.functions[0].attribute = channel.attribute.clone();
            channel
        })
        .collect();
    mode.splits[0].footprint = 5;
    mode.color_systems = vec![HeadColorSystem {
        head_id,
        correction_matrix: identity_color_correction(),
        calibration: Default::default(),
        system: ColorSystem::Subtractive {
            cyan_channel_id: mode.channels[0].id,
            magenta_channel_id: mode.channels[1].id,
            yellow_channel_id: mode.channels[2].id,
            filters: None,
        },
    }];
    let controls = mode.channels.iter().map(|channel| channel.id).collect();
    let filters = mode
        .channels
        .iter()
        .zip(["Cyan", "Magenta", "Yellow", "Wheel 1", "Wheel 2"])
        .map(|(channel, name)| OpticalFilter {
            id: Uuid::new_v4(),
            name: name.into(),
            binding: binding(channel),
            transmission: OpticalTransmission::Unknown,
            provenance: Default::default(),
        })
        .collect();
    let measurement = ColorRecipeMeasurement {
        recipe: recipe(mode, 128),
        xyz: Xyz {
            x: 0.2,
            y: 0.3,
            z: 0.1,
        },
        provenance: provenance(),
    };
    mode.color_physical = Some(ColorPhysicalModel {
        version: 1,
        revision: 7,
        paths: vec![HeadOpticalPath {
            id: Uuid::new_v4(),
            head_id,
            controls,
            source: OpticalSource::Fixed {
                xyz: Some(Xyz {
                    x: 0.95,
                    y: 1.0,
                    z: 1.09,
                }),
                spectrum: spectrum(1.0),
                provenance: provenance(),
            },
            filters,
            measurements: vec![measurement],
        }],
    });
    profile
}

fn identity(profile: &FixtureProfile) -> NativeColorIdentity {
    profile
        .native_color_identity(profile.modes[0].id, profile.modes[0].heads[0].id)
        .unwrap()
}

fn assert_invalid(profile: &FixtureProfile) {
    assert!(
        profile.validate().is_err(),
        "invalid optical data must fail profile validation"
    );
    assert!(
        crate::write_fixture_package(profile).is_err(),
        "invalid optical data must not export"
    );
}

#[test]
fn serial_cmy_and_two_wheels_keep_order_identity_and_measurements_through_json_and_package() {
    let profile = serial_profile();
    profile.validate().unwrap();
    let original = serde_json::to_value(&profile).unwrap();
    let json: FixtureProfile = serde_json::from_value(original.clone()).unwrap();
    let bytes = crate::write_fixture_package(&json).unwrap();
    let restored = crate::read_fixture_package(&bytes).unwrap();
    assert_eq!(serde_json::to_value(&restored).unwrap(), original);
    assert_eq!(identity(&restored), identity(&profile));
    let names: Vec<_> = optical_path(&restored)
        .filters
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(names, ["Cyan", "Magenta", "Yellow", "Wheel 1", "Wheel 2"]);
    assert_eq!(optical_path(&restored).measurements[0].recipe.len(), 5);
}

#[test]
fn existing_profiles_without_optical_paths_still_roundtrip_without_invented_metadata() {
    let mut profile = serial_profile();
    profile.modes[0].color_physical = None;
    let original = serde_json::to_value(&profile).unwrap();
    assert!(original["modes"][0].get("color_physical").is_none());
    let restored =
        crate::read_fixture_package(&crate::write_fixture_package(&profile).unwrap()).unwrap();
    assert!(restored.modes[0].color_physical.is_none());
    assert_eq!(serde_json::to_value(&restored).unwrap(), original);
}

#[test]
fn unknown_appearance_preserves_exact_native_ownership_without_inventing_white() {
    let mut profile = serial_profile();
    let path = optical_path_mut(&mut profile);
    path.source = OpticalSource::Unknown;
    path.measurements.clear();
    let restored =
        crate::read_fixture_package(&crate::write_fixture_package(&profile).unwrap()).unwrap();
    let path = optical_path(&restored);
    assert!(matches!(path.source, OpticalSource::Unknown));
    assert!(
        path.filters
            .iter()
            .all(|f| matches!(f.transmission, OpticalTransmission::Unknown))
    );
    assert!(path.measurements.is_empty());
    assert_eq!(path.controls.len(), 5);
    restored.modes[0]
        .validate_native_color_recipe(path, &recipe(&restored.modes[0], 0))
        .unwrap();
}

#[test]
fn invalid_versions_dangling_native_bindings_and_duplicate_stages_cannot_export() {
    let cases: [fn(&mut FixtureProfile); 8] = [
        |p| p.modes[0].color_physical.as_mut().unwrap().version = 2,
        |p| optical_path_mut(p).head_id = Uuid::new_v4(),
        |p| optical_path_mut(p).controls[0] = Uuid::new_v4(),
        |p| optical_path_mut(p).filters[0].binding.channel_id = Uuid::new_v4(),
        |p| optical_path_mut(p).filters[0].binding.function_id = Uuid::new_v4(),
        |p| {
            let path = optical_path_mut(p);
            path.filters[1].id = path.filters[0].id;
        },
        |p| {
            let path = optical_path_mut(p);
            path.filters[1].binding = path.filters[0].binding;
        },
        |p| {
            let path = optical_path_mut(p);
            path.controls.push(path.controls[0]);
        },
    ];
    for change in cases {
        let mut profile = serial_profile();
        change(&mut profile);
        assert_invalid(&profile);
    }
}

#[test]
fn a_function_uuid_on_another_channel_is_not_a_valid_native_binding() {
    let mut profile = serial_profile();
    let foreign_function = profile.modes[0].channels[1].functions[0].id;
    optical_path_mut(&mut profile).filters[0]
        .binding
        .function_id = foreign_function;
    assert_invalid(&profile);
}

#[test]
fn a_complete_recipe_requires_every_owned_channel_exactly_once_in_its_native_function() {
    let profile = serial_profile();
    let mode = &profile.modes[0];
    let path = optical_path(&profile);
    let original = recipe(mode, 128);
    mode.validate_native_color_recipe(path, &original).unwrap();
    let mut missing = original.clone();
    missing.pop();
    let mut duplicate = original.clone();
    duplicate[4] = duplicate[0].clone();
    let mut foreign = original.clone();
    foreign[0].channel_id = Uuid::new_v4();
    let mut wrong_function = original.clone();
    wrong_function[0].function_id = original[1].function_id;
    let mut outside_raw = original.clone();
    outside_raw[0].raw = 256;
    for invalid in [missing, duplicate, foreign, wrong_function, outside_raw] {
        assert!(mode.validate_native_color_recipe(path, &invalid).is_err());
    }
    let mut reversed = original;
    reversed.reverse();
    mode.validate_native_color_recipe(path, &reversed).unwrap();
}

#[test]
fn full_path_measurements_cannot_omit_parked_wheels() {
    let mut profile = serial_profile();
    optical_path_mut(&mut profile).measurements[0].recipe.pop();
    assert_invalid(&profile);
}

#[test]
fn u32_native_values_and_adjacent_spectral_ranges_keep_all_bits() {
    let mut profile = serial_profile();
    let mode = &mut profile.modes[0];
    mode.channels[4].resolution = ChannelResolution::U32;
    mode.channels[4].secondary_slots = vec![6, 7, 8];
    mode.channels[4].functions[0].dmx_to = u32::MAX;
    mode.splits[0].footprint = 8;
    let path = optical_path_mut(&mut profile);
    path.filters[4].transmission = OpticalTransmission::Spectral {
        samples: vec![
            FilterSpectrum {
                raw_from: u32::MAX - 1,
                raw_to: u32::MAX - 1,
                spectrum: spectrum(0.25),
            },
            FilterSpectrum {
                raw_from: u32::MAX,
                raw_to: u32::MAX,
                spectrum: spectrum(0.5),
            },
        ],
    };
    path.filters[4].provenance = provenance();
    path.measurements[0].recipe[4].raw = u32::MAX - 1;
    let mut black = path.measurements[0].clone();
    black.recipe[4].raw = u32::MAX;
    black.xyz = Xyz {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    path.measurements.push(black);
    let restored =
        crate::read_fixture_package(&crate::write_fixture_package(&profile).unwrap()).unwrap();
    let path = optical_path(&restored);
    assert_eq!(path.measurements[0].recipe[4].raw, u32::MAX - 1);
    assert_eq!(path.measurements[1].recipe[4].raw, u32::MAX);
    assert_eq!(
        path.measurements[1].xyz,
        Xyz {
            x: 0.0,
            y: 0.0,
            z: 0.0
        }
    );
    let OpticalTransmission::Spectral { samples } = &path.filters[4].transmission else {
        panic!("spectral wheel data lost")
    };
    assert_eq!(samples[0].raw_to + 1, samples[1].raw_from);
}

#[test]
fn canonical_alias_collisions_do_not_collapse_distinct_native_color_controls() {
    let mut profile = serial_profile();
    let mode = &mut profile.modes[0];
    // A red emitter and a cyan filter can both map to canonical Red.
    mode.channels[3].fixture_attribute = AttributeKey("color.red".into());
    mode.channels[3].attribute = AttributeKey("color.red".into());
    mode.channels[3].functions[0].attribute = AttributeKey("color.red".into());
    profile.validate().unwrap();
    let restored =
        crate::read_fixture_package(&crate::write_fixture_package(&profile).unwrap()).unwrap();
    let path = optical_path(&restored);
    assert_ne!(
        path.filters[0].binding.channel_id,
        path.filters[3].binding.channel_id
    );
    assert_ne!(
        path.filters[0].binding.function_id,
        path.filters[3].binding.function_id
    );
    assert_eq!(path.controls.len(), 5);
    restored.modes[0]
        .validate_native_color_recipe(path, &path.measurements[0].recipe)
        .unwrap();
}

#[test]
fn calibration_changes_pin_new_appearance_without_breaking_native_layout_compatibility() {
    let profile = serial_profile();
    let before = identity(&profile);
    let mut changed = profile.clone();
    changed.revision += 1;
    changed.modes[0].color_physical.as_mut().unwrap().revision += 1;
    optical_path_mut(&mut changed).measurements[0].xyz.y = 0.25;
    optical_path_mut(&mut changed).measurements[0]
        .provenance
        .revision += 1;
    let after = identity(&changed);
    assert_eq!(
        before.native_layout_signature,
        after.native_layout_signature
    );
    assert_ne!(before.profile_digest, after.profile_digest);
    assert_ne!(before.profile_revision, after.profile_revision);
    assert_ne!(before.model_revision, after.model_revision);
}

#[test]
fn native_behavior_or_function_identity_changes_invalidate_replay_compatibility() {
    let profile = serial_profile();
    let before = identity(&profile);
    let changes: [fn(&mut FixtureProfile); 3] = [
        |p| p.modes[0].channels[0].invert = true,
        |p| p.modes[0].channels[0].functions[0].dmx_to = 254,
        |p| {
            let new_id = Uuid::new_v4();
            p.modes[0].channels[0].functions[0].id = new_id;
            let path = optical_path_mut(p);
            path.filters[0].binding.function_id = new_id;
            path.measurements[0].recipe[0].function_id = new_id;
        },
    ];
    for change in changes {
        let mut changed = profile.clone();
        change(&mut changed);
        changed.validate().unwrap();
        assert_ne!(
            identity(&changed).native_layout_signature,
            before.native_layout_signature
        );
    }
}

#[test]
fn ownership_list_order_is_not_native_identity_but_optical_order_is_pinned_source_data() {
    let profile = serial_profile();
    let before = identity(&profile);
    let mut reordered = profile.clone();
    optical_path_mut(&mut reordered).controls.reverse();
    assert_eq!(
        identity(&reordered).native_layout_signature,
        before.native_layout_signature
    );
    optical_path_mut(&mut reordered).filters.swap(0, 4);
    assert_ne!(identity(&reordered).profile_digest, before.profile_digest);
}

#[test]
fn native_identity_refuses_invalid_channel_domains_and_invalid_profile_data() {
    let changes: [fn(&mut FixtureProfile); 3] = [
        |p| p.modes[0].channels[0].functions[0].dmx_to = 256,
        |p| p.modes[0].channels[0].default_raw = 256,
        |p| p.manufacturer.clear(),
    ];
    for change in changes {
        let mut profile = serial_profile();
        change(&mut profile);
        assert!(profile.validate().is_err());
        assert!(
            profile
                .native_color_identity(profile.modes[0].id, profile.modes[0].heads[0].id)
                .is_err(),
            "native replay identity must not certify an invalid profile"
        );
    }
}

fn additive_profile() -> FixtureProfile {
    let mut profile = serial_profile();
    let mode = &mut profile.modes[0];
    let emitters = mode.channels[..3]
        .iter()
        .map(|channel| OpticalEmitter {
            id: Uuid::new_v4(),
            name: channel.fixture_attribute.0.to_string(),
            binding: binding(channel),
            xyz: None,
            spectrum: vec![],
            band: OpticalEmitterBand::Visible,
            native_reversed: false,
            maximum_level: 1.0,
            response_exponent: 1.0,
            provenance: Default::default(),
        })
        .collect();
    mode.color_systems.clear();
    let path = optical_path_mut(&mut profile);
    path.source = OpticalSource::Additive { emitters };
    path.filters.drain(..3);
    profile
}

#[test]
fn missing_optional_appearance_and_provenance_remain_unknown_for_additive_emitters() {
    let profile = additive_profile();
    profile.validate().unwrap();
    let mut json = serde_json::to_value(&profile).unwrap();
    let emitters = json["modes"][0]["color_physical"]["paths"][0]["source"]["emitters"]
        .as_array_mut()
        .unwrap();
    for emitter in emitters {
        let object = emitter.as_object_mut().unwrap();
        object.remove("xyz");
        object.remove("spectrum");
        object.remove("provenance");
    }
    let restored: FixtureProfile = serde_json::from_value(json).unwrap();
    restored.validate().unwrap();
    let restored =
        crate::read_fixture_package(&crate::write_fixture_package(&restored).unwrap()).unwrap();
    let OpticalSource::Additive { emitters } = &optical_path(&restored).source else {
        panic!("additive source lost")
    };
    assert_eq!(emitters.len(), 3);
    for emitter in emitters {
        assert!(emitter.xyz.is_none());
        assert!(emitter.spectrum.is_empty());
        assert_eq!(emitter.provenance.quality, PhysicalDataQuality::Unknown);
        assert!(emitter.provenance.source.is_none());
    }
}

#[test]
fn additive_emitter_scaling_and_spectra_must_be_finite_unambiguous_and_nonnegative() {
    let changes: [fn(&mut OpticalEmitter); 6] = [
        |e| e.maximum_level = 0.0,
        |e| e.maximum_level = 1.01,
        |e| e.response_exponent = f32::NAN,
        |e| {
            e.xyz = Some(Xyz {
                x: 0.0,
                y: -1.0,
                z: 0.0,
            })
        },
        |e| {
            e.spectrum = vec![
                SpectrumSample {
                    wavelength_nm: 500.0,
                    value: 1.0,
                },
                SpectrumSample {
                    wavelength_nm: 500.0,
                    value: 0.5,
                },
            ]
        },
        |e| e.spectrum = spectrum(-0.1),
    ];
    for change in changes {
        let mut profile = additive_profile();
        let OpticalSource::Additive { emitters } = &mut optical_path_mut(&mut profile).source
        else {
            unreachable!()
        };
        change(&mut emitters[0]);
        assert_invalid(&profile);
    }
}

#[test]
fn measured_source_may_exceed_unit_output_but_filter_transmission_may_not() {
    let mut profile = additive_profile();
    let OpticalSource::Additive { emitters } = &mut optical_path_mut(&mut profile).source else {
        unreachable!()
    };
    emitters[0].xyz = Some(Xyz {
        x: 1.5,
        y: 2.0,
        z: 0.1,
    });
    emitters[0].spectrum = spectrum(2.0);
    emitters[0].provenance = provenance();
    profile.validate().unwrap();
    optical_path_mut(&mut profile).filters[0].transmission = OpticalTransmission::Spectral {
        samples: vec![FilterSpectrum {
            raw_from: 0,
            raw_to: 255,
            spectrum: spectrum(2.0),
        }],
    };
    assert_invalid(&profile);
}

#[test]
fn invalid_transmission_or_missing_measurement_provenance_is_rejected() {
    let changes: [fn(&mut FixtureProfile); 5] = [
        |p| optical_path_mut(p).measurements[0].provenance.source = None,
        |p| optical_path_mut(p).measurements[0].xyz.y = -0.1,
        |p| {
            optical_path_mut(p).filters[0].transmission = OpticalTransmission::Spectral {
                samples: vec![FilterSpectrum {
                    raw_from: 0,
                    raw_to: 256,
                    spectrum: spectrum(0.5),
                }],
            }
        },
        |p| {
            optical_path_mut(p).filters[0].transmission = OpticalTransmission::Spectral {
                samples: vec![FilterSpectrum {
                    raw_from: 0,
                    raw_to: 255,
                    spectrum: spectrum(1.01),
                }],
            }
        },
        |p| {
            optical_path_mut(p).filters[0].transmission = OpticalTransmission::Spectral {
                samples: vec![
                    FilterSpectrum {
                        raw_from: 0,
                        raw_to: 100,
                        spectrum: spectrum(0.5),
                    },
                    FilterSpectrum {
                        raw_from: 100,
                        raw_to: 255,
                        spectrum: spectrum(0.5),
                    },
                ],
            }
        },
    ];
    for change in changes {
        let mut profile = serial_profile();
        change(&mut profile);
        assert_invalid(&profile);
    }
}

fn profile_with_service_function() -> (FixtureProfile, Uuid) {
    let mut profile = serial_profile();
    let mode = &mut profile.modes[0];
    mode.channels[4].functions[0].dmx_to = 239;
    let action_id = Uuid::new_v4();
    let mut reset = ChannelFunction::continuous("Reset", AttributeKey("control.reset".into()), 255);
    reset.dmx_from = 240;
    reset.behavior = ChannelFunctionBehavior::Control { action_id };
    let function_id = reset.id;
    mode.channels[4].functions.push(reset);
    mode.control_actions.push(ControlAction {
        id: action_id,
        name: "Reset".into(),
        semantic: ControlActionSemantic::Reset,
        kind: ControlActionKind::TimedPulse,
        duration_millis: Some(1000),
        assignments: vec![ControlActionAssignment {
            channel_id: mode.channels[4].id,
            active_raw: 255,
            inactive_raw: 0,
        }],
    });
    profile.validate().unwrap();
    (profile, function_id)
}

#[test]
fn service_functions_sharing_a_color_channel_cannot_enter_a_recordable_color_recipe() {
    let (profile, reset_id) = profile_with_service_function();
    let mode = &profile.modes[0];
    let mut values = recipe(mode, 128);
    values[4].function_id = reset_id;
    values[4].raw = 255;
    assert!(
        mode.validate_native_color_recipe(optical_path(&profile), &values)
            .is_err(),
        "a reset function must not become recordable Direct Color merely by sharing its channel"
    );
}

#[test]
fn service_functions_cannot_be_modeled_as_optical_filters() {
    let (mut profile, reset_id) = profile_with_service_function();
    optical_path_mut(&mut profile).filters[4]
        .binding
        .function_id = reset_id;
    assert_invalid(&profile);
}

#[test]
fn adding_white_outside_the_legacy_system_requires_explicit_color_ownership() {
    let mut profile = serial_profile();
    let mode = &mut profile.modes[0];
    let mut white = channel(mode.heads[0].id, ChannelResolution::U8, vec![]);
    white.fixture_attribute = AttributeKey("color.white".into());
    white.attribute = white.fixture_attribute.clone();
    white.functions[0].attribute = white.attribute.clone();
    white.default_raw = 255;
    mode.channels.push(white);
    let error = mode.validate_color_physical().unwrap_err().to_string();
    assert!(
        error.contains("omits a declared native Color channel"),
        "{error}"
    );
}

#[test]
fn fixed_fixture_control_ranges_need_explicit_color_classification() {
    let mut profile = serial_profile();
    let mode = &mut profile.modes[0];
    let channel = &mut mode.channels[4];
    channel.fixture_attribute = AttributeKey("fixture.control".into());
    channel.attribute = channel.fixture_attribute.clone();
    channel.functions[0].attribute = channel.attribute.clone();
    channel.functions[0].behavior = ChannelFunctionBehavior::Fixed {
        semantic_id: "reset".into(),
        label: "Reset".into(),
        raw_value: 128,
    };
    assert!(!native_color_function_allowed(
        channel,
        &channel.functions[0]
    ));
    let values = recipe(mode, 128);
    assert!(
        mode.validate_native_color_recipe(&mode.color_physical.as_ref().unwrap().paths[0], &values)
            .is_err()
    );
    assert!(mode.validate_color_physical().is_err());
    // A real color-balance range must be explicitly reclassified in the profile;
    // a display label alone is never interpreted as permission to actuate it.
    let channel = &mut mode.channels[4];
    channel.functions[0].attribute = AttributeKey("color.temperature".into());
    channel.functions[0].behavior = ChannelFunctionBehavior::Fixed {
        semantic_id: "balance6500".into(),
        label: "6500 K".into(),
        raw_value: 128,
    };
    assert!(native_color_function_allowed(
        channel,
        &channel.functions[0]
    ));
    mode.validate_color_physical().unwrap();
}

fn installed_calibration(profile: &FixtureProfile) -> crate::InstalledColorCalibration {
    crate::InstalledColorCalibration {
        version: 1,
        revision: 2,
        paths: vec![crate::InstalledColorPathCalibration {
            source_identity: identity(profile),
            emitters: vec![],
            measurements: optical_path(profile).measurements.clone(),
        }],
    }
}

#[test]
fn installed_color_calibration_retains_complete_serial_recipe_and_becomes_stale_on_replacement() {
    let profile = serial_profile();
    let value = installed_calibration(&profile);
    let mode = profile.modes[0].id;
    assert_eq!(
        value.status(&profile, mode),
        crate::InstalledColorCalibrationStatus::Current
    );
    let bytes = serde_json::to_vec(&value).unwrap();
    let restored: crate::InstalledColorCalibration = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored, value);
    let mut replacement = profile.clone();
    optical_path_mut(&mut replacement).measurements[0].xyz.y = 0.8;
    assert_eq!(
        identity(&replacement).native_layout_signature,
        identity(&profile).native_layout_signature
    );
    assert!(matches!(
        restored.status(&replacement, mode),
        crate::InstalledColorCalibrationStatus::Stale { .. }
    ));
    assert!(
        restored.validate().is_ok(),
        "stale observations remain portable and loadable"
    );
    assert_eq!(
        serde_json::to_vec(&restored).unwrap(),
        bytes,
        "do not rewrite source identity"
    );
    let mut missing_wheel = value.clone();
    missing_wheel.paths[0].measurements[0].recipe.pop();
    assert!(
        missing_wheel
            .validate_for_profile(&profile, mode)
            .unwrap_err()
            .contains("every path control")
    );
}

#[test]
fn installed_color_gains_allow_zero_but_never_create_unknown_color_or_filter_transmission() {
    let mut profile = additive_profile();
    let emitter_id = match &optical_path(&profile).source {
        OpticalSource::Additive { emitters } => emitters[0].id,
        _ => unreachable!(),
    };
    let mut value = installed_calibration(&profile);
    value.paths[0].measurements.clear();
    value.paths[0].emitters = vec![crate::InstalledEmitterCalibration {
        emitter_id,
        output_gain: 0.0,
        provenance: provenance(),
    }];
    let mode = profile.modes[0].id;
    value.validate_for_profile(&profile, mode).unwrap();
    assert!(
        matches!(&optical_path(&profile).source,OpticalSource::Additive {emitters} if emitters[0].xyz.is_none() && emitters[0].spectrum.is_empty())
    );
    for gain in [-1.0, f32::INFINITY, f32::NAN] {
        value.paths[0].emitters[0].output_gain = gain;
        assert!(value.validate().is_err());
    }
    value.paths[0].emitters[0].output_gain = 2.0;
    if let OpticalSource::Additive { emitters } = &mut optical_path_mut(&mut profile).source {
        emitters[0].xyz = Some(Xyz {
            x: f32::MAX,
            y: 1.0,
            z: 1.0,
        });
    }
    value.paths[0].source_identity = identity(&profile);
    assert!(
        value
            .validate_for_profile(&profile, mode)
            .unwrap_err()
            .contains("overflows")
    );
    value.paths[0].emitters[0].output_gain = 0.0;
    value.validate_for_profile(&profile, mode).unwrap();
    value.paths[0].emitters[0].emitter_id = Uuid::new_v4();
    assert!(
        value
            .validate_for_profile(&profile, mode)
            .unwrap_err()
            .contains("missing additive emitter")
    );
}

#[test]
fn installed_color_observations_reject_ambiguous_or_fabricated_evidence() {
    let profile = serial_profile();
    let value = installed_calibration(&profile);
    let mut bad = value.clone();
    bad.version = 2;
    assert!(bad.validate().is_err());
    let mut bad = value.clone();
    bad.paths.push(bad.paths[0].clone());
    assert!(bad.validate().is_err());
    let mut bad = value.clone();
    bad.paths[0].source_identity.profile_digest = "missing".into();
    assert!(bad.validate().is_err());
    let mut bad = value.clone();
    bad.paths[0].measurements[0].provenance.source = None;
    assert!(bad.validate().is_err());
    let mut bad = value.clone();
    bad.paths[0].measurements[0].xyz.x = -0.1;
    assert!(bad.validate().is_err());
    let mut bad = value.clone();
    bad.paths[0]
        .measurements
        .push(value.paths[0].measurements[0].clone());
    assert!(bad.validate().is_err());
    let mut black = value.clone();
    black.paths[0].measurements[0].xyz = Xyz {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    black
        .validate_for_profile(&profile, profile.modes[0].id)
        .unwrap();
}

#[test]
fn installed_color_context_uses_full_profile_identity_and_preserves_independent_modes() {
    let mut profile = additive_profile();
    let mut second = profile.modes[0].clone();
    second.id = Uuid::new_v4();
    second.name = "Other mode".into();
    profile.modes.push(second);
    let value = installed_calibration(&profile);
    let context = crate::ColorCalibrationContext::new(&profile, profile.modes[0].id).unwrap();
    value.validate_for_context(&context).unwrap();
    assert_eq!(context.identities()[0], value.paths[0].source_identity);
    let mut compact = profile.clone();
    compact.modes.truncate(1);
    assert!(
        value
            .validate_for_profile(&compact, compact.modes[0].id)
            .is_err(),
        "a compact projection must not redefine authoring identity"
    );
    value.validate_for_context(&context).unwrap();
    assert!(
        value
            .validate_for_profile(&profile, profile.modes[1].id)
            .is_err()
    );
}

#[test]
fn uv_control_identity_survives_unknown_black_and_visible_appearance() {
    for xyz in [
        None,
        Some(Xyz {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }),
        Some(Xyz {
            x: 0.01,
            y: 0.001,
            z: 0.04,
        }),
    ] {
        let mut profile = additive_profile();
        let OpticalSource::Additive { emitters } = &mut optical_path_mut(&mut profile).source
        else {
            unreachable!()
        };
        emitters[0].band = OpticalEmitterBand::Ultraviolet;
        emitters[0].xyz = xyz;
        // Synthetic spectrum crossing the nominal UV/visible boundary; no physical fixture claim.
        emitters[0].spectrum = vec![
            SpectrumSample {
                wavelength_nm: 395.0,
                value: 1.0,
            },
            SpectrumSample {
                wavelength_nm: 405.0,
                value: 0.5,
            },
        ];
        let binding = emitters[0].binding;
        profile.validate().unwrap();
        let restored =
            crate::read_fixture_package(&crate::write_fixture_package(&profile).unwrap()).unwrap();
        let OpticalSource::Additive { emitters } = &optical_path(&restored).source else {
            unreachable!()
        };
        assert_eq!(emitters[0].band, OpticalEmitterBand::Ultraviolet);
        assert_eq!(emitters[0].xyz, xyz);
        assert_eq!(emitters[0].binding, binding);
        assert_eq!(emitters[0].spectrum.len(), 2);
    }
}

#[path = "color_physical_forward.rs"]
mod forward;

#[path = "native_color_edit.rs"]
mod native_edit;
