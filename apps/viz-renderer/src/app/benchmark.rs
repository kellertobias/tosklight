//! Measuring the renderer, for the `--benchmark` run.
//!
//! A benchmark is not a frame rate: it is what the renderer did with the frames it was given, per
//! view, so a regression can be told from a heavier scene. The report is printed in one place and
//! read by build machines as well as by people.

mod measurement;
#[cfg(test)]
mod tests;

use super::{Application, Measured};
use std::collections::HashSet;
use std::time::Instant;
use viz_scene::ViewMode;

const GPU_PASS_LABELS: [&str; 8] = [
    "cull",
    "shadow",
    "opaque",
    "beams",
    "lasers",
    "bloom",
    "composite",
    "overlay",
];

/// One measured stretch of frames in one named view.
pub(super) struct BenchmarkSample {
    mode: ViewMode,
    frames: u64,
    seconds: f64,
    started: Instant,
    last_presented: Option<Instant>,
    attempted: u64,
    skipped: u64,
    failed: u64,
    seen_gpu: HashSet<u64>,
    rejected_gpu: u64,
    frame_millis: Vec<f32>,
    /// Elapsed renderer wall time excluding drawable acquire; thread CPU is not measured.
    cpu_millis: Vec<f32>,
    /// Time each frame spent waiting for the display to release a drawable. This is time the event
    /// loop cannot deliver input in, so it is reported beside the frame rate rather than hidden
    /// inside it.
    wait_millis: Vec<f32>,
    /// What the GPU itself spent, where the adapter can time a frame. This is the headroom number:
    /// a frame rate held at the refresh interval says nothing about how much work is left over.
    gpu_millis: Vec<f32>,
    /// Same sampled GPU frames as `gpu_millis`, kept apart so benchmark output identifies the
    /// pass consuming the frame budget.
    gpu_pass_millis: [Vec<f32>; 8],
    latency_p50: f32,
    latency_p95: f32,
    latency_max: f32,
    dmx_hz: f32,
    lights: u32,
    degraded_frames: u64,
    crowd_authored: u32,
    crowd_drawn: u32,
    particles_requested: u32,
    particles_drawn: u32,
}

impl BenchmarkSample {
    fn new(mode: ViewMode, started: Instant) -> Self {
        Self {
            mode,
            frames: 0,
            seconds: 0.0,
            started,
            last_presented: None,
            attempted: 0,
            skipped: 0,
            failed: 0,
            seen_gpu: HashSet::new(),
            rejected_gpu: 0,
            frame_millis: Vec::new(),
            cpu_millis: Vec::new(),
            wait_millis: Vec::new(),
            gpu_millis: Vec::new(),
            gpu_pass_millis: std::array::from_fn(|_| Vec::new()),
            latency_p50: 0.0,
            latency_p95: 0.0,
            latency_max: 0.0,
            dmx_hz: 0.0,
            lights: 0,
            degraded_frames: 0,
            crowd_authored: 0,
            crowd_drawn: 0,
            particles_requested: 0,
            particles_drawn: 0,
        }
    }

    fn frame_percentiles(&self) -> (f32, f32) {
        (
            percentile(&self.frame_millis, 0.5),
            percentile(&self.frame_millis, 0.95),
        )
    }
}

/// One percentile of a set of per-frame millisecond samples.
fn percentile(samples: &[f32], fraction: f32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f32::total_cmp);
    sorted[((sorted.len() - 1) as f32 * fraction).round() as usize]
}

impl Application {
    /// Views the benchmark walks through. A single-view run measures whatever is selected.
    pub(super) fn benchmark_views(&self) -> Vec<ViewMode> {
        if self.options.benchmark_all_views {
            ViewMode::ALL.to_vec()
        } else {
            vec![self.options.view.unwrap_or(ViewMode::Full3d)]
        }
    }

    pub(super) fn start_benchmark(&mut self) {
        let mode = self.benchmark_views()[0];
        self.set_view_mode(mode);
        self.benchmark_view_started = Instant::now();
        self.benchmark
            .push(BenchmarkSample::new(mode, self.benchmark_view_started));
    }

