use super::*;
use crate::ultimate::UltimateEligibility;
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use viz_render::media::{FrozenMediaSource, MediaFrame};
use viz_scene::{MediaCrop, MediaSection, MediaSectionKind, MediaSourceBinding, uuid::Uuid};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let root = std::env::var_os("LIGHT_TMP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp")
            });
        let path = root.join(format!(
            "image-manifest-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn manifest(scene: &Scene, view: &ViewConfiguration, media: &FrozenMedia) -> ImageManifest {
    ImageManifest::build(scene, &SceneValues::default(), view, 1.25, media).unwrap()
}

#[test]
fn camera_keeps_exact_roll_and_resolved_projection_independently_of_live_quality() {
    let scene = Scene::default();
    let mut view = ViewConfiguration::default();
    view.mode = viz_scene::ViewMode::Lines3d;
    view.quality = viz_scene::RenderQuality::Draft;
    view.camera.up = glam::Quat::from_axis_angle(
        (view.camera.target - view.camera.position).normalize(),
        0.47,
    ) * Vec3::Y;
    view.camera.fov_degrees = 27.0;
    let image = manifest(&scene, &view, &FrozenMedia::default());
    let native = ResolvedCamera::resolve(&view.camera, view.mode, 16.0 / 9.0, scene.bounds);
    assert_eq!(
        image.camera.world_from_camera,
        (z_up_matrix() * native.view.inverse()).to_cols_array_2d()
    );
    assert!((image.camera.vertical_fov_degrees - 27.0).abs() < 0.0001);
    assert_eq!(image.camera.orthographic_clip_shift, 0.0);
    assert!(
        image
            .bounds
            .min
            .iter()
            .chain(&image.bounds.max)
            .all(|value| value.is_finite())
    );
    assert!(
        UltimateEligibility::inspect_image(&image)
            .unwrap()
            .is_supported()
    );
    assert!(!UltimateEligibility::inspect(&scene, &SceneValues::default(), &view).is_supported());
}

#[test]
fn orthographic_render_camera_shift_retains_native_depth_extent_and_half_height() {
    let scene = Scene::default();
    let mut view = ViewConfiguration::default();
    view.mode = viz_scene::ViewMode::TopDown;
    view.camera.orthographic_size = 3.5;
    let image = manifest(&scene, &view, &FrozenMedia::default());
    let native = ResolvedCamera::resolve(&view.camera, view.mode, 16.0 / 9.0, scene.bounds);
    let near = native.projection.w_axis.z / native.projection.z_axis.z;
    let far = (native.projection.w_axis.z - 1.0) / native.projection.z_axis.z;
    assert!((image.camera.vertical_half_height - 3.5).abs() < 1e-5);
    assert!((image.camera.near - image.camera.orthographic_clip_shift - near).abs() < 1e-4);
    assert!((image.camera.far - image.camera.orthographic_clip_shift - far).abs() < 1e-4);
}

fn media_scene() -> Scene {
    let mut scene = Scene::default();
    let source = Uuid::from_u128(1);
    let edge = viz_render::media::EDGE;
    let mut rgba = vec![255; (edge * edge * 4) as usize];
    for (i, pixel) in rgba.chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(if i as u32 % edge < edge / 2 {
            &[255, 0, 0, 255]
        } else {
            &[0, 0, 255, 255]
        });
    }
    scene.media_sources.push(MediaSourceBinding {
        id: source,
        server_id: Uuid::nil(),
        host: String::new(),
        port: 0,
        advertised_source_id: 0,
        name: "frozen halves".into(),
        aspect_ratio: Some(1.0),
        fallback_rgba: Some(rgba),
    });
    for (index, kind) in [
        MediaSectionKind::Tv {
            bezel_metres: 0.02,
            spill: 0.0,
        },
        MediaSectionKind::ProjectionScreen {
            colour: [1.0; 3],
            gain: 1.0,
            roughness: 0.6,
            edge_feather: 0.05,
        },
        MediaSectionKind::Led {
            rows: 2,
            columns: 3,
            occupied_cells: vec![0, 2, 4],
            module_size: [1.0, 1.0],
            module_gap: [0.1, 0.2],
            pixel_pitch_millimetres: 3.0,
        },
    ]
    .into_iter()
    .enumerate()
    {
        scene.media_sections.push(MediaSection {
            id: Uuid::from_u128(index as u128 + 10),
            surface_id: Uuid::from_u128(index as u128 + 20),
            name: format!("panel{index}"),
            source_id: Some(source),
            fallback_source_id: None,
            position: Vec3::new(index as f32 * 2.0, 1.0, 0.0),
            rotation_degrees: Vec3::new(0.0, 15.0, 5.0),
            size: Vec3::new(2.0, 1.0, 0.1),
            crop: MediaCrop {
                left: 0.5,
                top: 0.1,
                width: 0.5,
                height: 0.8,
            },
            kind,
        });
    }
    scene
}

