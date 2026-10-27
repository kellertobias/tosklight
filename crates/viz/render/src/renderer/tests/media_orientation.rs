//! Asymmetric image corners expose vertical flips, rotations and crop-row exchanges.
use super::*;
use glam::Vec3;
use viz_scene::{
    MediaCrop, MediaSection, MediaSectionKind, RenderQuality, Scene, SceneValues, ViewConfiguration,
};

#[test]
fn media_panel_uv_shader_validates_without_gpu() {
    let module = naga::front::wgsl::parse_str(&format!("{COMMON_WGSL}\n{MEDIA_WGSL}")).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap();
}
fn pixels() -> Vec<u8> {
    let edge = crate::media::EDGE;
    (0..edge)
        .flat_map(|y| {
            (0..edge).flat_map(move |x| match (x < edge / 2, y < edge / 2) {
                (true, true) => [230, 10, 10, 255],
                (false, true) => [10, 230, 10, 255],
                (true, false) => [10, 10, 230, 255],
                (false, false) => [230, 230, 10, 255],
            })
        })
        .collect()
}
fn retain(image: &CapturedImage, quality: RenderQuality, kind: &str) {
    let Some(root) = std::env::var_os("LIGHT_MEDIA_ORIENTATION_EVIDENCE_DIR") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    assert!(root.is_absolute());
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    assert!(root.components().any(|p| p.as_os_str() == ".artifacts"));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = root.join(format!("{kind}-{quality:?}-{stamp}.png"));
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let mut encoder = png::Encoder::new(output, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&image.rgba)
        .unwrap();
    println!("media orientation PNG: {}", path.display());
}
fn corners(image: &CapturedImage, scene: &Scene, view: &ViewConfiguration) {
    let camera = crate::ResolvedCamera::resolve(
        &view.camera,
        view.mode,
        image.width as f32 / image.height as f32,
        scene.framing_bounds(),
    );
    for (position, expected) in [
        (Vec3::new(-1.0, 0.5625, 0.05), [true, false, false]),
        (Vec3::new(1.0, 0.5625, 0.05), [false, true, false]),
        (Vec3::new(-1.0, -0.5625, 0.05), [false, false, true]),
        (Vec3::new(1.0, -0.5625, 0.05), [true, true, false]),
    ] {
        let (x, y) = camera
            .project(position, image.width as f32, image.height as f32)
            .unwrap();
        let offset = (y.round() as usize * image.width as usize + x.round() as usize) * 4;
        let rgb = &image.rgba[offset..offset + 3];
        for channel in 0..3 {
            if expected[channel] {
                assert!(
                    rgb[channel] > 60,
                    "{position:?}: missing corner colour {rgb:?}"
                );
            } else {
                assert!(
                    rgb[channel] < 40,
                    "{position:?}: wrong corner orientation {rgb:?}"
                );
            }
        }
    }
}
#[test]
#[ignore = "requires explicitly serialized real GPU media corner/crop orientation acceptance"]
fn media_corners_and_asymmetric_crop_stay_upright_for_screen_tv_and_led_at_all_filled_tiers() {
    let mut renderer = Renderer::headless(512, 384).expect("actual native GPU required");
    let source = viz_scene::uuid::Uuid::new_v4();
    renderer
        .update_media_frame(&crate::media::MediaFrame {
            source_id: source,
            sequence: 1,
            width: crate::media::EDGE,
            height: crate::media::EDGE,
            rgba: pixels(),
            persistent: true,
        })
        .unwrap();
    for (name, kind) in [
        (
            "screen",
            MediaSectionKind::ProjectionScreen {
                colour: [1.0; 3],
                gain: 1.0,
                roughness: 1.0,
                edge_feather: 0.01,
            },
        ),
        (
            "tv",
            MediaSectionKind::Tv {
                bezel_metres: 0.02,
                spill: 0.0,
            },
        ),
        (
            "led",
            MediaSectionKind::Led {
                rows: 2,
                columns: 2,
                occupied_cells: vec![0, 1, 2, 3],
                module_size: [2.0, 1.125],
                module_gap: [0.004, 0.004],
                pixel_pitch_millimetres: 3.9,
            },
        ),
    ] {
        let mut scene = Scene::default();
        scene.media_sections.push(MediaSection {
            id: viz_scene::uuid::Uuid::new_v4(),
            surface_id: viz_scene::uuid::Uuid::new_v4(),
            name: name.into(),
            source_id: Some(source),
            fallback_source_id: None,
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            size: Vec3::new(4.0, 2.25, 0.04),
            crop: MediaCrop {
                left: 0.1,
                top: 0.05,
                width: 0.8,
                height: 0.7,
            },
            kind,
        });
        scene.recompute_bounds();
        for quality in [
            RenderQuality::Draft,
            RenderQuality::Standard,
            RenderQuality::High,
            RenderQuality::Ultra,
        ] {
            let mut view = ViewConfiguration {
                quality,
                ambient: 0.5,
                floor_grid: false,
                show_labels: false,
                background: Some([0.0; 3]),
                ..Default::default()
            };
            view.camera.position = Vec3::new(0.0, 0.0, 6.0);
            view.camera.target = Vec3::ZERO;
            let image = renderer
                .capture(
                    &scene,
                    &SceneValues::default(),
                    &view,
                    &crate::Overlay::default(),
                    0.0,
                )
                .unwrap();
            retain(&image, quality, name);
            corners(&image, &scene, &view);
        }
    }
}
