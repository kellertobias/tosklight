//! The whole path, with the real Blender: capture a rig, write it, and export a `.blend`.
//!
//! Blender is not a build dependency and most machines running these tests will not have it, so
//! this reports and passes when there is none to run. When there is one, it is the only check that
//! the geometry file, the document, and the import script actually agree with each other — every
//! other test in this crate is one of the three on its own.

use std::path::Path;
use viz_scene::glam::Vec3;
use viz_scene::{
    BodyKind, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody,
    FixtureInstance, MotionAxis, Scene, SceneValues, SceneryKind, SceneryObject, uuid::Uuid,
};
use viz_snapshot::{
    CaptureContext, ExportError, SnapshotCamera, SnapshotLook, SnapshotStore, capture,
    export_blend, find_blender,
};

/// Four heads over a floor and a truss, at full and in colour.
fn rig() -> (Scene, SceneValues) {
    let mut scene = Scene::default();
    for index in 0..4 {
        scene.fixtures.push(FixtureInstance {
            instance_id: Uuid::new_v4(),
            fixture_id: Uuid::new_v4(),
            name: "Mac Aura".into(),
            number: Some(index + 1),
            position: Vec3::new(-3.0 + index as f32 * 2.0, 6.0, -1.0),
            rotation_degrees: Vec3::ZERO,
            position_master: None,
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody {
                size: Vec3::new(0.3, 0.5, 0.3),
                kind: BodyKind::MovingHead,
            },
            patched: true,
            address: Some((1, 1 + index as u16 * 20)),
            model: None,
            fallback: None,
        });
        scene.emitters.push(EmitterInstance {
            fixture_index: index,
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
            beam_angle_degrees: 10.0,
            field_angle_degrees: 30.0,
            optics: EmitterOptics::default(),
            kind: EmitterKind::Beam,
            cells: EmitterLayoutCells::single(),
            laser: None,
            effect: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        });
    }
    scene.scenery.push(SceneryObject {
        id: Uuid::new_v4(),
        name: "Stage".into(),
        position: Vec3::new(0.0, -0.05, 0.0),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(14.0, 0.1, 10.0),
        colour: [0.12, 0.12, 0.13],
        roughness: 0.9,
        kind: SceneryKind::Floor,
        chords: 0,
    });
    scene.scenery.push(SceneryObject {
        id: Uuid::new_v4(),
        name: "Front truss".into(),
        position: Vec3::new(0.0, 6.4, -1.0),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(12.0, 0.3, 0.3),
        colour: [0.5, 0.5, 0.52],
        roughness: 0.4,
        kind: SceneryKind::Truss,
        chords: 4,
    });
    scene.recompute_bounds();

    let mut values = SceneValues::default();
    values.resize(scene.emitters.len());
    for (index, emitter) in values.emitters.iter_mut().enumerate() {
        emitter.intensity = 1.0;
        emitter.tilt = 0.5;
        emitter.zoom = 0.3;
        emitter.colour = match index % 4 {
            0 => [1.0, 0.2, 0.15],
            1 => [0.2, 0.5, 1.0],
            2 => [1.0, 0.8, 0.4],
            _ => [0.3, 1.0, 0.4],
        };
    }
    (scene, values)
}

#[test]
fn a_capture_becomes_a_blender_file() {
    let Some(blender) = find_blender(None) else {
        eprintln!("skipped: no Blender on this machine");
        return;
    };
    let root = std::env::temp_dir().join("viz-snapshot-blender-export");
    std::fs::remove_dir_all(&root).ok();
    let store = SnapshotStore::new(&root, 4);

    let (scene, values) = rig();
    let context = CaptureContext {
        show: "Export check".into(),
        source: "test".into(),
        scene_revision: 1,
        look: SnapshotLook {
            fog: 0.5,
            ambient: 0.06,
            exposure: 1.0,
        },
        camera: SnapshotCamera {
            position: [0.0, 3.0, 14.0],
            target: [0.0, 2.5, -1.0],
            fov_degrees: 45.0,
            orthographic: false,
            orthographic_size: 8.0,
        },
    };
    let entry = store
        .write(&capture(&scene, &values, &context))
        .expect("the capture is written");
    assert_eq!(entry.counts.live_beams, 4);

    let destination = entry.blend_destination();
    match export_blend(&entry.directory, &destination, None) {
        Ok(exported) => {
            assert!(
                destination.is_file(),
                "no file at {}",
                destination.display()
            );
            assert_eq!(exported.path, destination);
            assert!(
                std::fs::metadata(&destination).expect("metadata").len() > 1024,
                "an empty Blender file is not an exported rig"
            );
            assert_eq!(
                store.list()[0].blend.as_deref(),
                Some(destination.as_path()),
                "an exported capture should list the file it produced"
            );
            assert_scene_is_complete(&blender, &destination);
            assert_the_rig_actually_lights(&blender, &destination);
        }
        Err(ExportError::BlenderNotFound) => eprintln!("skipped: no Blender on this machine"),
        Err(error) => panic!("{error}"),
    }
    std::fs::remove_dir_all(&root).ok();
}

