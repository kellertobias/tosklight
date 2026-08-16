use super::*;
use crate::{
    CanonicalTransform, ChannelResolution, ColorSystem, EmitterLayout,
    FIXTURE_PROFILE_SCHEMA_VERSION, FixtureProfile, FixtureSplit, ModelUnits, PatchPolicy,
    PositionMovementRepresentation,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
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

fn compatibility_profile(filename: &str) -> FixtureProfile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
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
        ("venue--disco-ball-50-cm.toskfixture", 1),
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
fn stage_lamp_packages_leave_body_models_to_visualizer_defaults() {
    let acl = shipped_profile("generic--acl.toskfixture");
    assert_eq!(acl.manufacturer, "Generic");
    assert_eq!(acl.name, "ACL");
    assert_eq!(acl.model_units, ModelUnits::Metres);
    assert_eq!(acl.physical.width_millimetres, Some(80.0));
    assert_eq!(acl.physical.height_millimetres, Some(80.0));
    assert_eq!(acl.physical.depth_millimetres, Some(200.0));
    assert!(acl.model_asset.is_none());
    assert!(acl.projection_assets.is_none());
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

#[test]
fn shipped_jbled_a7_uses_the_documented_safe_shutter_table_in_every_mode() {
    let profile = shipped_profile("jb-lighting--jbled-a7.toskfixture");
    assert_eq!(profile.revision, 2);
    assert!(profile.notes.contains("JBLED_A7_DMX_Protocol.pdf"));
    assert_eq!(profile.modes.len(), 4);
    for mode in &profile.modes {
        let shutter = mode
            .channels
            .iter()
            .find(|channel| channel.attribute.0 == "shutter")
            .unwrap();
        assert_eq!(shutter.default_raw, 16, "{} shutter home", mode.name);
        assert_eq!(shutter.highlight_raw, 255, "{} Highlight", mode.name);
        assert_eq!(shutter.functions.len(), 23, "{} function bands", mode.name);
        assert_eq!(
            (shutter.functions[0].dmx_from, shutter.functions[0].dmx_to),
            (0, 15)
        );
        assert_eq!(
            (shutter.functions[1].dmx_from, shutter.functions[1].dmx_to),
            (16, 95)
        );
        assert_eq!(shutter.functions[1].name, "Shutter open");
        assert_eq!(
            (shutter.functions[22].dmx_from, shutter.functions[22].dmx_to),
            (255, 255)
        );
        for pair in shutter.functions.windows(2) {
            assert_eq!(pair[0].dmx_to + 1, pair[1].dmx_from);
        }
    }
}

#[test]
fn shipped_audio_player_is_one_programmable_zero_dmx_internal_voice() {
    let profile = shipped_profile("tosklight--audio-player.toskfixture");
    assert_eq!(profile.manufacturer, "ToskLight");
    assert_eq!(profile.name, "Audio Player");
    assert_eq!(profile.patch_policy, PatchPolicy::Internal);
    assert_eq!(profile.modes.len(), 1);
    let mode = &profile.modes[0];
    assert_eq!(mode.name, "Internal Audio");
    assert_eq!(
        mode.splits,
        [FixtureSplit {
            number: 1,
            footprint: 0
        }]
    );
    assert_eq!(mode.heads.len(), 1);
    assert!(!mode.heads[0].master_shared);
    assert_eq!(
        mode.channels
            .iter()
            .map(|channel| channel.attribute.0.as_str())
            .collect::<Vec<_>>(),
        [
            "audio.folder",
            "audio.file",
            "audio.transport",
            "audio.repeat",
            "audio.volume",
        ]
    );
    assert!(
        mode.channels
            .iter()
            .all(|channel| channel.secondary_slots.is_empty())
    );
    let definition = profile.resolved_definition(mode.id).unwrap();
    assert_eq!(definition.footprint, 0);
    assert!(
        definition.heads[0]
            .parameters
            .iter()
            .all(|parameter| parameter.components.is_empty())
    );
}

#[test]
fn shipped_crowd_area_has_every_visual_only_mode() {
    let profile = shipped_profile("venue--crowd-area.toskfixture");
    assert_eq!(profile.manufacturer, "Venue");
    assert_eq!(profile.name, "Crowd Area");
    assert_eq!(profile.patch_policy, PatchPolicy::VisualOnly);
    assert_eq!(profile.modes.len(), 9);
    assert!(profile.modes.iter().all(|mode| {
        mode.splits
            == vec![FixtureSplit {
                number: 1,
                footprint: 0,
            }]
    }));
    let crowd = profile.crowd.expect("crowd profile contract");
    assert_eq!(crowd.default_width_metres, 5.0);
    assert_eq!(crowd.default_depth_metres, 3.0);
    assert_eq!(crowd.modes.len(), 9);
    for posture in [
        crate::CrowdPosture::Sitting,
        crate::CrowdPosture::StandingStill,
        crate::CrowdPosture::Dancing,
    ] {
        for density in [
            crate::CrowdDensity::Sparse,
            crate::CrowdDensity::Medium,
            crate::CrowdDensity::Dense,
        ] {
            assert!(
                crowd
                    .modes
                    .iter()
                    .any(|mode| mode.posture == posture && mode.density == density),
                "missing {posture:?} {density:?}"
            );
        }
    }
}

fn assert_moving_lamp_geometry(filename: &str) {
    let mover = shipped_profile(filename);
    assert_eq!(mover.model_units, ModelUnits::Metres);
    assert!(mover.model_asset.is_none());
    assert!(mover.projection_assets.is_none());
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
fn robe_dls_profile_exposes_canonical_framing_controls() {
    let profile = shipped_profile("robe--robin-dls-profile.toskfixture");
    assert_eq!(profile.revision, 3);
    assert!(profile.notes.contains("DMX protocol version 1.0"));
    assert!(profile.notes.contains("user manual version 1.3"));
    assert_eq!(
        profile
            .modes
            .iter()
            .map(|mode| (mode.name.as_str(), mode.splits[0].footprint))
            .collect::<Vec<_>>(),
        [("Mode 1", 47), ("Mode 2", 38), ("Mode 3", 36)]
    );

    for mode in &profile.modes {
        let rotation = mode
            .channels
            .iter()
            .find(|channel| channel.attribute.0 == "shaper.rotation")
            .unwrap();
        assert_eq!(rotation.fixture_attribute, rotation.attribute);
        assert_eq!(rotation.default_raw, 128);
        assert_eq!(rotation.highlight_raw, 128);
        assert_eq!(rotation.physical_min, Some(-45.0));
        assert_eq!(rotation.physical_max, Some(45.0));
        assert_eq!(rotation.unit.as_deref(), Some("degrees"));
        assert!(!rotation.snap);

        for blade in 1..=4 {
            let position_attribute = format!("shaper.blade.{blade}.position");
            let position = mode
                .channels
                .iter()
                .find(|channel| channel.attribute.0 == position_attribute)
                .unwrap();
            assert_eq!(position.fixture_attribute, position.attribute);
            assert_eq!(position.default_raw, 0);
            assert_eq!(position.highlight_raw, 0);
            assert_eq!(position.physical_min, Some(0.0));
            assert_eq!(position.physical_max, Some(1.0));
            assert_eq!(position.unit, None);
            assert!(!position.snap);

            let angle_attribute = format!("shaper.blade.{blade}.angle");
            let angle = mode
                .channels
                .iter()
                .find(|channel| channel.attribute.0 == angle_attribute)
                .unwrap();
            assert_eq!(angle.fixture_attribute, angle.attribute);
            assert_eq!(angle.default_raw, 128);
            assert_eq!(angle.highlight_raw, 128);
            assert_eq!(angle.physical_min, Some(-25.0));
            assert_eq!(angle.physical_max, Some(25.0));
            assert_eq!(angle.unit.as_deref(), Some("degrees"));
            assert!(!angle.snap);
        }

        let shutter = mode
            .channels
            .iter()
            .find(|channel| channel.attribute.0 == "shutter")
            .expect("the independent shutter/strobe channel remains canonical Shutter / Strobe");
        assert_eq!(shutter.default_raw, 32);
        assert_eq!(shutter.highlight_raw, 32);
        assert_eq!(
            shutter
                .functions
                .iter()
                .map(|function| {
                    let semantic = match &function.behavior {
                        crate::ChannelFunctionBehavior::Fixed { semantic_id, .. } => {
                            Some(semantic_id.as_str())
                        }
                        _ => None,
                    };
                    (
                        function.name.as_str(),
                        function.dmx_from,
                        function.dmx_to,
                        semantic,
                    )
                })
                .collect::<Vec<_>>(),
            [
                ("Shutter closed", 0, 31, Some("closed")),
                ("Shutter open", 32, 63, Some("open")),
                ("Strobe effect from slow to fast", 64, 95, None),
                ("Shutter open", 96, 127, Some("open")),
                (
                    "Opening pulse in sequences from slow to fast",
                    128,
                    143,
                    None,
                ),
                (
                    "Closing pulse in sequences from fast to slow",
                    144,
                    159,
                    None,
                ),
                ("Shutter open", 160, 191, Some("open")),
                ("Random strobe effect from slow to fast", 192, 223, None),
                ("Shutter open", 224, 255, Some("open")),
            ],
            "the package must retain the manufacturer's exact shutter bands"
        );
    }

    let exported = write_fixture_package(&profile).unwrap();
    let restored = read_fixture_package(&exported).unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(profile).unwrap(),
        "the corrected revision must export without changing its stable identities or ranges"
    );
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
fn showtec_sunstrip_thirty_channel_mode_projects_one_virtual_dimmer_per_pixel() {
    let profile = shipped_profile("showtec--sunstrip-led-rgb-42206.toskfixture");
    let mode = profile
        .modes
        .iter()
        .find(|mode| mode.name == "30 Channel")
        .expect("shipped 30-channel mode");
    assert_eq!(mode.splits[0].footprint, 30);

    let pixels = mode
        .heads
        .iter()
        .filter(|head| !head.master_shared)
        .collect::<Vec<_>>();
    assert_eq!(pixels.len(), 10);
    for head in &pixels {
        let channels = mode
            .channels
            .iter()
            .filter(|channel| channel.head_id == head.id)
            .collect::<Vec<_>>();
        assert_eq!(channels.len(), 3, "{} keeps exactly RGB on DMX", head.name);
        assert!(
            channels
                .iter()
                .all(|channel| channel.reacts_to_virtual_intensity),
            "{} RGB must be scaled by its virtual dimmer",
            head.name
        );
    }

    let definition = profile
        .resolved_definition(mode.id)
        .expect("Sunstrip definition");
    for pixel in definition.heads.iter().filter(|head| !head.shared) {
        let intensities = pixel
            .parameters
            .iter()
            .filter(|parameter| parameter.attribute.is_intensity())
            .collect::<Vec<_>>();
        assert_eq!(intensities.len(), 1, "{} has one intensity", pixel.name);
        assert!(intensities[0].virtual_dimmer);
        assert!(intensities[0].components.is_empty());
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

#[test]
fn shipped_cct_emitters_retain_physical_identity_and_map_to_white_and_amber() {
    for filename in [
        "generic--cct-led.toskfixture",
        "generic--rgbcct-led.toskfixture",
    ] {
        let profile = shipped_profile(filename);
        for mode in &profile.modes {
            for (fixture_attribute, canonical) in [
                ("color.cold_white", "color.white"),
                ("color.warm_white", "color.amber"),
            ] {
                let channels = mode
                    .channels
                    .iter()
                    .filter(|channel| channel.fixture_attribute.0 == fixture_attribute)
                    .collect::<Vec<_>>();
                assert_eq!(channels.len(), 1, "{filename} / {}", mode.name);
                let channel = channels[0];
                assert_eq!(channel.attribute.0, canonical);
                assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);
                assert!(
                    channel
                        .functions
                        .iter()
                        .all(|function| function.attribute.0 == canonical)
                );
            }
            assert!(
                mode.channels
                    .iter()
                    .all(|channel| channel.attribute.0 != "color.cold_white"
                        && channel.attribute.0 != "color.warm_white")
            );
        }
    }
}

#[test]
fn shipped_native_hsi_modes_bind_their_physical_coordinates_and_highlight_white() {
    let expected = [
        (
            "chauvet-professional--colorado-1-solo.toskfixture",
            vec![(
                "HSIC",
                "e73c1545-9a30-37db-046f-31a9af27a6c2",
                "0e431711-e4b3-a87d-8911-fc518c2af61e",
                "3ea9dc10-0fa0-77a2-4b4e-e28cb4af9901",
            )],
        ),
        (
            "etc--source-four-led-series-2-lustr.toskfixture",
            vec![
                (
                    "HSI",
                    "fb62238a-4b70-2872-56fe-f9392d5c5099",
                    "35b4669a-85f4-029a-0f32-0e34a335d33f",
                    "173b22d7-aa50-70ba-923b-f538631d48e5",
                ),
                (
                    "HSIC",
                    "3944d319-2fef-b619-cad8-f5ad72c8c100",
                    "9a592c8d-e910-a7bf-8947-058feee4acc8",
                    "06016ed1-03ef-e265-903b-92f45ea8e0cc",
                ),
                (
                    "HSI Plus 7",
                    "d3bf551a-e1fb-1fef-0ffb-4bc4907aaeba",
                    "09af8b34-111d-4585-d123-7c1c76cdb7b2",
                    "0061c934-7fd9-6e1f-bf13-c816082b6ce2",
                ),
                (
                    "HSIC Plus 7",
                    "9b226a1d-d655-833a-81e4-922c1da3fb31",
                    "923e8b62-60d2-8132-45d0-90e263763947",
                    "d6b870ba-62bf-bd62-2d29-76489826331d",
                ),
            ],
        ),
    ];

    for (filename, modes) in expected {
        let profile = shipped_profile(filename);
        assert_eq!(profile.revision, 2);
        for (mode_name, hue_id, saturation_id, intensity_id) in modes {
            let mode = profile
                .modes
                .iter()
                .find(|mode| mode.name == mode_name)
                .unwrap();
            assert_eq!(mode.color_systems.len(), 1);
            let color = &mode.color_systems[0];
            let ColorSystem::HueSaturation {
                hue_channel_id,
                saturation_channel_id,
                intensity_channel_id,
            } = &color.system
            else {
                panic!("{filename} / {mode_name} must remain native hue/saturation");
            };
            assert_eq!(hue_channel_id.to_string(), hue_id);
            assert_eq!(saturation_channel_id.to_string(), saturation_id);
            assert_eq!(intensity_channel_id.unwrap().to_string(), intensity_id);

            for (channel_id, highlight) in [
                (*hue_channel_id, 0),
                (*saturation_channel_id, 0),
                (*intensity_channel_id.as_ref().unwrap(), 255),
            ] {
                let channel = mode
                    .channels
                    .iter()
                    .find(|channel| channel.id == channel_id)
                    .unwrap();
                assert_eq!(channel.head_id, color.head_id);
                assert_eq!(channel.highlight_raw, highlight);
            }
        }
    }
}

#[test]
fn shipped_strobe_channels_keep_fixture_identity_and_program_canonical_shutter() {
    for filename in [
        "generic--strobe.toskfixture",
        "tosklight--visualizer-laser.toskfixture",
    ] {
        let profile = shipped_profile(filename);
        let channels = profile
            .modes
            .iter()
            .flat_map(|mode| &mode.channels)
            .filter(|channel| channel.fixture_attribute.0 == "strobe")
            .collect::<Vec<_>>();
        assert!(
            !channels.is_empty(),
            "{filename} must retain physical strobe"
        );
        assert!(channels.iter().all(|channel| {
            channel.attribute.0 == "shutter"
                && channel.canonical_transform == CanonicalTransform::Identity
                && channel
                    .functions
                    .iter()
                    .all(|function| function.attribute.0 == "shutter")
        }));
    }
}

#[test]
fn shipped_primary_frost_channels_program_canonical_softness() {
    let mut affected_modes = 0;
    for filename in [
        "claypaky--sharpy.toskfixture",
        "robe--robin-dls-profile.toskfixture",
    ] {
        let profile = shipped_profile(filename);
        for mode in &profile.modes {
            let channels = mode
                .channels
                .iter()
                .filter(|channel| channel.fixture_attribute.0 == "frost")
                .collect::<Vec<_>>();
            if channels.is_empty() {
                continue;
            }
            affected_modes += 1;
            assert_eq!(channels.len(), 1, "{filename} / {}", mode.name);
            let channel = channels[0];
            assert_eq!(channel.attribute.0, "softness");
            assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);
            assert!(
                channel
                    .functions
                    .iter()
                    .all(|function| function.attribute.0 == "softness")
            );
        }
    }
    assert_eq!(affected_modes, 5);
}

#[test]
fn rare_capability_profiles_remain_independent_and_round_trip() {
    let expected = [
        (
            "generic--endless-pan-tilt.toskfixture",
            "Endless Pan Tilt",
            "Endless Pan/Tilt 16-bit",
            4,
        ),
        (
            "generic--beam-size-edge.toskfixture",
            "Beam Size and Edge",
            "Intensity, Beam Size, Edge",
            3,
        ),
        (
            "generic--media-positioning.toskfixture",
            "Media Positioning",
            "Position X/Y",
            2,
        ),
    ];
    for (filename, name, mode_name, footprint) in expected {
        let profile = shipped_profile(filename);
        assert_eq!(profile.manufacturer, "Generic");
        assert_eq!(profile.name, name);
        assert_eq!(profile.reserved_source, None);
        assert_eq!(profile.modes.len(), 1);
        assert_eq!(profile.modes[0].name, mode_name);
        assert_eq!(
            profile.modes[0].splits,
            [FixtureSplit {
                number: 1,
                footprint
            }]
        );
        let exported = write_fixture_package(&profile).unwrap();
        let restored = read_fixture_package(&exported).unwrap();
        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            serde_json::to_value(&profile).unwrap(),
            "{filename}"
        );
    }

    let frost = compatibility_profile("generic--dual-frost.toskfixture");
    assert!(frost.notes.contains("compatibility"));
    let channels = &frost.modes[0].channels;
    assert_eq!(
        channels
            .iter()
            .map(|channel| (
                channel.fixture_attribute.0.as_str(),
                channel.attribute.0.as_str(),
                channel.default_raw,
                channel.highlight_raw,
            ))
            .collect::<Vec<_>>(),
        [
            ("intensity", "intensity", 0, 255),
            ("frost.1", "softness", 0, 0),
            ("frost.2", "frost.2", 0, 0),
        ]
    );
    let mut frost_frame = [0_u8; 512];
    for (channel, raw) in channels.iter().zip([255, 128, 64]) {
        frost.modes[0]
            .encode_channel(&mut frost_frame, 1, channel, raw)
            .unwrap();
    }
    assert_eq!(&frost_frame[..3], &[255, 128, 64]);

    let endless = shipped_profile("generic--endless-pan-tilt.toskfixture");
    let mode = &endless.modes[0];
    assert_eq!(
        mode.channels
            .iter()
            .map(|channel| (
                channel.fixture_attribute.0.as_str(),
                channel.attribute.0.as_str(),
                channel.resolution,
                channel.secondary_slots.as_slice(),
            ))
            .collect::<Vec<_>>(),
        [
            ("pan.continuous", "pan", ChannelResolution::U16, &[2][..]),
            ("tilt.continuous", "tilt", ChannelResolution::U16, &[4][..]),
        ]
    );
    let definition = endless.resolved_definition(mode.id).unwrap();
    assert!(definition.heads[0].parameters.iter().all(|parameter| {
        matches!(
            parameter.metadata.position_axis_representation,
            Some(crate::PositionAxisRepresentation::Endless)
        )
    }));
    let mut endless_frame = [0_u8; 512];
    mode.encode_channel(&mut endless_frame, 1, &mode.channels[0], 0x1234)
        .unwrap();
    mode.encode_channel(&mut endless_frame, 1, &mode.channels[1], 0xabcd)
        .unwrap();
    assert_eq!(&endless_frame[..4], &[0x12, 0x34, 0xab, 0xcd]);

    let beam = shipped_profile("generic--beam-size-edge.toskfixture");
    assert_eq!(
        beam.modes[0]
            .channels
            .iter()
            .map(|channel| (
                channel.fixture_attribute.0.as_str(),
                channel.attribute.0.as_str(),
            ))
            .collect::<Vec<_>>(),
        [
            ("intensity", "intensity"),
            ("zoom", "zoom"),
            ("beam.edge", "softness"),
        ]
    );
    let mut beam_frame = [0_u8; 512];
    for (channel, raw) in beam.modes[0].channels.iter().zip([255, 96, 160]) {
        beam.modes[0]
            .encode_channel(&mut beam_frame, 1, channel, raw)
            .unwrap();
    }
    assert_eq!(&beam_frame[..3], &[255, 96, 160]);

    let media = shipped_profile("generic--media-positioning.toskfixture");
    assert_eq!(
        media.modes[0]
            .channels
            .iter()
            .map(|channel| channel.attribute.0.as_str())
            .collect::<Vec<_>>(),
        ["media.position.x", "media.position.y"]
    );
    let mut media_frame = [0_u8; 512];
    media.modes[0]
        .encode_channel(&mut media_frame, 1, &media.modes[0].channels[0], 32)
        .unwrap();
    media.modes[0]
        .encode_channel(&mut media_frame, 1, &media.modes[0].channels[1], 224)
        .unwrap();
    assert_eq!(&media_frame[..2], &[32, 224]);

    let source_four = shipped_profile("etc--source-four-led-series-2-lustr.toskfixture");
    let studio = source_four
        .modes
        .iter()
        .find(|mode| mode.name == "Studio")
        .unwrap();
    assert!(studio.channels.iter().any(|channel| {
        channel.fixture_attribute.0 == "color.temperature"
            && channel.attribute.0 == "color.temperature"
    }));
    assert!(studio.channels.iter().any(|channel| {
        channel.fixture_attribute.0 == "fixture.tint" && channel.attribute.0 == "color.tint"
    }));
}

#[test]
fn visualizer_camera_package_keeps_its_exact_seventeen_slot_wire_contract() {
    let profile = shipped_profile("tosklight--visualizer-camera.toskfixture");
    assert_eq!(
        profile.id.0.to_string(),
        "ddf9c823-4062-490c-bd10-a15ca1c7cf4e"
    );
    assert_eq!(profile.manufacturer, "ToskLight");
    assert_eq!(profile.name, "Visualizer Camera");
    assert_eq!(profile.fixture_type, "visualizer_camera");
    assert_eq!(profile.reserved_source, None);
    assert_eq!(
        profile.signal_loss_policy,
        crate::SignalLossPolicy::HoldLast
    );
    assert!(profile.model_asset.is_none());
    assert_eq!(profile.modes.len(), 1);

    let mode = &profile.modes[0];
    assert_eq!(mode.id.to_string(), "323405fd-2b08-4df7-838a-d8cf5bd1cfa5");
    assert_eq!(mode.name, "External Camera 17-slot");
    assert_eq!(
        mode.splits,
        [FixtureSplit {
            number: 1,
            footprint: 17,
        }]
    );
    assert_eq!(mode.heads.len(), 1);
    assert_eq!(
        mode.heads[0].id.to_string(),
        "672d3fa5-d967-4473-a4f9-1de97d31066a"
    );
    assert!(mode.heads[0].master_shared);
    assert!(mode.geometry.nodes.is_empty());
    assert!(mode.geometry.emitters.is_empty());

    assert_eq!(
        mode.channels
            .iter()
            .map(|channel| (
                channel.fixture_attribute.0.as_str(),
                channel.attribute.0.as_str(),
                channel.resolution,
                channel.secondary_slots.as_slice(),
                channel.default_raw,
                channel.highlight_raw,
            ))
            .collect::<Vec<_>>(),
        [
            (
                "camera.position.x",
                "camera.position.x",
                ChannelResolution::U24,
                &[2, 3][..],
                0x80_0000,
                0x80_0000,
            ),
            (
                "camera.position.y",
                "camera.position.y",
                ChannelResolution::U24,
                &[5, 6][..],
                0x80_0000,
                0x80_0000,
            ),
            (
                "camera.position.z",
                "camera.position.z",
                ChannelResolution::U24,
                &[8, 9][..],
                0x80_0000,
                0x80_0000,
            ),
            (
                "camera.yaw",
                "camera.yaw",
                ChannelResolution::U16,
                &[11][..],
                0x8000,
                0x8000,
            ),
            (
                "camera.pitch",
                "camera.pitch",
                ChannelResolution::U16,
                &[13][..],
                0x8000,
                0x8000,
            ),
            (
                "camera.roll",
                "camera.roll",
                ChannelResolution::U16,
                &[15][..],
                0x8000,
                0x8000,
            ),
            (
                "camera.zoom",
                "camera.zoom",
                ChannelResolution::U16,
                &[17][..],
                0,
                0,
            ),
        ]
    );
    let primary_slots = mode.primary_slots().unwrap();
    assert_eq!(
        mode.channels
            .iter()
            .map(|channel| primary_slots[&channel.id])
            .collect::<Vec<_>>(),
        [1, 4, 7, 10, 12, 14, 16]
    );
    let stable_ids = std::iter::once(profile.id.0)
        .chain(std::iter::once(mode.id))
        .chain(mode.heads.iter().map(|head| head.id))
        .chain(mode.channels.iter().map(|channel| channel.id))
        .chain(
            mode.channels
                .iter()
                .flat_map(|channel| channel.functions.iter().map(|function| function.id)),
        )
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        stable_ids.len(),
        17,
        "every semantic object has a unique stable ID"
    );

    for channel in &mode.channels[..3] {
        assert_eq!(channel.physical_min, Some(-4_194.304));
        assert_eq!(channel.physical_max, Some(4_194.303_5));
        assert_eq!(channel.unit.as_deref(), Some("m"));
        assert_eq!(channel.functions.len(), 1);
        assert_eq!(channel.functions[0].dmx_from, 0);
        assert_eq!(channel.functions[0].dmx_to, 0xff_ffff);
        assert!(channel.functions[0].angular_motion.is_none());
    }
    for channel in &mode.channels[3..6] {
        assert_eq!(channel.physical_min, Some(-360.0));
        assert_eq!(channel.physical_max, Some(360.0));
        assert_eq!(channel.unit.as_deref(), Some("deg"));
        assert_eq!(channel.functions[0].dmx_to, u32::from(u16::MAX));
        assert!(channel.functions[0].angular_motion.is_none());
    }
    let zoom = &mode.channels[6];
    assert_eq!(zoom.physical_min, Some(18.0));
    assert_eq!(zoom.physical_max, Some(1_200.0));
    assert_eq!(zoom.unit.as_deref(), Some("mm"));
    assert!(profile.notes.contains("f = 18 * (1200 / 18)^(raw / 65535)"));

    let mut frame = [0_u8; 512];
    for (channel, raw) in mode.channels.iter().zip([
        0x12_3456, 0xab_cdef, 0x80_0000, 0x1357, 0x2468, 0x8000, 0xffff,
    ]) {
        mode.encode_channel(&mut frame, 1, channel, raw).unwrap();
    }
    assert_eq!(
        &frame[..17],
        &[
            0x12, 0x34, 0x56, 0xab, 0xcd, 0xef, 0x80, 0x00, 0x00, 0x13, 0x57, 0x24, 0x68, 0x80,
            0x00, 0xff, 0xff,
        ]
    );

    let exported = write_fixture_package(&profile).unwrap();
    let restored = read_fixture_package(&exported).unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(profile).unwrap()
    );
}

