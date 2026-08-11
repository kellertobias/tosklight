//! End-to-end scene assembly from desk read models, using real shipped fixture packages.

use crate::scene_build::{self, DeskReadModels};
use crate::wire::{ObjectRecord, PatchSnapshot, StageLayoutBody};
use serde_json::json;
use std::io::Read;
use viz_scene::{BodyKind, EmitterKind, euler_degrees};

/// Read one shipped `.toskfixture` package and return its profile snapshot as JSON.
fn shipped_profile(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
    let mut archive = zip::ZipArchive::new(file).expect("read fixture package");
    let mut entry = archive.by_name("fixture.json").expect("fixture.json");
    let mut text = String::new();
    entry.read_to_string(&mut text).expect("read fixture.json");
    let package: serde_json::Value = serde_json::from_str(&text).expect("parse fixture.json");
    package["profile"].clone()
}

/// The same package as the desk hands it to a renderer: assets inlined as data URLs, which is
/// what the reader does when it loads one and what a patched show then carries.
fn shipped_profile_with_assets(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    let bytes =
        std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let profile = light_fixture::read_fixture_package(&bytes).expect("read fixture package");
    serde_json::to_value(&profile).expect("profile as json")
}

fn models(profile: serde_json::Value, layout: StageLayoutBody) -> DeskReadModels {
    let mode_id = profile["modes"][0]["id"]
        .as_str()
        .expect("mode id")
        .to_owned();
    let profile_id = profile["id"].as_str().expect("profile id").to_owned();
    let fixture_id = "11111111-1111-4111-8111-111111111111";
    let patch: PatchSnapshot = serde_json::from_value(json!({
        "show_id": "22222222-2222-4222-8222-222222222222",
        "show_revision": 7,
        "patch_revision": 3,
        "fixtures": [{
            "fixture_id": fixture_id,
            "fixture_number": 1,
            "name": "Test head",
            "profile_id": profile_id,
            "profile_revision": 1,
            "mode_id": mode_id,
            "split_patches": [{"split": 1, "universe": 2, "address": 17}],
            "location": {"x": 1_000, "y": 4_000, "z": 6_000},
            "rotation": {"x": 0.0, "y": 0.0, "z": 0.0},
            "multipatch": [],
        }],
        "profile_revisions": [{
            "profile_id": profile_id,
            "profile_revision": 1,
            "manufacturer": profile["manufacturer"],
            "name": profile["name"],
            "fixture_type": profile["fixture_type"],
            "patch_policy": "dmx",
            "profile_snapshot": profile,
        }],
    }))
    .expect("patch snapshot");
    DeskReadModels {
        patch,
        stage_layout: layout,
        venue_objects: Vec::new(),
        show_name: "Test show".into(),
        server_identity: "http://127.0.0.1:5000".into(),
    }
}

#[test]
fn a_shipped_moving_head_becomes_a_beam_emitter_with_real_pan_and_tilt_travel() {
    let plan = scene_build::build(&models(
        shipped_profile("claypaky--sharpy"),
        StageLayoutBody::default(),
    ));
    assert_eq!(plan.scene.fixtures.len(), 1);
    let fixture = &plan.scene.fixtures[0];
    assert_eq!(fixture.body.kind, BodyKind::MovingHead);
    assert!(fixture.patched);
    assert_eq!(plan.scene.emitters.len(), 1);
    let emitter = &plan.scene.emitters[0];
    assert_eq!(emitter.kind, EmitterKind::Beam);
    let pan = emitter.pan.expect("the profile declares pan motion");
    let tilt = emitter.tilt.expect("the profile declares tilt motion");
    assert_eq!((pan.min_degrees, pan.max_degrees), (-270.0, 270.0));
    assert_eq!((tilt.min_degrees, tilt.max_degrees), (-135.0, 135.0));
    // A beam fixture keeps a narrow cone even without emitter geometry.
    assert!(emitter.beam_angle_degrees <= 10.0);
    // The fixture is unplaced by the stage layout, so its patch location is used.
    assert!((fixture.position.x - 1.0).abs() < 1e-4);
    assert!((fixture.position.y - 6.0).abs() < 1e-4);
    assert!((fixture.position.z + 4.0).abs() < 1e-4);
}

