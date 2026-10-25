//! Explicit CPU-only microbenchmark. No adapter, renderer, window, or GPU work is opened.
use super::*;
use std::{hint::black_box, time::Instant};

fn actual_model_rig() -> (Scene, SceneValues, FrameStyle) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/models/lamps/moving-head-profile.glb");
    let model = viz_scene::read_glb(&std::fs::read(path).unwrap()).unwrap();
    let mut scene = Scene {
        models: vec![model],
        ..Default::default()
    };
    for index in 0..500 {
        scene.fixtures.push(FixtureInstance {
            lamp_response: Default::default(),
            instance_id: viz_scene::uuid::Uuid::from_u128(index + 1),
            fixture_id: viz_scene::uuid::Uuid::from_u128(index + 1),
            position: Vec3::new((index % 25) as f32, 6.0, (index / 25) as f32),
            body: viz_scene::FixtureBody {
                kind: BodyKind::MovingHead,
                size: Vec3::splat(0.6),
            },
            installed_colour: [1.0; 3],
            model: Some(0),
            ..Default::default()
        });
        let mut emitter = crate::instances::tests::head();
        emitter.fixture_index = index as u32;
        emitter.pan = Some(viz_scene::MotionAxis {
            axis: Vec3::Y,
            min_degrees: -270.0,
            max_degrees: 270.0,
        });
        emitter.tilt = Some(viz_scene::MotionAxis {
            axis: Vec3::X,
            min_degrees: -135.0,
            max_degrees: 135.0,
        });
        scene.emitters.push(emitter);
    }
    scene.recompute_bounds();
    let values = SceneValues {
        emitters: vec![
            EmitterValues {
                lamp_response: None,
                intensity: 0.8,
                ..Default::default()
            };
            500
        ],
        ..Default::default()
    };
    let style = FrameStyle {
        quality: viz_scene::RenderQuality::Draft,
        draw_beams: false,
        draw_aim_lines: true,
        emitter_apertures: false,
        crowd_person_budget: 0,
        effect_particle_budget: 0,
        media_content: false,
        ..Default::default()
    };
    (scene, values, style)
}

#[test]
#[ignore = "explicit bounded CPU-only 500-fixture cache benchmark"]
fn cpu_cache_500_actual_model_timing() {
    let (scene, mut values, mut style) = actual_model_rig();
    let mut cache = TechnicalLinesInstances::default();
    cache.update(&scene, &values, &style);
    const LOOPS: usize = 100;
    let start = Instant::now();
    for index in 0..LOOPS {
        style.camera_position = Some(Vec3::new(index as f32 * 0.01, 3.0, 6.0));
        black_box(cache.update(black_box(&scene), black_box(&values), black_box(&style)));
    }
    let camera_micros = start.elapsed().as_secs_f64() * 1e6 / LOOPS as f64;
    let start = Instant::now();
    for index in 0..LOOPS {
        for value in &mut values.emitters {
            value.pan = 0.4 + index as f32 * 0.001;
        }
        black_box(cache.update(black_box(&scene), black_box(&values), black_box(&style)));
    }
    let moving_micros = start.elapsed().as_secs_f64() * 1e6 / LOOPS as f64;
    eprintln!(
        "CPU_CACHE_500 actual_model_parts={} loops={LOOPS} camera_mean_us={camera_micros:.3} all_moving_mean_us={moving_micros:.3} revision={}",
        scene.models[0].parts.len(),
        cache.revision()
    );
}
