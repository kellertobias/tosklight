use super::*;
use crate::{
    CanonicalTransform, ChannelResolution, EmitterLayout, FIXTURE_PROFILE_SCHEMA_VERSION,
    FixtureProfile, FixtureSplit, ModelUnits, PatchPolicy,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const PNG_1X1: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

fn profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Portable fixture".into();
    profile.short_name = "Portable".into();
    profile
}

fn shipped_profile(filename: &str) -> FixtureProfile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("assets/fixture-library")
        .join(filename);
    read_fixture_package(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn requested_generic_and_venue_packages_have_exact_portable_contracts() {
    let blinder = shipped_profile("generic--blinder.toskfixture");
    assert_eq!(
        blinder
            .modes
            .iter()
            .map(|mode| mode.name.as_str())
            .collect::<Vec<_>>(),
        [
            "One channel, two blind",
            "Two channel, two blind",
            "One channel, four blind",
            "Two channel, four blind",
            "One channel, eight blind",
            "Two channel, eight blind",
            "Four channel, eight blind",
        ]
    );
    for mode in &blinder.modes {
        assert!(mode.heads.iter().all(|head| !head.master_shared));
        assert_eq!(mode.heads.len(), mode.channels.len());
        assert_eq!(mode.splits[0].footprint as usize, mode.heads.len());
        assert!(mode.channels.iter().all(|channel| {
            channel.attribute.is_intensity()
                && channel.resolution == ChannelResolution::U8
                && channel.highlight_raw == 255
        }));
        assert!(mode.geometry.emitters.iter().all(|emitter| {
            !emitter.directional
                && emitter.orientation_degrees.x == 90.0
                && emitter.origin.z == -126.0
                && matches!(emitter.layout, EmitterLayout::ExplicitPixels { .. })
        }));
    }
    let demo_blinder = blinder
        .modes
        .iter()
        .find(|mode| mode.name == "Two channel, four blind")
        .unwrap();
    let lens_centres = demo_blinder
        .geometry
        .emitters
        .iter()
        .flat_map(|emitter| match &emitter.layout {
            EmitterLayout::ExplicitPixels { positions } => positions
                .iter()
                .map(|position| (position.x, -position.z))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        lens_centres,
        [
            (-115.0, 290.0),
            (115.0, 290.0),
            (-115.0, 70.0),
            (115.0, 70.0)
        ]
    );

    let fogger = shipped_profile("generic--fogger.toskfixture");
    assert_eq!(
        fogger
            .modes
            .iter()
            .map(|mode| mode.name.as_str())
            .collect::<Vec<_>>(),
        ["Fan, Fog", "Fog, Fan", "Fog 8-bit"]
    );
    let hazer = shipped_profile("generic--hazer.toskfixture");
    assert_eq!(
        hazer
            .modes
            .iter()
            .map(|mode| mode.name.as_str())
            .collect::<Vec<_>>(),
        ["Fan, Fog", "Fog, Fan"]
    );

    let venue = [
        ("venue--stage-element-1-1-m.toskfixture", 10),
        ("venue--stage-element-2-1-m.toskfixture", 10),
        ("venue--stage-element-1-0-5-m.toskfixture", 10),
        ("venue--stage-stairs.toskfixture", 10),
        ("venue--four-point-truss.toskfixture", 5),
        ("venue--three-point-truss.toskfixture", 5),
        ("venue--two-point-truss.toskfixture", 5),
        ("venue--one-point-truss-pipe.toskfixture", 6),
        ("venue--curtain-1-m.toskfixture", 10),
        ("venue--curtain-2-m.toskfixture", 10),
        ("venue--curtain-3-m.toskfixture", 10),
        ("venue--curtain-5-m.toskfixture", 10),
        ("venue--curtain-6-m.toskfixture", 10),
    ];
    for (filename, mode_count) in venue {
        let profile = shipped_profile(filename);
        assert_eq!(profile.manufacturer, "Venue");
        assert_eq!(profile.patch_policy, PatchPolicy::VisualOnly);
        assert_eq!(profile.model_units, ModelUnits::Metres);
        assert_eq!(profile.modes.len(), mode_count);
        assert!(
            profile
                .photograph_asset
                .as_deref()
                .is_some_and(|asset| asset.starts_with("data:image/png;base64,"))
        );
        assert!(
            profile
                .stage_icon_asset
                .as_deref()
                .is_some_and(|asset| asset.starts_with("data:image/png;base64,"))
        );
        assert!(
            profile
                .model_asset
                .as_deref()
                .is_some_and(|asset| asset.starts_with("data:model/gltf-binary;base64,"))
        );
        assert!(profile.modes.iter().all(|mode| mode.splits
            == [FixtureSplit {
                number: 1,
                footprint: 0
            }]
            && mode.channels.is_empty()));
    }
}

#[test]
fn stage_lamp_packages_bind_models_motion_and_emitters() {
    let acl = shipped_profile("generic--acl.toskfixture");
    assert_eq!(acl.manufacturer, "Generic");
    assert_eq!(acl.name, "ACL");
    assert_eq!(acl.model_units, ModelUnits::Metres);
    assert_eq!(acl.physical.width_millimetres, Some(80.0));
    assert_eq!(acl.physical.height_millimetres, Some(80.0));
    assert_eq!(acl.physical.depth_millimetres, Some(200.0));
    assert!(
        acl.model_asset
            .as_deref()
            .is_some_and(|asset| { asset.starts_with("data:model/gltf-binary;base64,") })
    );
    assert!(acl.modes.iter().all(|mode| {
        mode.geometry.nodes.len() == 1
            && mode.geometry.nodes[0].glb_node.as_deref() == Some("acl-body")
            && mode.geometry.emitters.len() == 1
            && mode.geometry.emitters[0].origin.y == -192.0
    }));

    let fresnel = shipped_profile("generic--dimmer-fresnel.toskfixture");
    assert_eq!(fresnel.model_units, ModelUnits::Metres);
    assert!(fresnel.modes.iter().all(|mode| {
        mode.geometry.nodes.len() == 1
            && mode.geometry.nodes[0].glb_node.as_deref() == Some("fresnel-body")
            && mode.geometry.emitters.len() == 1
            && mode.geometry.emitters[0].origin.y == -694.0
            && mode.geometry.emitters[0].orientation_degrees == crate::Vector3::default()
    }));

    for filename in [
        "robe--robin-dls-profile.toskfixture",
        "jb-lighting--jbled-a7.toskfixture",
    ] {
        assert_moving_lamp_geometry(filename);
    }
}

fn assert_moving_lamp_geometry(filename: &str) {
    let mover = shipped_profile(filename);
    assert_eq!(mover.model_units, ModelUnits::Metres);
    assert!(
        mover
            .model_asset
            .as_deref()
            .is_some_and(|asset| { asset.starts_with("data:model/gltf-binary;base64,") })
    );
    assert!(mover.modes.iter().all(|mode| {
        mode.geometry.nodes.len() == 3
            && mode.geometry.nodes[0].glb_node.as_deref() == Some("moving-base")
            && mode.geometry.nodes[1].glb_node.as_deref() == Some("moving-yoke")
            && mode.geometry.nodes[2].glb_node.as_deref() == Some("moving-head")
            && mode.geometry.nodes[0].motion.is_none()
            && mode.geometry.nodes[1]
                .motion
                .as_ref()
                .is_some_and(|motion| motion.attribute.0 == "pan")
            && mode.geometry.nodes[2]
                .motion
                .as_ref()
                .is_some_and(|motion| motion.attribute.0 == "tilt")
            && mode.geometry.nodes[2].transform.translation.y < 0.0
            && mode.geometry.emitters.len() == 1
            && mode.geometry.emitters[0].node_id == mode.geometry.nodes[2].id
            && mode.geometry.emitters[0].origin.y < 0.0
    }));
}

#[test]
fn generic_led_packages_keep_only_operator_useful_channel_orders() {
    let expected = [
        (
            "generic--rgbw-led.toskfixture",
            vec![
                "DRGBW 8-bit dimmer first",
                "RGBWD 8-bit dimmer last",
                "RGBW virtual dimmer",
            ],
        ),
        (
            "generic--rgbwa-led.toskfixture",
            vec![
                "DRGBWA 8-bit dimmer first",
                "RGBWAD 8-bit dimmer last",
                "RGBWA virtual dimmer",
            ],
        ),
        (
            "generic--rgbwauv-led.toskfixture",
            vec![
                "DRGBWAU 8-bit dimmer first",
                "RGBWAUD 8-bit dimmer last",
                "RGBWAU virtual dimmer",
            ],
        ),
        (
            "generic--rgbcct-led.toskfixture",
            vec![
                "DRGBCW 8-bit dimmer first",
                "RGBCWD 8-bit dimmer last",
                "RGBCW virtual dimmer",
                "DRGBWC 8-bit dimmer first",
                "RGBWCD 8-bit dimmer last",
                "RGBWC virtual dimmer",
                "DCRGBW 8-bit dimmer first",
                "CRGBWD 8-bit dimmer last",
                "CRGBW virtual dimmer",
                "DCWRGB 8-bit dimmer first",
                "CWRGBD 8-bit dimmer last",
                "CWRGB virtual dimmer",
                "DWRGBC 8-bit dimmer first",
                "WRGBCD 8-bit dimmer last",
                "WRGBC virtual dimmer",
                "DWCRGB 8-bit dimmer first",
                "WCRGBD 8-bit dimmer last",
                "WCRGB virtual dimmer",
            ],
        ),
    ];

    for (filename, mode_names) in expected {
        let profile = shipped_profile(filename);
        assert_eq!(
            profile
                .modes
                .iter()
                .map(|mode| mode.name.as_str())
                .collect::<Vec<_>>(),
            mode_names
        );
        for mode in &profile.modes {
            assert_eq!(mode.splits.len(), 1);
            assert_eq!(mode.splits[0].footprint as usize, mode.channels.len());
            let intensity = mode
                .channels
                .iter()
                .position(|channel| channel.attribute.is_intensity());
            if mode.name.ends_with("virtual dimmer") {
                assert_eq!(intensity, None);
                assert!(
                    mode.channels
                        .iter()
                        .all(|channel| channel.reacts_to_virtual_intensity)
                );
            } else if mode.name.starts_with('D') {
                assert_eq!(intensity, Some(0));
            } else {
                assert_eq!(intensity, Some(mode.channels.len() - 1));
            }
        }
    }
}

#[test]
fn shipped_generic_cmy_retains_fixture_identity_and_maps_into_canonical_rgb() {
    let profile = shipped_profile("generic--cmy-led.toskfixture");
    assert_eq!(profile.schema_version, FIXTURE_PROFILE_SCHEMA_VERSION);
    assert_eq!(profile.modes.len(), 18);
    for mode in &profile.modes {
        for channel in &mode.channels {
            let expected = match channel.fixture_attribute.0.as_str() {
                "color.cyan" => Some("color.red"),
                "color.magenta" => Some("color.green"),
                "color.yellow" => Some("color.blue"),
                _ => None,
            };
            if let Some(expected) = expected {
                assert_eq!(channel.attribute.0, expected);
                assert_eq!(
                    channel.canonical_transform,
                    CanonicalTransform::InvertNormalized
                );
                assert!(channel.functions.iter().all(|function| {
                    function.attribute == channel.attribute
                        || !matches!(
                            function.behavior,
                            crate::ChannelFunctionBehavior::Continuous { .. }
                        )
                }));
            } else {
                assert_eq!(channel.fixture_attribute, channel.attribute);
                assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);
            }
        }
    }
}

fn minimal_glb(external_uri: bool) -> Vec<u8> {
    let json = if external_uri {
        br#"{"asset":{"version":"2.0"},"buffers":[{"byteLength":0,"uri":"outside.bin"}]}"#.to_vec()
    } else {
        br#"{"asset":{"version":"2.0"}}"#.to_vec()
    };
    let padded = (json.len() + 3) & !3;
    let total = 12 + 8 + padded;
    let mut result = Vec::with_capacity(total);
    result.extend_from_slice(b"glTF");
    result.extend_from_slice(&2_u32.to_le_bytes());
    result.extend_from_slice(&(total as u32).to_le_bytes());
    result.extend_from_slice(&(padded as u32).to_le_bytes());
    result.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
    result.extend_from_slice(&json);
    result.resize(total, b' ');
    result
}

fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn round_trips_profile_and_embedded_assets() {
    let mut profile = profile();
    profile.photograph_asset = Some(format!("data:image/png;base64,{PNG_1X1}"));
    profile.stage_icon_asset = Some(format!("data:image/png;base64,{PNG_1X1}"));
    profile.model_asset = Some(format!(
        "data:model/gltf-binary;base64,{}",
        STANDARD.encode(minimal_glb(false))
    ));

    let bytes = write_fixture_package(&profile).unwrap();
    let restored = read_fixture_package(&bytes).unwrap();
    assert_eq!(restored.id, profile.id);
    assert_eq!(restored.modes[0].id, profile.modes[0].id);
    assert_eq!(restored.photograph_asset, profile.photograph_asset);
    assert_eq!(restored.stage_icon_asset, profile.stage_icon_asset);
    assert_eq!(restored.model_asset, profile.model_asset);
    assert_eq!(restored.reserved_source, None);

    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let names = (0..zip.len())
        .map(|index| zip.by_index(index).unwrap().name().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "fixture.json",
            "assets/photograph.png",
            "assets/icon.png",
            "assets/model.glb"
        ]
    );
}

