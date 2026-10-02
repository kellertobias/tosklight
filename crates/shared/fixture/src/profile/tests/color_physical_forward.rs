//! Analytical native-output tests; XYZ basis vectors here are synthetic, not lamp calibrations.
use super::*;
use crate::forward::{
    ColorForwardFlags as Flags, ColorForwardInputError, ColorForwardResult, CompiledColorForward,
};

fn xyz(x: f32, y: f32, z: f32) -> Xyz {
    Xyz { x, y, z }
}
fn emitters(profile: &mut FixtureProfile) -> &mut Vec<OpticalEmitter> {
    match &mut optical_path_mut(profile).source {
        OpticalSource::Additive { emitters } => emitters,
        _ => panic!("expected additive test source"),
    }
}
fn additive() -> FixtureProfile {
    let mut p = additive_profile();
    p.modes[0].channels.truncate(3);
    let path = optical_path_mut(&mut p);
    path.controls.truncate(3);
    path.filters.clear();
    path.measurements.clear();
    for (e, value) in
        emitters(&mut p)
            .iter_mut()
            .zip([xyz(1., 0., 0.), xyz(0., 1., 0.), xyz(0., 0., 1.)])
    {
        e.xyz = Some(value);
        e.provenance = provenance();
    }
    p
}
fn evaluate(p: &FixtureProfile, raw: &[u32]) -> ColorForwardResult {
    let compiled = CompiledColorForward::compile(p, p.modes[0].id, None)
        .unwrap()
        .unwrap();
    let mut output = compiled.create_output();
    compiled.evaluate(raw, &mut output).unwrap();
    output.remove(0)
}
fn full_spectrum(value: f32) -> Vec<SpectrumSample> {
    vec![
        SpectrumSample {
            wavelength_nm: 360.,
            value,
        },
        SpectrumSample {
            wavelength_nm: 830.,
            value,
        },
    ]
}
fn serial() -> FixtureProfile {
    let mut p = serial_profile();
    let path = optical_path_mut(&mut p);
    path.measurements.clear();
    path.source = OpticalSource::Fixed {
        xyz: None,
        spectrum: full_spectrum(1.),
        provenance: provenance(),
    };
    for f in &mut path.filters {
        f.provenance = provenance();
        f.transmission = OpticalTransmission::Spectral {
            samples: vec![FilterSpectrum {
                raw_from: 0,
                raw_to: 255,
                spectrum: full_spectrum(0.5),
            }],
        };
    }
    p
}

#[test]
fn final_raw_uses_native_emitter_direction_without_reapplying_canonical_or_channel_inversion() {
    let mut p = additive();
    // Test helper retains CMY -> inverted RGB canonical aliases, while these are additive sources.
    p.modes[0].channels[0].invert = true;
    emitters(&mut p)[0].xyz = Some(xyz(0., 1., 1.)); // native Cyan, not canonical Red
    let output = evaluate(&p, &[255, 0, 0]);
    assert_eq!(output.known_xyz, xyz(0., 1., 1.));
    assert!(output.visible_complete);
    assert_eq!(output.flags, Flags::default());
    emitters(&mut p)[0].native_reversed = true;
    assert_eq!(evaluate(&p, &[255, 0, 0]).known_xyz, xyz(0., 0., 0.));
    assert_eq!(evaluate(&p, &[0, 0, 0]).known_xyz, xyz(0., 1., 1.));
}

#[test]
fn uv_off_unknown_measured_zero_and_visible_leakage_have_distinct_results() {
    let mut p = additive();
    let uv = &mut emitters(&mut p)[2];
    uv.band = OpticalEmitterBand::Ultraviolet;
    uv.xyz = None;
    let off = evaluate(&p, &[255, 0, 0]);
    assert!(off.visible_complete);
    assert_eq!(off.known_xyz, xyz(1., 0., 0.));
    assert_eq!(off.uv_drive_max, 0.);
    assert_eq!(off.portable_uv.unwrap().amount, 0.);
    let unknown = evaluate(&p, &[255, 0, 255]);
    assert!(!unknown.visible_complete);
    assert!(unknown.flags.contains(Flags::UNKNOWN_EMITTER));
    assert_eq!(unknown.known_xyz, xyz(1., 0., 0.));
    assert_eq!(unknown.uv_drive_max, 1.);
    let uv = unknown.portable_uv.unwrap();
    assert_eq!(uv.amount, 1.);
    assert_eq!(uv.quality, PhysicalDataQuality::Estimated);
    emitters(&mut p)[2].xyz = Some(xyz(0., 0., 0.));
    let zero = evaluate(&p, &[0, 0, 255]);
    assert!(zero.visible_complete);
    assert_eq!(zero.known_xyz, xyz(0., 0., 0.));
    assert_eq!(zero.uv_drive_max, 1.);
    emitters(&mut p)[2].xyz = Some(xyz(0.1, 0.01, 0.3));
    let visible = evaluate(&p, &[255, 0, 255]);
    assert!(visible.visible_complete);
    assert_eq!(visible.known_xyz, xyz(1.1, 0.01, 0.3));
    assert_eq!(visible.uv_emitters[0].drive, 1.);
}