#[test]
fn tosklight_media_server_package_exposes_complete_multi_head_personalities() {
    let profile = shipped_profile("tosklight--media-server.toskfixture");
    assert_eq!(profile.manufacturer, "ToskLight");
    assert_eq!(profile.name, "Media Server");
    assert_eq!(profile.modes.len(), 2);

    for (mode, layer_count, footprint) in [
        (&profile.modes[0], 2_usize, 75_u16),
        (&profile.modes[1], 8_usize, 279_u16),
    ] {
        assert_eq!(
            mode.splits,
            vec![FixtureSplit {
                number: 1,
                footprint
            }]
        );
        assert_eq!(mode.heads.len(), layer_count + 1);
        assert_eq!(
            mode.heads.iter().filter(|head| head.master_shared).count(),
            1
        );
        assert_eq!(mode.heads[0].name, "Master");
        assert!(mode.heads[1..].iter().all(|head| !head.master_shared));
        assert_eq!(
            mode.heads[1..]
                .iter()
                .map(|head| head.name.as_str())
                .collect::<Vec<_>>(),
            (1..=layer_count)
                .map(|layer| format!("Layer {layer}"))
                .collect::<Vec<_>>()
        );
        assert!(mode.channels.iter().all(|channel| channel.split == 1));
        assert!(
            mode.channels
                .iter()
                .all(|channel| { mode.heads.iter().any(|head| head.id == channel.head_id) })
        );
        let primary_slots = mode.primary_slots().unwrap();
        let mut owned_slots = std::collections::BTreeSet::new();
        for channel in &mode.channels {
            assert!(owned_slots.insert(primary_slots[&channel.id]));
            for slot in &channel.secondary_slots {
                assert!(owned_slots.insert(*slot));
            }
        }
        assert_eq!(owned_slots, (1..=footprint).collect());
    }

    assert!(profile.notes.contains("Legacy Media Server Layer"));
    assert!(profile.notes.contains("existing show snapshots"));
}

