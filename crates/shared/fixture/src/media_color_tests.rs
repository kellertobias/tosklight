//! TL-593 translation references: semantic Color intent → Media tint/White Blend controls.
use super::*;
use crate::FixtureProfile;
use light_core::Xyz;
use light_core::color_intent::D65_WHITE;
use light_core::programming::{
    ColorComponent, ColorComponentSpread, UvIntent, VirtualColorAuthoringV1, VirtualColorRecipe,
    WhiteTarget,
};

fn intent(rgb: [f32; 3], amber: f32, white_blend: f32) -> ColorIntent {
    let recipe = VirtualColorRecipe {
        version: 1,
        rgb,
        amber,
        approximate: false,
    };
    ColorIntent {
        base_xyz: VirtualColorAuthoringV1::recipe_xyz(&recipe).unwrap(),
        recipe,
        white_blend,
        ..ColorIntent::default()
    }
}

fn layer(intent: &ColorIntent) -> (MediaColorControls, MediaColorLimitations) {
    MediaColorControls::from_intent(intent, MediaColorSurface::Layer).unwrap()
}

/// The IEC 61966-2-1 decode, written out so the reference does not reuse the code under test.
fn decoded(encoded: f32) -> f32 {
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

fn close(actual: [f32; 3], expected: [f32; 3]) {
    for (a, e) in actual.into_iter().zip(expected) {
        assert!((a - e).abs() < 2e-4, "{actual:?} != {expected:?}");
    }
}

pub(crate) fn shipped_media_server() -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library/tosklight--media-server.toskfixture");
    crate::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn white_blend_0_50_100_is_source_desaturation_with_the_tint_retained() {
    for (blend, raw) in [(0.0, 0), (0.5, 128), (1.0, 255)] {
        let (white, limits) = layer(&intent([1.0; 3], 0.0, blend));
        close(white.tint, [1.0; 3]);
        assert_eq!(white.white_blend, Some(blend));
        assert_eq!(limits, MediaColorLimitations::default());
        assert_eq!(
            white.encode(255),
            MediaColorRaw {
                subtractive: [0; 3],
                white_blend: Some(raw)
            }
        );

        // A red tint stays red at every blend: Media never whitens the base like a lamp does.
        let red = intent([1.0, 0.0, 0.0], 0.0, blend);
        let (controls, _) = layer(&red);
        close(controls.tint, [1.0, 0.0, 0.0]);
        assert_eq!(controls.encode(255).subtractive, [0, 255, 255]);
    }
    // The lamp envelope at 100% is the white target, which would erase the tint.
    let lamp = intent([1.0, 0.0, 0.0], 0.0, 1.0)
        .blend_visible(D65_WHITE)
        .unwrap();
    let whitened = xyz_to_linear_srgb(lamp);
    assert!(
        whitened.iter().all(|c| (c - 1.0).abs() < 1e-3),
        "{whitened:?}"
    );
}

#[test]
fn the_tint_is_linear_base_xyz_with_exactly_one_transfer_decode() {
    // Recipe levels are sRGB encoded. The single decode happened when the recipe became XYZ;
    // Media reads linear XYZ and must neither skip nor repeat it.
    for rgb in [[0.5, 0.5, 0.5], [1.0, 0.735, 0.0], [0.2, 0.9, 0.6]] {
        let (controls, limits) = layer(&intent(rgb, 0.0, 0.0));
        close(controls.tint, rgb.map(decoded));
        assert!(!limits.tint_gamut_mapped);
    }
    let (gray, _) = layer(&intent([0.5; 3], 0.0, 0.0));
    assert!((gray.tint[0] - 0.214).abs() < 1e-3, "not 0.5 and not 0.038");
}

#[test]
fn relative_output_dims_linearly_and_black_stays_black() {
    let mut dim = intent([1.0, 0.735, 0.0], 0.0, 0.5);
    dim.relative_output = 0.5;
    let (controls, _) = layer(&dim);
    close(controls.tint, [0.5, 0.5 * decoded(0.735), 0.0]);
    assert_eq!(controls.white_blend, Some(0.5), "White Blend is not scaled");

    for black in [intent([0.0; 3], 0.0, 0.5), {
        let mut off = intent([1.0; 3], 0.0, 1.0);
        off.relative_output = 0.0;
        off
    }] {
        let (controls, limits) = layer(&black);
        assert_eq!(controls.tint, [0.0; 3]);
        assert_eq!(controls.encode(255).subtractive, [255; 3]);
        assert!(!limits.tint_gamut_mapped);
    }
}

#[test]
fn out_of_range_bases_are_peak_normalized_and_reported() {
    // Full red plus full virtual amber exceeds the linear red primary.
    let (amber, limits) = layer(&intent([1.0, 0.0, 0.0], 1.0, 0.0));
    assert!(limits.tint_gamut_mapped);
    assert_eq!(amber.tint[0], 1.0);
    assert!(amber.tint[1] > 0.0 && amber.tint[1] < 0.2 && amber.tint[2] == 0.0);

    let mut boosted = intent([0.5; 3], 0.0, 0.0);
    boosted.relative_output = 10.0;
    let (boosted, limits) = layer(&boosted);
    assert!(limits.tint_gamut_mapped);
    close(boosted.tint, [1.0; 3]);

    // An Advanced coordinate outside sRGB clamps its negative primary.
    let mut green = intent([0.0, 1.0, 0.0], 0.0, 0.0);
    green.recipe.approximate = true;
    green.base_xyz = Xyz {
        x: 0.15,
        y: 0.6,
        z: 0.05,
    };
    let (green, limits) = layer(&green);
    assert!(limits.tint_gamut_mapped);
    assert!(green.tint.iter().all(|c| (0.0..=1.0).contains(c)));
}

#[test]
fn uv_and_white_target_never_alter_the_tint() {
    let plain = layer(&intent([1.0, 0.0, 1.0], 0.0, 0.25)).0;
    let mut other = intent([1.0, 0.0, 1.0], 0.0, 0.25);
    other.uv = UvIntent { amount: 0.9 };
    other.white_target = WhiteTarget {
        kelvin: 3200.0,
        duv: 0.004,
    };
    let (controls, limits) = layer(&other);
    assert_eq!(controls, plain);
    assert!(limits.uv_unsupported);
    assert!(!limits.white_blend_unsupported);
}

#[test]
fn the_master_has_no_white_blend_which_is_not_an_authored_zero() {
    for blend in [0.0, 0.7] {
        let request = intent([1.0, 0.5, 0.25], 0.0, blend);
        let (master, limits) =
            MediaColorControls::from_intent(&request, MediaColorSurface::Master).unwrap();
        assert_eq!(master.white_blend, None, "absent, never Some(0)");
        assert_eq!(master.encode(255).white_blend, None);
        assert_eq!(limits.white_blend_unsupported, blend > 0.0);
        assert_eq!(master.tint, layer(&request).0.tint);
    }
    let (authored_zero, _) = layer(&intent([1.0; 3], 0.0, 0.0));
    assert_eq!(authored_zero.white_blend, Some(0.0));
}

#[test]
fn decode_reads_back_the_quantized_wire_value() {
    for value in 0..=255u32 {
        let decoded = MediaColorControls::decode(
            MediaColorRaw {
                subtractive: [value; 3],
                white_blend: Some(value),
            },
            255,
        );
        assert_eq!(decoded.tint[0], 1.0 - value as f32 / 255.0);
        assert_eq!(decoded.white_blend, Some(value as f32 / 255.0));
        assert_eq!(
            decoded.encode(255).subtractive,
            [value; 3],
            "stable round trip"
        );
    }
}

#[test]
fn unresolved_or_invalid_intents_are_rejected() {
    let mut spread = intent([1.0; 3], 0.0, 0.0);
    spread.spreads = vec![ColorComponentSpread {
        component: ColorComponent::WhiteBlend,
        points: vec![0.0, 1.0],
    }];
    assert!(MediaColorControls::from_intent(&spread, MediaColorSurface::Layer).is_err());
    let mut invalid = intent([1.0; 3], 0.0, 0.0);
    invalid.white_blend = 1.5;
    assert!(MediaColorControls::from_intent(&invalid, MediaColorSurface::Layer).is_err());
}

#[test]
fn shipped_media_server_heads_are_recognised_and_lamps_are_not() {
    let profile = shipped_media_server();
    for mode in &profile.modes {
        let heads: Vec<_> = mode
            .heads
            .iter()
            .map(|head| (head, MediaColorHead::from_mode(mode, head.id)))
            .collect();
        assert!(
            heads.iter().all(|(_, media)| media.is_some()),
            "{}",
            mode.name
        );
        for (head, media) in heads {
            let media = media.unwrap();
            let names: Vec<_> = media
                .controls()
                .iter()
                .map(|c| {
                    mode.channels[c.channel_index as usize]
                        .fixture_attribute
                        .0
                        .clone()
                })
                .collect();
            if head.master_shared {
                assert_eq!(media.surface, MediaColorSurface::Master);
                assert_eq!(names, MASTER_ATTRIBUTES.map(Into::into));
            } else {
                assert_eq!(media.surface, MediaColorSurface::Layer);
                assert_eq!(names, LAYER_ATTRIBUTES.map(Into::into));
            }
            assert_eq!(media.raw_max(), 255);
        }
    }
    for lamp in [
        "generic--cmy-led.toskfixture",
        "generic--rgbw-led.toskfixture",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/fixture-library")
            .join(lamp);
        let lamp = crate::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
        let mode = &lamp.modes[0];
        assert!(
            mode.heads
                .iter()
                .all(|head| MediaColorHead::from_mode(mode, head.id).is_none())
        );
    }
}

#[test]
fn head_resolution_writes_every_control_and_publishes_the_decoded_result() {
    let profile = shipped_media_server();
    let mode = &profile.modes[0];
    let head = mode.heads.iter().find(|h| !h.master_shared).unwrap();
    let media = MediaColorHead::from_mode(mode, head.id).unwrap();
    let request = intent([1.0, 0.735, 0.0], 0.0, 0.5);
    let resolved = media.resolve(&request).unwrap();
    assert_eq!(resolved.raws.len(), media.controls().len());
    assert_eq!(resolved.raws[3], 128);
    assert_eq!(resolved.raws[..3], [0, 128, 255]);
    close(resolved.achieved.tint, [1.0, 1.0 - 128.0 / 255.0, 0.0]);
    assert_eq!(resolved.achieved.white_blend, Some(128.0 / 255.0));
    assert_eq!(resolved.requested, layer(&request).0);

    let master = mode.heads.iter().find(|h| h.master_shared).unwrap();
    let master = MediaColorHead::from_mode(mode, master.id).unwrap();
    let resolved = master.resolve(&request).unwrap();
    assert_eq!(resolved.raws, [0, 128, 255]);
    assert_eq!(resolved.achieved.white_blend, None);
    assert!(resolved.limitations.white_blend_unsupported);
}

#[test]
fn unsupported_media_control_contracts_are_declined_for_layers_and_masters() {
    use crate::{ChannelBehavior, ChannelFunctionBehavior, ChannelResolution};
    let profile = shipped_media_server();
    for source in &profile.modes {
        for head in &source.heads {
            let supported = MediaColorHead::from_mode(source, head.id).unwrap();
            let index = supported.controls()[0].channel_index as usize;
            for variation in 0..14 {
                let mut mode = source.clone();
                let channel = &mut mode.channels[index];
                match variation {
                    // Upgrade every color control so the shared-scale check still passes.
                    0..=2 => {
                        let resolution = [
                            ChannelResolution::U16,
                            ChannelResolution::U24,
                            ChannelResolution::U32,
                        ][variation];
                        for control in supported.controls() {
                            let channel = &mut mode.channels[control.channel_index as usize];
                            channel.resolution = resolution;
                            channel.functions[0].dmx_to = resolution.max_raw();
                        }
                    }
                    3 => {
                        channel.functions[0].behavior = ChannelFunctionBehavior::Fixed {
                            semantic_id: "service".into(),
                            label: "Service".into(),
                            raw_value: 200,
                        }
                    }
                    4 => {
                        channel.functions[0].behavior = ChannelFunctionBehavior::Control {
                            action_id: Uuid::new_v4(),
                        }
                    }
                    5 => channel.functions[0].dmx_from = 1,
                    6 => channel.functions[0].dmx_to = 254,
                    7 => channel.functions.push(channel.functions[0].clone()),
                    8 => channel.functions.clear(),
                    9 => channel.invert = true,
                    10 => channel.behavior = ChannelBehavior::Static,
                    11 => {
                        channel.functions[0].attribute =
                            light_core::AttributeKey("fixture.reset".into())
                    }
                    12 => {
                        channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
                            physical_min: 0.0,
                            physical_max: 1.0,
                            unit: None,
                        }
                    }
                    13 => channel.reacts_to_virtual_intensity = true,
                    _ => unreachable!(),
                }
                assert!(
                    MediaColorHead::from_mode(&mode, head.id).is_none(),
                    "{} head {} unsupported variation {variation}",
                    mode.name,
                    head.id
                );
            }
            let mut duplicate = source.clone();
            duplicate.channels.push(duplicate.channels[index].clone());
            assert!(MediaColorHead::from_mode(&duplicate, head.id).is_none());
            let mut missing = source.clone();
            missing.channels.remove(index);
            assert!(MediaColorHead::from_mode(&missing, head.id).is_none());
            // Required canonical CMY inversion remains supported on the shipped personality.
            assert!(MediaColorHead::from_mode(source, head.id).is_some());
        }
    }
}
