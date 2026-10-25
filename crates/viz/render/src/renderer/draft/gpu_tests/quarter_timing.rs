//! Synthetic, matched 500-shaft cost diagnostic, never an output/display acceptance result.
use super::*;
use std::{
    collections::HashSet,
    io::Write,
    time::{Duration, Instant},
};

fn diagnostic_rig() -> (Scene, SceneValues, ViewConfiguration) {
    let (mut scene, mut values, mut view) = rig();
    let fixture = scene.fixtures[0].clone();
    let emitter = scene.emitters[0].clone();
    let value = values.emitters[0].clone();
    scene.fixtures.clear();
    scene.emitters.clear();
    values.emitters.clear();
    for index in 0..500 {
        let mut fixture = fixture.clone();
        fixture.position.x = (index % 25) as f32 * 0.15 - 1.8;
        fixture.position.z = (index / 25) as f32 * 0.15 - 1.425;
        if index % 4 == 0 {
            fixture.installed_shaper_blades[0] = 0.5;
        }
        scene.fixtures.push(fixture);
        let mut emitter = emitter.clone();
        emitter.fixture_index = index;
        emitter.beam_angle_degrees = 48.0;
        emitter.field_angle_degrees = 48.0;
        emitter.optics.sharpness = 0.42;
        emitter.optics.uniformity = 0.55;
        scene.emitters.push(emitter);
        let mut value = value.clone();
        value.intensity = 0.01;
        values.emitters.push(value);
    }
    scene.recompute_bounds();
    values.atmosphere.density = 0.15;
    view.camera.position = Vec3::new(8.0, 5.0, 10.0);
    view.camera.target = Vec3::new(0.0, 2.0, 0.0);
    (scene, values, view)
}
fn sample_scope(
    renderer: &mut Renderer,
    target: &wgpu::TextureView,
    scene: &Scene,
    values: &SceneValues,
    view: &ViewConfiguration,
    label: &str,
    observed: &mut HashSet<u64>,
    output: &mut std::fs::File,
) -> Vec<super::quarter_intervals::IntervalEvidence> {
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut samples = Vec::new();
    let mut warmup = 0;
    while samples.len() < 12 && Instant::now() < deadline {
        let started = Instant::now();
        let stats = renderer
            .render_into(target, scene, values, view, &crate::Overlay::default(), 0.0)
            .unwrap();
        assert_eq!(stats.lights, 500);
        assert_eq!(stats.beams, 500);
        assert_eq!(stats.shadow_budget, 2);
        assert_eq!(stats.render_scale, 1.0);
        if stats.gpu_sample_matches_frame
            && let Some(id) = stats.gpu_sample_id
            && observed.insert(id)
        {
            let total = stats.gpu_micros.expect("unavailable GPU total");
            let beams = stats.gpu_passes.beams_micros.expect("missing shaft pass");
            let composite = stats
                .gpu_passes
                .composite_micros
                .expect("missing composite pass");
            assert!(
                total > 0 && beams > 0 && composite > 0,
                "zero/empty GPU evidence unavailable"
            );
            let raw = renderer
                .raw_gpu_timestamps_for_test()
                .expect("missing linked raw timestamp mapping");
            assert_eq!(
                raw.sample_id, id,
                "raw pairs do not belong to accepted FrameStats"
            );
            let intervals =
                super::quarter_intervals::intervals(raw).expect("invalid raw interval evidence");
            for (index, measured) in stats.gpu_passes.as_array().into_iter().enumerate() {
                assert_eq!(
                    intervals.raw_pass_pairs[index].is_some(),
                    measured.is_some(),
                    "pass mask mismatch"
                );
                if let Some([start, end]) = intervals.raw_pass_pairs[index] {
                    assert_eq!(
                        Some(((end - start) as f32 * raw.period_nanos / 1000.0) as u64),
                        measured,
                        "raw pair differs from accepted pass"
                    );
                }
            }
            let record = serde_json::json!({
                "kind":"matched500ShaftDiagnostic", "scope":label, "sampleId":id,
                "physicalPixels":[1920,1080], "sceneRevision":scene.revision,
                "quality":"draft", "timeSeconds":0.0, "lights":stats.lights,
                "beams":stats.beams, "warmup":warmup < 4,
                "passDurationSumRoundedMicros":total,"intervalEvidence":intervals,
                "coverage":renderer.gpu_timing_coverage(),
            });
            writeln!(output, "{record}").unwrap();
            println!("quarter shaft cost {record}");
            if warmup < 4 {
                warmup += 1;
            } else {
                samples.push(intervals);
            }
        }
        // Fixed 60Hz diagnostic submissions. Existing timer owns completion, second-phase
        // Metal query resolve and asynchronous mapping; no wall time substitutes for GPU time.
        std::thread::sleep(Duration::from_secs_f64(1.0 / 60.0).saturating_sub(started.elapsed()));
    }
    assert_eq!(
        samples.len(),
        12,
        "{label}: unavailable/incomplete timestamp coverage"
    );
    samples
}

