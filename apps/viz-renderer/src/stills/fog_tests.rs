//! Opt-in atmospheric image proof, distinct from exact CPU field tests and live FPS acceptance.
use super::*;
use glam::Vec3;
use std::{
    fs,
    io::{BufReader, Write},
    path::Path,
    time::Instant,
};
use viz_render::{CapturedImage, Renderer, ResolvedCamera, media::FrozenMedia, still::StillOrigin};
use viz_scene::{RenderQuality, ViewMode};

fn native_image(frozen: &FrozenStill) -> CapturedImage {
    let mut native = frozen.begin_native().expect("separate native 4K renderer");
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(image) = native.poll().unwrap() {
            assert!(!native.stats().degraded);
            assert_eq!(native.stats().render_scale, 1.0);
            return image;
        }
        assert!(
            Instant::now() < deadline,
            "native frozen fog readback timed out"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
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
fn air_level(image: &CapturedImage, camera: &ResolvedCamera) -> f32 {
    let (px, py) = camera
        .project(
            Vec3::new(0.0, 2.0, 0.0),
            image.width as f32,
            image.height as f32,
        )
        .unwrap();
    let mut sum = 0.0;
    let mut samples = 0;
    for dy in -8..=8 {
        for dx in -8..=8 {
            let x = px.round() as i32 + dx;
            let y = py.round() as i32 + dy;
            assert!(x >= 0 && y >= 0 && x < image.width as i32 && y < image.height as i32);
            let offset = (y as usize * image.width as usize + x as usize) * 4;
            sum += image.rgba[offset..offset + 3]
                .iter()
                .map(|v| f32::from(*v))
                .sum::<f32>()
                / 3.0;
            samples += 1;
        }
    }
    sum / samples as f32
}
fn assert_visible_air(image: &CapturedImage, camera: &ResolvedCamera, engine: &str) {
    assert_eq!((image.width, image.height), (STILL_WIDTH, STILL_HEIGHT));
    let bright = image
        .rgba
        .chunks_exact(4)
        .filter(|p| p[..3].iter().any(|c| *c > 3))
        .count();
    let level = air_level(image, camera);
    println!("{engine}: visible air pixels={bright}, empty-air center level={level:.3}");
    // There is no floor/receiver geometry and only a 2mm housing. Thousands of bright pixels
    // plus a nonzero probe two metres below the source establish actual volumetric visibility.
    assert!(
        bright > 1000,
        "{engine} produced an all-black or missing-air image"
    );
    assert!(
        level > 1.0,
        "{engine} must light empty air inside the frozen shaft"
    );
}
fn published_image(frozen: Arc<FrozenStill>, destination: PathBuf) -> (CapturedImage, Status) {
    let mut jobs = StillJobs::default();
    let _id = jobs
        .start(
            frozen,
            StillOptions {
                destination: Some(destination),
                ..Default::default()
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let status = jobs.status().unwrap();
        if status.phase.is_terminal() {
            let Phase::Saved { path } = &status.phase else {
                panic!("{:?}", status.phase)
            };
            assert_eq!(status.engine, Some(Engine::Ultra));
            println!(
                "Published native frozen fog visual evidence: {}",
                path.display()
            );
            return (decoded(path), status);
        }
        assert!(Instant::now() < deadline, "Native fog image job timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "requires serialized real native 4K fog rendering; retains visual evidence"]
fn frozen_default_cloudy_fog_is_visible_in_published_native_4k_without_live_draft_mutation() {
    let root = PathBuf::from(
        std::env::var_os("LIGHT_STILL_IMAGE_EVIDENCE_DIR").expect("canonical evidence directory"),
    );
    fs::create_dir_all(&root).unwrap();
    let (mut scene, mut values, mut view) = super::optics_tests::rig();
    scene.scenery.clear();
    scene.recompute_bounds();
    values.emitters[0].shaper_blades = [0.0; 4];
    values.atmosphere.density = 0.15;
    values.frame = 851;
    view.camera.position = Vec3::new(7.0, 4.5, 8.0);
    view.camera.target = Vec3::new(0.0, 2.0, 0.0);
    view.camera.up = Vec3::Y;
    view.fog_variation.lamp_cloudiness = 0.7;
    view.fog_variation.lamp_turbulence = 1.0;
    assert_eq!(view.quality, RenderQuality::Draft);
    let time = 42.5;
    let frozen = Arc::new(
        FrozenStill::new(
            &scene,
            &values,
            &view,
            time,
            FrozenMedia::default(),
            StillOrigin {
                pane_id: "frozen-fog-proof".into(),
                follow_preload: false,
            },
        )
        .unwrap(),
    );
    let mut live = Renderer::headless(256, 144).unwrap();
    let before = live
        .capture(
            &scene,
            &values,
            &view,
            &viz_render::Overlay::default(),
            time,
        )
        .unwrap();
    let camera = ResolvedCamera::resolve(&view.camera, ViewMode::Full3d, 16.0 / 9.0, scene.bounds);
    let native = native_image(&frozen);
    assert_visible_air(&native, &camera, "Native Ultra");
    let native_path = root.join(format!("native-frozen-fog-{}.png", uuid::Uuid::new_v4()));
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&native_path)
        .unwrap()
        .write_all(&crate::png::encode_rgba(
            native.width,
            native.height,
            &native.rgba,
        ))
        .unwrap();
    println!(
        "Native frozen fog visual evidence: {}",
        native_path.display()
    );
    let destination = root.join(format!(
        "published-native-frozen-fog-{}.png",
        uuid::Uuid::new_v4()
    ));
    let (published, status) = published_image(frozen.clone(), destination);
    assert_visible_air(&published, &camera, "Published native Ultra");
    assert_eq!(status.time_seconds, time);
    assert_eq!(status.values_frame, 851);
    assert_eq!(frozen.time_seconds(), time);
    assert_eq!(frozen.view().fog_variation.lamp_cloudiness, 0.7);
    let after = live
        .capture(
            &scene,
            &values,
            &view,
            &viz_render::Overlay::default(),
            time,
        )
        .unwrap();
    assert_eq!(
        before.rgba, after.rgba,
        "isolated stills cannot mutate the live Draft renderer"
    );
    assert_eq!(view.quality, RenderQuality::Draft);
    assert_eq!(values.atmosphere.density, 0.15);
}
