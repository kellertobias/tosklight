//! Capture and store behaviour, exercised without a window, a GPU, or Blender.

use super::*;
use viz_scene::glam::Vec3;
use viz_scene::{
    BodyKind, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody,
    FixtureInstance, MotionAxis, SceneryKind, SceneryObject, uuid::Uuid,
};

/// A rig with one moving head over a floor, aimed straight down at full.
fn rig() -> (Scene, SceneValues) {
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
        instance_id: Uuid::nil(),
        fixture_id: Uuid::nil(),
        name: "Mac Aura".into(),
        number: Some(12),
        position: Vec3::new(1.0, 6.0, -2.0),
        rotation_degrees: Vec3::ZERO,
        bracket_degrees: 0.0,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        body: FixtureBody {
            size: Vec3::new(0.3, 0.5, 0.3),
            kind: BodyKind::MovingHead,
        },
        patched: true,
        address: Some((1, 101)),
        model: None,
        fallback: None,
    });
    scene.emitters.push(EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: String::new(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: Some(MotionAxis {
            axis: Vec3::Y,
            min_degrees: -270.0,
            max_degrees: 270.0,
        }),
        tilt: Some(MotionAxis {
            axis: Vec3::X,
            min_degrees: -135.0,
            max_degrees: 135.0,
        }),
        beam_angle_degrees: 12.0,
        field_angle_degrees: 36.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
    scene.scenery.push(SceneryObject {
        id: Uuid::nil(),
        name: "Stage".into(),
        position: Vec3::ZERO,
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(12.0, 0.1, 8.0),
        colour: [0.15, 0.15, 0.15],
        roughness: 0.9,
        kind: SceneryKind::Floor,
        chords: 0,
    });
    scene.recompute_bounds();
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 1.0;
    values.emitters[0].colour = [1.0, 0.4, 0.1];
    (scene, values)
}

fn context() -> CaptureContext {
    CaptureContext {
        show: "Tour: Rig / 2026".into(),
        source: "http://127.0.0.1:5000".into(),
        scene_revision: 7,
        look: SnapshotLook {
            fog: 0.5,
            ambient: 0.06,
            exposure: 1.0,
        },
        camera: SnapshotCamera {
            position: [0.0, 3.0, 12.0],
            target: [0.0, 2.0, 0.0],
            fov_degrees: 45.0,
            orthographic: false,
            orthographic_size: 8.0,
        },
    }
}

/// A directory of this test's own, removed when it is done with it.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("viz-snapshot-{name}"));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("scratch directory");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn a_capture_carries_the_rig_the_lights_and_the_room() {
    let (scene, values) = rig();
    let capture = capture(&scene, &values, &context());
    let document = &capture.document;

    assert_eq!(document.format, FORMAT_VERSION);
    assert_eq!(document.counts.fixtures, 1);
    assert_eq!(document.counts.heads, 1);
    assert_eq!(document.counts.live_beams, 1);
    assert!(document.counts.triangles > 0, "no geometry was written");
    assert!(!capture.geometry.is_empty());
    assert_eq!(&capture.geometry[0..4], b"glTF");

    let light = &document.lights[0];
    assert_eq!(light.name, "12 Mac Aura");
    assert_eq!(light.address.as_deref(), Some("1.101"));
    assert!(light.power_watts > 0.0);
    assert!(
        light.cone_degrees > 12.0 && light.cone_degrees < 36.0,
        "a head at mid zoom sits between its narrow and wide figures: {}",
        light.cone_degrees
    );
    assert_eq!(light.colour, [1.0, 0.4, 0.1]);
}

#[test]
fn the_lamp_is_where_it_hangs_and_points_where_it_was_aimed() {
    let (scene, mut values) = rig();
    // Tilt at the centre of its travel, which is a moving head hanging straight down.
    values.emitters[0].tilt = 0.5;
    let capture = capture(&scene, &values, &context());
    let light = &capture.document.lights[0];

    // Six metres up in the visualizer is six metres up in the package, and the fixture is one
    // metre stage right and two metres upstage of centre. The light leaves the underside of the
    // lamp rather than its centre, which is the only slack here.
    assert!(
        (5.4..=6.0).contains(&light.position[2]),
        "{:?}",
        light.position
    );
    assert!(
        (light.position[0] - 1.0).abs() < 0.05,
        "{:?}",
        light.position
    );
    assert!(
        (light.position[1] - 2.0).abs() < 0.05,
        "{:?}",
        light.position
    );
    assert!(
        light.direction[2] < -0.98,
        "a head hanging straight down must light the stage, not the roof: {:?}",
        light.direction
    );
    assert!(light.reach > 1.0, "the beam has to get somewhere");
}

