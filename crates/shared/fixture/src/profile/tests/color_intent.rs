use super::*;
use light_core::ColorResolutionQuality as Quality;
use light_core::color_intent::{D65_WHITE, delta_uv};

fn srgb(red: f32, green: f32, blue: f32) -> Xyz {
    crate::srgb_to_xyz(red, green, blue)
}

fn scaled(value: Xyz, factor: f32) -> Xyz {
    Xyz {
        x: value.x * factor,
        y: value.y * factor,
        z: value.z * factor,
    }
}

fn blank_mode() -> (FixtureMode, Uuid) {
    let mut profile = FixtureProfile::blank();
    let mode = profile.modes.remove(0);
    let head_id = mode.heads[0].id;
    (mode, head_id)
}

fn attribute_channel(
    head_id: Uuid,
    attribute: &str,
    transform: CanonicalTransform,
) -> FixtureChannel {
    let mut channel = channel(head_id, ChannelResolution::U8, vec![]);
    channel.attribute = AttributeKey(attribute.into());
    channel.fixture_attribute = AttributeKey(attribute.into());
    channel.canonical_transform = transform;
    channel
}

fn calibration(status: ColorCalibrationStatus, revision: u32) -> ColorSystemCalibration {
    ColorSystemCalibration {
        status,
        revision,
        source: Some("test".into()),
    }
}

/// An RGB(W) head whose emitters are the sRGB primaries (and D65 white).
fn additive_mode(white: bool, status: ColorCalibrationStatus) -> (FixtureMode, Uuid, Vec<Uuid>) {
    let (mut mode, head_id) = blank_mode();
    let mut names = vec![
        ("color.red", srgb(1.0, 0.0, 0.0)),
        ("color.green", srgb(0.0, 1.0, 0.0)),
        ("color.blue", srgb(0.0, 0.0, 1.0)),
    ];
    if white {
        names.push(("color.white", D65_WHITE));
    }
    let channels = names
        .iter()
        .map(|(attribute, _)| attribute_channel(head_id, attribute, CanonicalTransform::Identity))
        .collect::<Vec<_>>();
    let ids = channels
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    mode.color_systems = vec![HeadColorSystem {
        head_id,
        correction_matrix: identity_color_correction(),
        system: ColorSystem::Additive {
            emitters: names
                .iter()
                .zip(&ids)
                .map(|((attribute, xyz), id)| EmitterBinding {
                    channel_id: *id,
                    name: attribute.trim_start_matches("color.").into(),
                    xyz: *xyz,
                    maximum_level: 1.0,
                    response_curve: 1.0,
                    visible: true,
                })
                .collect(),
        },
        calibration: calibration(status, 3),
    }];
    mode.channels = channels;
    (mode, head_id, ids)
}

fn wheel_slot(
    semantic: &str,
    label: &str,
    from: u32,
    to: u32,
    measured: Option<Xyz>,
) -> ColorWheelSlot {
    ColorWheelSlot {
        semantic_id: semantic.into(),
        label: label.into(),
        dmx_from: from,
        dmx_to: to,
        measured_xyz: measured,
        steady: None,
    }
}

fn wheel_system(
    head_id: Uuid,
    channel_id: Uuid,
    status: ColorCalibrationStatus,
) -> HeadColorSystem {
    HeadColorSystem {
        head_id,
        correction_matrix: identity_color_correction(),
        system: ColorSystem::DiscreteWheel {
            channel_id,
            slots: vec![
                wheel_slot("open", "Open", 0, 9, Some(D65_WHITE)),
                wheel_slot("red", "Red", 10, 29, Some(srgb(1.0, 0.0, 0.0))),
                wheel_slot("congo", "Congo Blue", 30, 49, Some(srgb(0.2, 0.0, 0.8))),
                wheel_slot("green", "Green", 50, 69, Some(srgb(0.0, 1.0, 0.0))),
                // Closer to orange than anything steady, but moving.
                wheel_slot(
                    "rainbow",
                    "Rainbow effect",
                    200,
                    255,
                    Some(srgb(1.0, 0.5, 0.0)),
                ),
            ],
        },
        calibration: calibration(status, 1),
    }
}

