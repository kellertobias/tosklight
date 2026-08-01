//! Measuring the renderer, for the `--benchmark` run.
//!
//! A benchmark is not a frame rate: it is what the renderer did with the frames it was given, per
//! view, so a regression can be told from a heavier scene. The report is printed in one place and
//! read by build machines as well as by people.

use super::{Application, Measured};
use std::time::Instant;
use viz_scene::ViewMode;

/// One measured stretch of frames in one named view.
pub(super) struct BenchmarkSample {
    mode: ViewMode,
    frames: u64,
    seconds: f32,
    frame_millis: Vec<f32>,
    /// CPU time inside the renderer, waiting for the display excluded, which shows the headroom
    /// under a vsync-capped frame rate.
    cpu_millis: Vec<f32>,
    /// Time each frame spent waiting for the display to release a drawable. This is time the event
    /// loop cannot deliver input in, so it is reported beside the frame rate rather than hidden
    /// inside it.
    wait_millis: Vec<f32>,
    /// What the GPU itself spent, where the adapter can time a frame. This is the headroom number:
    /// a frame rate held at the refresh interval says nothing about how much work is left over.
    gpu_millis: Vec<f32>,
    latency_p50: f32,
    latency_p95: f32,
    latency_max: f32,
    dmx_hz: f32,
    lights: u32,
    degraded_frames: u64,
}

impl BenchmarkSample {
    fn new(mode: ViewMode) -> Self {
        Self {
            mode,
            frames: 0,
            seconds: 0.0,
            frame_millis: Vec::new(),
            cpu_millis: Vec::new(),
            wait_millis: Vec::new(),
            gpu_millis: Vec::new(),
            latency_p50: 0.0,
            latency_p95: 0.0,
            latency_max: 0.0,
            dmx_hz: 0.0,
            lights: 0,
            degraded_frames: 0,
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
        self.benchmark.push(BenchmarkSample::new(mode));
        self.benchmark_view_started = Instant::now();
    }

    /// Fold this frame into the current benchmark sample and advance the view when its slice is
    /// over. Returns `true` when the whole run is finished.
    pub(super) fn record_benchmark(&mut self, frame_seconds: f32, measured: &Measured) -> bool {
        let Some(total) = self.options.benchmark_seconds else {
            return false;
        };
        let views = self.benchmark_views();
        let slice = total / views.len() as f32;
        let (p50, p95, max) = measured.latency;
        let dmx_hz = measured.dmx_hz;
        let degraded = self.stats.degraded;
        let lights = self.stats.lights;
        let wait_millis = self.stats.acquire_micros as f32 / 1000.0;
        let cpu_millis = self
            .stats
            .cpu_micros
            .saturating_sub(self.stats.acquire_micros) as f32
            / 1000.0;
        if let Some(sample) = self.benchmark.last_mut() {
            sample.frames += 1;
            sample.seconds += frame_seconds;
            sample.frame_millis.push(frame_seconds * 1000.0);
            sample.cpu_millis.push(cpu_millis);
            sample.wait_millis.push(wait_millis);
            if let Some(micros) = self.stats.gpu_micros {
                sample.gpu_millis.push(micros as f32 / 1000.0);
            }
            sample.latency_p50 = p50;
            sample.latency_p95 = p95;
            sample.latency_max = max;
            sample.dmx_hz = dmx_hz;
            sample.lights = sample.lights.max(lights);
            if degraded {
                sample.degraded_frames += 1;
            }
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
        self.benchmark.push(BenchmarkSample::new(mode));
        self.benchmark_view_started = Instant::now();
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
            "cpu p95",
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
                sample.frames as f32 / sample.seconds.max(1e-3),
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
