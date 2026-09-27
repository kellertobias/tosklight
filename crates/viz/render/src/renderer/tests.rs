use super::*;
use crate::instances::GpuLight;

fn light_fields(shader: &str) -> Vec<&str> {
    let declaration = shader
        .split_once("struct Light {")
        .expect("shader declares Light")
        .1
        .split_once("};")
        .expect("Light declaration is closed")
        .0;
    declaration
        .lines()
        .filter_map(|line| {
            let field = line.trim().split_once(':')?.0.trim();
            (!field.is_empty()).then_some(field)
        })
        .collect()
}

/// Every shader indexes the same storage-buffer array uploaded as `GpuLight`. WGSL derives
/// an array's stride from its local struct declaration, so even an unused trailing field must
/// remain present in the culling shader or every light after index zero is read at the wrong
/// byte offset and can disappear from its surface tiles.
#[test]
fn culling_and_surface_shaders_share_the_uploaded_light_layout() {
    let surface_fields = light_fields(COMMON_WGSL);
    let culling_fields = light_fields(CULL_WGSL);

    assert_eq!(culling_fields, surface_fields);
    for source in [COMMON_WGSL, CULL_WGSL] {
        let module = naga::front::wgsl::parse_str(source).expect("lighting layout parses");
        let span = module
            .types
            .iter()
            .find_map(|(_, ty)| {
                if ty.name.as_deref() != Some("Light") {
                    return None;
                }
                match ty.inner {
                    naga::TypeInner::Struct { span, .. } => Some(span),
                    _ => None,
                }
            })
            .expect("Light storage struct");
        assert_eq!(std::mem::size_of::<GpuLight>(), span as usize);
    }
    assert_eq!(
        std::mem::align_of::<GpuLight>(),
        std::mem::align_of::<f32>()
    );
}

/// The multisampled beam pass reads a different WGSL texture type, and it is reached by
/// substituting one declaration. A rename in the shader would leave the substitution a silent
/// no-op and the pipeline would fail validation on a multisampling adapter only.
#[test]
fn the_beam_shader_carries_the_scene_depth_declaration_that_is_substituted() {
    assert!(BEAM_WGSL.contains(SCENE_DEPTH_BINDING));
    assert_eq!(scene_depth_binding(1), SCENE_DEPTH_BINDING);
    assert!(scene_depth_binding(4).contains("texture_depth_multisampled_2d"));
    let multisampled = BEAM_WGSL.replace(SCENE_DEPTH_BINDING, scene_depth_binding(4));
    assert!(multisampled.contains("texture_depth_multisampled_2d"));
    assert!(!multisampled.contains(SCENE_DEPTH_BINDING));
}

#[test]
fn ultra_degrades_after_sustained_over_budget_samples() {
    let mut budget = UltraBudget::default();
    budget.observe(ULTRA_GPU_BUDGET_MICROS + 1);
    assert_eq!(budget.settings(), ULTRA_LADDER[0]);
    budget.observe(ULTRA_GPU_BUDGET_MICROS + 1);
    assert_eq!(budget.settings(), ULTRA_LADDER[1]);
    assert!(budget.degraded());
}

#[test]
fn ultra_recovers_slowly_and_never_leaves_its_ladder() {
    let mut budget = UltraBudget::default();
    for _ in 0..20 {
        budget.observe(ULTRA_GPU_BUDGET_MICROS + 1);
    }
    assert_eq!(budget.settings(), *ULTRA_LADDER.last().unwrap());
    for _ in 0..119 {
        budget.observe(11_000);
    }
    assert_eq!(budget.settings(), *ULTRA_LADDER.last().unwrap());
    budget.observe(11_000);
    assert_eq!(budget.settings(), ULTRA_LADDER[ULTRA_LADDER.len() - 2]);
}

