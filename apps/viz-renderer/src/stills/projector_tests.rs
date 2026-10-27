//! Explicit opt-in real 4K finite projector reach and no media repaint, through the native image job.
use super::*;
use glam::Vec3;
use std::{fs, io::BufReader, time::Instant};
use viz_render::{
    CapturedImage, ResolvedCamera,
    media::{EDGE, FrozenMedia},
    still::StillOrigin,
};
use viz_scene::*;

const NEAR: Vec3 = Vec3::new(-0.35, 1.0, 0.03);
const FAR: Vec3 = Vec3::new(0.8, 1.0, -5.97);
const SCREEN: Vec3 = Vec3::new(0.35, 1.0, 0.04);
const BLOCKED: Vec3 = Vec3::new(0.0, 1.5, 0.03);

fn rig() -> (Scene, SceneValues, ViewConfiguration) {
    let source = uuid::Uuid::new_v4();
    let surface = uuid::Uuid::new_v4();
    let mut scene = Scene::default();
    scene.bounds = Aabb {
        min: Vec3::new(-1.0, 0.0, -7.0),
        max: Vec3::new(1.0, 2.0, 4.0),
    };
    scene.media_sources.push(MediaSourceBinding {
        id: source,
        server_id: uuid::Uuid::nil(),
        host: String::new(),
        port: 0,
        advertised_source_id: 0,
        name: "Frozen white projector source".into(),
        aspect_ratio: Some(1.0),
        fallback_rgba: Some(vec![255; (EDGE * EDGE * 4) as usize]),
    });
    scene.media_sections.push(MediaSection {
        id: uuid::Uuid::new_v4(),
        surface_id: surface,
        name: "Reflective image must not repaint".into(),
        source_id: None,
        fallback_source_id: Some(source),
        position: Vec3::new(0.35, 1.0, 0.0),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(0.3, 0.3, 0.05),
        crop: MediaCrop {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        },
        kind: MediaSectionKind::ProjectionScreen {
            colour: [1.0; 3],
            gain: 1.0,
            roughness: 1.0,
            edge_feather: 0.0,
        },
    });
    scene.media_projectors.push(MediaProjector {
        id: uuid::Uuid::new_v4(),
        surface_id: surface,
        name: "Finite eight metre projector".into(),
        position: Vec3::new(0.0, 1.0, 3.0),
        rotation_degrees: Vec3::ZERO,
        cone_length_metres: 8.0,
        spill: 1.0,
    });
    for position in [
        Vec3::new(-0.35, 1.0, 0.0),
        Vec3::new(0.8, 1.0, -6.0),
        Vec3::new(0.0, 1.5, 0.0),
    ] {
        scene.scenery.push(SceneryObject {
            position,
            size: Vec3::new(0.3, 0.3, 0.05),
            colour: [0.8; 3],
            roughness: 1.0,
            ..Default::default()
        });
    }
    scene.scenery.push(SceneryObject {
        position: Vec3::new(0.0, 1.24, 1.4),
        size: Vec3::new(0.3, 0.3, 0.1),
        colour: [0.0; 3],
        roughness: 1.0,
        ..Default::default()
    });
    let mut view = ViewConfiguration {
        mode: ViewMode::Lines3d,
        quality: RenderQuality::Draft,
        ambient: 0.0,
        floor_grid: false,
        show_labels: false,
        background: Some([0.0; 3]),
        ..Default::default()
    };
    view.camera.position = Vec3::new(5.0, 3.0, 5.0);
    view.camera.target = Vec3::new(0.0, 1.0, -3.0);
    let mut values = SceneValues::default();
    values.atmosphere.density = 0.0;
    (scene, values, view)
}

fn level(image: &CapturedImage, camera: &ResolvedCamera, point: Vec3) -> f32 {
    let (x, y) = camera
        .project(point, image.width as f32, image.height as f32)
        .unwrap();
    let mut sum = 0.0;
    for dy in -3..=3 {
        for dx in -3..=3 {
            let offset = (((y.round() as i32 + dy) as usize * image.width as usize)
                + (x.round() as i32 + dx) as usize)
                * 4;
            sum += image.rgba[offset..offset + 3]
                .iter()
                .map(|v| f32::from(*v))
                .sum::<f32>()
                / 3.0;
        }
    }
    sum / 49.0
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
fn native(frozen: &FrozenStill) -> CapturedImage {
    let mut capture = frozen.begin_native().unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(image) = capture.poll().unwrap() {
            return image;
        }
        assert!(Instant::now() < deadline, "native capture timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn published(frozen: Arc<FrozenStill>, destination: PathBuf) -> CapturedImage {
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
        if let Phase::Saved { path } = status.phase {
            return decoded(&path);
        }
        assert!(!status.phase.is_terminal(), "{:?}", status.phase);
        assert!(Instant::now() < deadline, "Native image capture timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "requires serialized native GPU and real 4K projector pixel proof"]
fn native_image_projector_has_finite_reach_and_never_repaints_its_media_surface() {
    let root = PathBuf::from(
        std::env::var_os("LIGHT_STILL_IMAGE_EVIDENCE_DIR").expect("canonical evidence dir"),
    );
    fs::create_dir_all(&root).unwrap();
    let (scene, values, view) = rig();
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
            12.5,
            FrozenMedia::default(),
            StillOrigin {
                pane_id: "finite-projector".into(),
                follow_preload: true,
            },
        )
        .unwrap(),
    );
    let native = native(&frozen);
    let published = published(
        frozen,
        root.join(format!("native-projector-{}.png", uuid::Uuid::new_v4())),
    );
    assert_eq!(
        native.rgba, published.rgba,
        "publication retains frozen native pixels"
    );
    for (name, image) in [("native", native), ("published", published)] {
        assert_eq!((image.width, image.height), (STILL_WIDTH, STILL_HEIGHT));
        let levels = [
            level(&image, &camera, NEAR),
            level(&image, &camera, FAR),
            level(&image, &camera, SCREEN),
            level(&image, &camera, BLOCKED),
        ];
        println!("{name} projector near/far/media levels: {levels:?}");
        assert!(
            levels[0] > 25.0,
            "{name} projector did not illuminate its near receiver: {levels:?}"
        );
        assert!(
            levels[1] < levels[0] * 0.1 + 3.0,
            "{name} projected beyond finite reach: {levels:?}"
        );
        assert!(
            levels[2] < levels[0] * 0.1 + 3.0,
            "{name} repainted its reflective media surface: {levels:?}"
        );
    }
    assert_eq!(view.quality, RenderQuality::Draft);
}
