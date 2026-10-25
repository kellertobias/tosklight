//! Opt-in bounded diagnostic: GPU Cull span only, never an FPS/performance acceptance gate.
use super::*;
pub(super) struct Recorder {
    query: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
}
impl Recorder {
    fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        Self {
            query: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("Cull diagnostic timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Cull diagnostic resolve"),
                size: 16,
                usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Cull diagnostic readback"),
                size: 16,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
        }
    }
    pub fn writes(&self, beginning: bool, end: bool) -> wgpu::ComputePassTimestampWrites<'_> {
        wgpu::ComputePassTimestampWrites {
            query_set: &self.query,
            beginning_of_pass_write_index: beginning.then_some(0),
            end_of_pass_write_index: end.then_some(1),
        }
    }
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.query, 0..2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, 16);
    }
    pub fn completed_sample(&self, gpu: &Gpu) -> Sample {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        self.resolve(&mut encoder);
        gpu.queue.submit(Some(encoder.finish()));
        let words = mapped_u32(gpu, &self.readback);
        let first = u64::from(words[0]) | (u64::from(words[1]) << 32);
        let last = u64::from(words[2]) | (u64::from(words[3]) << 32);
        Sample::decode(first, last, gpu.queue.get_timestamp_period())
    }
}
#[derive(Clone, serde::Serialize)]
pub(super) struct Sample {
    start_ticks: String,
    end_ticks: String,
    micros: Option<f64>,
    unavailable: Option<&'static str>,
}
impl Sample {
    fn decode(first: u64, last: u64, period: f32) -> Self {
        let reason = if first == u64::MAX || last == u64::MAX {
            Some("Metal counter-error sentinel")
        } else if first == 0 || last == 0 || first == last {
            Some("zero timestamp or span")
        } else if last < first {
            Some("reversed timestamp span")
        } else if !period.is_finite() || period <= 0.0 {
            Some("invalid timestamp period")
        } else {
            None
        };
        let micros = reason
            .is_none()
            .then(|| (last - first) as f64 * f64::from(period) / 1000.0);
        let reason = reason.or_else(|| {
            micros
                .filter(|value| !value.is_finite() || *value <= 0.0)
                .map(|_| "invalid timestamp duration")
        });
        Self {
            start_ticks: first.to_string(),
            end_ticks: last.to_string(),
            micros: if reason.is_none() { micros } else { None },
            unavailable: reason,
        }
    }
}

fn distribution(samples: &[Sample]) -> serde_json::Value {
    let valid: Vec<_> = samples.iter().filter_map(|sample| sample.micros).collect();
    let coverage = serde_json::json!({ "total": samples.len(), "valid": valid.len(), "unavailable": samples.len() - valid.len() });
    if valid.len() != samples.len() || valid.is_empty() {
        return serde_json::json!({ "coverage": coverage, "distribution": null, "reason": "Incomplete timestamp coverage; no median/mean or speedup claim" });
    }
    let mut sorted = valid.clone();
    sorted.sort_by(f64::total_cmp);
    let percentile = |p: f64| sorted[((sorted.len() as f64 * p).ceil() as usize).saturating_sub(1)];
    serde_json::json!({ "coverage": coverage, "minimum": sorted[0], "median": percentile(0.5),
        "p95": percentile(0.95), "maximum": sorted[sorted.len()-1], "mean": valid.iter().sum::<f64>() / valid.len() as f64 })
}
#[test]
fn invalid_timestamp_spans_are_unavailable_and_incomplete_coverage_has_no_distribution() {
    for (first, last, period) in [
        (1, 1, 1.0),
        (u64::MAX, u64::MAX, 1.0),
        (3, 2, 1.0),
        (0, 1, 1.0),
        (1, 2, f32::NAN),
    ] {
        let sample = Sample::decode(first, last, period);
        assert!(sample.micros.is_none());
        assert!(sample.unavailable.is_some());
        let summary = distribution(&[Sample::decode(1, 1001, 1.0), sample]);
        assert_eq!(summary["coverage"]["valid"], 1);
        assert_eq!(summary["coverage"]["unavailable"], 1);
        assert!(summary["distribution"].is_null());
        assert!(summary.get("median").is_none());
        assert!(summary.get("mean").is_none());
    }
}

#[test]
#[ignore = "explicit bounded Cull-only GPU timestamp diagnostic; no performance gate"]
fn diagnostic_cull_timestamps_500_lights_1920x540() {
    let gpu = Gpu::headless(1920, 540).expect("native GPU required");
    if !gpu.timestamps {
        eprintln!(
            "Cull timing unavailable on {} [{}]",
            gpu.adapter_name, gpu.backend
        );
        return;
    }
    let layouts = Layouts::new(&gpu.device, 1);
    let kernels = Kernels::new(&gpu.device, &layouts, true);
    let mut scratch = ScreenBounds::new(&gpu.device, &layouts.bounds);
    let lights = fixture(500, false, false);
    let size = (1920, 540);
    let camera = Vec3::new(0.0, 0.0, 4.0);
    let mut inline = Vec::new();
    let mut precomputed = Vec::new();
    for pair in 0..24 {
        let mut results = Vec::new();
        for cached in if pair % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let timer = Recorder::new(&gpu);
            let (membership, micros) = run_with_timing(
                &gpu,
                &layouts,
                &kernels,
                &mut scratch,
                &lights,
                size,
                camera,
                cached,
                Some(&timer),
            );
            let micros =
                micros.expect("a timestamp sample carries raw ticks even when unavailable");
            if pair >= 4 {
                if cached {
                    precomputed.push(micros);
                } else {
                    inline.push(micros);
                }
            }
            results.push(membership);
        }
        assert_membership(&results[0], &results[1], 500);
        let tiles = results[0].len() / 257;
        assert!(
            results[0][..tiles].contains(&500),
            "crowded tiles retain all 500 lights"
        );
    }
    eprintln!(
        "TL874_CULL_DIAGNOSTIC {}",
        serde_json::json!({
            "adapter": gpu.adapter_name, "backend": gpu.backend, "timestamp_period_ns": gpu.queue.get_timestamp_period(),
            "size": size, "lights": 500, "warmup_pairs": 4, "measured_alternating_pairs": 20,
            "fixture": "same-origin equal narrow-cone lights; Cull-only, no fixture rendering",
            "scope": "inline cull only versus precompute plus cull; excludes uploads/readback; no FPS or whole-Stage claim",
        "query_readback": "two phase: complete measured membership submission, then resolve timestamps in a separate submission",
            "inline_samples": inline, "precomputed_samples": precomputed,
            "inline_distribution_micros": distribution(&inline), "precomputed_distribution_micros": distribution(&precomputed),
        })
    );
}