#[test]
fn shipped_fresnel_round_trips_without_identity_or_asset_loss() {
    let original = shipped_profile("generic--dimmer-fresnel.toskfixture");
    let exported = write_fixture_package(&original).unwrap();
    let restored = read_fixture_package(&exported).unwrap();
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(&original).unwrap()
    );
    assert_eq!(
        restored
            .modes
            .iter()
            .map(|mode| mode.id)
            .collect::<Vec<_>>(),
        original
            .modes
            .iter()
            .map(|mode| mode.id)
            .collect::<Vec<_>>()
    );
    assert!(restored.stage_icon_asset.is_some());
    assert!(restored.model_asset.is_some());
}

#[test]
fn rejects_unsafe_duplicate_and_unreferenced_paths() {
    let manifest = serde_json::to_vec(&FixturePackageManifest::new(profile())).unwrap();
    assert!(read_fixture_package(&archive(&[("../fixture.json", &manifest)])).is_err());
    assert!(
        read_fixture_package(&archive(&[
            ("fixture.json", &manifest),
            ("FIXTURE.JSON", &manifest),
        ]))
        .is_err()
    );
    assert!(
        read_fixture_package(&archive(&[
            ("fixture.json", &manifest),
            ("assets/unused.png", &[1, 2, 3]),
        ]))
        .is_err()
    );
}

