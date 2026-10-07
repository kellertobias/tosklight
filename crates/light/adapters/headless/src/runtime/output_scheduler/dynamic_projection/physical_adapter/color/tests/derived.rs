//! TL-552 colour fallback: shipped Generic profiles without an authored physical Color model are
//! driven through the derived nominal/uncalibrated model (runtime profile projection), resolved by
//! the unchanged adapter against a real captured frame, encoded to DMX and re-simulated, and
//! reported with honest provenance (Uncalibrated, Wheel-limited, Exact only when measured).
use super::super::super::super::output_transaction::lamp_quality;
use super::super::tests::{Rig, intent};
use super::super::*;
use light_core::{AttributeKey, ColorResolutionQuality};
use light_fixture::{
    ChannelFunction, ChannelFunctionBehavior, ColorCalibrationStatus, ColorSystem,
    ColorSystemCalibration, ColorWheelSlot, FixtureProfile, HeadColorSystem,
    apply_runtime_profile_compatibility, srgb_to_xyz,
};

fn package(name: &str) -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

/// One mode of a shipped package, as the runtime compiles it.
fn runtime(name: &str, mode: &str) -> FixtureProfile {
    let mut profile = package(name);
    profile.modes.retain(|m| m.name == mode);
    apply_runtime_profile_compatibility(&mut profile);
    profile.validate().unwrap();
    profile
}

fn report(rig: &Rig, rgb: [f32; 3]) -> (Vec<u32>, ColorResolutionQuality) {
    let resolved = rig.resolve(&intent(rgb, 0.));
    (resolved.raws(), lamp_quality(&resolved.result.quality))
}

#[test]
fn a_runtime_profile_without_the_derived_model_its_colour_source_names_is_refused() {
    // The authored package has no physical Color model; its native Color source is the derived
    // one (G7a). A runtime profile that lost that model is refused, not silently colourless.
    let mut profile = package("generic--rgb-led");
    profile
        .modes
        .retain(|m| m.name == "DRGB 8-bit dimmer first");
    assert!(profile.modes[0].color_physical.is_none());
    let fixture = profiles::patched(&profile, FixtureId::new(), 1);
    let error = compile_fitting(&fixture, None).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("differs from its authoritative source"),
        "{error}"
    );
}

#[test]
fn generic_rgb_rgbw_and_cmy_leds_output_exact_channels_and_report_uncalibrated() {
    let rgb = Rig::new(&runtime("generic--rgb-led", "DRGB 8-bit dimmer first"));
    assert_eq!(
        report(&rgb, [1., 0., 0.]),
        (vec![255, 0, 0], ColorResolutionQuality::Uncalibrated)
    );
    assert_eq!(report(&rgb, [0., 1., 0.]).0, [0, 255, 0]);

    let rgbw = Rig::new(&runtime("generic--rgbw-led", "DRGBW 8-bit dimmer first"));
    assert_eq!(
        report(&rgbw, [0., 0., 1.]),
        (vec![0, 0, 255, 0], ColorResolutionQuality::Uncalibrated)
    );

    // CMY flags: cyan, magenta, yellow.
    let cmy = Rig::new(&runtime("generic--cmy-led", "DCMY 8-bit dimmer first"));
    assert_eq!(
        report(&cmy, [1., 0., 0.]),
        (vec![0, 255, 255], ColorResolutionQuality::Uncalibrated)
    );
    assert_eq!(report(&cmy, [0., 0., 1.]).0, [255, 255, 0]);
}

#[test]
fn rgbwa_uv_keeps_uv_separate_and_never_claims_a_known_visible_total_with_uv_on() {
    let rig = Rig::new(&runtime(
        "generic--rgbwauv-led",
        "DRGBWAU 8-bit dimmer first",
    ));
    let red = rig.resolve(&intent([1., 0., 0.], 0.));
    assert_eq!(red.raw(6), 0, "UV parked off");
    let mut with_uv = intent([1., 0., 0.], 0.);
    with_uv.uv.amount = 1.;
    let uv = rig.resolve(&with_uv);
    assert_eq!(uv.result.quality.uv, UvFitStatus::Applied);
    assert_eq!(uv.raw(6), 255);
    assert_eq!(
        lamp_quality(&uv.result.quality),
        ColorResolutionQuality::Uncalibrated
    );
}

/// Generic Dimmer plus a measured colour wheel (open, red, blue) and a non-steady scroll.
fn measured_wheel() -> FixtureProfile {
    let mut profile = package("generic--dimmer");
    profile.modes.retain(|m| m.name == "8-bit");
    let mode = &mut profile.modes[0];
    let head = mode.heads[0].id;
    let mut wheel = mode.channels[0].clone();
    wheel.id = Uuid::new_v4();
    wheel.fixture_attribute = AttributeKey("color.wheel.1".into());
    wheel.attribute = wheel.fixture_attribute.clone();
    let names = [
        ("open", [1., 1., 1.]),
        ("red", [1., 0., 0.]),
        ("blue", [0., 0., 1.]),
    ];
    wheel.functions = names
        .iter()
        .enumerate()
        .map(|(i, (name, _))| ChannelFunction {
            id: Uuid::new_v4(),
            name: (*name).into(),
            dmx_from: i as u32 * 16,
            dmx_to: i as u32 * 16 + 15,
            attribute: wheel.attribute.clone(),
            priority: 0,
            physical_mapping: None,
            angular_motion: None,
            behavior: ChannelFunctionBehavior::Fixed {
                semantic_id: (*name).into(),
                label: (*name).into(),
                raw_value: i as u32 * 16,
            },
        })
        .collect();
    let mut scroll = ChannelFunction::continuous("Rainbow scroll", wheel.attribute.clone(), 255);
    scroll.dmx_from = 48;
    wheel.functions.push(scroll);
    mode.color_systems = vec![HeadColorSystem {
        head_id: head,
        correction_matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        calibration: ColorSystemCalibration {
            status: ColorCalibrationStatus::Measured,
            revision: 1,
            source: Some("TL-552 colorimeter".into()),
        },
        system: ColorSystem::DiscreteWheel {
            channel_id: wheel.id,
            slots: names
                .iter()
                .enumerate()
                .map(|(i, (name, rgb))| ColorWheelSlot {
                    semantic_id: (*name).into(),
                    label: (*name).into(),
                    dmx_from: i as u32 * 16,
                    dmx_to: i as u32 * 16 + 15,
                    measured_xyz: Some(srgb_to_xyz(rgb[0], rgb[1], rgb[2])),
                    steady: None,
                })
                .collect(),
        },
    }];
    mode.channels.push(wheel);
    mode.splits[0].footprint = 2;
    apply_runtime_profile_compatibility(&mut profile);
    profile.validate().unwrap();
    profile
}

#[test]
fn a_measured_wheel_is_exact_on_a_slot_and_wheel_limited_between_slots() {
    let rig = Rig::new(&measured_wheel());
    assert_eq!(
        report(&rig, [1., 0., 0.]),
        (vec![23], ColorResolutionQuality::Exact)
    );
    let (raws, quality) = report(&rig, [1., 0.5, 0.]);
    assert_eq!(quality, ColorResolutionQuality::WheelLimited);
    assert!(raws[0] < 48, "a steady slot, never the scroll: {raws:?}");
}
