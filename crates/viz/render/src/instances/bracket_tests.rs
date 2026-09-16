//! A bracketed lantern in 3D: the hanging hardware stays, the body and its beam turn about the
//! hinge — the same turn the CAD's side drawing gives it.

use super::*;
use viz_scene::{FixtureBody, bracket_turned, euler_degrees, is_mounting_hardware};

fn shipped_fresnel() -> viz_scene::FixtureModel {
    let shipped = viz_project::all_default_models()
        .iter()
        .find(|model| model.name == "fresnel-barn-doors")
        .expect("the Fresnel ships");
    viz_project::read_shipped_model(shipped).expect("the Fresnel reads")
}

fn fresnel_scene(bracket: f32, yaw: f32) -> (Scene, viz_scene::FixtureModel, f32) {
    let model = shipped_fresnel();
    let size = model.extent * 2.0 * 0.8;
    let scale = model.scale_to(size);
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
        position: Vec3::new(1.0, 6.0, -2.0),
        rotation_degrees: Vec3::new(0.0, yaw, 0.0),
        bracket_degrees: bracket,
        bracket_hinge: model.bracket_hinge.map(|hinge| hinge * scale),
        body: FixtureBody {
            size,
            kind: viz_scene::BodyKind::Lantern,
        },
        model: Some(0),
        ..FixtureInstance::default()
    });
    scene.models.push(model.clone());
    (scene, model, scale)
}

fn part_transforms(scene: &Scene, model: &viz_scene::FixtureModel) -> Vec<(String, Mat4)> {
    let mut frame = FrameInstances::default();
    push_model(&mut frame, scene, &scene.fixtures[0], 0, 0, model, &[], &[]);
    model
        .parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let transform = frame
                .meshes
                .iter()
                .find(|(kind, _)| *kind == MeshKind::ModelPart(0, index as u32))
                .map(|(_, instances)| Mat4::from_cols_array_2d(&instances[0].model))
                .expect("every part is drawn");
            (part.name.clone(), transform)
        })
        .collect()
}

/// The regression the issue names: a Fresnel at a 45° bracket angle, in several orientations.
#[test]
fn a_fresnel_at_45_degrees_turns_its_body_and_keeps_its_bracket() {
    for yaw in [0.0_f32, 90.0, -135.0] {
        let (scene, model, scale) = fresnel_scene(45.0, yaw);
        let fixture = &scene.fixtures[0];
        let hinge = model.bracket_hinge.expect("the Fresnel has a hinge");
        let mount = Mat4::from_rotation_translation(
            euler_degrees(fixture.rotation_degrees),
            fixture.position,
        ) * Mat4::from_scale(Vec3::splat(scale));
        let parts = part_transforms(&scene, &model);
        assert!(parts.iter().any(|(name, _)| is_mounting_hardware(name)));
        assert!(parts.iter().any(|(name, _)| name == "body"));
        let lens = model.emitter_anchor.expect("the Fresnel has a lens");
        for (name, transform) in &parts {
            if is_mounting_hardware(name) {
                assert!(
                    transform.abs_diff_eq(mount, 1e-5),
                    "{name} stays where it hangs at yaw {yaw}"
                );
                continue;
            }
            // Every other part is the lamp: turned 45° nose-down about the hinge.
            let expected = mount.transform_point3(bracket_turned(lens, hinge, 45.0));
            let actual = transform.transform_point3(lens);
            assert!((actual - expected).length() < 1e-5, "{name} at yaw {yaw}");
            let hinge_world = transform.transform_point3(hinge);
            assert!((hinge_world - mount.transform_point3(hinge)).length() < 1e-5);
            let down = transform.transform_vector3(Vec3::NEG_Y).normalize();
            let hanging = mount.transform_vector3(Vec3::NEG_Y).normalize();
            assert!(
                (down.angle_between(hanging).to_degrees() - 45.0).abs() < 1e-3,
                "{name} is turned exactly 45° at yaw {yaw}"
            );
        }

        // The beam leaves the turned lens and points where the turned body does.
        let emitter = EmitterInstance {
            fixture_index: 0,
            head_index: 0,
            label: "Lens".into(),
            local_origin: lens * scale,
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 8.0,
            field_angle_degrees: 40.0,
            optics: viz_scene::EmitterOptics::default(),
            kind: viz_scene::EmitterKind::Beam,
            cells: viz_scene::EmitterLayoutCells::single(),
            laser: None,
            effect: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        };
        let pose = emitter_pose(fixture, &emitter, 0.0, 0.0, 0.0, &[]);
        let expected = mount.transform_point3(bracket_turned(lens, hinge, 45.0));
        assert!(
            (pose.origin - expected).length() < 1e-5,
            "beam origin at yaw {yaw}"
        );
        let direction = euler_degrees(fixture.rotation_degrees)
            * Quat::from_rotation_x(45f32.to_radians())
            * Vec3::NEG_Y;
        assert!((pose.direction - direction).length() < 1e-5);
    }
}

/// With no bracket angle the lamp hangs exactly as modelled, hardware and body in one frame.
#[test]
fn a_level_bracket_draws_every_part_in_the_mount_frame() {
    let (scene, model, _) = fresnel_scene(0.0, 30.0);
    let parts = part_transforms(&scene, &model);
    let first = parts[0].1;
    assert!(
        parts
            .iter()
            .all(|(_, transform)| transform.abs_diff_eq(first, 1e-6))
    );
}
