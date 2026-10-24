//! Real two-engine asymmetric optical projection, including source self-occlusion and roll.
use super::*;
use glam::Vec3;
use std::{fs, io::BufReader, time::Instant};
use viz_render::{CapturedImage, ResolvedCamera, media::FrozenMedia, still::StillOrigin};
use viz_scene::*;

pub(super) fn rig() -> (Scene, SceneValues, ViewConfiguration) {
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
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
fn decoded(path: &std::path::Path) -> CapturedImage {
    let mut decoder = png::Decoder::new(BufReader::new(fs::File::open(path).unwrap()));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut rgba).unwrap();
    rgba.truncate(info.buffer_size());
    assert_eq!(info.color_type, png::ColorType::Rgba);
    CapturedImage {
        width: info.width,
        height: info.height,
        rgba,
    }
}
#[test]
#[ignore = "requires installed Blender, serialized native GPU and real 4K optical pixel proof"]
fn ultimate_asymmetric_gate_matches_native_receiver_orientation() {
    let blender = PathBuf::from(
        std::env::var_os("LIGHT_STILL_TEST_BLENDER").expect("explicit installed Blender"),
    );
    let root = PathBuf::from(
        std::env::var_os("LIGHT_STILL_IMAGE_EVIDENCE_DIR").expect("canonical evidence dir"),
    );
    fs::create_dir_all(&root).unwrap();
    let (scene, values, view) = rig();
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
    let native_pair = [
        receiver(&native_image, &camera, -0.65),
        receiver(&native_image, &camera, 0.65),
    ];
    println!("Native receiver levels: {native_pair:?}");
    assert!(native_pair[0].max(native_pair[1]) > native_pair[0].min(native_pair[1]) + 25.0);
    let mut jobs = StillJobs::default();
    let id = jobs
        .start(
            frozen,
            StillOptions {
                configured_blender: Some(blender),
                probe_timeout: Duration::from_secs(30),
                ultimate_timeout: Duration::from_secs(300),
                destination: Some(root.join(format!(
                    "ultimate-asymmetric-optics-{}.png",
                    uuid::Uuid::new_v4()
                ))),
                ..Default::default()
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(340);
    let mut chose = false;
    let status = loop {
        let status = jobs.status().unwrap();
        if let Phase::NeedsChoice {
            ultimate_reasons, ..
        } = &status.phase
        {
            assert!(ultimate_reasons.is_empty(), "{ultimate_reasons:?}");
            assert!(!chose);
            chose = true;
            jobs.choose(&id, Engine::Ultimate).unwrap();
        }
        if status.phase.is_terminal() {
            break status;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    };
    let Phase::Saved { path } = status.phase else {
        panic!("{:?}", status.phase)
    };
    let image = decoded(&path);
    assert_eq!((image.width, image.height), (STILL_WIDTH, STILL_HEIGHT));
    let pair = [
        receiver(&image, &camera, -0.65),
        receiver(&image, &camera, 0.65),
    ];
    println!(
        "Blender receiver levels: {pair:?}; image {}",
        path.display()
    );
    let bright = usize::from(native_pair[1] > native_pair[0]);
    assert!(
        pair[bright] > pair[1 - bright] + 25.0,
        "frozen native shutter orientation lost: {pair:?}"
    );
    assert_eq!(view.quality, RenderQuality::Draft);
}
