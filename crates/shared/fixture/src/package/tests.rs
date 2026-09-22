use super::*;
use crate::{
    CanonicalTransform, ChannelBehavior, ChannelResolution, ChannelScales, ColorSystem,
    EmitterLayout, FIXTURE_PROFILE_SCHEMA_VERSION, FixtureProfile, FixtureSplit, ModelUnits,
    PatchPolicy, PositionMovementRepresentation, ProfileSceneryKind, TrussPattern,
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

/// The library's two worked examples of a head's colour system: a colour wheel and additive
/// emitters, each bound to the fixture's own channels in every mode that has them.
#[test]
fn shipped_color_system_examples_bind_their_own_channels() {
    let spot = shipped_profile("cameo--auro-spot-z300.toskfixture");
    for mode in &spot.modes {
        let [head] = mode.color_systems.as_slice() else {
            panic!("{} should carry exactly one colour system", mode.name);
        };
        assert!(
            mode.heads
                .iter()
                .any(|candidate| candidate.id == head.head_id)
        );
        let ColorSystem::DiscreteWheel { channel_id, slots } = &head.system else {
            panic!("{} should be a colour wheel", mode.name);
        };
        let wheel = mode
            .channels
            .iter()
            .find(|channel| channel.id == *channel_id)
            .unwrap_or_else(|| panic!("{} binds a channel it does not have", mode.name));
        assert_eq!(&*wheel.attribute.0, "color.wheel.1");
        assert_eq!(slots.len(), 9);
        assert_eq!((slots[0].dmx_from, slots[0].dmx_to), (0, 5));
        assert!(
            slots
                .windows(2)
                .all(|pair| pair[0].dmx_to < pair[1].dmx_from),
            "{} wheel slots overlap",
            mode.name
        );
    }

    let par = shipped_profile("cameo--root-par-6.toskfixture");
    let mode = &par.modes[0];
    let [head] = mode.color_systems.as_slice() else {
        panic!("the Root PAR 6 should carry exactly one colour system");
    };
    let ColorSystem::Additive { emitters } = &head.system else {
        panic!("the Root PAR 6 should mix additively");
    };
    assert_eq!(
        emitters
            .iter()
            .map(|emitter| emitter.name.as_str())
            .collect::<Vec<_>>(),
        ["Red", "Green", "Blue", "White", "Amber", "UV"]
    );
    for (emitter, channel) in emitters.iter().zip(&mode.channels[..6]) {
        assert_eq!(
            emitter.channel_id, channel.id,
            "{} is bound out of order",
            emitter.name
        );
    }
    // Ultraviolet adds nothing a viewer sees, so it takes no part in the mix.
    assert!(!emitters[5].visible);
}

#[test]
fn suedbahnhof_plan_profiles_ship_with_the_explicit_venue_personalities() {
    let expected = [
        (
            "generic--dimmer-rgb-control-par.toskfixture",
            "Fixed 0, Red, Green, Blue, Fixed 0",
            5,
        ),
        ("cameo--auro-spot-z300.toskfixture", "17-Channel", 17),
        ("cameo--auro-spot-z300.toskfixture", "20-Channel", 20),
        (
            "cameo--root-par-6.toskfixture",
            "D7CH — Delay Off, virtual dimmer",
            7,
        ),
        ("martin--mac-300.toskfixture", "Mode 4", 13),
        ("martin--elp-cl-profile.toskfixture", "10-Channel", 10),
        ("martin--elp-ww-profile.toskfixture", "4-Channel", 4),
        (
            "prolights--ecl-fresnel-ct-plus-m.toskfixture",
            "STANDARD",
            11,
        ),
        (
            "claypaky--stage-zoom-1200.toskfixture",
            "16 bit - gobo fine - lamp control",
            20,
        ),
        (
            "claypaky--stage-zoom-1200-sv.toskfixture",
            "16 bit - gobo fine - lamp control",
            20,
        ),
    ];

    for (filename, mode_name, footprint) in expected {
        let profile = shipped_profile(filename);
        assert_eq!(profile.schema_version, FIXTURE_PROFILE_SCHEMA_VERSION);
        let mode = profile
            .modes
            .iter()
            .find(|mode| mode.name == mode_name)
            .unwrap_or_else(|| panic!("{filename} is missing {mode_name}"));
        assert_eq!(
            mode.splits,
            [FixtureSplit {
                number: 1,
                footprint
            }]
        );
        if ![
            "claypaky--stage-zoom-1200.toskfixture",
            "claypaky--stage-zoom-1200-sv.toskfixture",
            "generic--dimmer-rgb-control-par.toskfixture",
        ]
        .contains(&filename)
        {
            assert!(
                !profile.mode_geometry(mode).emitters.is_empty(),
                "{filename} must emit in Architect"
            );
        }
    }

    for (filename, mode_name, footprint) in [
        ("martin--mac-250-entour.toskfixture", "16 Bit", 15),
        ("robe--robin-300-ledwash.toskfixture", "Mode 3", 15),
        (
            "jb-lighting--jbled-a7.toskfixture",
            "Standard RGB 8 Bit (S8)",
            16,
        ),
        ("eurolite--ts-255-dmx-scan.toskfixture", "6-Channel", 6),
        ("generic--strobe.toskfixture", "Dimmer, Strobe", 2),
        ("generic--relay.toskfixture", "Off / On", 1),
    ] {
        let profile = shipped_profile(filename);
        let mode = profile
            .modes
            .iter()
            .find(|mode| mode.name == mode_name)
            .unwrap_or_else(|| panic!("{filename} is missing {mode_name}"));
        assert_eq!(mode.splits[0].footprint, footprint);
    }

    let root = shipped_profile("cameo--root-par-6.toskfixture");
    let root_mode = &root.modes[0];
    assert!(
        !root_mode
            .channels
            .iter()
            .any(|channel| channel.attribute.is_intensity())
    );
    assert_eq!(root_mode.channels.len(), 7);
    assert!(
        root_mode.channels[..6]
            .iter()
            .all(|channel| channel.reacts_to_virtual_intensity)
    );
    assert_eq!(
        &*root_mode.channels[6].fixture_attribute.0,
        "fixture.dmx_delay"
    );
    assert_eq!(root_mode.channels[6].default_raw, 0);

    let suedbahnhof_par = shipped_profile("generic--dimmer-rgb-control-par.toskfixture");
    assert_eq!(suedbahnhof_par.manufacturer, "Generic");
    assert_eq!(suedbahnhof_par.name, "Dimmer RGB Control PAR");
    assert_eq!(suedbahnhof_par.short_name, "LED PAR 56 SB");
    assert!(suedbahnhof_par.notes.contains("LED PAR 56 Suedbahnhof"));
    let suedbahnhof_mode = &suedbahnhof_par.modes[0];
    assert_eq!(
        suedbahnhof_mode
            .channels
            .iter()
            .map(|channel| &*channel.attribute.0)
            .collect::<Vec<_>>(),
        [
            "fixture.fixed_slot_1",
            "color.red",
            "color.green",
            "color.blue",
            "fixture.fixed_slot_5",
        ]
    );
    assert_eq!(
        suedbahnhof_mode
            .channels
            .iter()
            .map(|channel| channel.behavior)
            .collect::<Vec<_>>(),
        [
            ChannelBehavior::Static,
            ChannelBehavior::Controlled,
            ChannelBehavior::Controlled,
            ChannelBehavior::Controlled,
            ChannelBehavior::Static,
        ]
    );
    for channel in [&suedbahnhof_mode.channels[0], &suedbahnhof_mode.channels[4]] {
        assert_eq!((channel.default_raw, channel.highlight_raw), (0, 0));
        assert!(channel.functions.is_empty());
        assert!(!channel.invert);
        assert!(!channel.reacts_to_virtual_intensity);
        assert!(!channel.reacts_to_sequence_master);
        assert!(!channel.reacts_to_group_master);
        assert!(!channel.reacts_to_grand_master);
    }
    assert!(
        suedbahnhof_mode.channels[1..4]
            .iter()
            .all(|channel| channel.reacts_to_virtual_intensity)
    );
    assert_eq!(
        suedbahnhof_mode
            .primary_slots()
            .unwrap()
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>(),
        suedbahnhof_mode
            .channels
            .iter()
            .enumerate()
            .map(|(index, channel)| (channel.id, index as u16 + 1))
            .collect()
    );

    let values = std::collections::HashMap::from([
        (
            light_core::AttributeKey("color.red".into()),
            light_core::AttributeValue::Normalized(128.0 / 255.0),
        ),
        (
            light_core::AttributeKey("color.green".into()),
            light_core::AttributeValue::Normalized(64.0 / 255.0),
        ),
        (
            light_core::AttributeKey("color.blue".into()),
            light_core::AttributeValue::Normalized(32.0 / 255.0),
        ),
    ]);
    let resolved = suedbahnhof_mode
        .channels
        .iter()
        .map(|channel| {
            (
                channel.id,
                suedbahnhof_mode.resolve_channel_raw(
                    channel,
                    &values,
                    false,
                    None,
                    ChannelScales::default(),
                ),
            )
        })
        .collect::<Vec<_>>();
    let mut frame = [0xff; 512];
    suedbahnhof_mode
        .compile_encoding_plan()
        .unwrap()
        .encode_split(&mut frame, 1, 1, &resolved)
        .unwrap();
    assert_eq!(&frame[..5], &[0, 128, 64, 32, 0]);

    let round_tripped =
        read_fixture_package(&write_fixture_package(&suedbahnhof_par).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(round_tripped).unwrap(),
        serde_json::to_value(suedbahnhof_par).unwrap()
    );

    let stage_zoom = shipped_profile("claypaky--stage-zoom-1200.toskfixture");
    let venue_mode = stage_zoom
        .modes
        .iter()
        .find(|mode| mode.name == "16 bit - gobo fine - lamp control")
        .unwrap();
    let gobo = venue_mode
        .channels
        .iter()
        .find(|channel| &*channel.fixture_attribute.0 == "gobo.1")
        .unwrap();
    assert_eq!(gobo.resolution, ChannelResolution::U16);
    assert_eq!(gobo.secondary_slots, [20]);

    let q_spot = shipped_profile("cameo--q-spot-40-tw.toskfixture");
    assert_eq!(
        q_spot
            .modes
            .iter()
            .map(|mode| (mode.name.as_str(), mode.splits[0].footprint))
            .collect::<Vec<_>>(),
        [
            ("1-CHANNEL", 1),
            ("2-CHANNEL", 2),
            ("3-CHANNEL 1", 3),
            ("3-CHANNEL 2", 3),
            ("8-CHANNEL", 8),
        ]
    );
}

fn compatibility_profile(filename: &str) -> FixtureProfile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(filename);
    read_fixture_package(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn requested_generic_and_venue_packages_have_exact_portable_contracts() {
    // Three fixtures, each with the personalities that lamp count actually has.
    for (file, modes) in [
        (
            "generic--blinder-2.toskfixture",
            vec!["One channel", "Two channel"],
        ),
        (
            "generic--blinder-4.toskfixture",
            vec!["One channel", "Two channel"],
        ),
        (
            "generic--blinder-8.toskfixture",
            vec!["One channel", "Two channel", "Four channel"],
        ),
    ] {
        let blinder = shipped_profile(file);
        assert_eq!(
            blinder
                .modes
                .iter()
                .map(|mode| mode.name.as_str())
                .collect::<Vec<_>>(),
            modes,
            "{file}"
        );
        assert!(blinder.geometry.emitters.iter().all(|emitter| {
            !emitter.directional
                && emitter.orientation_degrees.x == 90.0
                && emitter.origin.z == -126.0
                && matches!(emitter.layout, EmitterLayout::ExplicitPixels { .. })
        }));
        for mode in &blinder.modes {
            assert!(mode.heads.iter().all(|head| !head.master_shared));
            assert_eq!(mode.heads.len(), mode.channels.len());
            assert_eq!(mode.splits[0].footprint as usize, mode.heads.len());
            assert!(mode.channels.iter().all(|channel| {
                channel.attribute.is_intensity()
                    && channel.resolution == ChannelResolution::U8
                    && channel.highlight_raw == 255
            }));
        }
    }
    // The four-lamp grid is the one the demo show is built on, and it did not move in the split.
    let lens_centres = shipped_profile("generic--blinder-4.toskfixture")
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

    // Every one of these is generated at the size it is placed, so it has one mode rather than a
    // mode per measurement, and carries no model made for one size.
    let generated = [
        (
            "venue--stage-element-1-1-m.toskfixture",
            ProfileSceneryKind::Riser,
            0,
        ),
        (
            "venue--stage-element-2-1-m.toskfixture",
            ProfileSceneryKind::Riser,
            0,
        ),
        (
            "venue--stage-element-1-0-5-m.toskfixture",
            ProfileSceneryKind::Riser,
            0,
        ),
        (
            "venue--stage-stairs.toskfixture",
            ProfileSceneryKind::Riser,
            0,
        ),
        (
            "venue--four-point-truss.toskfixture",
            ProfileSceneryKind::Truss,
            4,
        ),
        (
            "venue--three-point-truss.toskfixture",
            ProfileSceneryKind::Truss,
            3,
        ),
        (
            "venue--two-point-truss.toskfixture",
            ProfileSceneryKind::Truss,
            2,
        ),
        (
            "venue--one-point-truss-pipe.toskfixture",
            ProfileSceneryKind::Truss,
            1,
        ),
        (
            "venue--three-point-deco-truss.toskfixture",
            ProfileSceneryKind::Truss,
            3,
        ),
        (
            "venue--large-four-point-truss.toskfixture",
            ProfileSceneryKind::Truss,
            4,
        ),
        ("venue--chain.toskfixture", ProfileSceneryKind::Chain, 0),
        ("venue--curtain.toskfixture", ProfileSceneryKind::Curtain, 0),
        (
            "venue--curtain-1-m.toskfixture",
            ProfileSceneryKind::Curtain,
            0,
        ),
        (
            "venue--curtain-2-m.toskfixture",
            ProfileSceneryKind::Curtain,
            0,
        ),
        (
            "venue--curtain-3-m.toskfixture",
            ProfileSceneryKind::Curtain,
            0,
        ),
        (
            "venue--curtain-5-m.toskfixture",
            ProfileSceneryKind::Curtain,
            0,
        ),
        (
            "venue--curtain-6-m.toskfixture",
            ProfileSceneryKind::Curtain,
            0,
        ),
        ("venue--box.toskfixture", ProfileSceneryKind::Box, 0),
        (
            "venue--cylinder.toskfixture",
            ProfileSceneryKind::Cylinder,
            0,
        ),
        ("venue--ball.toskfixture", ProfileSceneryKind::Sphere, 0),
    ];
    for (filename, kind, chords) in generated {
        let profile = shipped_profile(filename);
        assert_eq!(profile.manufacturer, "Venue", "{filename}");
        assert_eq!(profile.patch_policy, PatchPolicy::VisualOnly, "{filename}");
        assert_eq!(profile.modes.len(), 1, "{filename}");
        assert!(profile.model_asset.is_none(), "{filename}");
        assert!(profile.projection_assets.is_none(), "{filename}");
        let scenery = profile.scenery.expect(filename);
        assert_eq!(scenery.kind, kind, "{filename}");
        assert_eq!(scenery.chords, chords, "{filename}");
        // Something is adjustable, or it would not need generating.
        let axes = scenery.adjustable;
        assert!(axes.width || axes.height || axes.depth, "{filename}");
    }

    let venue = [
        ("venue--stage-railing-2-m.toskfixture", 1),
        ("venue--disco-ball-50-cm.toskfixture", 1),
    ];
    for filename in [
        "venue--four-point-truss.toskfixture",
        "venue--three-point-truss.toskfixture",
        "venue--two-point-truss.toskfixture",
        "venue--one-point-truss-pipe.toskfixture",
        "venue--three-point-deco-truss.toskfixture",
        "venue--large-four-point-truss.toskfixture",
        "venue--chain.toskfixture",
    ] {
        assert_eq!(shipped_profile(filename).fixture_type, "rigging");
    }

    // A deco truss says so; every other truss is braced the standard way.
    let deco = shipped_profile("venue--three-point-deco-truss.toskfixture");
    assert_eq!(deco.scenery.expect("deco").pattern, TrussPattern::Deco);
    let standard = shipped_profile("venue--three-point-truss.toskfixture");
    assert_eq!(
        standard.scenery.expect("standard").pattern,
        TrussPattern::Standard
    );
    // The standard family is one product: every section is the 290 mm envelope the shipped
    // corner blocks are built to, so a corner joins a straight run flush.
    for filename in [
        "venue--four-point-truss.toskfixture",
        "venue--three-point-truss.toskfixture",
        "venue--two-point-truss.toskfixture",
        "venue--three-point-deco-truss.toskfixture",
    ] {
        let scenery = shipped_profile(filename).scenery.expect(filename);
        for size in [
            scenery.default_size_metres,
            scenery.minimum_size_metres,
            scenery.maximum_size_metres,
        ] {
            assert_eq!(size.y, 0.29, "{filename}");
            assert_eq!(size.z, 0.29, "{filename}");
        }
    }
    // A large truss is braced in its own, deeper section.
    let large = shipped_profile("venue--large-four-point-truss.toskfixture")
        .scenery
        .expect("large truss");
    assert!(large.default_size_metres.y > 0.29 && large.default_size_metres.z > 0.29);

    // A stage element is the base it is built on; only its rise is made to measure.
    for filename in [
        "venue--stage-element-1-1-m.toskfixture",
        "venue--stage-element-2-1-m.toskfixture",
        "venue--stage-element-1-0-5-m.toskfixture",
    ] {
        let scenery = shipped_profile(filename).scenery.expect(filename);
        assert!(
            !scenery.adjustable.width && scenery.adjustable.height && !scenery.adjustable.depth,
            "{filename}"
        );
    }

    // A chain is set by its length alone; its ends are chosen per placed chain.
    let chain = shipped_profile("venue--chain.toskfixture")
        .scenery
        .expect("chain");
    assert!(!chain.adjustable.width && chain.adjustable.height && !chain.adjustable.depth);
}

#[test]
fn shipped_truss_corners_and_legged_decks_carry_their_catalogue_models() {
    // A corner block and a deck on fixed legs are single parts, not made to measure, so each
    // is its own visual-only profile with the shipped model and its catalogue render.
    let parts = [
        ("Corner 2-Way", "corner-2-way"),
        ("T-Piece 3-Way", "t-piece-3-way"),
        ("Corner 3-Way Down", "corner-3-way-down"),
        ("Cross 4-Way", "cross-4-way"),
        ("T-Piece 4-Way Down", "t-piece-4-way-down"),
        ("Cross 5-Way Down", "cross-5-way-down"),
        ("Node 6-Way", "node-6-way"),
    ];
    let mut expected = Vec::new();
    for (section, slug) in [("Three-Point", "three-point"), ("Four-Point", "four-point")] {
        for (part, part_slug) in parts {
            expected.push((
                format!("venue--{slug}-truss-{part_slug}.toskfixture"),
                format!("{section} Truss {part}"),
                "rigging",
            ));
        }
    }
    for (size, size_slug) in [
        ("1 × 0.5 m", "1-0-5-m"),
        ("1 × 1 m", "1-1-m"),
        ("2 × 1 m", "2-1-m"),
    ] {
        for (legs, legs_slug) in [
            ("0.2 m", "0-2-m"),
            ("0.4 m", "0-4-m"),
            ("0.6 m", "0-6-m"),
            ("0.8 m", "0-8-m"),
            ("1 m", "1-m"),
        ] {
            expected.push((
                format!("venue--stage-deck-{size_slug}-legs-{legs_slug}.toskfixture"),
                format!("Stage Deck {size}, Legs {legs}"),
                "venue",
            ));
        }
    }
    assert_eq!(expected.len(), 29);
    let mut identities = std::collections::HashSet::new();
    for (filename, name, fixture_type) in &expected {
        let profile = shipped_profile(filename);
        assert_eq!(profile.manufacturer, "Venue", "{filename}");
        assert_eq!(&profile.name, name, "{filename}");
        assert_eq!(profile.fixture_type, *fixture_type, "{filename}");
        assert_eq!(profile.patch_policy, PatchPolicy::VisualOnly, "{filename}");
        assert_eq!(profile.model_units, ModelUnits::Metres, "{filename}");
        assert!(profile.scenery.is_none(), "{filename}");
        assert!(identities.insert(profile.id), "{filename}");
        let model = profile.model_asset.as_deref().expect(filename);
        assert!(model.starts_with("data:model/gltf-binary"), "{filename}");
        let photograph = profile.photograph_asset.as_deref().expect(filename);
        assert!(photograph.starts_with("data:image/png"), "{filename}");
        assert_eq!(profile.modes.len(), 1, "{filename}");
        let mode = &profile.modes[0];
        assert_eq!(mode.name, "Default", "{filename}");
        assert_eq!(mode.splits.len(), 1, "{filename}");
        assert_eq!(mode.splits[0].footprint, 0, "{filename}");
        assert!(mode.channels.is_empty(), "{filename}");
    }
    // Every corner block is 500 mm overall wherever it has arms, the way the real hardware is
    // sold, so adding arms makes a block busier and not bigger.
    for (filename, ..) in expected.iter().filter(|(name, ..)| name.contains("truss")) {
        let corner = shipped_profile(filename);
        assert_eq!(corner.physical.width_millimetres, Some(500.0), "{filename}");
        assert_eq!(corner.physical.depth_millimetres, Some(500.0), "{filename}");
    }
    let corner = shipped_profile("venue--four-point-truss-corner-2-way.toskfixture");
    assert_eq!(corner.physical.height_millimetres, Some(290.0));
    let node = shipped_profile("venue--four-point-truss-node-6-way.toskfixture");
    assert_eq!(node.physical.height_millimetres, Some(500.0));
    let deck = shipped_profile("venue--stage-deck-2-1-m-legs-0-4-m.toskfixture");
    assert_eq!(deck.physical.width_millimetres, Some(2000.0));
    assert_eq!(deck.physical.height_millimetres, Some(440.0));
    assert_eq!(deck.physical.depth_millimetres, Some(1000.0));
}

#[test]
fn shipped_fixture_library_uses_built_in_type_icons() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("assets/fixture-library");
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toskfixture") {
            continue;
        }
        let profile = read_fixture_package(&fs::read(&path).unwrap()).unwrap();
        assert!(
            profile.stage_icon_asset.is_none(),
            "{} must use its built-in fixture type icon",
            path.display()
        );
    }
}