#[test]
fn native_above_uv_limit_is_simulated_and_flagged_instead_of_clamped() {
    let mut p = additive();
    let uv = &mut emitters(&mut p)[2];
    uv.band = OpticalEmitterBand::Ultraviolet;
    uv.maximum_level = 0.2;
    uv.response_exponent = 2.;
    let actual = evaluate(&p, &[0, 0, 255]);
    assert_eq!(actual.known_xyz, xyz(0., 0., 1.));
    assert!(actual.flags.contains(Flags::NATIVE_OVER_LIMIT));
    assert!(actual.uv_emitters[0].above_limit);
    assert_eq!(actual.uv_drive_max, 1.);
    assert_eq!(actual.portable_uv.unwrap().amount, 1.);
}

#[test]
fn u32_adjacent_values_and_reversed_uv_do_not_round_through_f32() {
    let mut p = additive();
    let channel = &mut p.modes[0].channels[2];
    channel.resolution = ChannelResolution::U32;
    channel.secondary_slots = vec![4, 5, 6];
    channel.functions[0].dmx_from = u32::MAX - 2;
    channel.functions[0].dmx_to = u32::MAX;
    let uv = &mut emitters(&mut p)[2];
    uv.band = OpticalEmitterBand::Ultraviolet;
    uv.native_reversed = true;
    for (raw, expected) in [(u32::MAX - 2, 1.), (u32::MAX - 1, 0.5), (u32::MAX, 0.)] {
        let result = evaluate(&p, &[0, 0, raw]);
        assert_eq!(result.uv_drive_max, expected);
        assert_eq!(result.portable_uv.unwrap().amount, expected);
        assert_eq!(result.known_xyz.z, expected as f32);
    }
}

#[test]
fn modeled_non_uv_paths_have_known_zero_drive_even_with_unknown_visible_appearance() {
    let mut rgb = additive();
    emitters(&mut rgb)[0].xyz = None;
    let result = evaluate(&rgb, &[255, 0, 0]);
    assert!(!result.visible_complete);
    assert!(result.uv_emitters.is_empty());
    assert_eq!(result.portable_uv.unwrap().amount, 0.);

    let mut wheel = serial();
    optical_path_mut(&mut wheel).filters[0].transmission = OpticalTransmission::Unknown;
    let result = evaluate(&wheel, &[0; 5]);
    assert!(!result.visible_complete);
    assert_eq!(result.portable_uv.unwrap().amount, 0.);
}

#[test]
fn visible_measurements_do_not_establish_unknown_source_or_unmodeled_control_uv() {
    for unknown_source in [true, false] {
        let mut p = serial();
        let measurement = ColorRecipeMeasurement {
            recipe: recipe(&p.modes[0], 128),
            xyz: xyz(0.3, 0.4, 0.5),
            provenance: provenance(),
        };
        let path = optical_path_mut(&mut p);
        if unknown_source {
            path.source = OpticalSource::Unknown;
        } else {
            // The channel remains a participating Color control with no authored optical role.
            path.filters.pop();
        }
        path.measurements.push(measurement);
        let result = evaluate(&p, &[128; 5]);
        assert!(result.visible_complete);
        assert_eq!(result.known_xyz, xyz(0.3, 0.4, 0.5));
        assert_eq!(result.flags, Flags::default());
        assert!(result.uv_emitters.is_empty());
        assert_eq!(result.portable_uv, None);
    }

    let mut p = additive();
    emitters(&mut p)[2].band = OpticalEmitterBand::Ultraviolet;
    emitters(&mut p).remove(0);
    let measurement = ColorRecipeMeasurement {
        recipe: recipe(&p.modes[0], 255),
        xyz: xyz(0.3, 0.4, 0.5),
        provenance: provenance(),
    };
    optical_path_mut(&mut p).measurements.push(measurement);
    let result = evaluate(&p, &[255; 3]);
    assert!(result.visible_complete);
    assert_eq!(result.flags, Flags::default());
    assert_eq!(result.uv_emitters[0].drive, 1.);
    assert_eq!(
        result.portable_uv, None,
        "an unmodeled control may override the UV bank"
    );
}