#[test]
fn shipped_library_keeps_compound_prism_and_motion_migration_evidence_explicit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("assets/fixture-library");
    let mut prism_selection_modes = 0;
    let mut prism_rotation_modes = 0;
    let mut generic_control_modes = 0;
    let mut position_movement_modes = 0;
    let mut position_movement_sources = std::collections::HashMap::<String, usize>::new();
    let mut continuous_pan_or_tilt = Vec::new();
    let mut retired_placeholder_attributes = Vec::new();

    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toskfixture") {
            continue;
        }
        let profile = read_fixture_package(&fs::read(&path).unwrap()).unwrap();
        for mode in &profile.modes {
            let movement_channels = mode
                .channels
                .iter()
                .filter(|channel| channel.attribute.0 == "position.movement")
                .collect::<Vec<_>>();
            if !movement_channels.is_empty() {
                position_movement_modes += 1;
                assert_eq!(
                    movement_channels.len(),
                    1,
                    "{} / {} must expose one shared Position Movement channel",
                    profile.name,
                    mode.name
                );
                let channel = movement_channels[0];
                assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);
                assert!(
                    channel
                        .functions
                        .iter()
                        .all(|function| { function.attribute.0 == "position.movement" })
                );
                *position_movement_sources
                    .entry(channel.fixture_attribute.0.clone())
                    .or_default() += 1;
                let expected_representation = match channel.fixture_attribute.0.as_str() {
                    "fixture.pan_tilt_speed" => PositionMovementRepresentation::Speed,
                    "fixture.mspeed" | "fixture.pan_tilt_time" => {
                        PositionMovementRepresentation::Time
                    }
                    "fixture.pan_tilt_speed_time" => PositionMovementRepresentation::SpeedOrTime,
                    source => panic!("unexpected Position Movement source {source}"),
                };
                let definition = profile.resolved_definition(mode.id).unwrap();
                let projected = definition
                    .heads
                    .iter()
                    .flat_map(|head| &head.parameters)
                    .find(|parameter| parameter.attribute.0 == "position.movement")
                    .unwrap();
                assert_eq!(
                    projected.metadata.position_movement_representation,
                    Some(expected_representation),
                    "{} / {}",
                    profile.name,
                    mode.name
                );
            }
            let prism_selection = mode
                .channels
                .iter()
                .find(|channel| channel.attribute.0 == "prism.1");
            let prism_rotation = mode
                .channels
                .iter()
                .find(|channel| channel.attribute.0 == "prism.1.rotation");
            if prism_selection.is_some() {
                prism_selection_modes += 1;
            }
            if let Some(rotation) = prism_rotation {
                prism_rotation_modes += 1;
                assert_eq!(
                    prism_selection.map(|channel| channel.head_id),
                    Some(rotation.head_id),
                    "{} / {} must keep Prism 1 selection and rotation on one logical head",
                    profile.name,
                    mode.name
                );
            }
            if mode
                .channels
                .iter()
                .any(|channel| channel.fixture_attribute.0 == "fixture.control")
            {
                generic_control_modes += 1;
            }
            for channel in &mode.channels {
                for attribute in std::iter::once(&channel.attribute)
                    .chain(channel.functions.iter().map(|function| &function.attribute))
                {
                    if light_core::built_in_attribute_is_retired(&attribute.0) {
                        retired_placeholder_attributes.push(format!(
                            "{} / {} / {}",
                            profile.name, mode.name, attribute.0
                        ));
                    }
                }
                if channel.fixture_attribute == channel.attribute
                    && light_core::built_in_attribute_is_retired(&channel.fixture_attribute.0)
                {
                    retired_placeholder_attributes.push(format!(
                        "{} / {} / {}",
                        profile.name, mode.name, channel.fixture_attribute.0
                    ));
                }
                if matches!(
                    channel.attribute.0.as_str(),
                    "pan.continuous" | "tilt.continuous"
                ) {
                    continuous_pan_or_tilt.push(format!(
                        "{} / {} / {}",
                        profile.name, mode.name, channel.attribute.0
                    ));
                }
            }
        }
    }

    assert_eq!(prism_selection_modes, 7);
    assert_eq!(prism_rotation_modes, 5);
    assert_eq!(generic_control_modes, 5);
    assert_eq!(position_movement_modes, 26);
    assert_eq!(
        position_movement_sources,
        std::collections::HashMap::from([
            ("fixture.mspeed".into(), 2),
            ("fixture.pan_tilt_speed".into(), 4),
            ("fixture.pan_tilt_speed_time".into(), 19),
            ("fixture.pan_tilt_time".into(), 1),
        ])
    );
    assert!(
        continuous_pan_or_tilt.is_empty(),
        "continuous motion now needs a co-occurrence migration review: {continuous_pan_or_tilt:?}"
    );
    assert!(
        retired_placeholder_attributes.is_empty(),
        "shipped fixtures must map compatibility-only placeholder attributes before retirement: \
         {retired_placeholder_attributes:?}"
    );
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

