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

#[test]
#[ignore = "requires a real Pixel CITP test-pattern Program, serialized GPU and explicit evidence directory"]
fn native_job_freezes_real_pixel_program_on_independently_cropped_surfaces() {
    let endpoint: std::net::SocketAddr = std::env::var("LIGHT_MEDIA_CITP_PROOF_ENDPOINT")
        .expect("set the isolated Pixel CITP endpoint")
        .parse()
        .unwrap();
    let source: u16 = std::env::var("LIGHT_MEDIA_CITP_PROOF_SOURCE")
        .expect("set the enumerated numeric Program source")
        .parse()
        .unwrap();
    let root = PathBuf::from(
        std::env::var_os("LIGHT_STILL_IMAGE_EVIDENCE_DIR")
            .expect("set a canonical artifact evidence directory"),
    );
    let (mut scene, values, view) = mapped_walls();
    let binding = &mut scene.media_sources[0];
    binding.host = endpoint.ip().to_string();
    binding.port = endpoint.port();
    binding.advertised_source_id = source;
    binding.name = "Actual Pixel Program".into();
    binding.fallback_rgba = None;
    let source_id = binding.id;
    for section in &mut scene.media_sections {
        section.source_id = Some(source_id);
        section.fallback_source_id = None;
    }
    let mut workers = crate::media_worker::MediaWorkers::default();
    workers.reconcile(&scene);
    let mut renderer = viz_render::Renderer::headless(8, 8).expect("real GPU required");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut first = None;
    loop {
        let mut changed = false;
        for frame in workers.drain() {
            if frame.rgba.chunks_exact(4).any(|p| p[0] > 80 && p[1] < 50) {
                changed = first
                    .as_ref()
                    .is_some_and(|old: &Vec<u8>| *old != frame.rgba);
                first.get_or_insert_with(|| frame.rgba.clone());
                renderer.update_media_frame(&frame).unwrap();
            }
        }
        if changed {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "actual Program did not deliver changing test-pattern pixels"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let camera = ResolvedCamera::resolve(
        &view.camera,
        ViewMode::Full3d,
        STILL_WIDTH as f32 / STILL_HEIGHT as f32,
        scene.bounds,
    );
    let frozen = Arc::new(
        FrozenStill::new(
            &scene,
            &values,
            &view,
            3.25,
            renderer.frozen_media(),
            StillOrigin {
                pane_id: "actual-program".into(),
                follow_preload: false,
            },
        )
        .unwrap(),
    );
    assert!(
        frozen.media_notice().is_none(),
        "actual live pixels must be available at invocation"
    );
    // Retire every live pixel after invocation. The queued job must retain its own picture.
    renderer.retain_media_sources(&Default::default());
    drop(workers);
    fs::create_dir_all(&root).unwrap();
    let destination = root.join("frozen-real-pixel-program.png");
    let mut jobs = StillJobs::default();
    jobs.start(
        frozen,
        StillOptions {
            destination: Some(destination),
            ..Default::default()
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let saved = loop {
        if let Some(status) = jobs.status().filter(|s| s.phase.is_terminal()) {
            break status;
        }
        assert!(Instant::now() < deadline, "native job did not finish");
        std::thread::sleep(Duration::from_millis(2));
    };
    let Phase::Saved { path } = saved.phase else {
        panic!("native job failed: {:?}", saved.phase)
    };
    let (width, height, pixels) = png_pixels(&path);
    assert_eq!((width, height), (STILL_WIDTH, STILL_HEIGHT));
    for (x, channel) in [(-0.55, 0usize), (0.55, 1usize)] {
        // The real clip has a moving white vertical marker. Three separated samples keep
        // that marker from making a correctly mapped quadrant look like the wrong source.
        let matching = [-0.25, 0.0, 0.25]
            .into_iter()
            .filter(|offset| {
                let (px, py) = camera
                    .project(
                        Vec3::new(x + offset, 0.25, 0.0),
                        width as f32,
                        height as f32,
                    )
                    .unwrap();
                let start = (py.round() as usize * width as usize + px.round() as usize) * 4;
                let pixel = &pixels[start..start + 4];
                pixel[channel] > 80
                    && pixel[channel] > pixel[(channel + 1) % 3].saturating_add(40)
                    && pixel[channel] > pixel[(channel + 2) % 3].saturating_add(40)
            })
            .count();
        assert!(
            matching >= 2,
            "wrong real Program crop on wall {x}: {matching}/3 samples match"
        );
    }
    assert_eq!(view.mode, ViewMode::Lines3d);
    assert_eq!(view.quality, RenderQuality::Draft);
    println!("Real Program frozen 4K evidence: {}", path.display());
}