#[test]
fn measured_rgb_reproduces_a_target_exactly_at_full_brightness_whatever_its_luminance() {
    let (mode, head_id, ids) = additive_mode(false, ColorCalibrationStatus::Measured);
    let bright = mode.resolve_intent(head_id, srgb(1.0, 0.0, 0.0));
    let dim = mode.resolve_intent(head_id, scaled(srgb(1.0, 0.0, 0.0), 0.05));
    assert_eq!(
        bright, dim,
        "the target's luminance must not act as a dimmer"
    );
    assert_eq!(bright.quality, Quality::Exact);
    assert_eq!(bright.engine, Some(ColorIntentEngine::Additive));
    assert_eq!(bright.calibration_revision, Some(3));
    assert_eq!(bright.channels[&ids[0]], 255);
    assert_eq!(bright.channels[&ids[1]], 0);
    assert_eq!(bright.channels[&ids[2]], 0);

    let orange = mode.resolve_intent(head_id, srgb(1.0, 0.5, 0.0));
    assert_eq!(orange.quality, Quality::Exact);
    assert_eq!(
        orange.channels[&ids[0]], 255,
        "the brightest primary reaches full"
    );
    // Drive is linear light: sRGB 0.5 is 21% emitted green.
    assert!((50..60).contains(&orange.channels[&ids[1]]), "{orange:?}");
    assert_eq!(orange.channels[&ids[2]], 0);
}

#[test]
fn nominal_rgbw_uses_white_and_colour_emitters_for_white_and_reports_approximate() {
    let (mode, head_id, ids) = additive_mode(true, ColorCalibrationStatus::Nominal);
    let white = mode.resolve_intent(head_id, D65_WHITE);
    assert_eq!(white.quality, Quality::Approximate);
    assert!(white.delta_uv.unwrap() < 1e-3);
    assert_eq!(
        white.channels[&ids[3]], 255,
        "white emitter is used at full: {white:?}"
    );
    assert!(
        ids.iter().all(|id| white.channels[id] > 0),
        "load spreads over every emitter"
    );
}

#[test]
fn a_colour_outside_the_fixture_gamut_maps_to_the_nearest_and_says_so() {
    let (mode, head_id, _) = additive_mode(false, ColorCalibrationStatus::Measured);
    // A saturated spectral cyan, outside the sRGB triangle.
    let (x, y) = (0.03_f32, 0.45_f32);
    let target = Xyz {
        x: x / y,
        y: 1.0,
        z: (1.0 - x - y) / y,
    };
    let first = mode.resolve_intent(head_id, target);
    assert_eq!(first.quality, Quality::OutOfGamut);
    assert!(first.delta_uv.unwrap() > light_core::color_intent::APPROXIMATE_DELTA_UV);
    assert_eq!(
        first,
        mode.resolve_intent(head_id, target),
        "gamut mapping is deterministic"
    );
    assert!(first.channels.values().any(|raw| *raw == 255));
}

#[test]
fn unauthored_rgb_and_cmy_heads_resolve_through_inferred_uncalibrated_systems() {
    let (mut mode, head_id) = blank_mode();
    let red = attribute_channel(head_id, "color.red", CanonicalTransform::Identity);
    let green = attribute_channel(head_id, "color.green", CanonicalTransform::Identity);
    let blue = attribute_channel(head_id, "color.blue", CanonicalTransform::Identity);
    let ids = [red.id, green.id, blue.id];
    mode.channels = vec![red, green, blue];
    let result = mode.resolve_intent(head_id, srgb(0.0, 0.0, 1.0));
    assert_eq!(result.quality, Quality::Uncalibrated);
    assert_eq!(result.calibration_revision, None);
    assert_eq!(
        ids.map(|id| result.channels[&id]),
        [0, 0, 255],
        "{result:?}"
    );

    // Fixture-facing CMY flags are stored as inverted canonical RGB.
    let (mut mode, head_id) = blank_mode();
    let cyan = attribute_channel(head_id, "color.red", CanonicalTransform::InvertNormalized);
    let magenta = attribute_channel(head_id, "color.green", CanonicalTransform::InvertNormalized);
    let yellow = attribute_channel(head_id, "color.blue", CanonicalTransform::InvertNormalized);
    let ids = [cyan.id, magenta.id, yellow.id];
    mode.channels = vec![cyan, magenta, yellow];
    let result = mode.resolve_intent(head_id, srgb(1.0, 0.0, 0.0));
    assert_eq!(result.engine, Some(ColorIntentEngine::Subtractive));
    assert_eq!(result.quality, Quality::Uncalibrated);
    assert_eq!(
        ids.map(|id| result.channels[&id]),
        [0, 255, 255],
        "{result:?}"
    );
}

