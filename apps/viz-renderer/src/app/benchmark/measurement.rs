//! Outcome accounting is independent of window creation and GPU availability.
use super::BenchmarkSample;
use std::time::Instant;
use viz_render::{FrameStats, RenderError};

impl BenchmarkSample {
    pub(super) fn record_outcome(
        &mut self,
        result: &Result<FrameStats, RenderError>,
        observed: Instant,
    ) {
        self.attempted += 1;
        self.seconds = observed
            .saturating_duration_since(self.started)
            .as_secs_f64();
        match result {
            Ok(stats) => self.record_success(stats, observed),
            Err(RenderError::SkipFrame) => self.skipped += 1,
            Err(_) => self.failed += 1,
        }
    }

    fn record_success(&mut self, stats: &FrameStats, observed: Instant) {
        self.frames += 1;
        if let Some(previous) = self.last_presented.replace(observed) {
            self.frame_millis
                .push(observed.saturating_duration_since(previous).as_secs_f32() * 1000.0);
        }
        self.cpu_millis
            .push(stats.cpu_micros.saturating_sub(stats.acquire_micros) as f32 / 1000.0);
        self.wait_millis.push(stats.acquire_micros as f32 / 1000.0);
        self.record_gpu(stats);
        self.budget.record(stats);
        self.lights = self.lights.max(stats.lights);
        self.degraded_frames += u64::from(stats.degraded);
        self.crowd_authored = self.crowd_authored.max(stats.crowd_authored);
        self.crowd_drawn = self.crowd_drawn.max(stats.crowd_drawn);
        self.particles_requested = self.particles_requested.max(stats.particles_requested);
        self.particles_drawn = self.particles_drawn.max(stats.particles_drawn);
    }

    fn record_gpu(&mut self, stats: &FrameStats) {
        let Some(id) = stats.gpu_sample_id else {
            return;
        };
        // Remember rejected samples too: a late completion cannot become a new sample later.
        if !self.seen_gpu.insert(id) {
            return;
        }
        if !stats.gpu_sample_matches_frame {
            self.rejected_gpu += 1;
            return;
        }
        if let Some(micros) = stats.gpu_micros {
            self.overlapping_gpu_samples += u64::from(stats.gpu_passes.overlapping_passes);
            self.gpu_millis.push(micros as f32 / 1000.0);
        }
        for (samples, micros) in self
            .gpu_pass_millis
            .iter_mut()
            .zip(stats.gpu_passes.as_array())
        {
            if let Some(micros) = micros {
                samples.push(micros as f32 / 1000.0);
            }
        }
    }
}