#[test]
fn uniform_uv_banks_compare_exact_native_ratios_and_unequal_banks_stay_unknown() {
    let mut p = additive();
    for emitter in &mut emitters(&mut p)[1..] {
        emitter.band = OpticalEmitterBand::Ultraviolet;
    }
    assert_eq!(
        evaluate(&p, &[0, 128, 128]).portable_uv.unwrap().amount,
        128. / 255.
    );
    assert_eq!(evaluate(&p, &[0, 127, 128]).portable_uv, None);
    assert_eq!(evaluate(&p, &[0, 0, 0]).portable_uv.unwrap().amount, 0.);

    // Different resolutions can still encode exactly the same normalized request.
    let channel = &mut p.modes[0].channels[1];
    channel.resolution = ChannelResolution::U32;
    channel.secondary_slots = vec![4, 5, 6];
    channel.functions[0].dmx_to = u32::MAX;
    let exact = 128 * (u32::MAX / 255);
    let result = evaluate(&p, &[0, exact, 128]);
    assert_eq!(result.portable_uv.unwrap().amount, 128. / 255.);
    assert_eq!(evaluate(&p, &[0, exact + 1, 128]).portable_uv, None);
    emitters(&mut p)[1].native_reversed = true;
    assert_eq!(
        evaluate(&p, &[0, u32::MAX - exact, 128])
            .portable_uv
            .unwrap()
            .amount,
        128. / 255.
    );
}

#[test]
fn alternative_uv_functions_on_one_channel_are_one_bank() {
    let mut p = additive();
    let channel = &mut p.modes[0].channels[2];
    channel.functions[0].dmx_to = 127;
    let mut alternate_function = channel.functions[0].clone();
    alternate_function.id = Uuid::new_v4();
    alternate_function.dmx_from = 128;
    alternate_function.dmx_to = 255;
    let alternate_id = alternate_function.id;
    channel.functions.push(alternate_function);
    let emitters = emitters(&mut p);
    emitters[2].band = OpticalEmitterBand::Ultraviolet;
    let mut alternate = emitters[2].clone();
    alternate.id = Uuid::new_v4();
    alternate.binding.function_id = alternate_id;
    emitters.push(alternate);

    for raw in [0, 64, 127, 128, 191, 255] {
        let result = evaluate(&p, &[0, 0, raw]);
        let expected = f64::from(raw % 128) / 127.;
        assert_eq!(result.portable_uv.unwrap().amount, expected);
        assert_eq!(result.uv_emitters.len(), 2);
    }
    // A different explicitly modeled function may put the bank in a non-UV mode.
    if let OpticalSource::Additive { emitters } = &mut optical_path_mut(&mut p).source {
        emitters[3].band = OpticalEmitterBand::Visible;
    }
    assert_eq!(evaluate(&p, &[0, 0, 255]).portable_uv.unwrap().amount, 0.);
}

#[test]
fn five_serial_transmissions_multiply_spectra_without_normalizing_each_source() {
    let p = serial();
    let out = evaluate(&p, &[0; 5]);
    assert!(out.visible_complete);
    // Equal-energy observer integral Y=106.856915, five neutral half-transmission stages.
    assert!((out.known_xyz.y - 106.856915 / 32.).abs() < 1e-5);
    let mut twice = p.clone();
    if let OpticalSource::Fixed { spectrum, .. } = &mut optical_path_mut(&mut twice).source {
        *spectrum = full_spectrum(2.);
    }
    assert_eq!(evaluate(&twice, &[0; 5]).known_xyz.y, out.known_xyz.y * 2.);
}