#[test]
fn measured_cmy_filters_with_cross_talk_still_land_on_the_target() {
    let (mut mode, head_id) = blank_mode();
    let channels = ["color.red", "color.green", "color.blue"].map(|attribute| {
        attribute_channel(head_id, attribute, CanonicalTransform::InvertNormalized)
    });
    let ids = channels.each_ref().map(|channel| channel.id);
    mode.channels = channels.to_vec();
    // Real flags leak: full cyan still passes 8% red and eats 10% of green.
    let open = D65_WHITE;
    let filter = |red: f32, green: f32, blue: f32| {
        let linear = [red, green, blue];
        Xyz {
            x: 0.4124 * linear[0] + 0.3576 * linear[1] + 0.1805 * linear[2],
            y: 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2],
            z: 0.0193 * linear[0] + 0.1192 * linear[1] + 0.9505 * linear[2],
        }
    };
    mode.color_systems = vec![HeadColorSystem {
        head_id,
        correction_matrix: identity_color_correction(),
        system: ColorSystem::Subtractive {
            cyan_channel_id: ids[0],
            magenta_channel_id: ids[1],
            yellow_channel_id: ids[2],
            filters: Some(SubtractiveCalibration {
                open_xyz: open,
                cyan_xyz: filter(0.08, 0.9, 1.0),
                magenta_xyz: filter(1.0, 0.06, 0.92),
                yellow_xyz: filter(0.95, 1.0, 0.1),
            }),
        },
        calibration: calibration(ColorCalibrationStatus::Measured, 2),
    }];
    let target = srgb(0.5, 0.8, 1.0);
    let result = mode.resolve_intent(head_id, target);
    assert_eq!(result.engine, Some(ColorIntentEngine::Subtractive));
    assert!(
        matches!(result.quality, Quality::Exact | Quality::Approximate),
        "{result:?}"
    );
    assert!(
        delta_uv(result.achieved.unwrap(), target) < 0.01,
        "{result:?}"
    );
    assert_eq!(result.calibration_revision, Some(2));
}

#[test]
fn a_wheel_only_head_picks_the_nearest_steady_slot_and_reports_wheel_limited() {
    let (mut mode, head_id) = blank_mode();
    let wheel = attribute_channel(head_id, "color.wheel.1", CanonicalTransform::Identity);
    let wheel_id = wheel.id;
    mode.channels = vec![wheel];
    mode.color_systems = vec![wheel_system(
        head_id,
        wheel_id,
        ColorCalibrationStatus::Measured,
    )];

    let red = mode.resolve_intent(head_id, srgb(1.0, 0.0, 0.0));
    assert_eq!(red.quality, Quality::Exact);
    assert_eq!(red.channels[&wheel_id], 19);

    // Orange sits nearest the rainbow effect, which is never used for a steady colour.
    let orange = mode.resolve_intent(head_id, srgb(1.0, 0.5, 0.0));
    assert_eq!(orange.engine, Some(ColorIntentEngine::Wheel));
    assert_eq!(orange.quality, Quality::WheelLimited);
    assert_ne!(orange.channels[&wheel_id], 227);
    assert_eq!(orange.channels[&wheel_id], 19, "{orange:?}");
}

#[test]
fn wheel_slots_named_as_splits_scrolls_or_effects_are_not_steady_unless_declared() {
    let mut slot = wheel_slot("split", "Red/Blue split", 0, 10, None);
    assert!(!slot.is_steady());
    slot.steady = Some(true);
    assert!(slot.is_steady());
    for label in [
        "Rotation",
        "Rainbow effect (with fade time)",
        "Red->up, Green=full",
        "Scroll",
    ] {
        assert!(!wheel_slot("x", label, 0, 1, None).is_steady(), "{label}");
    }
    assert!(wheel_slot("red", "Deep Red", 0, 1, None).is_steady());
}

#[test]
fn a_hybrid_head_prefers_its_continuous_engine_and_parks_the_wheel_open() {
    let (mut mode, head_id, ids) = additive_mode(false, ColorCalibrationStatus::Measured);
    let wheel = attribute_channel(head_id, "color.wheel.1", CanonicalTransform::Identity);
    let wheel_id = wheel.id;
    mode.channels.push(wheel);
    mode.color_systems.push(wheel_system(
        head_id,
        wheel_id,
        ColorCalibrationStatus::Measured,
    ));
    let green = mode.resolve_intent(head_id, srgb(0.0, 1.0, 0.0));
    assert_eq!(green.engine, Some(ColorIntentEngine::Additive));
    assert_eq!(
        green.channels[&wheel_id], 4,
        "wheel parked in its open slot"
    );
    assert_eq!(green.channels[&ids[1]], 255);
}