#[test]
fn rejects_missing_mistyped_and_non_self_contained_assets() {
    let mut missing = profile();
    missing.stage_icon_asset = Some("assets/icon.png".into());
    let manifest = serde_json::to_vec(&FixturePackageManifest::new(missing)).unwrap();
    assert!(read_fixture_package(&archive(&[("fixture.json", &manifest)])).is_err());

    let mut mistyped = profile();
    mistyped.stage_icon_asset = Some("assets/icon.jpg".into());
    let manifest = serde_json::to_vec(&FixturePackageManifest::new(mistyped)).unwrap();
    let png = STANDARD.decode(PNG_1X1).unwrap();
    assert!(
        read_fixture_package(&archive(&[
            ("fixture.json", &manifest),
            ("assets/icon.jpg", &png),
        ]))
        .is_err()
    );

    let mut external = profile();
    external.model_asset = Some("assets/model.glb".into());
    let manifest = serde_json::to_vec(&FixturePackageManifest::new(external)).unwrap();
    let glb = minimal_glb(true);
    assert!(
        read_fixture_package(&archive(&[
            ("fixture.json", &manifest),
            ("assets/model.glb", &glb),
        ]))
        .is_err()
    );
}

#[test]
fn rejects_unknown_manifest_fields_and_reserved_sources() {
    let json = serde_json::json!({
        "format": FIXTURE_PACKAGE_FORMAT,
        "format_version": FIXTURE_PACKAGE_FORMAT_VERSION,
        "profile": profile(),
        "typo": true
    });
    let manifest = serde_json::to_vec(&json).unwrap();
    assert!(read_fixture_package(&archive(&[("fixture.json", &manifest)])).is_err());

    let mut reserved = profile();
    reserved.reserved_source = Some("builtin:anything".into());
    let manifest = serde_json::to_vec(&FixturePackageManifest::new(reserved)).unwrap();
    assert!(read_fixture_package(&archive(&[("fixture.json", &manifest)])).is_err());
}
