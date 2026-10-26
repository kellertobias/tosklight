use super::*;
use crate::instances::{BeamInstance, FrameInstances, GpuLight, MeshInstance, MeshKind};
use crate::renderer::Globals;
use bytemuck::Zeroable;
use glam::Mat4;
fn inputs() -> (viz_scene::Scene, FrameInstances, Globals) {
    let scene = viz_scene::Scene::default();
    let mut frame = FrameInstances::default();
    frame.lights.push(GpuLight::zeroed());
    frame.beams.push(BeamInstance::zeroed());
    let mut instance = MeshInstance::zeroed();
    instance.model = Mat4::IDENTITY.to_cols_array_2d();
    frame.meshes.push((MeshKind::Cube, vec![instance]));
    (scene, frame, Globals::default())
}
fn key(
    scene: &viz_scene::Scene,
    frame: &FrameInstances,
    globals: &Globals,
    generation: u64,
) -> Key {
    Key::build(scene, frame, globals, generation, (160, 90), 1, 1)
}
#[test]
fn only_submitted_exact_dependencies_hit_and_abandoned_or_overwritten_candidates_never_hit() {
    let (scene, mut frame, globals) = inputs();
    let mut cache = BeamCache::default();
    assert!(!cache.prepare(key(&scene, &frame, &globals, 0)));
    cache.begin_frame(); // dropped encoder/acquire failure; pending identity did not commit
    assert!(!cache.prepare(key(&scene, &frame, &globals, 0)));
    cache.submitted();
    assert!(cache.prepare(key(&scene, &frame, &globals, 0)));
    frame.lights[0].params[0] = 0.5;
    assert!(!cache.prepare(key(&scene, &frame, &globals, 0)));
    // A second failed encoder cannot leave an old identity attached to a single overwritten target.
    cache.begin_frame();
    frame.lights[0].params[0] = 0.0;
    assert!(!cache.prepare(key(&scene, &frame, &globals, 0)));
    cache.submitted();
    cache.resources_changed();
    assert!(!cache.prepare(key(&scene, &frame, &globals, cache.resource_generation)));
}
#[test]
fn clocks_media_colour_and_laser_only_changes_do_not_change_beam_dependencies() {
    let (scene, mut frame, mut globals) = inputs();
    let before = key(&scene, &frame, &globals, 0);
    globals.camera_position[3] = 2.0;
    globals.params3[2] = 3.0;
    globals.params3[3] = 4.0;
    globals.params[0] = 1.7;
    globals.params2[2] = 0.4;
    globals.params5[2] = 0.5;
    globals.params5[3] = 0.7;
    frame.meshes[0].1[0].emissive = [0.8, 0.2, 0.4, 0.0];
    frame.meshes[0].1[0].base_colour = [0.3; 4];
    frame.lasers.push(crate::instances::LaserInstance::zeroed());
    assert_eq!(before, key(&scene, &frame, &globals, 0));
}
#[test]
fn exact_light_camera_geometry_curtain_and_resource_dependencies_never_false_hit() {
    let (mut scene, mut frame, mut globals) = inputs();
    let original = key(&scene, &frame, &globals, 0);
    for word in [0, 1, 2] {
        globals.camera_position[word] = 1.0;
        assert_ne!(original, key(&scene, &frame, &globals, 0));
        globals.camera_position[word] = 0.0;
    }
    globals.view_projection[0][0] = 1.0;
    assert_ne!(original, key(&scene, &frame, &globals, 0));
    globals = Globals::default();
    globals.params[1] = 0.3;
    assert_ne!(original, key(&scene, &frame, &globals, 0));
    globals = Globals::default();
    globals.params2[1] = 12.0;
    assert_ne!(original, key(&scene, &frame, &globals, 0));
    globals = Globals::default();
    frame.lights[0].colour_intensity[0] = f32::from_bits(1);
    assert_ne!(original, key(&scene, &frame, &globals, 0));
    frame.lights[0].colour_intensity[0] = 0.0;
    frame.beams[0].params[0] = 1.0;
    assert_ne!(original, key(&scene, &frame, &globals, 0));
    frame.beams[0].params[0] = 0.0;
    frame.meshes[0].1[0].model[3][1] = 1.0;
    assert_ne!(original, key(&scene, &frame, &globals, 0));
    frame.meshes[0].1[0].model[3][1] = 0.0;
    frame.meshes[0].1[0].surface[1] = 1.0;
    assert_ne!(original, key(&scene, &frame, &globals, 0));
    frame.meshes[0].1[0].surface[1] = 0.0;
    frame
        .curtain_blockers
        .push(crate::instances::GpuCurtainBlocker::zeroed());
    assert_ne!(original, key(&scene, &frame, &globals, 0));
    frame.curtain_blockers.clear();
    assert_ne!(original, key(&scene, &frame, &globals, 1));
    assert_ne!(
        original,
        Key::build(&scene, &frame, &globals, 0, (161, 90), 1, 1)
    );
    scene.revision += 1;
    assert_ne!(original, key(&scene, &frame, &globals, 0));
}

#[test]
fn fused_absent_beams_and_nonuniform_or_nonfinite_fog_cannot_reuse() {
    assert!(BeamCache::eligible(true, false, 0.0));
    for (beams, fused, fog) in [
        (false, false, 0.0),
        (true, true, 0.0),
        (true, false, 0.01),
        (true, false, f32::NAN),
        (true, false, f32::INFINITY),
    ] {
        assert!(!BeamCache::eligible(beams, fused, fog));
    }
}

#[test]
fn media_source_crop_and_gain_are_colour_only_but_panel_depth_pose_is_exact() {
    let (scene, mut frame, globals) = inputs();
    frame.media_panels.push(crate::instances::MediaPanel {
        source_id: None,
        fallback_source_id: None,
        model: Mat4::IDENTITY,
        crop: [0.0, 0.0, 1.0, 1.0],
        material: [1.0, 1.0, 0.0, 0.0],
    });
    let first = key(&scene, &frame, &globals, 0);
    frame.media_panels[0].source_id = Some(viz_scene::uuid::Uuid::new_v4());
    frame.media_panels[0].crop = [0.3, 0.0, 0.3, 1.0];
    frame.media_panels[0].material[1] = 0.3;
    assert_eq!(first, key(&scene, &frame, &globals, 0));
    frame.media_panels[0].model = Mat4::from_translation(glam::Vec3::Z);
    assert_ne!(first, key(&scene, &frame, &globals, 0));
}
