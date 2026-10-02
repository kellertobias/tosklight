use super::*;

fn physical_channel(
    resolution: ChannelResolution,
    raw_from: u32,
    raw_to: u32,
    physical_from: f32,
    physical_to: f32,
) -> FixtureChannel {
    let mut value = channel(Uuid::new_v4(), resolution, Vec::new());
    value.attribute = AttributeKey("pan".into());
    value.fixture_attribute = value.attribute.clone();
    value.physical_min = Some(physical_from.min(physical_to));
    value.physical_max = Some(physical_from.max(physical_to));
    value.unit = Some("deg".into());
    let function = &mut value.functions[0];
    function.attribute = value.attribute.clone();
    function.dmx_from = raw_from;
    function.dmx_to = raw_to;
    function.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: physical_from,
        physical_max: physical_to,
        unit: Some("deg".into()),
    };
    value
}

fn compile(channel: &FixtureChannel) -> CompiledPhysicalMapping {
    CompiledPhysicalMapping::compile(channel, &channel.functions[0])
        .expect("valid physical mapping")
        .expect("continuous function")
}

fn samples(channel: &mut FixtureChannel, points: &[(u32, f32)]) {
    channel.functions[0].physical_mapping = Some(PhysicalMappingCalibration {
        samples: points
            .iter()
            .map(|&(raw, physical)| PhysicalMappingPoint { raw, physical })
            .collect(),
        ..Default::default()
    });
}

fn assert_rejected(channel: &FixtureChannel) {
    assert!(
        CompiledPhysicalMapping::compile(channel, &channel.functions[0]).is_err(),
        "invalid physical data must not compile"
    );
}

#[test]
fn signed_multiturn_mapping_preserves_direction_and_exact_mechanical_zero() {
    for (from, to) in [(-720.0, 720.0), (720.0, -720.0)] {
        let channel = physical_channel(ChannelResolution::U16, 100, 65_500, from, to);
        let mapping = compile(&channel);
        assert_eq!(mapping.physical_for_raw(100).physical, f64::from(from));
        assert_eq!(mapping.physical_for_raw(65_500).physical, f64::from(to));
        let zero = mapping.raw_for_physical(0.0).unwrap();
        assert_eq!(zero.raw, 32_800);
        assert_eq!(zero.physical, 0.0);
        assert!(!zero.clipped);
        let left = mapping.raw_for_physical(-360.0).unwrap();
        let right = mapping.raw_for_physical(360.0).unwrap();
        assert_eq!(left.physical, -360.0);
        assert_eq!(right.physical, 360.0);
        assert_eq!(left.raw < right.raw, from < to);
    }
}

#[test]
fn piecewise_calibration_changes_the_middle_without_rewriting_endpoints() {
    for direction in [1.0, -1.0] {
        let mut channel = physical_channel(
            ChannelResolution::U16,
            100,
            500,
            -100.0 * direction,
            100.0 * direction,
        );
        samples(
            &mut channel,
            &[
                (100, -100.0 * direction),
                (200, -20.0 * direction),
                (500, 100.0 * direction),
            ],
        );
        let mapping = compile(&channel);
        for (raw, expected) in [
            (100, -100.0),
            (150, -60.0),
            (200, -20.0),
            (350, 40.0),
            (500, 100.0),
        ] {
            let expected = expected * f64::from(direction);
            assert!((mapping.physical_for_raw(raw).physical - expected).abs() < 1e-10);
            assert_eq!(mapping.raw_for_physical(expected).unwrap().raw, raw);
        }
    }
}

#[test]
fn calibration_samples_must_retain_both_authoritative_endpoints() {
    for points in [
        vec![(0, -90.0)],
        vec![(1, -90.0), (255, 90.0)],
        vec![(0, -89.0), (255, 90.0)],
        vec![(0, -90.0), (254, 90.0)],
        vec![(0, -90.0), (255, 89.0)],
    ] {
        let mut channel = physical_channel(ChannelResolution::U8, 0, 255, -90.0, 90.0);
        samples(&mut channel, &points);
        assert_rejected(&channel);
    }
}