fn projection_set(model: &[u8]) -> crate::ProfileProjectionSet {
    let hash = Sha256::digest(model)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    crate::ProfileProjectionSet {
        source_model_sha256: hash,
        generator: "test-generator".into(),
        generator_version: "1".into(),
        pose_contract_version: 1,
        views: crate::ProfileProjectionView::ALL
            .into_iter()
            .map(|view| {
                let svg = format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-5 -10 10 20\" width=\"10mm\" height=\"20mm\" data-tosklight-view=\"{}\"><path d=\"M -5 -10 L 5 -10 L 0 10 Z\" fill=\"#445566\" fill-rule=\"nonzero\"/></svg>",
                    view.wire()
                );
                crate::ProfileProjectionAsset {
                    view,
                    artwork_asset: format!(
                        "data:image/svg+xml;base64,{}",
                        STANDARD.encode(svg)
                    ),
                    view_box_millimetres: [-5.0, -10.0, 10.0, 20.0],
                    physical_width_millimetres: 10.0,
                    physical_height_millimetres: 20.0,
                    origin_millimetres: [0.0, 0.0],
                    orientation: view.orientation(),
                    pose: crate::ProfileProjectionPose::AuthoredHome,
                }
            })
            .collect(),
    }
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
fn round_trips_five_safe_svg_projections_at_canonical_paths() {
    let mut profile = profile();
    let model = minimal_glb(false);
    profile.model_asset = Some(format!(
        "data:model/gltf-binary;base64,{}",
        STANDARD.encode(&model)
    ));
    profile.projection_assets = Some(projection_set(&model));

    let bytes = write_fixture_package(&profile).unwrap();
    let restored = read_fixture_package(&bytes).unwrap();
    assert_eq!(restored.projection_assets, profile.projection_assets);

    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let names = (0..zip.len())
        .map(|index| zip.by_index(index).unwrap().name().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "fixture.json",
            "assets/model.glb",
            "assets/projections/top.svg",
            "assets/projections/left.svg",
            "assets/projections/right.svg",
            "assets/projections/front.svg",
            "assets/projections/back.svg",
        ]
    );
}