#[test]
fn filter_alternatives_on_one_channel_are_not_stacked_and_gaps_stay_unknown() {
    let mut p = serial();
    let mode = &mut p.modes[0];
    mode.channels.truncate(1);
    mode.color_systems.clear();
    let first = &mut mode.channels[0].functions[0];
    first.dmx_to = 100;
    let mut second = first.clone();
    second.id = Uuid::new_v4();
    second.dmx_from = 110;
    second.dmx_to = 255;
    let second_id = second.id;
    mode.channels[0].functions.push(second);
    let path = optical_path_mut(&mut p);
    path.controls.truncate(1);
    path.filters.truncate(1);
    path.filters[0].transmission = OpticalTransmission::Spectral {
        samples: vec![FilterSpectrum {
            raw_from: 0,
            raw_to: 100,
            spectrum: full_spectrum(0.5),
        }],
    };
    let mut alternate = path.filters[0].clone();
    alternate.id = Uuid::new_v4();
    alternate.binding.function_id = second_id;
    alternate.transmission = OpticalTransmission::Spectral {
        samples: vec![FilterSpectrum {
            raw_from: 110,
            raw_to: 255,
            spectrum: full_spectrum(0.25),
        }],
    };
    path.filters.push(alternate);
    let a = evaluate(&p, &[50]);
    let b = evaluate(&p, &[150]);
    assert!(a.visible_complete && b.visible_complete);
    assert_eq!(a.known_xyz.y, b.known_xyz.y * 2.);
    let gap = evaluate(&p, &[105]);
    assert!(!gap.visible_complete);
    assert!(gap.flags.contains(Flags::UNMODELED_CONTROL));
    assert_eq!(
        gap.known_xyz,
        xyz(0., 0., 0.),
        "an unknown optical function must not expose the unfiltered source as known output"
    );
}

#[test]
fn unknown_filter_sample_gaps_and_missing_spectrum_never_show_unfiltered_xyz() {
    let mut p = serial();
    optical_path_mut(&mut p).filters[0].transmission = OpticalTransmission::Unknown;
    let unknown = evaluate(&p, &[0; 5]);
    assert!(!unknown.visible_complete);
    assert_eq!(unknown.known_xyz, xyz(0., 0., 0.));
    assert!(unknown.flags.contains(Flags::UNKNOWN_FILTER));
    let mut p = serial();
    if let OpticalTransmission::Spectral { samples } =
        &mut optical_path_mut(&mut p).filters[0].transmission
    {
        samples[0].raw_from = 10;
    }
    let gap = evaluate(&p, &[0; 5]);
    assert!(!gap.visible_complete);
    assert!(gap.flags.contains(Flags::FILTER_SAMPLE_GAP));
    let mut p = serial();
    optical_path_mut(&mut p).source = OpticalSource::Fixed {
        xyz: Some(xyz(1., 1., 1.)),
        spectrum: vec![],
        provenance: provenance(),
    };
    let no_spectrum = evaluate(&p, &[0; 5]);
    assert!(!no_spectrum.visible_complete);
    assert_eq!(no_spectrum.known_xyz, xyz(0., 0., 0.));
    assert!(no_spectrum.flags.contains(Flags::SPECTRAL_COVERAGE));
    optical_path_mut(&mut p).source = OpticalSource::Fixed {
        xyz: Some(xyz(0., 0., 0.)),
        spectrum: vec![],
        provenance: provenance(),
    };
    assert!(
        evaluate(&p, &[0; 5]).visible_complete,
        "measured zero survives any passive transmission"
    );
}

#[test]
fn partial_spectrum_is_unknown_unless_independent_xyz_can_resolve_unfiltered_output() {
    let mut p = additive();
    let e = &mut emitters(&mut p)[0];
    e.spectrum = spectrum(1.);
    e.xyz = None;
    let unknown = evaluate(&p, &[255, 0, 0]);
    assert!(!unknown.visible_complete);
    assert!(unknown.flags.contains(Flags::SPECTRAL_COVERAGE));
    emitters(&mut p)[0].xyz = Some(xyz(0.2, 0.3, 0.4));
    let known = evaluate(&p, &[255, 0, 0]);
    assert!(known.visible_complete);
    assert_eq!(known.known_xyz, xyz(0.2, 0.3, 0.4));
}