#[test]
fn calibration_rejects_ambiguous_or_nonfinite_sample_curves() {
    for direction in [1.0, -1.0] {
        for middle in [
            vec![(100, 0.0), (100, 10.0)],
            vec![(150, 0.0), (100, 10.0)],
            vec![(100, 0.0), (150, 0.0)],
            vec![(100, 10.0), (150, -10.0)],
            vec![(100, f32::NAN)],
            vec![(100, f32::INFINITY)],
            vec![(100, f32::NEG_INFINITY)],
        ] {
            let mut points = vec![(0, -90.0 * direction)];
            points.extend(
                middle
                    .into_iter()
                    .map(|(raw, physical)| (raw, physical * direction)),
            );
            points.push((255, 90.0 * direction));
            let mut channel = physical_channel(
                ChannelResolution::U8,
                0,
                255,
                -90.0 * direction,
                90.0 * direction,
            );
            samples(&mut channel, &points);
            assert_rejected(&channel);
        }
    }
}

#[test]
fn compile_rejects_invalid_raw_spans_and_physical_endpoints() {
    for (raw_from, raw_to) in [(20, 20), (30, 20), (0, 256)] {
        assert_rejected(&physical_channel(
            ChannelResolution::U8,
            raw_from,
            raw_to,
            -90.0,
            90.0,
        ));
    }
    for (from, to) in [
        (1.0, 1.0),
        (f32::NAN, 90.0),
        (-90.0, f32::INFINITY),
        (f32::NEG_INFINITY, 90.0),
    ] {
        assert_rejected(&physical_channel(ChannelResolution::U8, 0, 255, from, to));
    }
}

#[test]
fn physical_requests_reject_nonfinite_values_instead_of_emitting_a_raw_command() {
    let mapping = compile(&physical_channel(
        ChannelResolution::U8,
        0,
        255,
        -90.0,
        90.0,
    ));
    for requested in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(mapping.raw_for_physical(requested).is_err());
    }
}

#[test]
fn raw_roundtrips_keep_fine_bytes_at_all_four_resolutions() {
    for resolution in [
        ChannelResolution::U8,
        ChannelResolution::U16,
        ChannelResolution::U24,
        ChannelResolution::U32,
    ] {
        let max = resolution.max_raw();
        for (from, to) in [(-270.0, 270.0), (270.0, -270.0)] {
            let mapping = compile(&physical_channel(resolution, 0, max, from, to));
            let mut raws = vec![0, 1, max / 2, max / 2 + 1, max - 1, max];
            // Include adjacent fine-byte values across the whole channel, not just endpoints.
            for index in 0..=1_024_u64 {
                let raw = (u64::from(max) * index / 1_024) as u32;
                raws.push(raw);
                if raw < max {
                    raws.push(raw + 1);
                }
            }
            for raw in raws {
                let physical = mapping.physical_for_raw(raw);
                let restored = mapping.raw_for_physical(physical.physical).unwrap();
                assert_eq!(restored.raw, raw, "{resolution:?} raw {raw} lost precision");
                assert!(!restored.clipped);
            }
            let half_step = 540.0 / f64::from(max) / 2.0;
            for requested in [-269.99, -137.123, -0.000_001, 0.0, 41.345, 269.99] {
                let reached = mapping.raw_for_physical(requested).unwrap();
                assert!(
                    (reached.physical - requested).abs() <= half_step + 1e-10,
                    "{resolution:?}: {requested} reached {}",
                    reached.physical
                );
                assert!(!reached.clipped, "quantization is not range clipping");
            }
        }
    }
}

#[test]
fn nonzero_raw_spans_clip_only_to_their_own_function() {
    let mapping = compile(&physical_channel(
        ChannelResolution::U8,
        20,
        220,
        60.0,
        10.0,
    ));
    for (requested, raw, physical, clipped) in [
        (0, 20, 60.0, true),
        (20, 20, 60.0, false),
        (120, 120, 35.0, false),
        (220, 220, 10.0, false),
        (255, 220, 10.0, true),
    ] {
        let reached = mapping.physical_for_raw(requested);
        assert_eq!(
            (reached.raw, reached.physical, reached.clipped),
            (raw, physical, clipped)
        );
    }
    for (requested, raw, physical, clipped) in [
        (70.0, 20, 60.0, true),
        (60.0, 20, 60.0, false),
        (35.0, 120, 35.0, false),
        (10.0, 220, 10.0, false),
        (-5.0, 220, 10.0, true),
    ] {
        let reached = mapping.raw_for_physical(requested).unwrap();
        assert_eq!(
            (reached.raw, reached.physical, reached.clipped),
            (raw, physical, clipped)
        );
    }
}