#[test]
fn immutable_media_crops_modules_pixels_and_native_panel_transforms_survive_packaging() {
    let mut scene = media_scene();
    let frozen = FrozenMedia::default().with_scene_fallbacks(&scene).unwrap();
    let image = manifest(&scene, &ViewConfiguration::default(), &frozen);
    assert_eq!(image.media.panels.len(), 5);
    assert_eq!(image.media.assets.len(), 1);
    assert_eq!(image.media.panels[0].crop, [0.5, 0.1, 0.5, 0.8]);
    assert!(
        image.media.panels.iter().all(
            |panel| panel.selected_file.as_deref() == Some(image.media.assets[0].file.as_str())
        )
    );
    scene.media_sources[0]
        .fallback_rgba
        .as_mut()
        .unwrap()
        .fill(0);
    let scratch = Scratch::new();
    image.write_package(&scratch.0).unwrap();
    let eligibility = UltimateEligibility::inspect_image(&image).unwrap();
    eligibility.validate(&scratch.0).unwrap();
    fs::write(scratch.0.join(&image.geometry_file), b"replaced").unwrap();
    assert!(eligibility.validate(&scratch.0).is_err());
}

#[test]
fn expired_primary_uses_fallback_even_when_fallback_availability_is_false() {
    let source = |id, available| FrozenMediaSource {
        frame: Arc::new(MediaFrame {
            source_id: Uuid::from_u128(id),
            sequence: 7,
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 255],
            persistent: false,
        }),
        available,
        flicker: 0.25,
    };
    let mut sources = [source(1, false), source(2, false)];
    let select = |sources: &[FrozenMediaSource]| {
        media::selected_source(sources, Some(Uuid::from_u128(1)), Some(Uuid::from_u128(2)))
            .unwrap()
            .frame
            .source_id
    };
    assert_eq!(select(&sources), Uuid::from_u128(2));
    sources[0].available = true;
    assert_eq!(select(&sources), Uuid::from_u128(1));
    assert!(media::selected_source(&[], Some(Uuid::from_u128(1)), None).is_none());
}

#[test]
fn manifest_and_media_mutation_or_asset_escape_are_rejected() {
    let scene = media_scene();
    let frozen = FrozenMedia::default().with_scene_fallbacks(&scene).unwrap();
    let image = manifest(&scene, &ViewConfiguration::default(), &frozen);
    let scratch = Scratch::new();
    image.write_package(&scratch.0).unwrap();
    let eligibility = UltimateEligibility::inspect_image(&image).unwrap();
    fs::write(scratch.0.join(&image.media.assets[0].file), b"not PNG").unwrap();
    assert!(eligibility.validate(&scratch.0).is_err());
    fs::write(scratch.0.join(IMAGE_MANIFEST_FILE), b"{}").unwrap();
    assert!(eligibility.validate(&scratch.0).is_err());
    assert!(validation::checked_asset(&scratch.0, "../outside").is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            std::env::current_exe().unwrap(),
            scratch.0.join("escape.bin"),
        )
        .unwrap();
        assert!(validation::checked_asset(&scratch.0, "escape.bin").is_err());
    }
}

#[test]
fn frozen_crowd_amount_controls_actual_glb_population_without_capture_budget_cap() {
    let mut scene = Scene::default();
    scene.crowds.push(viz_scene::CrowdArea {
        id: Uuid::from_u128(951),
        name: "Frozen audience".into(),
        position: Vec3::ZERO,
        rotation_degrees: Vec3::ZERO,
        width_metres: 30.0,
        depth_metres: 20.0,
        posture: viz_scene::CrowdPosture::StandingStill,
        density: viz_scene::CrowdDensity::Dense,
        seed: 42,
    });
    let view = ViewConfiguration::default();
    let values = SceneValues::default();
    let media = FrozenMedia::default();
    let build = |amount| {
        ImageManifest::build_with_crowd_amount(&scene, &values, &view, 1.0, &media, amount).unwrap()
    };
    let nodes = |image: &ImageManifest| {
        let length = u32::from_le_bytes(image.geometry[12..16].try_into().unwrap()) as usize;
        let gltf: serde_json::Value =
            serde_json::from_slice(&image.geometry[20..20 + length]).unwrap();
        gltf["nodes"].as_array().unwrap().len()
    };
    let authored = scene.crowds[0].width_metres
        * scene.crowds[0].depth_metres
        * scene.crowds[0].density.people_per_square_metre();
    assert!(
        authored * 0.5 > 384.0,
        "partial capture must exceed the live budget"
    );
    let none = build(0.0);
    let half = build(0.5);
    let full = build(1.0);
    assert_eq!(none.crowd_amount, 0.0);
    assert_eq!(half.crowd_amount, 0.5);
    assert_eq!(full.crowd_amount, 1.0);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&half.bytes().unwrap()).unwrap()["crowd_amount"],
        0.5
    );
    assert!(
        nodes(&half) > nodes(&none),
        "requested audience must reach the GLB"
    );
    assert_eq!(
        nodes(&full) - nodes(&none),
        2 * (nodes(&half) - nodes(&none)),
        "half amount is preserved even when it exceeds the live 384-person budget"
    );
    for amount in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(
            ImageManifest::build_with_crowd_amount(&scene, &values, &view, 1.0, &media, amount,)
                .is_err()
        );
    }
}