#[test]
fn conflicting_xyz_and_complete_spd_cannot_switch_brightness_when_filtered() {
    let mut p = serial();
    if let OpticalSource::Fixed { xyz, .. } = &mut optical_path_mut(&mut p).source {
        *xyz = Some(Xyz {
            x: 1.,
            y: 1.,
            z: 1.,
        });
    }
    let result = evaluate(&p, &[0; 5]);
    assert!(!result.visible_complete);
    assert!(result.flags.contains(Flags::INCONSISTENT_SOURCE));
}

#[test]
fn complete_recipe_observation_resolves_unknown_appearance_but_preserves_uv_activity() {
    let mut p = additive();
    emitters(&mut p)[2].band = OpticalEmitterBand::Ultraviolet;
    emitters(&mut p)[2].xyz = None;
    let measurement = ColorRecipeMeasurement {
        recipe: recipe(&p.modes[0], 255),
        xyz: xyz(0.3, 0.4, 0.5),
        provenance: provenance(),
    };
    optical_path_mut(&mut p).measurements.push(measurement);
    let out = evaluate(&p, &[255; 3]);
    assert!(out.visible_complete);
    assert_eq!(out.known_xyz, xyz(0.3, 0.4, 0.5));
    assert_eq!(out.uv_drive_max, 1.);
    assert_eq!(
        out.portable_uv.unwrap().quality,
        PhysicalDataQuality::Estimated
    );
    assert_eq!(out.flags, Flags::default());
    assert!(!evaluate(&p, &[255, 0, 255]).visible_complete);
}

#[test]
fn installed_observation_overrides_profile_without_double_gain_and_stale_data_is_ignored() {
    let mut p = additive();
    let measurement = ColorRecipeMeasurement {
        recipe: recipe(&p.modes[0], 255),
        xyz: xyz(1., 2., 3.),
        provenance: provenance(),
    };
    optical_path_mut(&mut p).measurements.push(measurement);
    let emitter_id = emitters(&mut p)[0].id;
    let mut installed = installed_calibration(&p);
    installed.paths[0].measurements[0].xyz = xyz(4., 5., 6.);
    installed.paths[0]
        .emitters
        .push(crate::InstalledEmitterCalibration {
            emitter_id,
            output_gain: 0.,
            provenance: provenance(),
        });
    let compiled = CompiledColorForward::compile(&p, p.modes[0].id, Some(&installed))
        .unwrap()
        .unwrap();
    let mut out = compiled.create_output();
    compiled.evaluate(&[255; 3], &mut out).unwrap();
    assert_eq!(out[0].known_xyz, xyz(4., 5., 6.));
    compiled.evaluate(&[255, 0, 0], &mut out).unwrap();
    assert_eq!(out[0].known_xyz, xyz(0., 0., 0.));
    installed.paths[0].measurements.clear();
    let compiled = CompiledColorForward::compile(&p, p.modes[0].id, Some(&installed))
        .unwrap()
        .unwrap();
    compiled.evaluate(&[255; 3], &mut out).unwrap();
    assert_eq!(
        out[0].known_xyz,
        xyz(0., 1., 1.),
        "whole-path profile observation cannot receive per-emitter gain"
    );
    p.revision += 1;
    let stale = CompiledColorForward::compile(&p, p.modes[0].id, Some(&installed))
        .unwrap()
        .unwrap();
    stale.evaluate(&[255, 0, 0], &mut out).unwrap();
    assert_eq!(out[0].known_xyz, xyz(1., 0., 0.));
    assert!(out[0].flags.contains(Flags::STALE_CALIBRATION));
}

#[test]
fn zero_installed_uv_gain_keeps_native_activity() {
    let mut p = additive();
    emitters(&mut p)[2].band = OpticalEmitterBand::Ultraviolet;
    let id = emitters(&mut p)[2].id;
    let mut installed = installed_calibration(&p);
    installed.paths[0]
        .emitters
        .push(crate::InstalledEmitterCalibration {
            emitter_id: id,
            output_gain: 0.,
            provenance: provenance(),
        });
    let compiled = CompiledColorForward::compile(&p, p.modes[0].id, Some(&installed))
        .unwrap()
        .unwrap();
    let mut out = compiled.create_output();
    compiled.evaluate(&[0, 0, 255], &mut out).unwrap();
    assert_eq!(out[0].known_xyz, xyz(0., 0., 0.));
    assert!(out[0].visible_complete);
    assert_eq!(out[0].uv_drive_max, 1.);
}

