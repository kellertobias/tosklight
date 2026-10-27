//! Explicit CPU-only before/after benchmark. Production constructor remains independent.
use super::*;
use std::{hint::black_box, path::PathBuf, time::Instant};
use viz_scene::*;
fn scene() -> (Scene, SceneValues, FrameStyle) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/models/lamps");
    let mut scene = Scene::default();
    for file in [
        "moving-head-led-wash-300.glb",
        "par-64-long-nose-black.glb",
        "fresnel-barn-doors.glb",
    ] {
        scene
            .models
            .push(read_glb(&std::fs::read(root.join(file)).unwrap()).unwrap());
    }
    for index in 0..500 {
        scene.fixtures.push(FixtureInstance {
            name: format!("Representative fixture {index}"),
            number: Some(index + 1),
            model: Some(index % 3),
            fixture_id: uuid::Uuid::from_u128(index as u128 + 1),
            position: glam::Vec3::new((index % 25) as f32, 4.0, (index / 25) as f32),
            ..Default::default()
        });
        scene.emitters.push(EmitterInstance {
            fixture_index: index,
            head_index: 0,
            label: format!("Emitter {index}"),
            local_origin: glam::Vec3::ZERO,
            tilt_pivot: glam::Vec3::ZERO,
            local_orientation_degrees: glam::Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 25.0,
            field_angle_degrees: 50.0,
            optics: Default::default(),
            kind: EmitterKind::Beam,
            cells: EmitterLayoutCells::single(),
            laser: None,
            effect: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [true; 4],
            live_shaper_rotation_role: false,
        });
        scene
            .emitter_ids
            .push(uuid::Uuid::from_u128(index as u128 + 1001));
    }
    scene.recompute_bounds();
    let mut values = SceneValues::default();
    values.resize(500);
    for (index, value) in values.emitters.iter_mut().enumerate() {
        value.intensity = (index % 100) as f32 / 100.0;
        value.colour = [0.1, 0.5, 0.8];
        value.pan = (index % 80) as f32 / 80.0;
        value.shaper_blades = [0.0, 0.1, 0.2, 0.3];
    }
    (scene, values, FrameStyle::default())
}
fn measure<H: Hasher + Default>(
    scene: &Scene,
    values: &SceneValues,
    style: &FrameStyle,
    iterations: u32,
) -> [f64; 2] {
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(fingerprint_with::<H>(
            black_box(scene),
            black_box(values),
            black_box(style),
        ));
    }
    let preparation = start.elapsed().as_secs_f64() * 1e6 / f64::from(iterations);
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(crate::renderer::shaded_assets::benchmark_fingerprints::<H>(
            black_box(scene),
        ));
    }
    [
        preparation,
        start.elapsed().as_secs_f64() * 1e6 / f64::from(iterations),
    ]
}
#[test]
#[ignore = "explicit release CPU-only representative hasher benchmark; no GPU or application"]
fn representative_renderer_hash_cpu_before_after() {
    assert!(!cfg!(debug_assertions), "benchmark must use --release");
    let destination = PathBuf::from(
        std::env::var_os("LIGHT_HASH_BENCH_EVIDENCE").expect("explicit canonical artifact path"),
    );
    assert!(destination.is_absolute());
    let (scene, values, style) = scene();
    let mut rounds = Vec::new();
    let iterations = 256;
    measure::<DefaultHasher>(&scene, &values, &style, 16);
    measure::<rustc_hash::FxHasher>(&scene, &values, &style, 16);
    for round in 0..7 {
        let (before, after) = if round % 2 == 0 {
            (
                measure::<DefaultHasher>(&scene, &values, &style, iterations),
                measure::<rustc_hash::FxHasher>(&scene, &values, &style, iterations),
            )
        } else {
            let after = measure::<rustc_hash::FxHasher>(&scene, &values, &style, iterations);
            (
                measure::<DefaultHasher>(&scene, &values, &style, iterations),
                after,
            )
        };
        rounds.push(serde_json::json!({"defaultMicros":before,"fxMicros":after}));
    }
    let report = serde_json::json!({"schema":1,"cpuOnly":true,"release":true,"fixtures":scene.fixtures.len(),
        "emitters":scene.emitters.len(),"models":scene.models.len(),"parts":scene.models.iter().map(|m|m.parts.len()).sum::<usize>(),
        "vertices":scene.models.iter().flat_map(|m|&m.parts).map(|p|p.positions.len()).sum::<usize>(),
        "iterationsPerRound":iterations,"rounds":rounds,"measurement":"elapsed wall CPU-work benchmark; not thread CPU, GPU time, FPS or output acceptance",
        "traversal":"identical generic production preparation/Postcard and full model positions/normals/indices hashing; only Hasher differs"});
    std::fs::write(&destination, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("{}", serde_json::to_string(&report).unwrap());
}