#[test]
fn rejects_active_or_external_svg_content() {
    for unsafe_svg in [
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\" width=\"1mm\" height=\"1mm\" data-tosklight-view=\"top\"><script/></svg>",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\" width=\"1mm\" height=\"1mm\" data-tosklight-view=\"top\"><path d=\"M 0 0 L 1 0 L 0 1 Z\" fill=\"#000000\" onerror=\"alert(1)\"/></svg>",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\" width=\"1mm\" height=\"1mm\" data-tosklight-view=\"top\"><image href=\"https://example.invalid/a.png\"/></svg>",
    ] {
        let mut profile = profile();
        let model = minimal_glb(false);
        profile.model_asset = Some(format!(
            "data:model/gltf-binary;base64,{}",
            STANDARD.encode(&model)
        ));
        let mut projections = projection_set(&model);
        projections.views[0].artwork_asset =
            format!("data:image/svg+xml;base64,{}", STANDARD.encode(unsafe_svg));
        profile.projection_assets = Some(projections);
        assert!(
            write_fixture_package(&profile).is_err(),
            "accepted {unsafe_svg}"
        );
    }
}

#[test]
fn rejects_projection_cache_metadata_after_the_model_changes() {
    let mut profile = profile();
    let model = minimal_glb(false);
    profile.model_asset = Some(format!(
        "data:model/gltf-binary;base64,{}",
        STANDARD.encode(&model)
    ));
    profile.projection_assets = Some(projection_set(&model));
    let mut changed = model;
    changed.push(0);
    profile.model_asset = Some(format!(
        "data:model/gltf-binary;base64,{}",
        STANDARD.encode(changed)
    ));
    let error = write_fixture_package(&profile).unwrap_err().to_string();
    assert!(error.contains("stale"), "unexpected error: {error}");
}