    /// Fold this frame into the current benchmark sample and advance the view when its slice is
    /// over. Returns `true` when the whole run is finished.
    pub(super) fn record_benchmark(
        &mut self,
        observed: Instant,
        outcome: &Result<viz_render::FrameStats, viz_render::RenderError>,
        measured: &Measured,
    ) -> bool {
        let Some(total) = self.options.benchmark_seconds else {
            return false;
        };
        let views = self.benchmark_views();
        // Redraw events already queued when exit is requested must not reopen the closed window.
        if self.benchmark_view_index >= views.len() {
            return false;
        }
        let slice = total / views.len() as f32;
        let (p50, p95, max) = measured.latency;
        let dmx_hz = measured.dmx_hz;
        if let Some(sample) = self.benchmark.last_mut() {
            sample.record_outcome(outcome, observed);
            sample.latency_p50 = p50;
            sample.latency_p95 = p95;
            sample.latency_max = max;
            sample.dmx_hz = dmx_hz;
        }
        if self.benchmark_view_started.elapsed().as_secs_f32() < slice {
            return false;
        }
        self.benchmark_view_index += 1;
        if self.benchmark_view_index >= views.len() {
            return true;
        }
        let mode = views[self.benchmark_view_index];
        self.set_view_mode(mode);
        self.benchmark_view_started = Instant::now();
        self.benchmark
            .push(BenchmarkSample::new(mode, self.benchmark_view_started));
        false
    }

    pub(super) fn print_benchmark(&self, measured: &Measured) {
        println!("\nViz renderer benchmark");
        println!(
            "scene: {} fixtures, {} heads, show {}",
            measured.fixtures, measured.emitters, measured.show,
        );
        println!("renderer: {}", self.renderer_label());
        println!(
            "{:<16} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9} {:>9} {:>9} {:>8} {:>7}",
            "view",
            "frames",
            "fps",
            "p50 ms",
            "p95 ms",
            "wall p95",
            "gpu p95",
            "wait p95",
            "lat p50",
            "lat p95",
            "lat max",
            "dmx hz",
            "lights"
        );
        for sample in &self.benchmark {
            // Frame pacing matters more than average FPS, so report the frame-time percentiles.
            let (frame_p50, frame_p95) = sample.frame_percentiles();
            println!(
                "{:<16} {:>7} {:>8.1} {:>8.2} {:>8.2} {:>8.2} {:>8} {:>8.2} {:>9.1} {:>9.1} {:>9.1} {:>8.1} {:>7}",
                sample.mode.label(),
                sample.frames,
                sample.frames as f64 / sample.seconds.max(1e-3),
                frame_p50,
                frame_p95,
                percentile(&sample.cpu_millis, 0.95),
                // An adapter that cannot time a pass reports nothing rather than a zero that
                // would read as a frame costing the GPU no time at all.
                if sample.gpu_millis.is_empty() {
                    "-".to_owned()
                } else {
                    format!("{:.2}", percentile(&sample.gpu_millis, 0.95))
                },
                percentile(&sample.wait_millis, 0.95),
                sample.latency_p50,
                sample.latency_p95,
                sample.latency_max,
                sample.dmx_hz,
                sample.lights,
            );
            println!(
                "  attempts: {}  presented: {}  skipped: {}  errors: {}  observed seconds: {:.3}  intervals: {}",
                sample.attempted,
                sample.frames,
                sample.skipped,
                sample.failed,
                sample.seconds,
                sample.frame_millis.len()
            );
            println!(
                "  GPU unique samples: {}  rejected provenance: {}  thread CPU: unavailable; wall excludes acquire; presented means submitted to the presentation queue, not display-confirmed",
                sample.gpu_millis.len(),
                sample.rejected_gpu
            );
            let pass_p95 = GPU_PASS_LABELS
                .iter()
                .zip(&sample.gpu_pass_millis)
                .filter(|(_, samples)| !samples.is_empty())
                .map(|(label, samples)| format!("{label} {:.2}", percentile(samples, 0.95)))
                .collect::<Vec<_>>();
            if !pass_p95.is_empty() {
                println!("  GPU pass p95 ms: {}", pass_p95.join("  |  "));
            }
            println!(
                "  crowd authored/drawn: {}/{}",
                sample.crowd_authored, sample.crowd_drawn
            );
            println!(
                "  particles requested/drawn: {}/{}",
                sample.particles_requested, sample.particles_drawn
            );
        }
        for input in &measured.inputs {
            println!(
                "input {} {} u{}->{} {} accepted:{} duplicate:{} malformed:{} out-of-order:{}",
                input.protocol.label(),
                input.health.label(),
                input.logical_universe,
                input.destination_universe,
                input.delivery,
                input.accepted_packets,
                input.duplicate_packets,
                input.malformed_packets,
                input.out_of_order_packets,
            );
        }
        let degraded: u64 = self
            .benchmark
            .iter()
            .map(|sample| sample.degraded_frames)
            .sum();
        println!("degraded frames: {degraded}");
    }
}
