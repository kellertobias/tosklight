//! Optional benchmark-only observations; no timing operation waits for GPU completion.
use serde::Serialize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

pub(super) fn open_device(
    adapter: &wgpu::Adapter,
) -> Result<(wgpu::Device, wgpu::Queue, bool, bool), String> {
    let benchmark = std::env::var_os("LIGHT_STAGE_PACKAGED_BENCH_REPORT").is_some();
    let timestamps = benchmark && adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: if timestamps {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        },
        ..Default::default()
    }))
    .map_err(|error| format!("the desk's window could not open a device: {error}"))?;
    Ok((device, queue, benchmark, timestamps))
}

const MAX_SAMPLES: usize = 256;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompositorScope {
    pub generation: u64,
    pub surface_width: u32,
    pub surface_height: u32,
    pub pane: Option<[u32; 4]>,
    pub source_size: Option<[u32; 2]>,
    pub transport: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompositorGpuSample {
    pub sample_id: u64,
    pub submission_id: u64,
    pub submitted_epoch_micros: u64,
    pub scope: CompositorScope,
    pub gpu_micros: f64,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompositorMeasurements {
    pub draw_attempts: u64,
    pub submissions: u64,
    pub clean_skips: u64,
    pub hidden_skips: u64,
    pub busy_skips: u64,
    pub paced_skips: u64,
    pub retries: u64,
    pub occluded: u64,
    pub timeouts: u64,
    pub reconfigures: u64,
    pub errors: u64,
    pub gpu_readback_errors: u64,
    pub gpu_invalid_samples: u64,
    pub gpu_sample_overflow: u64,
    pub cpu_wall_micros: u64,
    pub acquire_wall_micros: u64,
    pub gpu_samples: Vec<CompositorGpuSample>,
    pub gpu_timing_unavailable: Option<&'static str>,
    pub actual_displayed_frames: Option<u64>,
    pub timing_coverage: Option<viz_scene::gpu_timing::TimingCoverage>,
    pub measurement_boundary: &'static str,
}

pub(super) struct Measurement {
    pub stats: CompositorMeasurements,
    pub timer: Option<GpuTiming>,
    pub submission_id: u64,
}

impl Measurement {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        supported: bool,
    ) -> Result<Self, String> {
        let admission = viz_scene::gpu_timing::TimingAdmission::from_environment("compositor")?;
        Ok(Self {
            stats: CompositorMeasurements {
                gpu_timing_unavailable: (!supported).then_some("TIMESTAMP_QUERY unsupported"),
                measurement_boundary: "CPU is elapsed wall time. GPU samples cover only the compositor render pass; submissions and GPU completions are not actual displayed frames. Sparse samples do not estimate total GPU time per second.",
                ..Default::default()
            },
            timer: supported.then(|| GpuTiming::new(device, queue, admission)),
            submission_id: 0,
        })
    }

    pub fn collect(&mut self, device: &wgpu::Device) {
        let Some(timer) = self.timer.as_mut() else {
            return;
        };
        // Poll performs no blocking wait, including when the window has stopped submitting.
        if device.poll(wgpu::PollType::Poll).is_err() {
            self.stats.errors += 1;
        }
        match timer.collect(&mut self.stats.gpu_invalid_samples) {
            Ok(Some(sample)) if self.stats.gpu_samples.len() < MAX_SAMPLES => {
                self.stats.gpu_samples.push(sample)
            }
            Ok(Some(_)) => self.stats.gpu_sample_overflow += 1,
            Ok(None) => {}
            Err(()) => {
                self.stats.gpu_readback_errors += 1;
                self.stats.gpu_timing_unavailable = Some("GPU timestamp readback failed");
                self.timer = None;
            }
        }
    }

    pub fn drain(&mut self) -> CompositorMeasurements {
        self.stats.timing_coverage = self.timer.as_ref().map(|timer| {
            timer
                .admission
                .coverage(timer.completed_samples, timer.valid_samples)
        });
        let empty = CompositorMeasurements {
            gpu_timing_unavailable: self.stats.gpu_timing_unavailable,
            measurement_boundary: self.stats.measurement_boundary,
            ..Default::default()
        };
        std::mem::replace(&mut self.stats, empty)
    }
}

pub(super) struct GpuTiming {
    device: wgpu::Device,
    queue: wgpu::Queue,
    queries: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    readback: wgpu::Buffer,
    period: f64,
    // 0 = waiting, 1 = frame GPU work complete, 2 = mapped, 3 = mapping failed.
    ready: Arc<AtomicU8>,
    cancelled: Arc<AtomicBool>,
    phase: ReadbackPhase,
    pending: Option<CompositorGpuSample>,
    next_sample_id: u64,
    completed_samples: u64,
    valid_samples: u64,
    admission: viz_scene::gpu_timing::TimingAdmission,
    diagnostic: bool,
    frame_admitted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadbackPhase {
    Idle,
    AwaitingSubmission,
    AwaitingFrame,
    AwaitingMap,
}

impl GpuTiming {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        admission: viz_scene::gpu_timing::TimingAdmission,
    ) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("stage compositor timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolved: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("stage compositor timestamp resolve"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("stage compositor timestamp readback"),
                size: 16,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            period: f64::from(queue.get_timestamp_period()),
            ready: Arc::new(AtomicU8::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
            phase: ReadbackPhase::Idle,
            pending: None,
            next_sample_id: 0,
            completed_samples: 0,
            valid_samples: 0,
            admission,
            diagnostic: true,
            frame_admitted: false,
        }
    }

    pub fn set_diagnostic(&mut self, diagnostic: bool) {
        self.diagnostic = diagnostic;
    }

    pub fn begin_frame(&mut self) {
        self.frame_admitted = self.admission.begin_frame(
            self.diagnostic,
            self.pending.is_some() || self.cancelled.load(Ordering::Acquire),
        );
    }

    pub fn writes(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        (self.frame_admitted && self.pending.is_none() && !self.cancelled.load(Ordering::Acquire))
            .then_some(wgpu::RenderPassTimestampWrites {
                query_set: &self.queries,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            })
    }

    pub fn resolve(&mut self, submission_id: u64, scope: CompositorScope) -> bool {
        if !self.frame_admitted || self.pending.is_some() || self.cancelled.load(Ordering::Acquire)
        {
            return false;
        }
        // Reserve this query pair and its immutable identity only. Metal timestamp counters
        // must not be resolved in the command buffer that is still writing those counters.
        self.ready.store(0, Ordering::Release);
        self.phase = ReadbackPhase::AwaitingSubmission;
        self.next_sample_id = self.next_sample_id.saturating_add(1);
        self.pending = Some(CompositorGpuSample {
            sample_id: self.next_sample_id,
            submission_id,
            submitted_epoch_micros: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_micros().min(u128::from(u64::MAX)) as u64),
            scope,
            gpu_micros: 0.0,
        });
        true
    }

    /// Called exactly once after the render submission. This callback never waits or maps.
    pub fn request_readback(&mut self) {
        if self.phase != ReadbackPhase::AwaitingSubmission || self.cancelled.load(Ordering::Acquire)
        {
            return;
        }
        self.phase = ReadbackPhase::AwaitingFrame;
        let ready = self.ready.clone();
        let cancelled = self.cancelled.clone();
        self.queue.on_submitted_work_done(move || {
            if !cancelled.load(Ordering::Acquire) {
                ready.store(1, Ordering::Release);
            }
        });
    }

    /// Keep resources alive for already queued operations, but never enqueue a deferred resolve
    /// after retirement's terminal queue fence. Called on the same UI thread as collect().
    pub fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Encode the resolve in a fresh command buffer after the frame's completion callback.
    fn begin_mapping(&mut self) {
        if self.cancelled.load(Ordering::Acquire) {
            return;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("stage compositor deferred timestamp resolve"),
            });
        encoder.resolve_query_set(&self.queries, 0..2, &self.resolved, 0);
        encoder.copy_buffer_to_buffer(&self.resolved, 0, &self.readback, 0, 16);
        self.queue.submit(Some(encoder.finish()));
        self.phase = ReadbackPhase::AwaitingMap;
        let ready = self.ready.clone();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                ready.store(if result.is_ok() { 2 } else { 3 }, Ordering::Release);
            });
    }

    fn collect(&mut self, invalid: &mut u64) -> Result<Option<CompositorGpuSample>, ()> {
        if self.pending.is_none() || self.cancelled.load(Ordering::Acquire) {
            return Ok(None);
        }
        match self.ready.swap(0, Ordering::Acquire) {
            0 => Ok(None),
            1 if self.phase == ReadbackPhase::AwaitingFrame => {
                self.begin_mapping();
                Ok(None)
            }
            2 if self.phase == ReadbackPhase::AwaitingMap => {
                let decoded = {
                    let slice = self.readback.slice(..);
                    let bytes = slice.get_mapped_range().map_err(|_| ())?;
                    decode_micros(&bytes, self.period)
                };
                self.readback.unmap();
                let mut sample = self.pending.take().ok_or(())?;
                self.phase = ReadbackPhase::Idle;
                self.completed_samples = self.completed_samples.saturating_add(1);
                let Some(micros) = decoded else {
                    *invalid = invalid.saturating_add(1);
                    return Ok(None);
                };
                self.valid_samples = self.valid_samples.saturating_add(1);
                sample.gpu_micros = micros;
                Ok(Some(sample))
            }
            _ => Err(()),
        }
    }
}