#[test]
fn the_stage_layout_position_overrides_the_patch_location() {
    let layout: StageLayoutBody = serde_json::from_value(json!({
        "positions3d": {
            "11111111-1111-4111-8111-111111111111": {
                "x": -2.0, "y": 3.0, "z": 7.5,
                "rotationX": 0.0, "rotationY": 15.0, "rotationZ": 0.0
            }
        }
    }))
    .expect("stage layout");
    let plan = scene_build::build(&models(shipped_profile("claypaky--sharpy"), layout));
    let fixture = &plan.scene.fixtures[0];
    assert!((fixture.position.x + 2.0).abs() < 1e-4);
    assert!((fixture.position.y - 7.5).abs() < 1e-4);
    assert!((fixture.position.z + 3.0).abs() < 1e-4);
}

#[test]
fn installed_appearance_from_the_patch_tints_the_compiled_scene() {
    let profile = shipped_profile("claypaky--sharpy");
    let mut models = models(profile.clone(), StageLayoutBody::default());
    models.patch.fixtures[0]
        .installed_appearance
        .color_temperature_kelvin = Some(3_200);
    models.patch.fixtures[0].installed_appearance.gel = light_fixture::GelAssignment::Custom {
        name: "Red".into(),
        color_srgb: "#FF0000".into(),
        note: None,
    };
    let expected = viz_project::installed_appearance_linear_rgb(
        &serde_json::from_value(profile).expect("fixture profile"),
        &models.patch.fixtures[0].installed_appearance,
    );

    let plan = scene_build::build(&models);
    assert_eq!(plan.scene.fixtures[0].installed_colour, expected);
    assert_eq!(expected[1], 0.0);
    assert_eq!(expected[2], 0.0);
}

/// A profile that ships a wheel projects its own glass, in the number of slots it actually has.
#[test]
fn a_shipped_gobo_wheel_reaches_the_scene_as_artwork() {
    let plan = scene_build::build(&models(
        shipped_profile_with_assets("robe--robin-dls-profile"),
        StageLayoutBody::default(),
    ));
    let wheel = &plan.scene.emitters[0].optics.gobo_wheel;
    assert_eq!(wheel.len(), 8, "seven patterns and the open slot");
    assert!(
        wheel[0].artwork.is_none(),
        "the open slot is glass nobody etched"
    );
    for (slot, entry) in wheel.iter().enumerate().skip(1) {
        assert!(entry.artwork.is_some(), "slot {slot} carries no artwork");
        assert!(!entry.name.is_empty(), "slot {slot} is unnamed");
    }
    // One image per piece of glass, whatever the fixture count.
    assert_eq!(plan.scene.gobo_artwork.len(), 7);
    for image in &plan.scene.gobo_artwork {
        assert_eq!(image.edge, viz_project::GOBO_ARTWORK_EDGE);
        assert_eq!(image.mask.len(), (image.edge * image.edge) as usize);
        assert!(
            image.mask.iter().any(|value| *value > 200)
                && image.mask.iter().any(|value| *value < 50),
            "a mask that is all one value is not a pattern"
        );
    }
}

/// A profile with no wheel keeps the drawn patterns, which is most of the library.
#[test]
fn a_profile_without_a_wheel_declares_none() {
    let plan = scene_build::build(&models(
        shipped_profile("generic--dimmer-profile"),
        StageLayoutBody::default(),
    ));
    assert!(plan.scene.emitters[0].optics.gobo_wheel.is_empty());
    assert!(plan.scene.gobo_artwork.is_empty());
}

#[test]
fn a_shipped_hazer_becomes_an_atmosphere_emitter_rather_than_a_beam() {
    let plan = scene_build::build(&models(
        shipped_profile("generic--hazer"),
        StageLayoutBody::default(),
    ));
    assert_eq!(plan.scene.fixtures[0].body.kind, BodyKind::Machine);
    assert!(
        plan.scene
            .emitters
            .iter()
            .any(|emitter| emitter.kind == EmitterKind::Atmosphere),
        "a hazer must contribute atmosphere, not a beam"
    );
}

