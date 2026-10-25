//! Opt-in real demo.show geometry/installed-optics proof; no show or live values are modified.
use glam::Vec3;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use viz_desk::{DeskConnection, DeskProvider};
use viz_render::{CapturedImage, Overlay, Renderer, ResolvedCamera};
use viz_scene::{
    ProviderEvent, RenderQuality, Scene, SceneProvider, SceneValues, ViewConfiguration, ViewMode,
};

struct PrivateSource {
    provider: DeskProvider,
    _hosted: crate::showfile::HostedShow,
}
impl Drop for PrivateSource {
    fn drop(&mut self) {
        // Stop the reader before HostedShow closes its exclusively owned snapshot server.
        self.provider.shutdown();
    }
}
fn actual_demo_scene() -> Scene {
    let show = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/demo.show");
    let hosted = crate::showfile::HostedShow::open(&show).expect("private real demo.show snapshot");
    let provider = DeskProvider::start(
        DeskConnection {
            port: hosted.port(),
            values_from_desk_output: true,
            ..Default::default()
        },
        Instant::now(),
    );
    let mut source = PrivateSource {
        provider,
        _hosted: hosted,
    };
    // Native audio discovery can consume its own 20s startup deadline on shared test hosts.
    // Bound cold server+provider startup separately from the actual pixel acceptance.
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        for event in source.provider.poll() {
            match event {
                ProviderEvent::Snapshot { scene, .. } => return *scene,
                ProviderEvent::Connection(state) => eprintln!("demo source: {state:?}"),
                ProviderEvent::Diagnostics(diagnostics) => {
                    for warning in diagnostics.warnings.iter().take(3) {
                        eprintln!("demo source warning: {warning}");
                    }
                }
                _ => {}
            }
        }
        assert!(
            Instant::now() < deadline,
            "private demo.show scene was not available within 90s"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn front_values(scene: &Scene) -> SceneValues {
    let mut values = SceneValues::default();
    values.resize(scene.emitters.len());
    values.atmosphere.density = 0.0;
    let mut found = std::collections::BTreeSet::new();
    for (index, emitter) in scene.emitters.iter().enumerate() {
        let fixture = &scene.fixtures[emitter.fixture_index as usize];
        if let Some(number @ 1..=6) = fixture.number {
            assert!(
                found.insert(number),
                "front fixture has unexpected duplicate emitters"
            );
            assert!(
                fixture.position.x.abs() <= 4.25,
                "front fixture actual scene x is outside expected wash bar"
            );
            assert!(
                (fixture.position.y - 4.15).abs() <= 0.25,
                "actual scene height must include current stage layout"
            );
            assert!(
                (fixture.position.z - 3.0).abs() <= 0.25,
                "actual scene front bar z must include current stage layout"
            );
            assert!(
                fixture.name.starts_with("Front Truss"),
                "preserve the authored front-wash fixture identity"
            );
            // Keep the selected immutable package's authored feather (currently .3), rather
            // than assuming the model-less Generic fallback's .58 feather.
            assert!((0.0..0.95).contains(&emitter.optics.sharpness));
            assert!((emitter.optics.uniformity - 0.55).abs() < 0.01);
            let model = fixture
                .model
                .expect("front Fresnel must use its real model");
            assert!(
                !scene.models[model as usize].parts.is_empty(),
                "real GLB model has geometry"
            );
            assert!(
                (emitter.local_origin.y + 0.3074666).abs() < 0.0002,
                "the real GLB lens moved; refocus the authored screenshot wash"
            );
            assert!((emitter.beam_angle_degrees - 48.0).abs() < 0.01);
            assert!((emitter.field_angle_degrees - 48.0).abs() < 0.01);
            assert!(
                fixture.installed_shaper_blades.iter().any(|v| *v > 0.0),
                "installed manual cuts must survive scene loading"
            );
            values.emitters[index].intensity = 1.0;
            values.emitters[index].colour = [1.0; 3];
        } else {
            values.emitters[index].intensity = 0.0;
        }
    }
    assert_eq!(found, (1..=6).collect());
    values
}
fn top_down() -> ViewConfiguration {
    let mut view = ViewConfiguration {
        mode: ViewMode::Full3d,
        ambient: 0.0,
        floor_grid: false,
        show_labels: false,
        background: Some([0.0; 3]),
        ..Default::default()
    };
    view.camera.position = Vec3::new(0.0, 14.0, -2.0);
    view.camera.target = Vec3::new(0.0, 0.5, -2.0);
    view.camera.up = Vec3::NEG_Z;
    view
}
fn level(image: &CapturedImage, camera: &ResolvedCamera, point: Vec3) -> f32 {
    let (x, y) = camera
        .project(point, image.width as f32, image.height as f32)
        .expect("all receiver probes must be in the chosen camera");
    let mut total = 0.0;
    for dy in -2..=2 {
        for dx in -2..=2 {
            let px = x.round() as i32 + dx;
            let py = y.round() as i32 + dy;
            assert!(px >= 0 && py >= 0 && px < image.width as i32 && py < image.height as i32);
            let offset = (py as usize * image.width as usize + px as usize) * 4;
            total += image.rgba[offset..offset + 3]
                .iter()
                .map(|v| f32::from(*v))
                .sum::<f32>()
                / 3.0;
        }
    }
    total / 25.0
}
fn probes(image: &CapturedImage, camera: &ResolvedCamera) -> (Vec<f32>, Vec<f32>) {
    // Actual stage top is y=.5, x=-5..5, z=-4..0. Interior permits receiver occlusion.
    let inside = (0..13)
        .flat_map(|z| {
            (0..31).map(move |x| {
                Vec3::new(
                    -4.75 + x as f32 * 9.5 / 30.0,
                    0.501,
                    -3.8 + z as f32 * 3.6 / 12.0,
                )
            })
        })
        .map(|p| level(image, camera, p))
        .collect();
    let mut outside = Vec::new();
    for step in 0..=20 {
        let t = step as f32 / 20.0;
        for point in [
            Vec3::new(-5.5, 0.501, -4.0 + t * 4.0),
            Vec3::new(5.5, 0.501, -4.0 + t * 4.0),
            Vec3::new(-5.0 + t * 10.0, 0.501, 0.5),
            Vec3::new(-5.0 + t * 10.0, 0.501, -4.5),
        ] {
            outside.push(level(image, camera, point));
        }
    }
    (inside, outside)
}
fn retain_and_measure(
    root: &Path,
    scene: &Scene,
    image: CapturedImage,
    camera: &ResolvedCamera,
    name: &str,
) -> (bool, bool) {
    fs::write(
        root.join(format!("{name}.png")),
        crate::png::encode_rgba(image.width, image.height, &image.rgba),
    )
    .unwrap();
    let (inside, outside) = probes(&image, camera);
    let peak = inside.iter().copied().fold(0.0f32, f32::max);
    let threshold = 8.0f32.max(peak * 0.02);
    let coverage = inside.iter().filter(|v| **v > threshold).count() as f32 / inside.len() as f32;
    let spill = outside.iter().filter(|v| **v > threshold).count();
    let poses = scene
        .fixtures
        .iter()
        .filter(|f| matches!(f.number, Some(1..=6)))
        .map(|f| {
            serde_json::json!({"number":f.number,"name":f.name,"position":f.position.to_array(),
            "rotation":f.rotation_degrees.to_array(),"bracket":f.bracket_degrees,
            "blades":f.installed_shaper_blades,"model":f.model})
        })
        .collect::<Vec<_>>();
    fs::write(root.join(format!("{name}.json")), serde_json::to_vec_pretty(&serde_json::json!({
        "quality":name,"width":image.width,"height":image.height,"poses":poses,
        "blackThreshold":threshold,"interiorCoverage":coverage,"outsideBrightProbes":spill,
        "insideLevels":inside,"outsideLevels":outside,
        "probeBoundary":"Projected world points sample the visible receiver at each pixel; occluded interior probes remain counted, no replacement geometry"
    })).unwrap()).unwrap();
    println!(
        "{name}: coverage={coverage:.3}, outside bright probes={spill}, threshold={threshold:.3}"
    );
    (coverage >= 0.8, spill == 0)
}
#[test]
#[ignore = "requires regenerated actual demo.show, explicit private headless server and serialized native GPU"]
fn actual_demo_six_front_fresnels_cover_stage_without_outside_spill() {
    let root = PathBuf::from(
        std::env::var_os("LIGHT_FRONT_WASH_EVIDENCE_DIR")
            .expect("explicit canonical evidence root"),
    );
    assert!(root.is_absolute());
    fs::create_dir_all(&root).unwrap();
    let root = root.join(format!("front-wash-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let scene = actual_demo_scene();
    let fixture_poses = scene
        .fixtures
        .iter()
        .filter(|f| matches!(f.number, Some(1..=6)))
        .map(|f| {
            serde_json::json!({"number":f.number,"name":f.name,"position":f.position.to_array(),
            "rotation":f.rotation_degrees.to_array(),"bracket":f.bracket_degrees,
            "blades":f.installed_shaper_blades,"model":f.model})
        })
        .collect::<Vec<_>>();
    let emitter_poses = scene.emitters.iter().filter(|e| matches!(scene.fixtures[e.fixture_index as usize].number, Some(1..=6)))
        .map(|e| serde_json::json!({"fixtureIndex":e.fixture_index,"localOrigin":e.local_origin.to_array(),
            "orientation":e.local_orientation_degrees.to_array(),"beamDegrees":e.beam_angle_degrees,
            "fieldDegrees":e.field_angle_degrees})).collect::<Vec<_>>();
    fs::write(
        root.join("actual-scene-poses.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "fixtures":fixture_poses,"emitters":emitter_poses
        }))
        .unwrap(),
    )
    .unwrap();
    let values = front_values(&scene);
    let mut view = top_down();
    let camera = ResolvedCamera::resolve(&view.camera, view.mode, 16.0 / 9.0, scene.bounds);
    // Fresh independent renderers avoid any live-state/cache coupling between quality passes.
    let images = [
        (RenderQuality::Draft, "draft"),
        (RenderQuality::Ultra, "ultra"),
    ]
    .map(|(quality, name)| {
        view.quality = quality;
        let mut renderer =
            Renderer::headless(1920, 1080).expect("real native GPU required, never skip");
        let image = renderer
            .capture(&scene, &values, &view, &Overlay::default(), 0.0)
            .expect("native front wash capture");
        (name, image)
    });
    // Retain both quality images even if the first physical coverage assertion fails.
    for (name, image) in &images {
        fs::write(
            root.join(format!("{name}.png")),
            crate::png::encode_rgba(image.width, image.height, &image.rgba),
        )
        .unwrap();
    }
    println!("Actual front wash evidence: {}", root.display());
    let verdicts = images.map(|(name, image)| {
        (
            name,
            retain_and_measure(&root, &scene, image, &camera, name),
        )
    });
    for (name, (coverage, contained)) in verdicts {
        assert!(
            coverage,
            "{name}: actual front wash covers less than 80% of stage interior; see retained PNG/probes"
        );
        assert!(
            contained,
            "{name}: light spills outside actual stage rectangle; see retained PNG/probes"
        );
    }
}