/// A gobo wheel is the only asset list in a package — every other asset is a single field — so
/// each slot has to reach its own canonical file and come back as its own data URL, in the slot
/// it was declared in.
#[test]
fn round_trips_a_gobo_wheel() {
    let mut profile = profile();
    profile.gobos = vec![
        crate::ProfileGobo {
            slot: 1,
            name: Some("Breakup".into()),
            artwork_asset: Some(format!("data:image/png;base64,{PNG_1X1}")),
        },
        crate::ProfileGobo {
            slot: 4,
            name: Some("Rings".into()),
            artwork_asset: Some(format!("data:image/png;base64,{PNG_1X1}")),
        },
        // A slot the manual names but nothing is etched on: still part of the wheel.
        crate::ProfileGobo {
            slot: 5,
            name: Some("Open".into()),
            artwork_asset: None,
        },
    ];

    let bytes = write_fixture_package(&profile).unwrap();
    let mut zip = ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
    let names = (0..zip.len())
        .map(|index| zip.by_index(index).unwrap().name().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        ["fixture.json", "assets/gobo-1.png", "assets/gobo-4.png"],
        "each slot is filed under the slot it is in"
    );

    let restored = read_fixture_package(&bytes).unwrap();
    assert_eq!(restored.gobos.len(), 3);
    assert_eq!(restored.gobos[0].slot, 1);
    assert_eq!(restored.gobos[1].slot, 4);
    assert_eq!(restored.gobos[2].artwork_asset, None);
    for gobo in restored.gobos.iter().take(2) {
        assert!(
            gobo.artwork_asset
                .as_deref()
                .is_some_and(|asset| asset.starts_with("data:image/png;base64,")),
            "slot {} did not come back as a data URL",
            gobo.slot
        );
    }
}