/// Check that each lamp's light is not sealed inside the lamp.
///
/// This is the failure worth a whole test of its own. Every count can be right, every angle can be
/// right, the file opens and looks correct in the outliner — and the render is black, because a
/// spot placed at the middle of the head that carries it is inside a closed body and nothing gets
/// out. It cannot be seen in the data; it can only be seen by asking whether anything is in the
/// way of the light.
fn assert_the_rig_actually_lights(blender: &Path, blend: &Path) {
    const REPORT: &str = r#"
import bpy, sys
from mathutils import Vector
scene = bpy.context.scene
graph = bpy.context.evaluated_depsgraph_get()
blocked = []
for light in [o for o in bpy.data.objects if o.type == 'LIGHT']:
    aim = light.matrix_world.to_quaternion() @ Vector((0.0, 0.0, -1.0))
    # Look a short way along the aim: anything hit that close is the fixture's own body.
    hit, _, _, _, obj, _ = scene.ray_cast(graph, light.matrix_world.translation, aim, distance=0.4)
    if hit:
        blocked.append('%s blocked by %s' % (light.name, obj.name))
print('LIGHTING blocked=%d %s' % (len(blocked), '; '.join(blocked[:4])))
"#;
    let script = blend.with_file_name("lighting.py");
    std::fs::write(&script, REPORT).expect("write the lighting script");
    let output = std::process::Command::new(blender)
        .args([
            "--background".as_ref(),
            blend.as_os_str(),
            "--python".as_ref(),
            script.as_os_str(),
        ])
        .output()
        .expect("Blender runs");
    let text = String::from_utf8_lossy(&output.stdout);
    let report = text
        .lines()
        .find(|line| line.starts_with("LIGHTING "))
        .unwrap_or_else(|| panic!("no lighting report from Blender:\n{text}"));
    assert!(
        report.contains("blocked=0"),
        "a lamp cannot light the stage from inside its own body: {report}"
    );
}

/// Open the exported file with Blender and count what is in it.
///
/// A `.blend` that opens is not the same as a rig that arrived: this is what catches an import
/// that silently produced no lights, or a haze volume that was never linked.
fn assert_scene_is_complete(blender: &Path, blend: &Path) {
    const REPORT: &str = r#"
import bpy, sys
lights = [o for o in bpy.data.objects if o.type == 'LIGHT']
meshes = [o for o in bpy.data.objects if o.type == 'MESH']
haze = [o for o in meshes if any(s.material and s.material.name.startswith('Haze') for s in o.material_slots)]
spots = [o for o in lights if o.data.type == 'SPOT']
aimed_down = [o for o in spots if (o.matrix_world.to_quaternion() @ __import__('mathutils').Vector((0,0,-1))).z < -0.9]
print('REPORT lights=%d spots=%d meshes=%d haze=%d cameras=%d engine=%s aimed_down=%d' % (
    len(lights), len(spots), len(meshes), len(haze),
    len([o for o in bpy.data.objects if o.type == 'CAMERA']),
    bpy.context.scene.render.engine, len(aimed_down)))
"#;
    let script = blend.with_file_name("report.py");
    std::fs::write(&script, REPORT).expect("write the report script");
    let output = std::process::Command::new(blender)
        .args([
            "--background".as_ref(),
            blend.as_os_str(),
            "--python".as_ref(),
            script.as_os_str(),
        ])
        .output()
        .expect("Blender runs");
    let text = String::from_utf8_lossy(&output.stdout);
    let report = text
        .lines()
        .find(|line| line.starts_with("REPORT "))
        .unwrap_or_else(|| panic!("no report from Blender:\n{text}"));

    assert!(report.contains("lights=4"), "{report}");
    assert!(report.contains("spots=4"), "{report}");
    assert!(
        report.contains("haze=1"),
        "the rig has no air in it: {report}"
    );
    assert!(report.contains("cameras=1"), "{report}");
    assert!(report.contains("engine=CYCLES"), "{report}");
    assert!(
        report.contains("aimed_down=4"),
        "every head hangs straight down, so every spot must: {report}"
    );
    // Bodies, yokes, lamp faces, the floor and a four-chord truss are all in there.
    let meshes: usize = report
        .split_whitespace()
        .find_map(|field| field.strip_prefix("meshes="))
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    assert!(meshes > 20, "the rig came across nearly empty: {report}");
}