#[test]
fn declared_evidence_survives_compilation_and_requires_a_source_when_asserted() {
    for quality in [
        PhysicalDataQuality::Unknown,
        PhysicalDataQuality::Estimated,
        PhysicalDataQuality::Manufacturer,
        PhysicalDataQuality::Measured,
    ] {
        let mut channel = physical_channel(ChannelResolution::U8, 0, 255, -90.0, 90.0);
        channel.functions[0].physical_mapping = Some(PhysicalMappingCalibration {
            quality,
            source: Some("Manufacturer manual rev 2, page 17".into()),
            revision: 7,
            ..Default::default()
        });
        let mapping = compile(&channel);
        assert_eq!(mapping.quality, quality);
        assert_eq!(
            mapping.source.as_deref(),
            Some("Manufacturer manual rev 2, page 17")
        );
        assert_eq!(mapping.calibration_revision, 7);
        assert_eq!(mapping.channel_id, channel.id);
        assert_eq!(mapping.function_id, channel.functions[0].id);
        for source in [None, Some(String::new()), Some(" \t\n".into())] {
            channel.functions[0]
                .physical_mapping
                .as_mut()
                .unwrap()
                .source = source;
            let result = CompiledPhysicalMapping::compile(&channel, &channel.functions[0]);
            assert_eq!(
                result.is_err(),
                matches!(
                    quality,
                    PhysicalDataQuality::Manufacturer | PhysicalDataQuality::Measured
                )
            );
        }
    }
}

#[test]
fn zoom_opening_convention_requires_an_explicit_degree_function() {
    for convention in [OpeningConvention::Beam, OpeningConvention::Field] {
        let mut channel = physical_channel(ChannelResolution::U8, 0, 255, 60.0, 10.0);
        channel.functions[0].attribute = AttributeKey("zoom".into());
        channel.functions[0].physical_mapping = Some(PhysicalMappingCalibration {
            opening_convention: Some(convention),
            ..Default::default()
        });
        let mapping = compile(&channel);
        assert_eq!(mapping.unit, PhysicalUnit::Degrees);
        assert_eq!(mapping.opening_convention, Some(convention));
        assert_eq!(mapping.physical_for_raw(0).physical, 60.0);
        assert_eq!(mapping.physical_for_raw(255).physical, 10.0);
        for unit in [Some("percent"), None, Some("custom-angle")] {
            let ChannelFunctionBehavior::Continuous { unit: current, .. } =
                &mut channel.functions[0].behavior
            else {
                unreachable!()
            };
            *current = unit.map(str::to_owned);
            assert_rejected(&channel);
        }
        let ChannelFunctionBehavior::Continuous { unit, .. } = &mut channel.functions[0].behavior
        else {
            unreachable!()
        };
        *unit = Some("deg".into());
        channel.functions[0].attribute = AttributeKey("focus".into());
        assert_rejected(&channel);
    }
}

#[test]
fn unit_parsing_keeps_unknown_data_distinct_from_percent_and_degrees() {
    for (input, expected) in [
        (Some(" DEGREES "), PhysicalUnit::Degrees),
        (Some("°/s"), PhysicalUnit::DegreesPerSecond),
        (Some("%"), PhysicalUnit::Percent),
        (Some("normalised"), PhysicalUnit::Normalized),
        (Some("meters"), PhysicalUnit::Metres),
        (None, PhysicalUnit::Unknown),
        (Some("  "), PhysicalUnit::Unknown),
        (
            Some("focal-power"),
            PhysicalUnit::Custom("focal-power".into()),
        ),
    ] {
        assert_eq!(PhysicalUnit::parse(input), expected);
    }
}

#[test]
fn absent_calibration_is_compatible_and_does_not_invent_measured_quality() {
    let mut channel = physical_channel(ChannelResolution::U8, 0, 255, -90.0, 90.0);
    let mut json = serde_json::to_value(&channel.functions[0]).unwrap();
    json.as_object_mut().unwrap().remove("physical_mapping");
    channel.functions[0] = serde_json::from_value(json).unwrap();
    assert!(channel.functions[0].physical_mapping.is_none());
    let mapping = compile(&channel);
    assert_eq!(mapping.quality, PhysicalDataQuality::Unknown);
    assert_eq!(mapping.source, None);
    assert_eq!(mapping.calibration_revision, 0);
    assert_eq!(mapping.opening_convention, None);
    assert_eq!(mapping.physical_for_raw(0).physical, -90.0);
    assert_eq!(mapping.physical_for_raw(255).physical, 90.0);
}