/// Two slots with the same number would silently lose one of them.
#[test]
fn rejects_a_wheel_with_a_slot_declared_twice() {
    let mut profile = profile();
    profile.gobos = vec![
        crate::ProfileGobo {
            slot: 2,
            name: Some("Breakup".into()),
            artwork_asset: None,
        },
        crate::ProfileGobo {
            slot: 2,
            name: Some("Rings".into()),
            artwork_asset: None,
        },
    ];
    let error = write_fixture_package(&profile).expect_err("a duplicated slot is invalid");
    assert!(format!("{error}").contains("declared twice"), "{error}");
}

/// A laser's scan engine is the only asset in a package that is source text rather than binary
/// media, and it is the only one whose loss would leave the fixture silently projecting nothing.
/// It has to survive the trip out to a canonical `assets/scan.js` and back to a data URL exactly
/// as the photograph and the model do.
#[test]
fn round_trips_a_laser_scan_script() {
    const SCRIPT: &str = "export function scan() { return { points: [] }; }\n";
    let mut profile = profile();
    profile.laser = Some(crate::ProfileLaser {
        scan_script_asset: Some(format!(
            "data:text/javascript;base64,{}",
            STANDARD.encode(SCRIPT)
        )),
        scan_angle_degrees: Some(50.0),
        points_per_second: Some(30_000.0),
        ..crate::ProfileLaser::default()
    });

    let bytes = write_fixture_package(&profile).unwrap();
    let mut zip = ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
    let names = (0..zip.len())
        .map(|index| zip.by_index(index).unwrap().name().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(names, ["fixture.json", "assets/scan.js"]);

    let restored = read_fixture_package(&bytes).unwrap();
    let laser = restored.laser.expect("the laser block must survive");
    assert_eq!(laser.scan_angle_degrees, Some(50.0));
    assert_eq!(laser.points_per_second, Some(30_000.0));
    let encoded = laser.scan_script_asset.expect("the script must survive");
    let payload = encoded
        .strip_prefix("data:text/javascript;base64,")
        .expect("the runtime form is a self-contained data URL");
    assert_eq!(
        String::from_utf8(STANDARD.decode(payload).unwrap()).unwrap(),
        SCRIPT
    );
}

/// A script that is not text cannot be compiled, and the package is the last place that can say so
/// before a laser fails in front of an audience.
#[test]
fn rejects_a_scan_script_that_is_not_text() {
    let mut profile = profile();
    profile.laser = Some(crate::ProfileLaser {
        scan_script_asset: Some(format!(
            "data:text/javascript;base64,{}",
            STANDARD.encode([0xff, 0xfe, 0x00])
        )),
        ..crate::ProfileLaser::default()
    });
    let error = write_fixture_package(&profile).unwrap_err().to_string();
    assert!(
        error.contains("scan script is not valid UTF-8"),
        "unexpected error: {error}"
    );
}

#[test]
fn round_trips_a_versioned_effect_script() {
    const SCRIPT: &str = "export function effect() { return { version: 1, emitters: [] }; }\n";
    let mut profile = profile();
    profile.fixture_type = "effect".into();
    profile.effect = Some(crate::ProfileEffect {
        effect_script_asset: Some(format!(
            "data:text/javascript;base64,{}",
            STANDARD.encode(SCRIPT)
        )),
        result_version: 1,
    });
    let bytes = write_fixture_package(&profile).unwrap();
    let mut zip = ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
    let names = (0..zip.len())
        .map(|index| zip.by_index(index).unwrap().name().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(names, ["fixture.json", "assets/effect.js"]);
    let restored = read_fixture_package(&bytes).unwrap();
    let effect = restored.effect.expect("effect contract survives");
    assert_eq!(effect.result_version, 1);
    let payload = effect
        .effect_script_asset
        .unwrap()
        .strip_prefix("data:text/javascript;base64,")
        .unwrap()
        .to_owned();
    assert_eq!(
        String::from_utf8(STANDARD.decode(payload).unwrap()).unwrap(),
        SCRIPT
    );
}

#[test]
fn shipped_effect_fixtures_keep_their_programs_and_exact_dmx_footprints() {
    for (filename, name, footprint) in [
        ("generic--cold-spark.toskfixture", "Cold Spark Fountain", 3),
        ("generic--flame-jet.toskfixture", "Flame Jet", 3),
        (
            "generic--five-nozzle-flame.toskfixture",
            "Five-nozzle Flame Unit",
            3,
        ),
    ] {
        let profile = shipped_profile(filename);
        assert_eq!(profile.fixture_type, "effect");
        assert_eq!(profile.name, name);
        assert_eq!(profile.modes[0].splits[0].footprint, footprint);
        assert_eq!(profile.modes[0].channels.len(), usize::from(footprint));
        assert!(
            profile
                .effect
                .as_ref()
                .and_then(|effect| effect.effect_script_asset.as_ref())
                .is_some()
        );
        let restored = read_fixture_package(&write_fixture_package(&profile).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(restored).unwrap(),
            serde_json::to_value(profile).unwrap()
        );
    }
}

#[test]
fn shipped_flame_jet_and_kabuki_keep_highlight_operator_safe() {
    let flame = shipped_profile("generic--flame-jet.toskfixture");
    let flame_intensity = &flame.modes[0].channels[0];
    assert_eq!(flame_intensity.default_raw, 0);
    assert_eq!(flame_intensity.highlight_raw, 0);
    assert!(flame.notes.contains("not a manufacturer hardware profile"));

    let kabuki = shipped_profile("generic--kabuki-curtain.toskfixture");
    let release = &kabuki.modes[0].channels[0];
    assert_eq!(release.default_raw, 0);
    assert_eq!(release.highlight_raw, 0);
}

#[test]
fn shipped_disco_ball_is_visual_only_geometry_with_no_dmx_channels() {
    let profile = shipped_profile("venue--disco-ball-50-cm.toskfixture");
    assert_eq!(profile.name, "Disco Ball 50 cm");
    assert_eq!(profile.patch_policy, PatchPolicy::VisualOnly);
    assert_eq!(profile.model_units, ModelUnits::Metres);
    assert_eq!(profile.modes.len(), 1);
    let mode = &profile.modes[0];
    assert_eq!(mode.name, "50 cm");
    assert_eq!(mode.splits[0].footprint, 0);
    assert!(mode.channels.is_empty());
    assert_eq!(
        mode.geometry
            .nodes
            .iter()
            .filter_map(|node| node.glb_node.as_deref())
            .collect::<Vec<_>>(),
        ["truss-coupler", "ball-core", "ball-tiles"]
    );
}

#[test]
fn shipped_kabuki_round_trips_its_portable_physics_contract() {
    let profile = shipped_profile("generic--kabuki-curtain.toskfixture");
    assert_eq!(profile.name, "Kabuki Curtain");
    assert_eq!(profile.modes[0].splits[0].footprint, 1);
    assert_eq!(profile.modes[0].channels.len(), 1);
    let physics = profile.physics.as_ref().expect("physics contract");
    assert_eq!(physics.result_version, 1);
    assert_eq!(physics.size_metres, [6.0, 5.0, 0.08]);
    assert!(physics.scenery_collision);
    assert!(!physics.self_collision);
    assert!(physics.control_script_asset.is_some());
    let restored = read_fixture_package(&write_fixture_package(&profile).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(profile).unwrap()
    );
}

#[test]
fn effect_scripts_are_utf8_and_only_belong_to_effect_fixtures() {
    let mut profile = profile();
    profile.effect = Some(crate::ProfileEffect {
        effect_script_asset: Some(format!(
            "data:text/javascript;base64,{}",
            STANDARD.encode([0xff, 0xfe])
        )),
        result_version: 1,
    });
    let error = write_fixture_package(&profile).unwrap_err().to_string();
    assert!(error.contains("only an Effect fixture"), "{error}");
    profile.fixture_type = "effect".into();
    let error = write_fixture_package(&profile).unwrap_err().to_string();
    assert!(
        error.contains("effect script is not valid UTF-8"),
        "{error}"
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

fn shipped_profiles() -> Vec<(String, FixtureProfile)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("assets/fixture-library");
    let mut names = fs::read_dir(directory)
        .unwrap()
        .filter_map(|entry| {
            let name = entry.unwrap().file_name().into_string().unwrap();
            name.ends_with(".toskfixture").then_some(name)
        })
        .collect::<Vec<_>>();
    names.sort();
    assert!(!names.is_empty(), "the shipped fixture library is empty");
    names
        .into_iter()
        .map(|name| {
            let profile = shipped_profile(&name);
            (name, profile)
        })
        .collect()
}

/// Every shipped lantern parks at the desk home look while nothing drives it: physical white and
/// centred position. The assertion runs against the projected canonical definition, so a
/// subtractive CMY head parked at raw zero has to read as full canonical Red, Green and Blue.
#[test]
fn shipped_library_homes_to_white_and_centred_position() {
    const WHITE: [&str; 6] = [
        "color.red",
        "color.green",
        "color.blue",
        "color.white",
        "color.cold_white",
        "color.warm_white",
    ];
    for (name, profile) in shipped_profiles() {
        for mode in &profile.modes {
            let definition = profile.resolved_definition(mode.id).unwrap();
            for head in &definition.heads {
                for parameter in &head.parameters {
                    let attribute = parameter.attribute.0.as_str();
                    let expected = if WHITE.contains(&attribute) {
                        1.0
                    } else if attribute == "color.saturation" {
                        0.0
                    } else if matches!(attribute, "pan" | "tilt")
                        && parameter.metadata.position_axis_representation
                            != Some(crate::PositionAxisRepresentation::Endless)
                    {
                        0.5
                    } else {
                        continue;
                    };
                    assert!(
                        (parameter.default - expected).abs() <= 1.0 / 255.0,
                        "{name} mode {} head {} parks {attribute} at {} instead of {expected}",
                        mode.name,
                        head.name,
                        parameter.default
                    );
                }
            }
        }
    }
}