/// A show laser is a box with a window in its front face, so the beam both starts and points
/// there — not at the clamp, and not down the shared rest aim every lantern hangs on.
///
/// Its chart calls the position of the figure inside the scan field pan and tilt, which is a
/// scanner's deflection rather than a yoke. Drawing that as a moving head, or letting it swing the
/// whole projector on top of the deflection the scan engine has already applied, is the difference
/// between a laser and a searchlight.
#[test]
fn a_shipped_laser_projects_from_the_window_in_its_front_face() {
    let plan = scene_build::build(&models(
        shipped_profile("tosklight--visualizer-laser"),
        StageLayoutBody::default(),
    ));
    let fixture = &plan.scene.fixtures[0];
    assert_eq!(fixture.body.kind, BodyKind::Machine);
    assert!(
        fixture.model.is_some(),
        "the laser is drawn as a body, not as a proxy box"
    );
    let emitter = &plan.scene.emitters[0];
    assert_eq!(emitter.kind, EmitterKind::Laser);
    let aim = euler_degrees(emitter.local_orientation_degrees) * glam::Vec3::NEG_Y;
    assert!(
        aim.z > 0.99,
        "the beam leaves the projector aiming {aim} rather than out of its window"
    );
    assert!(
        emitter.local_origin.z > 0.05 && emitter.local_origin.y < -0.05,
        "the beam starts at {}, not at the window in the front of the body",
        emitter.local_origin
    );
    assert!(
        emitter.pan.is_none() && emitter.tilt.is_none(),
        "the desk swung a laser that had already deflected itself"
    );
}

#[test]
fn a_shipped_strobe_is_emissive_and_never_invents_a_projected_beam() {
    let plan = scene_build::build(&models(
        shipped_profile("generic--strobe"),
        StageLayoutBody::default(),
    ));
    assert!(!plan.scene.emitters.is_empty());
    assert!(
        plan.scene
            .emitters
            .iter()
            .all(|emitter| emitter.kind == EmitterKind::Emissive),
        "a strobe is face-visible, not a projector"
    );
}

#[test]
fn a_multi_head_bar_produces_one_emitter_per_head() {
    let plan = scene_build::build(&models(
        shipped_profile("showtec--sunstrip-led-rgb-42206"),
        StageLayoutBody::default(),
    ));
    assert!(
        plan.scene.emitters.len() > 1,
        "a pixel bar must keep its per-head identity, got {}",
        plan.scene.emitters.len()
    );
    assert!(
        plan.scene
            .emitters
            .iter()
            .all(|emitter| emitter.kind == EmitterKind::Emissive)
    );
    // Heads are spread along the body rather than stacked on one point.
    let first = plan.scene.emitters[0].local_origin;
    let last = plan.scene.emitters[plan.scene.emitters.len() - 1].local_origin;
    assert!((first - last).length() > 0.1);
}

/// A blinder and a sunstrip are recognised by their lamps, and a lamp is round glass.
///
/// Neither package declares its optics, so this is entirely the fallback's answer. Drawn as one
/// rectangular panel per lamp — or as lamp faces wider than the pitch they sit at — a bank reads as
/// a continuous glowing tube, which is not what either of these fixtures looks like switched on.
#[test]
fn a_shipped_lamp_bank_lights_up_as_round_lamps_that_do_not_run_into_each_other() {
    for package in ["generic--blinder", "showtec--sunstrip-active-dmx"] {
        let plan = scene_build::build(&models(
            shipped_profile(package),
            StageLayoutBody::default(),
        ));
        let mut lamps = plan
            .scene
            .emitters
            .iter()
            .map(|emitter| (emitter.local_origin, emitter.optics.source))
            .collect::<Vec<_>>();
        assert!(!lamps.is_empty(), "{package} has no lamps");
        for (_, source) in &lamps {
            assert_eq!(
                source.form,
                viz_scene::SourceForm::Round,
                "{package} draws a panel where its lamp lenses are"
            );
        }
        lamps.sort_by(|left, right| left.0.x.total_cmp(&right.0.x));
        for pair in lamps.windows(2) {
            let pitch = (pair[1].0 - pair[0].0).length();
            if pitch < 1e-4 {
                continue;
            }
            let widest = pair[0].1.width.max(pair[1].1.width);
            assert!(
                widest <= pitch,
                "{package} draws {widest} m lamps {pitch} m apart"
            );
        }
    }
}

#[test]
fn an_unpatched_fixture_stays_in_the_scene_without_dmx() {
    let profile = shipped_profile("claypaky--sharpy");
    let mut models = models(profile, StageLayoutBody::default());
    models.patch.fixtures[0].split_patches[0].universe = None;
    models.patch.fixtures[0].split_patches[0].address = None;
    let plan = scene_build::build(&models);
    assert_eq!(plan.scene.fixtures.len(), 1);
    assert!(!plan.scene.fixtures[0].patched);
    // It is visible and selectable, it simply reads no DMX.
    assert_eq!(plan.scene.emitters.len(), 1);
    assert!(plan.bindings[0].universes.is_empty());
}