#[test]
#[ignore = "requires explicitly serialized native GPU full-sampling 500-shaft cost diagnostic"]
fn quarter_shafts_matched_35_25_abba_valid_gpu_cost() {
    assert_eq!(
        std::env::var("LIGHT_STAGE_GPU_TIMING_CADENCE").as_deref(),
        Ok("full")
    );
    let report = std::env::var_os("LIGHT_STAGE_PACKAGED_BENCH_REPORT")
        .expect("explicit diagnostic REPORT required");
    assert!(std::path::Path::new(&report).is_absolute());
    assert!(
        std::env::var_os("TOSKLIGHT_GPU_TIMING_DIAGNOSTIC").is_some(),
        "retain raw ticks in test log"
    );
    let directory =
        super::quarter_budget::evidence().expect("canonical evidence directory required");
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("samples.jsonl"))
        .unwrap();
    let mut renderer = Renderer::headless(1920, 1080).expect("requested native GPU required");
    let (scene, values, view) = diagnostic_rig();
    let image = capture(&mut renderer, &scene, &values, &view);
    assert!(
        image
            .rgba
            .chunks_exact(4)
            .filter(|pixel| pixel[0] > 8)
            .count()
            > 100,
        "nonblack scene required"
    );
    assert_eq!(
        renderer
            .gpu_timing_coverage()
            .expect("GPU timestamp support unavailable")
            .diagnostic_cadence,
        viz_scene::gpu_timing::TimingCadence::Full
    );
    let target = renderer
        .device()
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("matched quarter shaft timing output"),
            size: wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: renderer.output_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default());
    let mut observed = HashSet::new();
    let mut totals = [Vec::new(), Vec::new()];
    for (phase, quarter) in [
        ("A35", false),
        ("B25", true),
        ("B25-repeat", true),
        ("A35-return", false),
    ] {
        renderer.set_draft_beam_size_for_test(if quarter { (480, 270) } else { (672, 378) });
        let samples = sample_scope(
            &mut renderer,
            &target,
            &scene,
            &values,
            &view,
            phase,
            &mut observed,
            &mut output,
        );
        totals[usize::from(quarter)].extend(samples);
    }
    let coverage = renderer.gpu_timing_coverage().unwrap();
    assert_eq!(
        coverage.valid_samples, coverage.completed_readbacks,
        "invalid timestamp readbacks make evidence unavailable"
    );
    for (label, rows) in ["35", "25"].into_iter().zip(totals) {
        let median = |mut values: Vec<f64>| {
            values.sort_by(f64::total_cmp);
            values[values.len() / 2]
        };
        let pass_medians: Vec<_> = (0..8)
            .map(|index| {
                let values: Vec<_> = rows
                    .iter()
                    .filter_map(|row| row.individual_pass_micros[index])
                    .collect();
                (!values.is_empty()).then(|| median(values))
            })
            .collect();
        let summary = serde_json::json!({
            "kind":"summary", "scalePercent":label, "samples":rows.len(),
            "passLabels":["cull","shadow","opaque","beams","lasers","bloom","composite","overlay"],
            "medianIndividualPassMicros":pass_medians,
            "medianPassDurationSumMicros":median(rows.iter().map(|r|r.pass_duration_sum_micros).collect()),
            "medianPassIntervalUnionMicros":median(rows.iter().map(|r|r.pass_interval_union_micros).collect()),
            "medianPassIntervalSpanMicros":median(rows.iter().map(|r|r.pass_interval_span_micros).collect()),
            "coverage":coverage,
            "limitation":"synthetic matched offscreen 500-shaft timestamp interval diagnostic; interval union/span are not GPU-active time, utilization or whole-Stage GPU cost; no output/display acceptance or frequency control"
        });
        writeln!(output, "{summary}").unwrap();
        println!("quarter shaft cost {summary}");
    }
    output.flush().unwrap();
    println!("quarter shaft timing evidence: {}", directory.display());
}