#[test]
fn fixed_functions_are_not_misrepresented_as_physical_curves() {
    let mut channel = physical_channel(ChannelResolution::U8, 0, 255, 0.0, 1.0);
    channel.functions[0].behavior = ChannelFunctionBehavior::Fixed {
        semantic_id: "open".into(),
        label: "Open".into(),
        raw_value: 128,
    };
    assert!(
        CompiledPhysicalMapping::compile(&channel, &channel.functions[0])
            .unwrap()
            .is_none()
    );
    channel.functions[0].physical_mapping = Some(PhysicalMappingCalibration::default());
    assert_rejected(&channel);
}

fn portable_profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Physical mapping calibration".into();
    let mode = &mut profile.modes[0];
    let mut channel = physical_channel(ChannelResolution::U8, 0, 255, 60.0, 10.0);
    channel.head_id = mode.heads[0].id;
    channel.attribute = AttributeKey("zoom".into());
    channel.fixture_attribute = channel.attribute.clone();
    channel.functions[0].attribute = channel.attribute.clone();
    samples(&mut channel, &[(0, 60.0), (100, 40.0), (255, 10.0)]);
    let calibration = channel.functions[0].physical_mapping.as_mut().unwrap();
    calibration.quality = PhysicalDataQuality::Measured;
    calibration.source = Some("Bench measurement 2026-09-28, full beam angle".into());
    calibration.revision = 3;
    calibration.opening_convention = Some(OpeningConvention::Beam);
    mode.splits[0].footprint = 1;
    mode.channels = vec![channel];
    profile
}

#[test]
fn metadata_and_curve_roundtrip_through_json_and_the_portable_fixture_codec() {
    let profile = portable_profile();
    profile.validate().unwrap();
    let json = serde_json::to_string(&profile).unwrap();
    let from_json: FixtureProfile = serde_json::from_str(&json).unwrap();
    let bytes = crate::package::write_fixture_package(&from_json).unwrap();
    let restored = crate::package::read_fixture_package(&bytes).unwrap();
    assert_eq!(restored.id, profile.id);
    assert_eq!(restored.modes[0].id, profile.modes[0].id);
    let before = &profile.modes[0].channels[0];
    let after = &restored.modes[0].channels[0];
    assert_eq!(after.id, before.id);
    assert_eq!(after.functions[0].id, before.functions[0].id);
    assert_eq!(
        after.functions[0].physical_mapping,
        before.functions[0].physical_mapping
    );
    let original = compile(before);
    let reloaded = compile(after);
    for raw in 0..=255 {
        assert_eq!(
            reloaded.physical_for_raw(raw),
            original.physical_for_raw(raw)
        );
    }
}

#[test]
fn profile_validation_and_package_export_refuse_invalid_mapping_data() {
    let mut profile = portable_profile();
    profile.modes[0].channels[0].functions[0]
        .physical_mapping
        .as_mut()
        .unwrap()
        .samples[1]
        .physical = 70.0;
    assert!(profile.validate().is_err());
    assert!(crate::package::write_fixture_package(&profile).is_err());
}

#[test]
fn declaring_a_physical_curve_does_not_change_the_existing_native_output_path() {
    let profile = portable_profile();
    let calibrated = &profile.modes[0];
    let mut plain = calibrated.clone();
    plain.channels[0].functions[0].physical_mapping = None;
    for requested in [0.0, 0.13, 0.5, 0.87, 1.0] {
        let values = HashMap::from([(
            AttributeKey("zoom".into()),
            AttributeValue::Normalized(requested),
        )]);
        let old = plain.resolve_channel_raw(
            &plain.channels[0],
            &values,
            false,
            None,
            ChannelScales::default(),
        );
        let new = calibrated.resolve_channel_raw(
            &calibrated.channels[0],
            &values,
            false,
            None,
            ChannelScales::default(),
        );
        assert_eq!(
            new, old,
            "metadata alone must not opt the engine into physical programming"
        );
    }
}

#[path = "optics_physical_forward.rs"]
mod forward;