#[test]
fn room_light_separates_venue_faces_and_respects_blackout() {
    let shader = naga::front::wgsl::parse_str(&format!("{COMMON_WGSL}\n{SURFACE_WGSL}"))
        .expect("surface WGSL parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&shader)
    .expect("surface WGSL validates");
    let Ok(mut renderer) = Renderer::headless(256, 256) else {
        eprintln!("GPU capture unavailable; validated surface shader only");
        return;
    };
    let mut scene = viz_scene::Scene::default();
    scene.scenery.push(viz_scene::SceneryObject {
        kind: viz_scene::SceneryKind::Box,
        position: glam::Vec3::ZERO,
        size: glam::Vec3::splat(2.0),
        colour: [0.5; 3],
        roughness: 1.0,
        ..Default::default()
    });
    scene.recompute_bounds();
    let mut view = viz_scene::ViewConfiguration {
        ambient: 0.06,
        floor_grid: false,
        background: Some([0.0; 3]),
        ..Default::default()
    };
    view.camera.position = glam::Vec3::new(5.0, 3.0, 5.0);
    view.camera.target = glam::Vec3::ZERO;
    let camera = crate::camera::ResolvedCamera::resolve(&view.camera, view.mode, 1.0, scene.bounds);
    let samples = [glam::Vec3::X, glam::Vec3::Z].map(|point| {
        let (x, y) = camera.project(point, 256.0, 256.0).unwrap();
        (x as usize, y as usize)
    });
    let capture = |renderer: &mut Renderer, view: &viz_scene::ViewConfiguration| {
        renderer
            .capture(
                &scene,
                &viz_scene::SceneValues::default(),
                view,
                &crate::Overlay::default(),
                0.0,
            )
            .unwrap()
    };
    let image = capture(&mut renderer, &view);
    if let Some(path) = std::env::var_os("VIZ_ROOM_LIGHT_CAPTURE") {
        let mut ppm = b"P6\n256 256\n255\n".to_vec();
        for pixel in image.rgba.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(path, ppm).expect("write room lighting inspection capture");
    }
    let level = |image: &CapturedImage, (x, y): (usize, usize)| {
        let pixel = (y * image.width as usize + x) * 4;
        image.rgba[pixel..pixel + 3]
            .iter()
            .map(|v| *v as f32)
            .sum::<f32>()
            / 3.0
    };
    let levels = samples.map(|sample| level(&image, sample));
    eprintln!("venue face samples {samples:?}: {levels:?}");
    assert!(
        (levels[0] - levels[1]).abs() > 8.0,
        "equally camera-facing venue sides must have distinct room illumination: {levels:?}"
    );
    view.ambient = 0.0;
    let dark = capture(&mut renderer, &view);
    for sample in samples {
        assert!(level(&dark, sample) <= 1.0, "unlit scenery must black out");
    }
    // A diagram stays flat ink even though its camera sees the same solid faces.
    view.mode = viz_scene::ViewMode::Lines3d;
    let plot_dark = capture(&mut renderer, &view);
    view.ambient = 0.06;
    let plot_bright = capture(&mut renderer, &view);
    assert_eq!(
        plot_dark.rgba, plot_bright.rgba,
        "room light must not shade diagrams"
    );
}

#[test]
fn media_shader_validates_and_captures_when_a_gpu_is_available() {
    let shader = naga::front::wgsl::parse_str(&format!("{COMMON_WGSL}\n{MEDIA_WGSL}"))
        .expect("media WGSL parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&shader)
    .expect("media WGSL validates");

    let Ok(mut renderer) = Renderer::headless(96, 64) else {
        // Shader validation above is platform-independent. Runtime capture is additionally
        // exercised wherever CI or the workstation exposes a Metal/software adapter.
        return;
    };
    let source = viz_scene::uuid::Uuid::new_v4();
    let frame = crate::media::MediaFrame {
        source_id: source,
        sequence: 7,
        width: crate::media::EDGE,
        height: crate::media::EDGE,
        rgba: [220, 30, 20, 255].repeat((crate::media::EDGE * crate::media::EDGE) as usize),
        persistent: true,
    };
    assert!(renderer.update_media_frame(&frame).unwrap());
    assert!(!renderer.update_media_frame(&frame).unwrap());
    assert_eq!(renderer.media_upload_count(), 1);

    let mut scene = viz_scene::Scene::default();
    scene.media_sections.push(viz_scene::MediaSection {
        id: viz_scene::uuid::Uuid::new_v4(),
        surface_id: viz_scene::uuid::Uuid::new_v4(),
        name: "Program screen".to_owned(),
        source_id: Some(source),
        fallback_source_id: None,
        position: glam::Vec3::ZERO,
        rotation_degrees: glam::Vec3::ZERO,
        size: glam::Vec3::new(4.0, 2.25, 0.04),
        crop: viz_scene::MediaCrop {
            left: 0.1,
            top: 0.1,
            width: 0.8,
            height: 0.8,
        },
        kind: viz_scene::MediaSectionKind::Tv {
            bezel_metres: 0.03,
            spill: 0.2,
        },
    });
    scene.recompute_bounds();
    let image = renderer
        .capture(
            &scene,
            &viz_scene::SceneValues::default(),
            &viz_scene::ViewConfiguration::default(),
            &crate::Overlay::default(),
            0.0,
        )
        .expect("media pipeline capture");
    assert_eq!((image.width, image.height), (96, 64));
}