#[test]
fn conflicting_duplicate_recipe_is_rejected_and_invalid_input_never_partially_writes_output() {
    let mut p = additive();
    let m = ColorRecipeMeasurement {
        recipe: recipe(&p.modes[0], 255),
        xyz: xyz(1., 2., 3.),
        provenance: provenance(),
    };
    optical_path_mut(&mut p).measurements = vec![m.clone(), m];
    optical_path_mut(&mut p).measurements[1].xyz.y = 7.;
    assert!(
        CompiledColorForward::compile(&p, p.modes[0].id, None)
            .unwrap_err()
            .to_string()
            .contains("conflicting")
    );
    optical_path_mut(&mut p).measurements.clear();
    let compiled = CompiledColorForward::compile(&p, p.modes[0].id, None)
        .unwrap()
        .unwrap();
    let mut out = compiled.create_output();
    let before = out.clone();
    assert_eq!(
        compiled.evaluate(&[0], &mut out),
        Err(ColorForwardInputError::ChannelCount)
    );
    assert_eq!(
        compiled.evaluate(&[0, 256, 0], &mut out),
        Err(ColorForwardInputError::RawOutOfRange)
    );
    assert_eq!(out, before);
}

#[test]
fn finite_input_sum_overflow_is_not_a_valid_achieved_color() {
    let mut p = additive();
    for e in emitters(&mut p) {
        e.xyz = Some(xyz(f32::MAX, 1., 1.));
    }
    let out = evaluate(&p, &[255; 3]);
    assert!(!out.visible_complete);
    assert!(out.flags.contains(Flags::NUMERIC_OVERFLOW));
    assert_eq!(out.known_xyz, xyz(0., 0., 0.));
}

#[test]
fn installed_recipe_capacity_is_bounded_and_uv_output_identity_cannot_be_reused_from_another_plan()
{
    let mut p = additive();
    emitters(&mut p)[2].band = OpticalEmitterBand::Ultraviolet;
    let compiled = CompiledColorForward::compile(&p, p.modes[0].id, None)
        .unwrap()
        .unwrap();
    let mut out = compiled.create_output();
    out[0].uv_emitters[0].emitter_id = Uuid::new_v4();
    let previous = out.clone();
    assert_eq!(
        compiled.evaluate(&[0, 0, 0], &mut out),
        Err(ColorForwardInputError::OutputLayout)
    );
    assert_eq!(out, previous);
    let mut installed = installed_calibration(&p);
    for n in 0..5462_u32 {
        let mut recipe = recipe(&p.modes[0], 0);
        recipe[0].raw = n % 256;
        recipe[1].raw = n / 256;
        installed.paths[0]
            .measurements
            .push(ColorRecipeMeasurement {
                recipe,
                xyz: xyz(0., 0., 0.),
                provenance: provenance(),
            });
    }
    installed.validate_for_profile(&p, p.modes[0].id).unwrap();
    assert!(
        CompiledColorForward::compile(&p, p.modes[0].id, Some(&installed))
            .unwrap_err()
            .to_string()
            .contains("bounded forward capacity")
    );
}

#[test]
fn unmeasured_installed_filter_keeps_uv_drive_without_unfiltered_visible_fallback() {
    let mut profile = additive();
    emitters(&mut profile)[2].band = OpticalEmitterBand::Ultraviolet;
    let mut appearance = crate::InstalledFixtureAppearance::default();
    appearance.color_temperature_kelvin = Some(3200);
    let model = CompiledColorForward::compile(&profile, profile.modes[0].id, None)
        .unwrap()
        .unwrap()
        .with_installed_appearance(&appearance);
    let mut output = model.create_output();
    model.evaluate(&[255, 255, 255], &mut output).unwrap();
    assert_eq!(output[0].known_xyz, xyz(0., 0., 0.));
    assert_eq!(output[0].uv_drive_max, 1.);
    assert_eq!(output[0].portable_uv.unwrap().amount, 1.);
    assert!(!output[0].visible_complete);
    assert!(output[0].flags.contains(Flags::UNKNOWN_SOURCE));
}