fn decode_micros(bytes: &[u8], period: f64) -> Option<f64> {
    if !period.is_finite() || period <= 0.0 {
        return None;
    }
    let start = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    let end = u64::from_le_bytes(bytes.get(8..16)?.try_into().ok()?);
    if start == u64::MAX || end == u64::MAX {
        return None;
    }
    let micros = end.checked_sub(start)? as f64 * period / 1000.0;
    (micros.is_finite() && micros >= 0.0).then_some(micros)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_decode_rejects_invalid_order_size_and_period() {
        let bytes = [100_u64.to_le_bytes(), 350_u64.to_le_bytes()].concat();
        assert_eq!(decode_micros(&bytes, 2.0), Some(0.5));
        assert_eq!(decode_micros(&bytes[..8], 2.0), None);
        assert_eq!(decode_micros(&bytes, f64::NAN), None);
        assert_eq!(decode_micros(&bytes, 0.0), None);
        let unavailable = [u64::MAX.to_le_bytes(), u64::MAX.to_le_bytes()].concat();
        assert_eq!(decode_micros(&unavailable, 1.0), None);
        let reversed = [350_u64.to_le_bytes(), 100_u64.to_le_bytes()].concat();
        assert_eq!(decode_micros(&reversed, 1.0), None);
    }
}
