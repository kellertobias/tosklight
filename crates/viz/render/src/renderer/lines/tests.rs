use crate::renderer::{CapturedImage, Renderer};
use glam::Vec3;
use viz_scene::*;

#[test]
fn standalone_wireframe_shader_validates_without_lighting_bindings() {
    let source = include_str!("../../shaders/wireframe.wgsl");
    let module = naga::front::wgsl::parse_str(source).expect("technical shader parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("technical shader validates");
    assert_eq!(
        module
            .global_variables
            .iter()
            .filter(|(_, v)| v.binding.is_some())
            .count(),
        7
    );
}

fn fixture_scene() -> Scene {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/models/lamps/par-64-long-nose-silver.glb");
    let model = read_glb(&std::fs::read(path).unwrap()).unwrap();
    let mut scene = Scene::default();
    scene.models.push(model);
    scene.fixtures.push(FixtureInstance {
        model: Some(0),
        position: Vec3::ZERO,
        body: FixtureBody {
            kind: BodyKind::Lantern,
            size: Vec3::splat(1.0),
        },
        ..Default::default()
    });
    scene.recompute_bounds();
    scene
}
fn view() -> ViewConfiguration {
    let mut v = ViewConfiguration {
        mode: ViewMode::Lines3d,
        floor_grid: false,
        show_labels: false,
        background: Some([0.0; 3]),
        ..Default::default()
    };
    v.camera.position = Vec3::new(1.5, 1.0, 2.2);
    v.camera.target = Vec3::ZERO;
    v
}
fn capture(r: &mut Renderer, s: &Scene, v: &ViewConfiguration) -> CapturedImage {
    r.capture(
        s,
        &SceneValues::default(),
        v,
        &crate::Overlay::default(),
        0.0,
    )
    .unwrap()
}

#[test]
#[ignore = "requires a real GPU; run explicitly with --ignored --test-threads=1"]
fn actual_glb_wireframe_is_crisp_lazy_and_independent_of_lighting_quality() {
    let mut renderer = Renderer::headless(256, 256).expect("GPU required for pixel acceptance");
    assert_eq!(renderer.initialized_resources(), (false, false, false));
    let scene = fixture_scene();
    let mut v = view();
    let first = capture(&mut renderer, &scene, &v);
    evidence("actual-par-feature-edges", &first);
    assert_eq!(renderer.initialized_resources(), (false, true, false));
    assert_eq!(renderer.samples(), 1);
    let lit = first
        .rgba
        .chunks_exact(4)
        .filter(|p| p[0] > 20 || p[1] > 20 || p[2] > 20)
        .count();
    assert!(
        lit > 100 && lit < 15000,
        "recognizable model-derived sparse edges, got {lit}"
    );
    v.ambient = 1.0;
    v.exposure = 4.0;
    v.quality = RenderQuality::Ultra;
    assert_eq!(first.rgba, capture(&mut renderer, &scene, &v).rgba);
    renderer.resize(320, 180);
    let resized = capture(&mut renderer, &scene, &v);
    assert_eq!((resized.width, resized.height), (320, 180));
    assert_eq!(renderer.initialized_resources(), (false, true, false));
    renderer
        .update_media_frame(&crate::media::MediaFrame {
            source_id: uuid::Uuid::new_v4(),
            sequence: 1,
            width: crate::media::EDGE,
            height: crate::media::EDGE,
            rgba: vec![128; (crate::media::EDGE * crate::media::EDGE * 4) as usize],
            persistent: true,
        })
        .unwrap();
    assert_eq!(
        renderer.initialized_resources(),
        (false, true, true),
        "media update does not initialize shaded resources"
    );
}

#[test]
#[ignore = "requires a real GPU; run explicitly with --ignored --test-threads=1"]
fn reused_model_slot_and_shaded_transition_do_not_retain_old_wireframe_or_lights() {
    let mut renderer = Renderer::headless(256, 256).unwrap();
    let mut scene = fixture_scene();
    let v = view();
    let first = capture(&mut renderer, &scene, &v);
    for part in &mut scene.models[0].parts {
        for p in &mut part.positions {
            p[0] *= 1.8;
        }
    }
    let replacement = capture(&mut renderer, &scene, &v);
    assert_ne!(
        first.rgba, replacement.rgba,
        "same revision/index replacement invalidates actual geometry"
    );
    let mut shaded = v.clone();
    shaded.mode = ViewMode::Full3d;
    shaded.quality = RenderQuality::High;
    let shaded_before = capture(&mut renderer, &scene, &shaded);
    assert!(renderer.initialized_resources().0);
    assert_eq!(
        replacement.rgba,
        capture(&mut renderer, &scene, &v).rgba,
        "shaded tiles/light state cannot affect Lines"
    );
    for part in &mut scene.models[0].parts {
        for p in &mut part.positions {
            p[0] *= 0.5;
        }
    }
    let _ = capture(&mut renderer, &scene, &v);
    assert_ne!(
        shaded_before.rgba,
        capture(&mut renderer, &scene, &shaded).rgba,
        "returning from Lines refreshes an already initialized shaded asset slot"
    );
}

#[test]
#[ignore = "requires a real GPU; run explicitly with --ignored --test-threads=1"]
fn rear_model_edges_are_occluded_by_actual_front_triangles() {
    let mut scene = Scene::default();
    let mesh = crate::mesh::unit_cube();
    scene.models.push(FixtureModel {
        parts: vec![ModelPart {
            positions: mesh.vertices.iter().map(|v| v.position).collect(),
            normals: mesh.vertices.iter().map(|v| v.normal).collect(),
            indices: mesh.indices,
            ..Default::default()
        }],
        extent: Vec3::splat(0.5),
        ..Default::default()
    });
    scene.fixtures.push(FixtureInstance {
        model: Some(0),
        body: FixtureBody {
            kind: BodyKind::Bar,
            size: Vec3::splat(2.0),
        },
        ..Default::default()
    });
    scene.recompute_bounds();
    let mut v = view();
    v.camera.position = Vec3::new(0.0, 0.0, 6.0);
    let mut renderer = Renderer::headless(256, 256).unwrap();
    let front = capture(&mut renderer, &scene, &v);
    let mut back = scene.fixtures[0].clone();
    back.position = Vec3::new(0.0, 0.0, -2.0);
    back.body.size = Vec3::splat(0.7);
    scene.fixtures.push(back);
    scene.revision += 1;
    scene.recompute_bounds();
    assert_eq!(
        front.rgba,
        capture(&mut renderer, &scene, &v).rgba,
        "no x-ray rear edges or proxy occluders"
    );
    // Positive control: move the second instance through its existing batch mapping, without a
    // new scene revision. It must appear after leaving the actual foreground occluder.
    scene.fixtures[1].position.x = 2.5;
    assert_ne!(
        front.rgba,
        capture(&mut renderer, &scene, &v).rgba,
        "changed instance transforms update retained batch references"
    );
}

fn evidence(name: &str, image: &CapturedImage) {
    if let Some(directory) = std::env::var_os("VIZ_WIREFRAME_CAPTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        assert!(directory.is_absolute());
        std::fs::create_dir_all(&directory).unwrap();
        let mut ppm = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();
        for pixel in image.rgba.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(directory.join(format!("{name}.ppm")), ppm).unwrap();
    }
}