#[test]
fn shipped_control_fixtures_leave_models_to_renderer_defaults() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("assets/fixture-library");
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toskfixture") {
            continue;
        }
        let profile = read_fixture_package(&fs::read(&path).unwrap()).unwrap();
        if profile.patch_policy == PatchPolicy::VisualOnly {
            continue;
        }
        assert!(
            profile.model_asset.is_none(),
            "{} must use the renderer-owned default model selected from fixture semantics",
            path.display()
        );
        assert!(
            profile.projection_assets.is_none(),
            "{} must derive plan views from that renderer-owned default model",
            path.display()
        );
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
    assert_eq!(acl.geometry.nodes.len(), 1);
    assert_eq!(acl.geometry.nodes[0].glb_node.as_deref(), Some("acl-body"));
    assert_eq!(acl.geometry.emitters.len(), 1);
    assert_eq!(acl.geometry.emitters[0].origin.y, -192.0);
    assert!(
        acl.modes
            .iter()
            .all(|mode| { acl.mode_geometry(mode).emitters.len() == 1 })
    );

    let fresnel = shipped_profile("generic--dimmer-fresnel.toskfixture");
    assert_eq!(fresnel.model_units, ModelUnits::Metres);
    assert_eq!(fresnel.geometry.nodes.len(), 1);
    assert_eq!(
        fresnel.geometry.nodes[0].glb_node.as_deref(),
        Some("fresnel-body")
    );
    assert!(fresnel.modes.iter().all(|mode| {
        fresnel.mode_geometry(mode).emitters.len() == 1
            && fresnel.geometry.emitters[0].origin.y == -694.0
            && fresnel.geometry.emitters[0].orientation_degrees == crate::Vector3::default()
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
    assert_eq!(profile.revision, 3);
    assert!(profile.notes.contains("JBLED_A7_DMX_Protocol.pdf"));
    assert_eq!(profile.modes.len(), 4);
    for mode in &profile.modes {
        let shutter = mode
            .channels
            .iter()
            .find(|channel| *channel.attribute.0 == *"shutter")
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
fn shipped_auxiliary_controls_do_not_drive_the_optical_parameter() {
    for (filename, physical, auxiliary, optical) in [
        (
            "chauvet-professional--colorado-1-solo.toskfixture",
            "fixture.zoom_control",
            "fixture.reset",
            "zoom",
        ),
        (
            "chauvet-professional--colorado-1-solo.toskfixture",
            "fixture.dimmer_speed",
            "fixture.control",
            "intensity",
        ),
        (
            "robe--robin-dlf-wash.toskfixture",
            "fixture.wide_zoom",
            "fixture.function",
            "zoom",
        ),
        (
            "robe--robin-dls-profile.toskfixture",
            "fixture.autofocus",
            "fixture.function",
            "focus",
        ),
    ] {
        let profile = shipped_profile(filename);
        let decoded = read_fixture_package(&write_fixture_package(&profile).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&decoded).unwrap(),
            serde_json::to_value(&profile).unwrap()
        );
        for mode in &profile.modes {
            let channel = mode
                .channels
                .iter()
                .find(|channel| channel.fixture_attribute.0.as_ref() == physical)
                .unwrap();
            assert_eq!(channel.attribute.0.as_ref(), auxiliary);
            assert!(
                channel
                    .functions
                    .iter()
                    .all(|function| function.attribute.0.as_ref() == auxiliary)
            );
            assert!(!channel.reacts_to_grand_master);
            assert!(!channel.reacts_to_sequence_master);
            assert!(!channel.reacts_to_group_master);
            assert!(!channel.reacts_to_virtual_intensity);
            assert_eq!(
                mode.channels
                    .iter()
                    .filter(|channel| channel.attribute.0.as_ref() == optical)
                    .count(),
                1
            );
            let definition = profile.resolved_definition(mode.id).unwrap();
            let parameters = &definition.heads[0].parameters;
            assert_eq!(
                parameters
                    .iter()
                    .filter(|parameter| parameter.attribute.0.as_ref() == optical)
                    .count(),
                1
            );
            assert_eq!(
                parameters
                    .iter()
                    .filter(|parameter| parameter.attribute.0.as_ref() == auxiliary)
                    .count(),
                1
            );
        }
    }
}

#[test]
fn shipped_sharpy_vector_colour_time_is_independent_of_colour_selection() {
    let profile = shipped_profile("claypaky--sharpy.toskfixture");
    let mode = profile
        .modes
        .iter()
        .find(|mode| mode.name == "Vector")
        .unwrap();
    let channel = mode
        .channels
        .iter()
        .find(|channel| channel.fixture_attribute.0.as_ref() == "fixture.colour_time")
        .unwrap();
    assert_eq!(channel.attribute.0.as_ref(), "fixture.effects_speed");
    assert_eq!(
        channel.id.to_string(),
        "d29a1659-da77-f734-6b16-e5d46377c8c9"
    );
    assert_eq!(
        mode.channels
            .iter()
            .filter(|channel| channel.attribute.0.as_ref() == "color.wheel.1")
            .count(),
        1
    );
    let decoded = read_fixture_package(&write_fixture_package(&profile).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(decoded).unwrap(),
        serde_json::to_value(profile).unwrap()
    );
}

#[test]
fn shipped_dls_zoom_preserves_raw_travel_and_declares_descending_beam_width() {
    let profile = shipped_profile("robe--robin-dls-profile.toskfixture");
    for mode in &profile.modes {
        let zoom = mode
            .channels
            .iter()
            .find(|channel| channel.attribute.0.as_ref() == "zoom")
            .unwrap();
        assert_eq!((zoom.physical_min, zoom.physical_max), (None, None));
        assert_eq!(zoom.unit, None);
        assert!(!zoom.invert);
        assert_eq!(zoom.canonical_transform, CanonicalTransform::Identity);
        assert_eq!(zoom.default_raw, 0);
        assert_eq!(zoom.highlight_raw, 0);
        assert_eq!(zoom.functions.len(), 1);
        for normalized in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
            let values = std::collections::HashMap::from([(
                zoom.attribute.clone(),
                light_core::AttributeValue::Normalized(normalized),
            )]);
            assert_eq!(
                mode.resolve_channel_raw(
                    zoom,
                    &values,
                    false,
                    None,
                    crate::ChannelScales::default()
                ),
                (normalized * zoom.resolution.max_raw() as f32).round() as u32
            );
        }

        let crate::ChannelFunctionBehavior::Continuous {
            physical_min,
            physical_max,
            unit,
        } = &zoom.functions[0].behavior
        else {
            panic!("Zoom is continuous");
        };
        assert_eq!((*physical_min, *physical_max), (1.0, 0.0));
        assert_eq!(*unit, None);
        let expected_max = if mode.name == "Mode 1" { 65535 } else { 255 };
        assert_eq!(
            (zoom.functions[0].dmx_from, zoom.functions[0].dmx_to),
            (0, expected_max)
        );
        assert_eq!(
            zoom.secondary_slots,
            if mode.name == "Mode 1" {
                vec![30]
            } else {
                vec![]
            }
        );
    }
}

/// Every play-mode range the desk decodes, named, in the order `PlayMode::from_dmx` reads them.
/// A shipped Media personality must name all of them, so the desk shows a mode rather than a
/// percentage and the raw value each mode sends stays exactly what the runtime already decodes.
fn assert_play_mode_is_named(functions: &[crate::ChannelFunction]) {
    let expected = [
        (0_u32, 19_u32, "loop"),
        (20, 39, "reverse"),
        (40, 59, "bounce"),
        (60, 67, "once_hold"),
        (68, 75, "once_black"),
        (76, 83, "once_transparent"),
        (84, 91, "reverse_once_hold"),
        (92, 99, "reverse_once_black"),
        (100, 107, "reverse_once_transparent"),
        (108, 127, "loop_synced"),
        (128, 147, "reverse_synced"),
        (148, 167, "bounce_synced"),
        (168, 175, "once_synced_hold"),
        (176, 183, "once_synced_black"),
        (184, 191, "once_synced_transparent"),
        (192, 199, "reverse_once_synced_hold"),
        (200, 207, "reverse_once_synced_black"),
        (208, 215, "reverse_once_synced_transparent"),
        (216, 235, "stop"),
        (236, 255, "pause"),
    ];
    assert_eq!(functions.len(), expected.len(), "every play mode is named");
    for (function, (from, to, semantic)) in functions.iter().zip(expected) {
        assert_eq!(function.dmx_from, from);
        assert_eq!(function.dmx_to, to);
        match &function.behavior {
            crate::ChannelFunctionBehavior::Fixed {
                semantic_id,
                raw_value,
                label,
            } => {
                assert_eq!(semantic_id, semantic);
                assert!(
                    !label.trim().is_empty(),
                    "{semantic} needs an operator label"
                );
                assert_eq!(
                    u32::from(*raw_value),
                    from,
                    "{semantic} sends the first raw value of its own range"
                );
            }
            other => panic!("{semantic} must be a named fixed function, found {other:?}"),
        }
    }
}

#[test]
fn shipped_three_d_point_is_a_zero_dmx_reference_object_on_six_position_axes() {
    let profile = shipped_profile("tosklight--3d-point.toskfixture");
    assert_eq!(profile.manufacturer, "ToskLight");
    assert_eq!(profile.name, "3D Point");
    // A point is never patched to a universe: it is a reference an operator moves, not an output.
    assert_eq!(profile.patch_policy, PatchPolicy::Internal);
    assert_eq!(profile.modes.len(), 1);
    let mode = &profile.modes[0];
    assert_eq!(
        mode.splits,
        [FixtureSplit {
            number: 1,
            footprint: 0
        }]
    );
    assert_eq!(mode.heads.len(), 1);
    assert_eq!(
        mode.channels
            .iter()
            .map(|channel| &*channel.attribute.0)
            .collect::<Vec<_>>(),
        [
            "point.position.x",
            "point.position.y",
            "point.position.z",
            "point.rotation.x",
            "point.rotation.y",
            "point.rotation.z",
        ]
    );
    // Every axis rests at the centre of its range, so a freshly patched point sits exactly on
    // its own origin rather than jumping the rig the moment it is added.
    for channel in &mode.channels {
        assert_eq!(channel.default_raw, 8_388_608);
        assert_eq!(channel.highlight_raw, 8_388_608);
        assert!(channel.secondary_slots.is_empty());
    }
    // It emits no light, so nothing about it reacts to a master.
    for channel in &mode.channels {
        assert!(!channel.reacts_to_virtual_intensity);
        assert!(!channel.reacts_to_grand_master);
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
            .map(|channel| &*channel.attribute.0)
            .collect::<Vec<_>>(),
        ["media.folder", "media.file", "media.play_mode", "volume",]
    );
    // TL-367: the voice is addressed through the canonical Media attributes, so the Media encoder
    // group and Media pane control it exactly like any other media source. Play mode carries both
    // transport and repeat, and Stop is the safe patched default.
    assert_eq!(
        mode.channels
            .iter()
            .map(|channel| &*channel.fixture_attribute.0)
            .collect::<Vec<_>>(),
        [
            "audio.folder",
            "audio.file",
            "audio.play_mode",
            "audio.volume",
        ]
    );
    assert_eq!(
        mode.channels
            .iter()
            .map(|channel| (&*channel.attribute.0, channel.default_raw))
            .collect::<Vec<_>>(),
        [
            ("media.folder", 0),
            ("media.file", 0),
            ("media.play_mode", 216),
            ("volume", 255),
        ]
    );
    assert!(
        mode.channels
            .iter()
            .all(|channel| channel.secondary_slots.is_empty())
    );
    // TL-371: play mode names every mode it can be in, so an operator reads Stop or Loop on the
    // encoder instead of a percentage and can program a mode by name.
    assert_play_mode_is_named(
        &mode
            .channels
            .iter()
            .find(|channel| *channel.attribute.0 == *"media.play_mode")
            .expect("the Audio Player carries a play mode")
            .functions,
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

#[test]
fn shipped_stage_railing_has_portable_visual_only_geometry() {
    let profile = shipped_profile("venue--stage-railing-2-m.toskfixture");
    assert_eq!(profile.name, "Stage Railing 2 m");
    assert_eq!(profile.patch_policy, PatchPolicy::VisualOnly);
    assert_eq!(profile.model_units, ModelUnits::Metres);
    assert_eq!(profile.physical.width_millimetres, Some(2040.0));
    assert_eq!(profile.physical.height_millimetres, Some(1000.0));
    assert_eq!(profile.physical.depth_millimetres, Some(38.0));
    assert_eq!(profile.modes.len(), 1);
    assert_eq!(profile.modes[0].name, "2 m");
    assert_eq!(profile.modes[0].splits[0].footprint, 0);
    assert!(profile.modes[0].channels.is_empty());
    assert!(profile.model_asset.is_some());
    assert!(profile.notes.contains("not a structural certification"));
}

fn assert_moving_lamp_geometry(filename: &str) {
    let mover = shipped_profile(filename);
    assert_eq!(mover.model_units, ModelUnits::Metres);
    assert!(mover.model_asset.is_none());
    assert!(mover.projection_assets.is_none());
    // The yoke belongs to the lantern, so it is read from the fixture; each mode only says which
    // of its heads the emitter on the head node belongs to.
    let nodes = &mover.geometry.nodes;
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0].glb_node.as_deref(), Some("moving-base"));
    assert_eq!(nodes[1].glb_node.as_deref(), Some("moving-yoke"));
    assert_eq!(nodes[2].glb_node.as_deref(), Some("moving-head"));
    assert!(nodes[0].motion.is_none());
    // The fixture's axes name no attribute: each mode binds pan to the yoke and tilt to the head.
    assert!(nodes[1..].iter().all(|node| {
        node.motion
            .as_ref()
            .is_some_and(|motion| motion.attribute.is_none())
    }));
    let motion_attribute = |graph: &crate::GeometryGraph, index: usize| {
        graph.nodes[index]
            .motion
            .as_ref()
            .and_then(|motion| motion.attribute.clone())
            .map(|attribute| attribute.0.to_string())
    };
    for mode in &mover.modes {
        let bound = mover.mode_geometry(mode);
        assert_eq!(motion_attribute(&bound, 1).as_deref(), Some("pan"));
        assert_eq!(motion_attribute(&bound, 2).as_deref(), Some("tilt"));
    }
    assert!(nodes[2].transform.translation.y < 0.0);
    assert_eq!(mover.geometry.emitters.len(), 1);
    assert_eq!(mover.geometry.emitters[0].node_id, nodes[2].id);
    assert!(mover.geometry.emitters[0].origin.y < 0.0);
    assert!(
        mover
            .modes
            .iter()
            .all(|mode| mover.mode_geometry(mode).emitters.len() == 1)
    );
}

#[test]
fn robe_dls_profile_exposes_canonical_framing_controls() {
    let profile = shipped_profile("robe--robin-dls-profile.toskfixture");
    assert_eq!(profile.revision, 6);
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
            .find(|channel| *channel.attribute.0 == *"shaper.rotation")
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
                .find(|channel| &*channel.attribute.0 == position_attribute)
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
                .find(|channel| &*channel.attribute.0 == angle_attribute)
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
            .find(|channel| *channel.attribute.0 == *"shutter")
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
        // Each pixel owns its own colour system, bound to its own three channels in chart order.
        let systems = mode
            .color_systems
            .iter()
            .filter(|system| system.head_id == head.id)
            .collect::<Vec<_>>();
        let [system] = systems.as_slice() else {
            panic!("{} should carry exactly one colour system", head.name);
        };
        let ColorSystem::Additive { emitters } = &system.system else {
            panic!("{} should mix additively", head.name);
        };
        assert_eq!(
            emitters
                .iter()
                .map(|emitter| (emitter.name.as_str(), emitter.channel_id, emitter.visible))
                .collect::<Vec<_>>(),
            [
                ("Red", channels[0].id, true),
                ("Green", channels[1].id, true),
                ("Blue", channels[2].id, true)
            ],
            "{} binds its own channels",
            head.name
        );
    }
    assert_eq!(mode.color_systems.len(), 10);
    assert_eq!(profile.revision, 2);

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
            let expected = match &*channel.fixture_attribute.0 {
                "color.cyan" => Some("color.red"),
                "color.magenta" => Some("color.green"),
                "color.yellow" => Some("color.blue"),
                _ => None,
            };
            if let Some(expected) = expected {
                assert_eq!(&*channel.attribute.0, expected);
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
                    .filter(|channel| &*channel.fixture_attribute.0 == fixture_attribute)
                    .collect::<Vec<_>>();
                assert_eq!(channels.len(), 1, "{filename} / {}", mode.name);
                let channel = channels[0];
                assert_eq!(&*channel.attribute.0, canonical);
                assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);
                assert!(
                    channel
                        .functions
                        .iter()
                        .all(|function| &*function.attribute.0 == canonical)
                );
            }
            assert!(
                mode.channels
                    .iter()
                    .all(|channel| *channel.attribute.0 != *"color.cold_white"
                        && *channel.attribute.0 != *"color.warm_white")
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
        assert_eq!(
            profile.revision,
            if filename.starts_with("chauvet-") {
                4
            } else {
                3
            }
        );
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
            .filter(|channel| *channel.fixture_attribute.0 == *"strobe")
            .collect::<Vec<_>>();
        assert!(
            !channels.is_empty(),
            "{filename} must retain physical strobe"
        );
        assert!(channels.iter().all(|channel| {
            *channel.attribute.0 == *"shutter"
                && channel.canonical_transform == CanonicalTransform::Identity
                && channel
                    .functions
                    .iter()
                    .all(|function| *function.attribute.0 == *"shutter")
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
                .filter(|channel| *channel.fixture_attribute.0 == *"frost")
                .collect::<Vec<_>>();
            if channels.is_empty() {
                continue;
            }
            affected_modes += 1;
            assert_eq!(channels.len(), 1, "{filename} / {}", mode.name);
            let channel = channels[0];
            assert_eq!(&*channel.attribute.0, "softness");
            assert_eq!(channel.canonical_transform, CanonicalTransform::Identity);
            assert!(
                channel
                    .functions
                    .iter()
                    .all(|function| *function.attribute.0 == *"softness")
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
                &*channel.fixture_attribute.0,
                &*channel.attribute.0,
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
                &*channel.fixture_attribute.0,
                &*channel.attribute.0,
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
            .map(|channel| (&*channel.fixture_attribute.0, &*channel.attribute.0,))
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
            .map(|channel| &*channel.attribute.0)
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
        *channel.fixture_attribute.0 == *"color.temperature"
            && *channel.attribute.0 == *"color.temperature"
    }));
    assert!(studio.channels.iter().any(|channel| {
        *channel.fixture_attribute.0 == *"fixture.tint" && *channel.attribute.0 == *"color.tint"
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
                &*channel.fixture_attribute.0,
                &*channel.attribute.0,
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
    assert_eq!(profile.revision, 9);
    assert_eq!(
        profile.direct_control_protocols,
        vec![crate::DirectControlProtocol::Citp],
        "the shipped profile must permit the desk to store its CITP endpoint"
    );
    assert_eq!(profile.modes.len(), 2);

    for (mode, layer_count, footprint) in [
        (&profile.modes[0], 2_usize, 158_u16),
        (&profile.modes[1], 8_usize, 512_u16),
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

        for (attribute, count) in [
            ("media.folder", layer_count),
            ("media.file", layer_count),
            ("media.mask.folder", layer_count),
            ("media.mask.file", layer_count + 1),
        ] {
            assert_eq!(
                mode.channels
                    .iter()
                    .filter(|channel| &*channel.attribute.0 == attribute)
                    .count(),
                count,
                "{attribute} must use the desk's canonical programmer attribute"
            );
        }

        for head in &mode.heads {
            let channels = mode
                .channels
                .iter()
                .filter(|channel| channel.head_id == head.id)
                .collect::<Vec<_>>();
            let channel = |attribute: &str| {
                channels
                    .iter()
                    .copied()
                    .find(|channel| &*channel.attribute.0 == attribute)
                    .unwrap_or_else(|| panic!("{} is missing {attribute}", head.name))
            };
            let intensity = channel("intensity");
            assert_eq!(
                intensity.default_raw,
                if head.master_shared { 255 } else { 0 },
                "{} intensity must home at the expected level (master_shared={})",
                head.name,
                head.master_shared,
            );
            assert_eq!(intensity.highlight_raw, 255);
            assert!(intensity.reacts_to_sequence_master);
            assert!(intensity.reacts_to_group_master);
            assert!(intensity.reacts_to_grand_master);
            assert_eq!(channel("volume").default_raw, 255);
            for attribute in ["media.mask.position.x", "media.mask.position.y"] {
                assert_eq!(channel(attribute).default_raw, 32_768);
            }
            for attribute in ["color.red", "color.green", "color.blue"] {
                assert_eq!(channel(attribute).default_raw, 0);
            }
            assert!(
                mode.color_systems
                    .iter()
                    .any(|system| system.head_id == head.id)
            );
            if head.master_shared {
                for attribute in [
                    "media.scale.x",
                    "media.scale.y",
                    "media.scaling_mode",
                    "media.position.x",
                    "media.position.y",
                    "position.rotation",
                    "shaper.blade.1.position",
                    "shaper.blade.1.angle",
                    "shaper.blade.2.position",
                    "shaper.blade.2.angle",
                    "shaper.blade.3.position",
                    "shaper.blade.3.angle",
                    "shaper.blade.4.position",
                    "shaper.blade.4.angle",
                    "shaper.rotation",
                ] {
                    let control = channel(attribute);
                    if attribute == "media.scaling_mode" {
                        assert_eq!(control.resolution, ChannelResolution::U8);
                        assert_eq!(control.default_raw, 0);
                    } else {
                        assert_eq!(control.resolution, ChannelResolution::U16);
                        let inserted_edge = attribute.starts_with("shaper.blade")
                            && attribute.ends_with(".position");
                        // Master scale is signed: 32768 is 0x and 40960 is the 1x home.
                        let signed_scale = attribute.starts_with("media.scale.");
                        assert_eq!(
                            control.default_raw,
                            if inserted_edge {
                                0
                            } else if signed_scale {
                                40_960
                            } else {
                                32_768
                            },
                            "Master {attribute} home"
                        );
                        if signed_scale {
                            assert_eq!(control.highlight_raw, 40_960);
                            assert_eq!(
                                (control.physical_min, control.physical_max),
                                (Some(-4.0), Some(4.0))
                            );
                        }
                    }
                }
            }
        }
        for retired in [
            "media.flip_mirror",
            "media.playback_bpm",
            "media.layer.playback.blur",
            "media.effect.bank.1.parameter.5",
            "media.effect.bank.2.parameter.6",
        ] {
            assert!(
                mode.channels
                    .iter()
                    .all(|channel| &*channel.attribute.0 != retired),
                "{retired} is not part of the mapping personality"
            );
        }

        for attribute in [
            "media.play_mode",
            "media.playback_speed",
            "media.model",
            "media.blend_mode",
            "media.in_point",
            "media.out_point",
            "media.model.pan",
            "media.model.tilt",
            "media.mask.scale.x",
            "media.mask.scale.y",
            "media.mask.invert",
            "media.mask.opacity",
            "media.effect.bank.1.select",
            "media.effect.bank.1.strength",
            "media.effect.bank.2.select",
            "media.effect.bank.2.strength",
        ] {
            assert_eq!(
                mode.channels
                    .iter()
                    .filter(|channel| &*channel.attribute.0 == attribute)
                    .count(),
                layer_count,
                "every layer must expose canonical {attribute} encoder ownership"
            );
        }
        // Each 59-slot layer block, 1-based: 34 3D model, 35 Blend mode, 40..=47 the two banks'
        // four parameters, 48/50 In/Out point (16-bit), 52..=55 Visualizer parameters, and
        // 56/58 model pan/tilt (16-bit).
        for (layer, head) in mode.heads[1..].iter().enumerate() {
            let block_start = u16::try_from(layer).unwrap() * 59;
            let control = |attribute: &str| {
                mode.channels
                    .iter()
                    .find(|channel| {
                        channel.head_id == head.id && &*channel.attribute.0 == attribute
                    })
                    .unwrap_or_else(|| panic!("{} lacks {attribute}", head.name))
            };
            let functions = |channel: &crate::FixtureChannel| {
                channel
                    .functions
                    .iter()
                    .map(|function| (function.name.clone(), function.dmx_from, function.dmx_to))
                    .collect::<Vec<_>>()
            };
            let owned = |name: &str, from: u32, to: u32| (name.to_owned(), from, to);
            let mut byte_parameters = Vec::new();
            for bank in 1..=2_u16 {
                for parameter in 1..=4_u16 {
                    byte_parameters.push((
                        format!("media.effect.bank.{bank}.parameter.{parameter}"),
                        39 + (bank - 1) * 4 + parameter,
                        "Preset value",
                    ));
                }
            }
            for parameter in 1..=4_u16 {
                byte_parameters.push((
                    format!("media.visualizer.parameter.{parameter}"),
                    51 + parameter,
                    "Configured value",
                ));
            }
            for (attribute, slot, zero) in byte_parameters {
                let channel = control(&attribute);
                assert_eq!(
                    primary_slots[&channel.id],
                    block_start + slot,
                    "{} {attribute} slot",
                    head.name
                );
                assert_eq!(channel.resolution, ChannelResolution::U8);
                assert_eq!(channel.default_raw, 0);
                assert_eq!(
                    functions(channel),
                    vec![owned(zero, 0, 0), owned("Parameter", 1, 255)]
                );
            }

            let model = control("media.model");
            assert_eq!(primary_slots[&model.id], block_start + 34);
            assert_eq!(
                functions(model),
                vec![owned("Flat", 0, 0), owned("Model", 1, 255)]
            );
            let blend = control("media.blend_mode");
            assert_eq!(primary_slots[&blend.id], block_start + 35);
            assert_eq!(blend.default_raw, 0);
            assert_eq!(
                functions(blend),
                vec![
                    owned("Normal", 0, 15),
                    owned("Add", 16, 31),
                    owned("Screen", 32, 47),
                    owned("Multiply", 48, 63),
                    owned("Overlay", 64, 79),
                    owned("Difference", 80, 95),
                    owned("Lighten", 96, 111),
                    owned("Darken", 112, 127),
                    owned("Strobe slow–fast", 128, 249),
                    owned("Normal, no strobe", 250, 255),
                ]
            );
            for (attribute, slot, default) in [
                ("media.in_point", 48, 0),
                ("media.out_point", 50, 0),
                ("media.model.pan", 56, 32_768),
                ("media.model.tilt", 58, 32_768),
            ] {
                let channel = control(attribute);
                assert_eq!(channel.resolution, ChannelResolution::U16, "{attribute}");
                assert_eq!(
                    primary_slots[&channel.id],
                    block_start + slot,
                    "{attribute}"
                );
                assert_eq!(channel.secondary_slots, vec![block_start + slot + 1]);
                assert_eq!(channel.default_raw, default, "{attribute}");
            }
            for attribute in ["media.model.pan", "media.model.tilt"] {
                let channel = control(attribute);
                assert_eq!(*channel.fixture_attribute.0, *attribute);
                assert_eq!(
                    (channel.physical_min, channel.physical_max),
                    (Some(-360.0), Some(360.0))
                );
                assert!(
                    channel
                        .functions
                        .iter()
                        .all(|function| *function.attribute.0 == *attribute),
                    "{attribute} functions"
                );
            }
            // The model turns on media attributes of its own, so Aim, position presets and the
            // moving-light Pan/Tilt tools never rotate a media layer.
            assert!(
                mode.channels
                    .iter()
                    .filter(|channel| channel.head_id == head.id)
                    .all(|channel| !matches!(&*channel.attribute.0, "pan" | "tilt")),
                "{} must not expose moving-light pan/tilt",
                head.name
            );
        }
        assert_eq!(
            mode.channels
                .iter()
                .filter(|channel| &*channel.attribute.0 == "media.master.effect.opacity_cycle")
                .count(),
            1,
            "the shared Master must own exactly one Layer Opacity Cycle control"
        );
        for attribute in [
            "media.scaling_mode",
            "media.position.x",
            "media.position.y",
            "media.scale.x",
            "media.scale.y",
            "position.rotation",
        ] {
            assert_eq!(
                mode.channels
                    .iter()
                    .filter(|channel| &*channel.attribute.0 == attribute)
                    .count(),
                layer_count + 1,
                "every layer and the master must expose canonical {attribute} encoder ownership"
            );
        }
        for attribute in ["media.mask.position.x", "media.mask.position.y"] {
            assert_eq!(
                mode.channels
                    .iter()
                    .filter(|channel| &*channel.attribute.0 == attribute)
                    .count(),
                layer_count + 1,
                "every layer and the master must expose canonical {attribute} encoder ownership"
            );
        }
    }

    assert!(profile.notes.contains("Legacy Media Server Layer"));
    assert!(profile.notes.contains("existing show snapshots"));
    assert!(profile.notes.contains("must repatch this personality"));
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
                .filter(|channel| *channel.attribute.0 == *"position.movement")
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
                        .all(|function| { *function.attribute.0 == *"position.movement" })
                );
                *position_movement_sources
                    .entry(channel.fixture_attribute.0.to_string())
                    .or_default() += 1;
                let expected_representation = match &*channel.fixture_attribute.0 {
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
                    .find(|parameter| *parameter.attribute.0 == *"position.movement")
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
                .find(|channel| *channel.attribute.0 == *"prism.1");
            let prism_rotation = mode
                .channels
                .iter()
                .find(|channel| *channel.attribute.0 == *"prism.1.rotation");
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
                .any(|channel| *channel.fixture_attribute.0 == *"fixture.control")
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
                if matches!(&*channel.attribute.0, "pan.continuous" | "tilt.continuous") {
                    continuous_pan_or_tilt.push(format!(
                        "{} / {} / {}",
                        profile.name, mode.name, channel.attribute.0
                    ));
                }
            }
        }
    }

    // The Stage Zoom 1200 SV carries the 1200's shared 20-slot venue personality as well, which
    // adds one mode with Prism 1 selection and rotation, and the AURO SPOT Z300's 17-Channel
    // personality adds another.
    assert_eq!(prism_selection_modes, 15);
    assert_eq!(prism_rotation_modes, 13);
    assert_eq!(generic_control_modes, 6);
    assert_eq!(position_movement_modes, 28);
    assert_eq!(
        position_movement_sources,
        std::collections::HashMap::from([
            ("fixture.mspeed".into(), 2),
            ("fixture.pan_tilt_speed".into(), 6),
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
        profile
            .mode_geometry(mode)
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
    assert!(restored.stage_icon_asset.is_none());
    assert!(restored.model_asset.is_none());
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
                    let attribute = &*parameter.attribute.0;
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

/// A wheel, macro or control channel modelled as one function spanning the whole channel leaves
/// the desk with nothing to name: an encoder sweeps it as a percentage and the operator reads
/// "62%" where the manual says "Gobo 3". Each repaired package is probed at a value inside a band
/// its own manufacturer table prints, so a transcription that drifts fails here rather than on a
/// rig.
#[test]
fn shipped_wheel_channels_name_the_manufacturer_slot_at_a_probed_value() {
    let expected: &[(&str, &str, &str, u32, &str)] = &[
        (
            "chauvet-professional--colorado-1-solo.toskfixture",
            "TOUR",
            "fixture.colour_macros",
            203,
            "White 1",
        ),
        (
            "chauvet-professional--colorado-1-solo.toskfixture",
            "TOUR",
            "fixture.programs",
            45,
            "Auto 1",
        ),
        (
            "chauvet-professional--colorado-1-solo.toskfixture",
            "SSP",
            "shutter",
            50,
            "0–20 Hz",
        ),
        (
            "robe--robin-300-ledwash.toskfixture",
            "Mode 1",
            "color.wheel.1",
            11,
            "White 5600 K",
        ),
        (
            "robe--robin-300-ledwash.toskfixture",
            "Mode 1",
            "fixture.power_special_functions",
            185,
            "Zoom reset",
        ),
        (
            "robe--robin-600x-ledwash.toskfixture",
            "Mode 1",
            "color.wheel.1",
            252,
            "Zone effect 3",
        ),
        (
            "robe--robin-ledbeam-150.toskfixture",
            "Mode 1 – Standard 16-bit",
            "color.wheel.1",
            5,
            "Filter 19 (Fire)",
        ),
        (
            "robe--robin-ledbeam-150.toskfixture",
            "Mode 1 – Standard 16-bit",
            "fixture.colour_mix_control",
            45,
            "Addition mode (Virtual + Colour mix)",
        ),
        (
            "robe--robin-dlf-wash.toskfixture",
            "Mode 1",
            "fixture.barndoor_macros",
            8,
            "Macro 2",
        ),
        (
            "robe--robin-dls-profile.toskfixture",
            "Mode 1",
            "gobo.1",
            12,
            "Gobo 3",
        ),
        (
            "robe--robin-dls-profile.toskfixture",
            "Mode 1",
            "prism.1",
            130,
            "Macro 1",
        ),
        // 16-bit channel: the manual's 8-bit table is carried onto the whole 16-bit range, so
        // "Closed" starts where coarse 180 does.
        (
            "robe--robin-dls-profile.toskfixture",
            "Mode 1",
            "iris",
            46_080,
            "Closed",
        ),
        (
            "claypaky--sharpy.toskfixture",
            "Standard",
            "color.wheel.1",
            9,
            "RED",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "color.wheel.1",
            8,
            "Deep Red",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "color.wheel.1",
            50,
            "Congo Blue",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "color.wheel.1",
            224,
            "Color wheel rotation stop",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "17-Channel",
            "color.wheel.1",
            8,
            "Deep Red",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "gobo.1",
            21,
            "Gobo 2",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "gobo.2",
            150,
            "Gobo 4 shake, slow to fast",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "17-Channel",
            "gobo.2",
            190,
            "Open",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "gobo.1.rotation",
            192,
            "Gobo 1 rotation stop",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "prism.1",
            128,
            "Prism 2 linear",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "prism.1.rotation",
            60,
            "Prism position 0° to 540°",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "softness",
            5,
            "No frost",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "fixture.auto_program",
            130,
            "Program 3 slow to fast",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "fixture.pan_tilt_auto_movement",
            181,
            "Circle inverse, small to large",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "20-Channel",
            "fixture.device_settings",
            210,
            "Reset pan/tilt (hold 5 s)",
        ),
        (
            "cameo--auro-spot-z300.toskfixture",
            "17-Channel",
            "fixture.device_settings",
            126,
            "PWM frequency 800 Hz (hold 5 s)",
        ),
        (
            "claypaky--sharpy.toskfixture",
            "Standard",
            "gobo.1",
            12,
            "GOBO 3",
        ),
        (
            "jb-lighting--jbled-a7.toskfixture",
            "Standard RGB 8 Bit (S8)",
            "fixture.control",
            240,
            "reset (after 2 second)",
        ),
        (
            "etc--source-four-led-series-2-lustr.toskfixture",
            "HSI Plus 7",
            "fixture.plus_7_control",
            130,
            "Plus Seven activated",
        ),
        (
            "glp--jdc1.toskfixture",
            "Normal 23-channel",
            "fixture.special_control",
            41,
            "Position feedback on",
        ),
        (
            "high-end-systems--trackspot.toskfixture",
            "DMX Low Resolution",
            "gobo.1",
            55,
            "gobo 3",
        ),
        (
            "high-end-systems--trackspot.toskfixture",
            "DMX High Resolution",
            "color.wheel.1",
            130,
            "color 2",
        ),
    ];

    for (filename, mode_name, attribute, probe, slot) in expected {
        let profile = shipped_profile(filename);
        let mode = profile
            .modes
            .iter()
            .find(|mode| mode.name == *mode_name)
            .unwrap_or_else(|| panic!("{filename} has no mode {mode_name}"));
        let channel = mode
            .channels
            .iter()
            .find(|channel| *channel.attribute.0 == **attribute)
            .unwrap_or_else(|| panic!("{filename} {mode_name} has no {attribute} channel"));
        assert!(
            channel.functions.len() > 1,
            "{filename} {mode_name} {attribute} is still one blanket function"
        );
        let function = channel
            .functions
            .iter()
            .find(|function| function.dmx_from <= *probe && *probe <= function.dmx_to)
            .unwrap_or_else(|| panic!("{filename} {attribute} names no slot at {probe}"));
        assert_eq!(
            function.name, *slot,
            "{filename} {mode_name} {attribute} at {probe}"
        );
    }
}

/// How far the lift actually gets across the shipped library, named rather than assumed.
///
/// A profile whose modes agree about geometry moves it to the fixture. One whose modes genuinely
/// differ keeps what it has, and is listed here so the families still to be reworked are visible
/// rather than silently carrying the old shape.
#[test]
fn the_shipped_library_reports_which_fixtures_still_carry_geometry_per_mode() {
    let mut still_per_mode = Vec::new();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("assets/fixture-library");
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toskfixture") {
            continue;
        }
        let profile = read_fixture_package(&fs::read(&path).unwrap()).unwrap();
        if profile.geometry.nodes.is_empty()
            && profile
                .modes
                .iter()
                .any(|mode| !mode.geometry.nodes.is_empty())
        {
            still_per_mode.push(path.file_name().unwrap().to_string_lossy().to_string());
        }
    }
    still_per_mode.sort();
    // Nothing is left. Geometry belongs to the fixture, and the Venue objects whose modes used to
    // be their measurements are generated at the size they are placed instead.
    assert_eq!(
        still_per_mode,
        Vec::<String>::new(),
        "a fixture left this list, or a new one arrived carrying a graph per mode"
    );
}

/// The Blinder was one profile whose modes were three different physical fixtures: a two-lamp bar
/// and a four cannot be the same lantern in two personalities. Split, each has its own geometry
/// and its modes are only groupings of the lamps that fixture actually has.
#[test]
fn split_blinders_carry_one_geometry_and_bind_every_lamp() {
    for (file, lamps, modes) in [
        (
            "generic--blinder-2.toskfixture",
            2usize,
            vec![("One channel", 1usize), ("Two channel", 2)],
        ),
        (
            "generic--blinder-4.toskfixture",
            4,
            vec![("One channel", 1), ("Two channel", 2)],
        ),
        (
            "generic--blinder-8.toskfixture",
            8,
            vec![("One channel", 1), ("Two channel", 2), ("Four channel", 4)],
        ),
    ] {
        let profile = shipped_profile(file);
        assert_eq!(profile.geometry.emitters.len(), lamps, "{file}");
        assert!(
            profile
                .modes
                .iter()
                .all(|mode| mode.geometry.nodes.is_empty()),
            "{file}"
        );
        assert_eq!(
            profile
                .modes
                .iter()
                .map(|mode| (mode.name.as_str(), mode.heads.len()))
                .collect::<Vec<_>>(),
            modes,
            "{file}"
        );
        for mode in &profile.modes {
            let bound = profile.mode_geometry(mode);
            assert_eq!(bound.emitters.len(), lamps, "{file} {}", mode.name);
            let heads: std::collections::HashSet<_> =
                bound.emitters.iter().filter_map(|e| e.head_id).collect();
            assert_eq!(heads.len(), mode.heads.len(), "{file} {}", mode.name);
            assert_eq!(
                mode.splits[0].footprint as usize,
                mode.heads.len(),
                "{file} {}",
                mode.name
            );
        }
        profile.validate().unwrap();
    }
}

/// A generated Venue object is the size it is placed at, not the size somebody shipped a model of.
///
/// This is the whole point of generating it: a curtain is made to measure, and a truss repeats its
/// chords over whatever length it is built to. A size an operator cannot have is held to what the
/// object can be built at rather than silently accepted.
#[test]
fn generated_scenery_declares_a_size_an_operator_can_actually_set() {
    let curtain = shipped_profile("venue--curtain-2-m.toskfixture");
    let scenery = curtain.scenery.expect("the curtain is generated");
    // The name says what it is out of the box.
    assert_eq!(scenery.default_size_metres.x, 2.0);
    assert!(scenery.adjustable.width && scenery.adjustable.height);
    // A curtain has no adjustable thickness: it is cloth.
    assert!(!scenery.adjustable.depth);
    assert!(scenery.minimum_size_metres.x < scenery.default_size_metres.x);
    assert!(scenery.maximum_size_metres.x > scenery.default_size_metres.x);
    curtain.validate().unwrap();

    // A truss keeps the cross-section it is built from and is made to length.
    let truss = shipped_profile("venue--three-point-truss.toskfixture");
    let scenery = truss.scenery.expect("the truss is generated");
    assert_eq!(scenery.chords, 3);
    assert!(scenery.adjustable.width);
    assert!(!scenery.adjustable.height && !scenery.adjustable.depth);
    assert_eq!(
        scenery.minimum_size_metres.y, scenery.maximum_size_metres.y,
        "a truss cross-section is not made to measure"
    );
}

/// Generated scenery belongs to a fixture that emits no light; anything else would be a lantern
/// drawn as a prop.
#[test]
fn generated_scenery_requires_a_visual_only_fixture() {
    let mut profile = shipped_profile("venue--curtain-2-m.toskfixture");
    profile.patch_policy = PatchPolicy::Dmx;
    assert!(profile.validate().is_err());
}

/// The backline, PA, flight cases and figures: the Visualizer shipped these models long before any
/// profile pointed at one, so each is one visual-only Venue profile carrying that exact model, its
/// Model-Catalogue render, and the size the model manifest measures. They are what **Add venue
/// element** lists, so a missing or renamed one empties that dialog of everything but the railings
/// and crowds.
#[test]
fn shipped_backline_case_and_figure_packages_carry_their_catalogue_models() {
    let expected = [
        ("venue--dj-mixer.toskfixture", "DJ Mixer", "dj-mixer"),
        (
            "venue--dj-media-player.toskfixture",
            "DJ Media Player",
            "dj-player",
        ),
        ("venue--drum-kit.toskfixture", "Drum Kit", "drum-kit"),
        (
            "venue--guitar-stack.toskfixture",
            "Guitar Stack",
            "guitar-amp",
        ),
        (
            "venue--electric-guitar-on-a-stand.toskfixture",
            "Electric Guitar on a Stand",
            "guitar-in-stand",
        ),
        (
            "venue--line-array-hang.toskfixture",
            "Line Array Hang",
            "line-array-hang",
        ),
        (
            "venue--microphone-stand.toskfixture",
            "Microphone Stand",
            "microphone-stand",
        ),
        (
            "venue--saxophone-on-a-stand.toskfixture",
            "Saxophone on a Stand",
            "saxophone-in-stand",
        ),
        (
            "venue--pa-top-on-a-pole-stand.toskfixture",
            "PA Top on a Pole Stand",
            "speaker-on-pole",
        ),
        ("venue--pa-top.toskfixture", "PA Top", "speaker-top"),
        (
            "venue--stage-monitor-wedge.toskfixture",
            "Stage Monitor Wedge",
            "stage-monitor",
        ),
        (
            "venue--stage-piano.toskfixture",
            "Stage Piano",
            "stage-piano",
        ),
        ("venue--subwoofer.toskfixture", "Subwoofer", "subwoofer"),
        (
            "venue--flight-case-rack-2u.toskfixture",
            "Flight Case Rack 2U",
            "rack-02u",
        ),
        (
            "venue--flight-case-rack-4u.toskfixture",
            "Flight Case Rack 4U",
            "rack-04u",
        ),
        (
            "venue--flight-case-rack-6u.toskfixture",
            "Flight Case Rack 6U",
            "rack-06u",
        ),
        (
            "venue--flight-case-rack-8u-on-castors.toskfixture",
            "Flight Case Rack 8U on Castors",
            "rack-08u-wheels",
        ),
        (
            "venue--flight-case-rack-14u-on-castors.toskfixture",
            "Flight Case Rack 14U on Castors",
            "rack-14u-wheels",
        ),
        (
            "venue--flight-case-rack-18u-on-castors.toskfixture",
            "Flight Case Rack 18U on Castors",
            "rack-18u-wheels",
        ),
        (
            "venue--figure-deejay.toskfixture",
            "Figure, Deejay",
            "figure-deejay",
        ),
        (
            "venue--figure-guitarist.toskfixture",
            "Figure, Guitarist",
            "figure-guitarist",
        ),
        (
            "venue--figure-pianist.toskfixture",
            "Figure, Pianist",
            "figure-pianist",
        ),
        (
            "venue--figure-singer.toskfixture",
            "Figure, Singer",
            "figure-singer",
        ),
    ];
    assert_eq!(expected.len(), 23);
    let models = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("assets/models");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(models.join("manifest.json")).unwrap()).unwrap();
    let mut identities = std::collections::HashSet::new();
    for (filename, name, model) in expected {
        let profile = shipped_profile(filename);
        assert_eq!(profile.manufacturer, "Venue", "{filename}");
        assert_eq!(profile.name, name, "{filename}");
        assert_eq!(profile.fixture_type, "venue", "{filename}");
        assert_eq!(profile.patch_policy, PatchPolicy::VisualOnly, "{filename}");
        assert_eq!(profile.model_units, ModelUnits::Metres, "{filename}");
        // Fixed geometry, not a generated object: nothing here is made to measure.
        assert!(profile.scenery.is_none(), "{filename}");
        assert!(profile.crowd.is_none(), "{filename}");
        assert!(identities.insert(profile.id), "{filename}");
        assert_eq!(profile.modes.len(), 1, "{filename}");
        let mode = &profile.modes[0];
        assert_eq!(mode.name, "Default", "{filename}");
        assert_eq!(mode.splits.len(), 1, "{filename}");
        assert_eq!(mode.splits[0].footprint, 0, "{filename}");
        assert!(mode.channels.is_empty(), "{filename}");

        let photograph = profile.photograph_asset.as_deref().expect(filename);
        assert!(photograph.starts_with("data:image/png"), "{filename}");
        // The package carries the shipped model itself, byte for byte, not a re-export of it.
        let carried = profile
            .model_asset
            .as_deref()
            .expect(filename)
            .strip_prefix("data:model/gltf-binary;base64,")
            .unwrap_or_else(|| panic!("{filename} carries a GLB data URL"));
        let entry = manifest["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["model"] == model)
            .unwrap_or_else(|| panic!("{model} is not in the model manifest"));
        let shipped = fs::read(models.join(entry["file"].as_str().unwrap())).unwrap();
        assert_eq!(
            STANDARD.decode(carried).unwrap(),
            shipped,
            "{filename} should carry {model} unchanged"
        );
        // And it is placed at the size that model was measured at, to the whole millimetre the
        // profile editor works in.
        for (measured, authored) in [
            ("width_millimetres", profile.physical.width_millimetres),
            ("height_millimetres", profile.physical.height_millimetres),
            ("depth_millimetres", profile.physical.depth_millimetres),
        ] {
            assert_eq!(
                authored,
                Some(entry[measured].as_f64().unwrap().round() as f32),
                "{filename} {measured}"
            );
        }
    }
}
