//! Explicit real native/Cycles evidence for a tiny, obliquely lit audience, never a perf run.
use super::*;
use glam::Vec3;
use std::{fs, io::BufReader, path::Path, time::Instant};
use viz_render::{CapturedImage, media::FrozenMedia, still::StillOrigin};
use viz_scene::*;
fn frozen(detailed: bool) -> FrozenStill {
    let (mut scene, mut values, mut view) = super::optics_tests::rig();
    scene.fixtures[0].position = Vec3::new(1.5, 4.0, 2.0);
    scene.emitters[0].beam_angle_degrees = 110.0;
    scene.emitters[0].field_angle_degrees = 110.0;
    scene.emitters[0].optics.sharpness = 0.42;
    scene.emitters[0].optics.uniformity = 0.55;
    values.emitters[0].shaper_blades = [0.0; 4];
    values.emitters[0].intensity = 0.35;
    scene.scenery[0].colour = [0.12; 3];
    for (index, posture) in [CrowdPosture::StandingStill, CrowdPosture::Sitting]
        .into_iter()
        .enumerate()
    {
        scene.crowds.push(CrowdArea {
            id: uuid::Uuid::from_u128(index as u128 + 1),
            name: format!("Person {index}"),
            position: Vec3::new(index as f32 * 1.4 - 0.7, 0.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            width_metres: 1.0,
            depth_metres: 1.0,
            posture,
            density: CrowdDensity::Sparse,
            seed: 42 + index as u64,
        });
    }
    scene.recompute_bounds();
    view.camera.position = Vec3::new(3.4, 2.5, 4.5);
    view.camera.target = Vec3::new(0.0, 0.8, 0.0);
    view.camera.up = Vec3::Y;
    view.ambient = 0.02;
    view.detailed_people = detailed; // Live stays Draft; capture performs its separate Ultra upgrade.
    let result = FrozenStill::new(
        &scene,
        &values,
        &view,
        12.5,
        FrozenMedia::default(),
        StillOrigin {
            pane_id: "detailed-people".into(),
            follow_preload: false,
        },
    )
    .unwrap();
    assert_eq!(view.quality, RenderQuality::Draft);
    result
}
fn native(frozen: &FrozenStill) -> CapturedImage {
    let mut job = frozen.begin_native().expect("explicit native GPU required");
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(image) = job.poll().unwrap() {
            assert!(!job.stats().degraded);
            return image;
        }
        assert!(Instant::now() < deadline, "native people capture timeout");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn retain(path: &Path, image: &CapturedImage) {
    let file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap();
    let mut encoder = png::Encoder::new(file, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&image.rgba).unwrap();
    writer.finish().unwrap();
}
fn decoded(path: &Path) -> CapturedImage {
    let mut decoder = png::Decoder::new(BufReader::new(fs::File::open(path).unwrap()));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut rgba).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    rgba.truncate(info.buffer_size());
    CapturedImage {
        width: info.width,
        height: info.height,
        rgba,
    }
}
fn assert_image(image: &CapturedImage, name: &str) {
    assert_eq!((image.width, image.height), (STILL_WIDTH, STILL_HEIGHT));
    assert!(
        image
            .rgba
            .chunks_exact(4)
            .filter(|p| p[..3].iter().any(|v| *v > 8))
            .count()
            > 1000,
        "{name} actual 4K image must not be black"
    );
}
#[test]
#[ignore = "requires explicitly serialized native GPU, installed Blender, and human 3D audience image review"]
fn detailed_people_native_ultra_and_real_ultimate_small_oblique_audience() {
    let root = PathBuf::from(
        std::env::var_os("LIGHT_STILL_IMAGE_EVIDENCE_DIR")
            .expect("canonical tl876-people evidence root"),
    );
    assert!(root.is_absolute());
    fs::create_dir_all(&root).unwrap();
    let root = root.join(format!("people-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let off = frozen(false);
    let on = Arc::new(frozen(true));
    let simple = native(&off);
    let detailed = native(&on);
    assert_image(&simple, "native silhouettes");
    assert_image(&detailed, "native detailed");
    let changed = simple
        .rgba
        .chunks_exact(4)
        .zip(detailed.rgba.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|c| (i16::from(a[c]) - i16::from(b[c])).abs() > 4))
        .count();
    assert!(
        changed > 500,
        "detail must change actual audience pixels: {changed}"
    );
    retain(&root.join("native-ultra-silhouettes.png"), &simple);
    retain(&root.join("native-ultra-detailed.png"), &detailed);
    let blender = PathBuf::from(
        std::env::var_os("LIGHT_STILL_TEST_BLENDER").expect("explicit installed Blender"),
    );
    let mut jobs = StillJobs::default();
    let id = jobs
        .start(
            on.clone(),
            StillOptions {
                destination: Some(root.join("ultimate-detailed.png")),
                configured_blender: Some(blender),
                probe_timeout: Duration::from_secs(30),
                ultimate_timeout: Duration::from_secs(300),
                ..Default::default()
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(340);
    let mut chose = false;
    loop {
        let status = jobs.status().unwrap();
        if let Phase::NeedsChoice {
            ultimate_reasons, ..
        } = &status.phase
        {
            assert!(ultimate_reasons.is_empty(), "{ultimate_reasons:?}");
            assert!(!chose);
            jobs.choose(&id, Engine::Ultimate).unwrap();
            chose = true;
        }
        if status.phase.is_terminal() {
            let Phase::Saved { path } = status.phase else {
                panic!("{:?}", status.phase)
            };
            assert!(chose);
            assert_eq!(status.engine, Some(Engine::Ultimate));
            assert_image(&decoded(&path), "actual Blender detailed");
            break;
        }
        assert!(Instant::now() < deadline, "Ultimate people job timeout");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(on.view().quality, RenderQuality::Draft);
    assert!(on.view().detailed_people);
    println!("TL876 actual 3D crowd visual evidence: {}", root.display());
}
