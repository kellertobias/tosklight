//! Real native asymmetric optical projection, including source self-occlusion and roll.
use super::*;
use glam::Vec3;
use std::{fs, time::Instant};
use viz_render::{CapturedImage, ResolvedCamera, media::FrozenMedia, still::StillOrigin};
use viz_scene::*;

pub(super) fn rig() -> (Scene, SceneValues, ViewConfiguration) {
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
        lamp_response: Default::default(),
        position: Vec3::new(0.0, 4.0, 0.0),
        installed_colour: [1.0; 3],
        body: FixtureBody {
            kind: BodyKind::Lantern,
            size: Vec3::splat(0.002),
        },
        ..Default::default()
    });
    scene.emitters.push(EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "Asymmetric frozen gate".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 50.0,
        field_angle_degrees: 50.0,
        optics: EmitterOptics {
            sharpness: 1.0,
            uniformity: 1.0,
            ..Default::default()
        },
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [true; 4],
        shaper_roles: [true; 4],
        live_shaper_rotation_role: true,
    });
    scene.scenery.push(SceneryObject {
        kind: SceneryKind::Floor,
        position: Vec3::new(0.0, -0.05, 0.0),
        size: Vec3::new(12.0, 0.1, 12.0),
        colour: [0.7; 3],
        roughness: 1.0,
        ..Default::default()
    });
    scene.recompute_bounds();
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 1.0;
    values.emitters[0].colour = [1.0; 3];
    values.emitters[0].shaper_blades[0] = 1.0;
    values.atmosphere.density = 0.0;
    let mut view = ViewConfiguration {
        mode: ViewMode::Full3d,
        quality: RenderQuality::Draft,
        ambient: 0.0,
        floor_grid: false,
        show_labels: false,
        background: Some([0.0; 3]),
        ..Default::default()
    };
    view.camera.position = Vec3::new(0.0, 7.0, 1.0);
    view.camera.target = Vec3::ZERO;
    view.camera.up = Vec3::NEG_Z;
    (scene, values, view)
}
fn receiver(image: &CapturedImage, camera: &ResolvedCamera, x: f32) -> f32 {
    let (px, py) = camera
        .project(
            Vec3::new(x, 0.001, 0.0),
            image.width as f32,
            image.height as f32,
        )
        .unwrap();
    let mut sum = 0.0;
    for dy in -4..=4 {
        for dx in -4..=4 {
            let offset = (((py.round() as i32 + dy) as usize * image.width as usize)
                + (px.round() as i32 + dx) as usize)
                * 4;
            sum += image.rgba[offset..offset + 3]
                .iter()
                .map(|v| f32::from(*v))
                .sum::<f32>()
                / 3.0;
        }
    }
    sum / 81.0
}
#[test]
#[ignore = "requires serialized native GPU and real 4K optical pixel proof"]
fn native_asymmetric_gate_preserves_receiver_orientation() {
    render_optical_case(false);
}

#[test]
#[ignore = "requires serialized native GPU and real 4K installed Fresnel cut proof"]
fn native_installed_manual_soft_fresnel_preserves_receiver_orientation() {
    render_optical_case(true);
}

fn optical_case(manual: bool) -> (Scene, SceneValues, ViewConfiguration) {
    let (mut scene, mut values, view) = rig();
    if manual {
        let emitter = &mut scene.emitters[0];
        emitter.live_shaper_angle_roles = [false; 4];
        emitter.shaper_roles = [false; 4];
        emitter.live_shaper_rotation_role = false;
        emitter.beam_angle_degrees = 48.0;
        emitter.field_angle_degrees = 48.0;
        emitter.optics.sharpness = 0.42;
        emitter.optics.uniformity = 0.55;
        scene.fixtures[0].installed_shaper_blades[0] = 0.5;
        scene.fixtures[0].shaper_degrees = Some(30.0);
        values.emitters[0].shaper_blades = [0.0; 4];
        values.emitters[0].shaper_rotation_degrees = 0.0;
        values.emitters[0].shaper_blade_angles_degrees = [0.0; 4];
    }
    (scene, values, view)
}

fn retain_native_manual_image(root: &std::path::Path, image: &CapturedImage) {
    let path = root.join(format!(
        "native-manual-soft-fresnel-{}.png",
        uuid::Uuid::new_v4()
    ));
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let mut encoder = png::Encoder::new(file, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&image.rgba).unwrap();
    writer.finish().unwrap();
    println!("Native manual Fresnel evidence: {}", path.display());
}

fn render_optical_case(manual: bool) {
    let root = PathBuf::from(
        std::env::var_os("LIGHT_STILL_IMAGE_EVIDENCE_DIR").expect("canonical evidence dir"),
    );
    fs::create_dir_all(&root).unwrap();
    let (scene, values, view) = optical_case(manual);
    let frozen = Arc::new(
        FrozenStill::new(
            &scene,
            &values,
            &view,
            12.5,
            FrozenMedia::default(),
            StillOrigin {
                pane_id: "asymmetric-optics".into(),
                follow_preload: false,
            },
        )
        .unwrap(),
    );
    let camera = ResolvedCamera::resolve(
        &view.camera,
        ViewMode::Full3d,
        STILL_WIDTH as f32 / STILL_HEIGHT as f32,
        scene.bounds,
    );
    let mut native = frozen.begin_native().unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let native_image = loop {
        if let Some(image) = native.poll().unwrap() {
            break image;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    };
    drop(native);
    if manual {
        assert_eq!(
            (native_image.width, native_image.height),
            (STILL_WIDTH, STILL_HEIGHT)
        );
        retain_native_manual_image(&root, &native_image);
    }
    let native_pair = [
        receiver(&native_image, &camera, -0.65),
        receiver(&native_image, &camera, 0.65),
    ];
    println!("Native receiver levels: {native_pair:?}");
    assert!(native_pair[0].max(native_pair[1]) > native_pair[0].min(native_pair[1]) + 25.0);
    assert_eq!(view.quality, RenderQuality::Draft);
    if manual {
        assert_eq!(scene.fixtures[0].installed_shaper_blades[0], 0.5);
        assert_eq!(values.emitters[0].shaper_blades, [0.0; 4]);
        assert!(!scene.emitters[0].live_shaper_rotation_role);
        assert_eq!(scene.fixtures[0].shaper_degrees, Some(30.0));
    }
}