#[test]
fn a_scene_always_carries_a_floor_and_upstage_wall_for_beams_to_land_on() {
    let plan = scene_build::build(&models(
        shipped_profile("claypaky--sharpy"),
        StageLayoutBody::default(),
    ));
    assert!(
        plan.scene
            .scenery
            .iter()
            .any(|object| object.kind == viz_scene::SceneryKind::Floor)
    );
    assert!(!plan.scene.bounds.is_empty());
}

#[test]
fn objects_that_cannot_be_read_are_reported_rather_than_dropped_silently() {
    let mut models = models(
        shipped_profile("claypaky--sharpy"),
        StageLayoutBody::default(),
    );
    models.patch.profile_revisions[0].profile_snapshot = serde_json::Value::Null;
    let plan = scene_build::build(&models);
    assert!(plan.scene.fixtures.is_empty());
    assert_eq!(plan.warnings.len(), 1);
    assert!(plan.warnings[0].contains("no snapshot"));
}

/// Unused import guard: `ObjectRecord` is part of the read-model contract these tests exercise.
#[test]
fn venue_objects_are_accepted_as_scenery() {
    let mut models = models(
        shipped_profile("claypaky--sharpy"),
        StageLayoutBody::default(),
    );
    models.venue_objects.push(ObjectRecord {
        id: "riser".into(),
        revision: 1,
        body: json!({"name": "Riser", "position": {"x": 0.0, "y": 2.0, "z": 0.4},
                     "size": {"x": 4.0, "y": 2.0, "z": 0.6}}),
    });
    let plan = scene_build::build(&models);
    assert!(
        plan.scene
            .scenery
            .iter()
            .any(|object| object.name == "Riser")
    );
}

/// Read a shipped package the way the running desk does, so relative asset paths become the
/// self-contained data URLs a profile snapshot actually carries. The lighter helper above reads
/// `fixture.json` straight out of the archive and would leave every asset an unresolved path.
fn shipped_package(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    let bytes =
        std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let profile = light_fixture::read_fixture_package(&bytes).expect("read fixture package");
    serde_json::to_value(&profile).expect("serialize profile")
}

/// The end of the chain the whole feature hangs on: a shipped laser package, read the way the desk
/// reads it, becomes an emitter that draws a scan path and carries the script that decides what
/// that path is.
#[test]
fn the_shipped_laser_becomes_a_scanning_emitter_carrying_its_own_engine() {
    let plan = scene_build::build(&models(
        shipped_package("tosklight--visualizer-laser"),
        StageLayoutBody::default(),
    ));
    let emitter = plan
        .scene
        .emitters
        .iter()
        .find(|emitter| emitter.kind == EmitterKind::Laser)
        .expect("a laser package must produce a laser emitter, not a beam");
    let optics = emitter
        .laser
        .as_ref()
        .expect("a laser emitter must carry its scanner");
    let script = optics
        .script
        .as_ref()
        .expect("the packaged scan engine must reach the scene");
    assert!(
        script.contains("export function scan"),
        "the script did not survive as source text"
    );
    assert_ne!(optics.script_key, 0, "an unkeyed script can never reload");
    // 25 degrees full scan, so half of that either side of centre.
    assert!((optics.scan_half_angle_x - 12.5_f32.to_radians()).abs() < 1e-4);
    assert_eq!(optics.points_per_second, 30_000.0);
    assert!((optics.optical_power_watts - 0.5).abs() < 1e-4);
}

/// Model ownership, from the side that can be tested without authoring geometry.
///
/// TL-68 settles that an exact model belongs in the fixture's own transferable package, with one
/// audited generic set for everything else. `model_asset` names an asset carried *inside* the
/// package, so a real preference test needs a package that ships one — and none do yet.
///
/// What can be pinned now is the other half of the rule, and it is the half that goes wrong
/// quietly: a package that names a model nobody can read.
/// A package that names a model nobody can read still draws, and says why.
///
/// Silently falling back would leave an operator wondering why their fixture looks wrong; refusing
/// to draw would lose the rig over a missing file. It does both: the built-in body, and a warning.
#[test]
fn a_model_that_cannot_be_read_falls_back_and_says_so() {
    let mut profile = shipped_profile("claypaky--sharpy");
    profile["model_asset"] = json!("lamps/there-is-no-such-model.glb");
    let plan = scene_build::build(&models(profile, StageLayoutBody::default()));

    assert!(
        plan.warnings
            .iter()
            .any(|warning| warning.contains("built-in body")),
        "the operator is told why the model they chose is not what they see: {:?}",
        plan.warnings
    );
    assert!(
        !plan.scene.fixtures.is_empty(),
        "the rig is still drawn rather than lost over a missing file"
    );
}
