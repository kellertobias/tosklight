//! Real native job publication and invocation-time media/crop proof, outside ordinary CPU runs.
use super::*;
use glam::Vec3;
use std::{fs, io::BufReader, time::Instant};
use viz_render::{
    ResolvedCamera,
    media::{EDGE, FrozenMedia},
    still::StillOrigin,
};
use viz_scene::*;

fn mapped_walls() -> (Scene, SceneValues, ViewConfiguration) {
    let id = uuid::Uuid::new_v4();
    let mut rgba = vec![0; (EDGE * EDGE * 4) as usize];
    for (index, pixel) in rgba.chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(if index % (EDGE as usize) < (EDGE / 2) as usize {
            &[255, 0, 0, 255]
        } else {
            &[0, 0, 255, 255]
        });
    }
    let mut scene = Scene::default();
    scene.revision = 42;
    scene.bounds = Aabb {
        min: Vec3::new(-1.2, -0.6, -0.2),
        max: Vec3::new(1.2, 0.6, 0.2),
    };
    scene.media_sources.push(MediaSourceBinding {
        id,
        server_id: uuid::Uuid::nil(),
        host: String::new(),
        port: 0,
        advertised_source_id: 0,
        name: "Frozen split image".into(),
        aspect_ratio: Some(1.0),
        fallback_rgba: Some(rgba),
    });
    for (x, left) in [(-0.55, 0.0), (0.55, 0.5)] {
        scene.media_sections.push(MediaSection {
            id: uuid::Uuid::new_v4(),
            surface_id: uuid::Uuid::new_v4(),
            name: "Wall".into(),
            source_id: None,
            fallback_source_id: Some(id),
            position: Vec3::new(x, 0.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            size: Vec3::new(1.0, 1.0, 0.1),
            crop: MediaCrop {
                left,
                top: 0.0,
                width: 0.5,
                height: 1.0,
            },
            kind: MediaSectionKind::Tv {
                bezel_metres: 0.01,
                spill: 0.0,
            },
        });
    }
    let mut view = ViewConfiguration {
        mode: ViewMode::Lines3d,
        quality: RenderQuality::Draft,
        ambient: 0.0,
        floor_grid: false,
        show_labels: false,
        background: Some([0.0; 3]),
        ..Default::default()
    };
    view.camera.position = Vec3::new(0.0, 0.0, 4.0);
    view.camera.target = Vec3::ZERO;
    (
        scene,
        SceneValues {
            frame: 91,
            ..Default::default()
        },
        view,
    )
}
fn png_pixels(path: &std::path::Path) -> (u32, u32, Vec<u8>) {
    let mut decoder = png::Decoder::new(BufReader::new(fs::File::open(path).unwrap()));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    pixels.truncate(info.buffer_size());
    assert_eq!(info.color_type, png::ColorType::Rgba);
    (info.width, info.height, pixels)
}

#[test]
#[ignore = "requires explicitly serialized native GPU and real 4K PNG publication"]
fn native_job_publishes_frozen_preload_media_to_two_cropped_surfaces() {
    let (mut scene, mut values, view) = mapped_walls();
    let frozen = Arc::new(
        FrozenStill::new(
            &scene,
            &values,
            &view,
            3.25,
            FrozenMedia::default(),
            StillOrigin {
                pane_id: "preload-proof".into(),
                follow_preload: true,
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
    scene.media_sources[0]
        .fallback_rgba
        .as_mut()
        .unwrap()
        .fill(128);
    values.frame = 999;
    let root = std::env::var_os("LIGHT_STILL_IMAGE_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("native-still-proof-{}", uuid::Uuid::new_v4()))
        });
    fs::create_dir_all(&root).unwrap();
    let destination = root.join(format!(
        "frozen-preload-mapped-walls-{}.png",
        uuid::Uuid::new_v4()
    ));
    let mut jobs = StillJobs::default();
    let id = jobs
        .start(
            frozen,
            StillOptions {
                request_id: Some("native-proof".into()),
                destination: Some(destination.clone()),
                ..Default::default()
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let saved = loop {
        if let Some(status) = jobs.status().filter(|status| status.phase.is_terminal()) {
            break status;
        }
        assert!(Instant::now() < deadline, "native job did not finish");
        std::thread::sleep(Duration::from_millis(2));
    };
    let Phase::Saved { path } = saved.phase else {
        panic!("native job failed: {:?}", saved.phase)
    };
    assert_eq!(saved.job_id, id);
    assert_eq!(saved.values_frame, 91);
    assert!(saved.follow_preload);
    assert_eq!(saved.engine, Some(Engine::Ultra));
    assert_eq!(saved.time_seconds, 3.25);
    let (width, height, pixels) = png_pixels(&path);
    assert_eq!((width, height), (STILL_WIDTH, STILL_HEIGHT));
    for (x, channel) in [(-0.55, 0usize), (0.55, 2usize)] {
        let (px, py) = camera
            .project(Vec3::new(x, 0.0, 0.0), width as f32, height as f32)
            .unwrap();
        let start = (py.round() as usize * width as usize + px.round() as usize) * 4;
        let pixel = &pixels[start..start + 4];
        assert!(
            pixel[channel]
                > pixel[(channel + 1) % 3]
                    .saturating_mul(3)
                    .saturating_add(10)
                && pixel[channel]
                    > pixel[(channel + 2) % 3]
                        .saturating_mul(3)
                        .saturating_add(10),
            "wrong frozen crop: {pixel:?}"
        );
    }
    assert_eq!(view.mode, ViewMode::Lines3d);
    assert_eq!(view.quality, RenderQuality::Draft);
    println!("4K frozen media evidence: {}", path.display());
}