#[test]
fn a_hybrid_head_falls_back_to_a_closer_wheel_slot_only_outside_the_continuous_gamut() {
    // A CMY engine whose yellow flag barely works cannot make a saturated green; the wheel can.
    let (mut mode, head_id) = blank_mode();
    let flags = ["color.red", "color.green", "color.blue"].map(|attribute| {
        attribute_channel(head_id, attribute, CanonicalTransform::InvertNormalized)
    });
    let ids = flags.each_ref().map(|channel| channel.id);
    let wheel = attribute_channel(head_id, "color.wheel.1", CanonicalTransform::Identity);
    let wheel_id = wheel.id;
    mode.channels = flags.to_vec();
    mode.channels.push(wheel);
    let weak = |red: f32, green: f32, blue: f32| crate::srgb_to_xyz(red, green, blue);
    mode.color_systems = vec![
        HeadColorSystem {
            head_id,
            correction_matrix: identity_color_correction(),
            system: ColorSystem::Subtractive {
                cyan_channel_id: ids[0],
                magenta_channel_id: ids[1],
                yellow_channel_id: ids[2],
                filters: Some(SubtractiveCalibration {
                    open_xyz: D65_WHITE,
                    cyan_xyz: weak(0.7, 1.0, 1.0),
                    magenta_xyz: weak(1.0, 0.7, 1.0),
                    yellow_xyz: weak(1.0, 1.0, 0.7),
                }),
            },
            calibration: calibration(ColorCalibrationStatus::Measured, 1),
        },
        wheel_system(head_id, wheel_id, ColorCalibrationStatus::Measured),
    ];
    let green = mode.resolve_intent(head_id, srgb(0.0, 1.0, 0.0));
    assert_eq!(green.engine, Some(ColorIntentEngine::Wheel), "{green:?}");
    assert_eq!(green.channels[&wheel_id], 59);
    assert_eq!(
        ids.map(|id| green.channels[&id]),
        [0, 0, 0],
        "flags parked open"
    );

    let pale = mode.resolve_intent(head_id, srgb(1.0, 0.95, 0.9));
    assert_eq!(
        pale.engine,
        Some(ColorIntentEngine::Subtractive),
        "{pale:?}"
    );
    assert_eq!(pale.channels[&wheel_id], 4);
}

#[test]
fn hue_saturation_engines_keep_their_own_intensity_full() {
    let (mut mode, head_id) = blank_mode();
    let hue = attribute_channel(head_id, "color.hue", CanonicalTransform::Identity);
    let saturation = attribute_channel(head_id, "color.saturation", CanonicalTransform::Identity);
    let brightness = attribute_channel(head_id, "color.brightness", CanonicalTransform::Identity);
    let ids = [hue.id, saturation.id, brightness.id];
    mode.channels = vec![hue, saturation, brightness];
    mode.color_systems = vec![HeadColorSystem {
        head_id,
        correction_matrix: identity_color_correction(),
        system: ColorSystem::HueSaturation {
            hue_channel_id: ids[0],
            saturation_channel_id: ids[1],
            intensity_channel_id: Some(ids[2]),
        },
        calibration: Default::default(),
    }];
    let result = mode.resolve_intent(head_id, scaled(srgb(0.0, 0.0, 1.0), 0.1));
    assert_eq!(result.engine, Some(ColorIntentEngine::HueSaturation));
    assert_eq!(result.quality, Quality::Approximate);
    assert_eq!(result.channels[&ids[2]], 255);
    assert_eq!(result.channels[&ids[1]], 255);
    assert_eq!(result.channels[&ids[0]], 170);
}

#[test]
fn a_head_without_any_colour_engine_is_unsupported_and_untouched() {
    let (mut mode, head_id) = blank_mode();
    mode.channels = vec![channel(head_id, ChannelResolution::U8, vec![])];
    let result = mode.resolve_intent(head_id, srgb(1.0, 0.0, 0.0));
    assert_eq!(result.quality, Quality::Unsupported);
    assert!(result.channels.is_empty());
}

#[test]
fn legacy_profile_json_without_calibration_reads_as_nominal_revision_zero() {
    let json = serde_json::json!({
        "head_id": Uuid::nil(),
        "system": {
            "type": "subtractive",
            "cyan_channel_id": Uuid::nil(),
            "magenta_channel_id": Uuid::nil(),
            "yellow_channel_id": Uuid::nil()
        }
    });
    let system: HeadColorSystem = serde_json::from_value(json).unwrap();
    assert_eq!(system.calibration, ColorSystemCalibration::default());
    assert_eq!(system.calibration.status, ColorCalibrationStatus::Nominal);
    let ColorSystem::Subtractive { filters, .. } = &system.system else {
        unreachable!()
    };
    assert!(filters.is_none());
    let written = serde_json::to_value(&system).unwrap();
    assert!(
        written["system"].get("filters").is_none(),
        "absent calibration stays absent"
    );
}