#[test]
fn a_lamp_lights_from_outside_its_own_body() {
    // A light left inside the head that carries it is blocked by that head in any renderer that
    // traces light, and the whole rig comes out black. This is the one thing a capture cannot get
    // away with being approximately right about.
    let (scene, mut values) = rig();
    values.emitters[0].tilt = 0.5;
    let capture = capture(&scene, &values, &context());
    let light = &capture.document.lights[0];

    let fixture = to_z_up(scene.fixtures[0].position.to_array());
    let from_centre: Vec<f32> = light
        .position
        .iter()
        .zip(fixture.iter())
        .map(|(light, fixture)| light - fixture)
        .collect();
    let distance = from_centre
        .iter()
        .map(|axis| axis * axis)
        .sum::<f32>()
        .sqrt();
    let half_height = scene.fixtures[0].body.size.y * 0.5;
    assert!(
        distance > half_height,
        "the light is still inside the lamp: {distance} within {half_height}"
    );
    // It leaves along the aim, not sideways, so the beam still lands where the desk pointed it.
    assert!(
        from_centre[2] < 0.0,
        "a head pointing down should light from underneath itself: {from_centre:?}"
    );
}

#[test]
fn a_dark_rig_captures_its_bodies_and_no_lights() {
    let (scene, mut values) = rig();
    values.emitters[0].intensity = 0.0;
    let capture = capture(&scene, &values, &context());
    assert!(capture.document.lights.is_empty());
    assert_eq!(capture.document.counts.live_beams, 0);
    assert_eq!(capture.document.counts.fixtures, 1);
    assert!(
        capture.document.counts.triangles > 0,
        "a fixture at zero is still on the truss"
    );
}

#[test]
fn every_lamp_in_a_rig_of_identical_ones_gets_its_own_name() {
    let (mut scene, mut values) = rig();
    for _ in 0..2 {
        let mut fixture = scene.fixtures[0].clone();
        fixture.number = None;
        fixture.name = "Blinder".into();
        scene.fixtures.push(fixture);
        let mut emitter = scene.emitters[0].clone();
        emitter.fixture_index = scene.fixtures.len() as u32 - 1;
        scene.emitters.push(emitter);
    }
    values.resize(scene.emitters.len());
    for emitter in &mut values.emitters {
        emitter.intensity = 1.0;
    }
    let capture = capture(&scene, &values, &context());
    let names: Vec<&str> = capture
        .document
        .lights
        .iter()
        .map(|light| light.name.as_str())
        .collect();
    let unique: std::collections::HashSet<&&str> = names.iter().collect();
    assert_eq!(unique.len(), names.len(), "collided: {names:?}");
}

#[test]
fn the_bounding_box_survives_the_change_of_up_axis() {
    let (scene, values) = rig();
    let capture = capture(&scene, &values, &context());
    let bounds = capture.document.bounds;
    for axis in 0..3 {
        assert!(
            bounds.min[axis] <= bounds.max[axis],
            "axis {axis} came back inside out: {bounds:?}"
        );
    }
    // The rig is six metres up, so the box has to reach it.
    assert!(bounds.max[2] >= 5.9, "{bounds:?}");
}

#[test]
fn a_capture_is_written_as_a_folder_that_names_itself() {
    let scratch = Scratch::new("write");
    let store = SnapshotStore::new(&scratch.0, 12);
    let (scene, values) = rig();
    let entry = store
        .write(&capture(&scene, &values, &context()))
        .expect("the capture is written");

    assert!(entry.directory.join(GEOMETRY_FILE).is_file());
    assert!(entry.directory.join(DOCUMENT_FILE).is_file());
    assert!(entry.blend.is_none(), "nothing has been exported yet");
    let name = entry
        .directory
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a folder name");
    assert!(
        name.contains("Tour Rig 2026"),
        "the show should be recognisable, and the punctuation gone: {name}"
    );
    assert!(!name.contains(':'), "a colon is not a folder name: {name}");
    assert_eq!(store.list().len(), 1);
    assert_eq!(store.list()[0].counts.fixtures, 1);
}

#[test]
fn a_show_with_an_unreasonable_name_still_makes_a_reasonable_folder() {
    let scratch = Scratch::new("longname");
    let store = SnapshotStore::new(&scratch.0, 4);
    let (scene, values) = rig();
    let context = CaptureContext {
        show: "Summer/Tour \u{2014} \"Main\" Rig: 1ac6d107-089c-4e43-b782-480dd44c40a7".into(),
        ..context()
    };
    let entry = store
        .write(&capture(&scene, &values, &context))
        .expect("written");
    let name = entry
        .directory
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a folder name");
    assert!(name.len() < 80, "{name}");
    assert!(name.starts_with("2026-"), "the time still leads: {name}");
    assert!(name.contains("Summer Tour"), "{name}");
    assert!(!name.ends_with(' '), "{name}");
}

#[test]
fn the_oldest_capture_falls_off_the_end() {
    let scratch = Scratch::new("prune");
    let store = SnapshotStore::new(&scratch.0, 3);
    let (scene, values) = rig();
    let capture = capture(&scene, &values, &context());
    // Written by hand: five captures in the same second would otherwise share one folder.
    for minute in 0..5 {
        let mut copy = SnapshotDocument {
            captured_at: format!("2026-07-31 14-2{minute}-00"),
            ..capture.document.clone()
        };
        copy.scene_revision = minute;
        store
            .write(&Capture {
                document: copy,
                geometry: capture.geometry.clone(),
            })
            .expect("written");
    }
    let kept = store.list();
    assert_eq!(kept.len(), 3);
    assert_eq!(kept[0].captured_at, "2026-07-31 14-24-00", "newest first");
    assert!(
        !scratch.0.join("2026-07-31 14-20-00 Tour Rig 2026").exists(),
        "the oldest should have been dropped"
    );
}

#[test]
fn something_that_is_not_a_capture_is_left_where_it_is() {
    let scratch = Scratch::new("neighbours");
    let store = SnapshotStore::new(&scratch.0, 1);
    let neighbour = scratch.0.join("renders");
    std::fs::create_dir_all(&neighbour).expect("neighbour");
    std::fs::write(neighbour.join("keep me.txt"), b"mine").expect("write");

    let (scene, values) = rig();
    store
        .write(&capture(&scene, &values, &context()))
        .expect("written");
    assert!(neighbour.join("keep me.txt").is_file());
    assert!(store.list().len() == 1);
}

#[test]
fn a_capture_reads_back_as_the_document_it_was_written_as() {
    let scratch = Scratch::new("roundtrip");
    let store = SnapshotStore::new(&scratch.0, 12);
    let (scene, values) = rig();
    let entry = store
        .write(&capture(&scene, &values, &context()))
        .expect("written");
    let text = std::fs::read_to_string(entry.directory.join(DOCUMENT_FILE)).expect("read");
    let document: SnapshotDocument = serde_json::from_str(&text).expect("valid document");
    assert_eq!(document.show, "Tour: Rig / 2026");
    assert_eq!(document.up_axis, "z");
    assert_eq!(document.geometry_file, GEOMETRY_FILE);
    assert!(!document.notes.is_empty(), "say what did not come across");
    assert!(
        document
            .notes
            .iter()
            .any(|note| note.contains("procedural proxies")),
        "a rig drawn from proxies should say so: {:?}",
        document.notes
    );
}

#[test]
fn a_snapshot_is_labelled_by_the_time_it_was_taken() {
    let entry = SnapshotEntry {
        directory: PathBuf::from("/tmp/x"),
        captured_at: "2026-07-31 14:22:08".into(),
        show: "Tour".into(),
        counts: SnapshotCounts {
            fixtures: 301,
            heads: 383,
            live_beams: 130,
            triangles: 42,
        },
        blend: None,
    };
    assert_eq!(entry.label(), "14:22:08");
    assert_eq!(entry.summary(), "301 fixtures, 130 live");
    assert!(entry.blend_destination().ends_with(BLEND_FILE));
}
