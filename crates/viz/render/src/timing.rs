//! What the GPU itself spent on a frame, split by named render pass.
//!
//! Timestamp results are copied to a mapping buffer and collected on a later frame. Nothing in
//! the presentation path waits for the GPU; while one readback is in flight, frames simply run
//! without timing queries and keep the most recent completed measurement.

use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const PASS_COUNT: usize = 8;
const QUERY_COUNT: u32 = (PASS_COUNT * 2) as u32;
const TIMESTAMP_BYTES: u64 = QUERY_COUNT as u64 * 8;
const RESOLVED_BYTES: u64 = PASS_COUNT as u64 * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;

/// A stable slot for every pass reported by the renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuPass {
    Cull,
    Shadow,
    Opaque,
    Beams,
    Lasers,
    Bloom,
    Composite,
    Overlay,
}

impl GpuPass {
    const ALL: [Self; PASS_COUNT] = [
        Self::Cull,
        Self::Shadow,
        Self::Opaque,
        Self::Beams,
        Self::Lasers,
        Self::Bloom,
        Self::Composite,
        Self::Overlay,
    ];

    const fn index(self) -> usize {
        self as usize
    }

    const fn queries(self) -> (u32, u32) {
        let beginning = self.index() as u32 * 2;
        (beginning, beginning + 1)
    }
}

/// Most recently completed GPU cost for each logical frame pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuPassTimings {
    pub cull_micros: Option<u64>,
    pub shadow_micros: Option<u64>,
    pub opaque_micros: Option<u64>,
    pub beams_micros: Option<u64>,
    pub lasers_micros: Option<u64>,
    pub bloom_micros: Option<u64>,
    pub composite_micros: Option<u64>,
    pub overlay_micros: Option<u64>,
}

impl GpuPassTimings {
    /// Sum of the passes that actually ran. Skipped passes remain absent rather than looking free.
    pub fn total_micros(self) -> Option<u64> {
        let values = [
            self.cull_micros,
            self.shadow_micros,
            self.opaque_micros,
            self.beams_micros,
            self.lasers_micros,
            self.bloom_micros,
            self.composite_micros,
            self.overlay_micros,
        ];
        values.into_iter().flatten().reduce(u64::saturating_add)
    }

    /// Stable pass order for benchmark columns and diagnostics serialization.
    pub fn as_array(self) -> [Option<u64>; PASS_COUNT] {
        [
            self.cull_micros,
            self.shadow_micros,
            self.opaque_micros,
            self.beams_micros,
            self.lasers_micros,
            self.bloom_micros,
            self.composite_micros,
            self.overlay_micros,
        ]
    }

    fn set(&mut self, pass: GpuPass, micros: u64) {
        *match pass {
            GpuPass::Cull => &mut self.cull_micros,
            GpuPass::Shadow => &mut self.shadow_micros,
            GpuPass::Opaque => &mut self.opaque_micros,
            GpuPass::Beams => &mut self.beams_micros,
            GpuPass::Lasers => &mut self.lasers_micros,
            GpuPass::Bloom => &mut self.bloom_micros,
            GpuPass::Composite => &mut self.composite_micros,
            GpuPass::Overlay => &mut self.overlay_micros,
        } = Some(micros);
    }
}

/// Timestamp queries for one sampled frame, and the asynchronous readback that follows.
pub struct GpuTimer {
    queries: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    readback: wgpu::Buffer,
    period_nanos: f32,
    ready: Arc<AtomicBool>,
    broken: Arc<AtomicBool>,
    in_flight: bool,
    /// Passes written into the command encoder currently being built.
    active_mask: Cell<u8>,
    /// Passes in the mapping currently in flight.
    readback_mask: u8,
    last: GpuPassTimings,
    sample_id: u64,
}

impl GpuTimer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, supported: bool) -> Option<Self> {
        if !supported {
            return None;
        }
        let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("viz pass timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: QUERY_COUNT,
        });
        let resolved = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viz timestamps resolved"),
            // Each optional pair is resolved independently, so each destination starts at the
            // alignment required by WebGPU. The compact readback below still uses only 16 bytes
            // per pass.
            size: RESOLVED_BYTES,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viz timestamps readback"),
            size: TIMESTAMP_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Some(Self {
            queries,
            resolved,
            readback,
            period_nanos: queue.get_timestamp_period(),
            ready: Arc::new(AtomicBool::new(false)),
            broken: Arc::new(AtomicBool::new(false)),
            in_flight: false,
            active_mask: Cell::new(0),
            readback_mask: 0,
            last: GpuPassTimings::default(),
            sample_id: 0,
        })
    }

    pub fn render_writes(&self, pass: GpuPass) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.render_writes_between(pass, true, true)
    }

    pub fn render_opening(&self, pass: GpuPass) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.render_writes_between(pass, true, false)
    }

    pub fn render_closing(&self, pass: GpuPass) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.render_writes_between(pass, false, true)
    }

    fn render_writes_between(
        &self,
        pass: GpuPass,
        beginning: bool,
        end: bool,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        if self.in_flight || self.is_broken() {
            return None;
        }
        self.active_mask
            .set(self.active_mask.get() | 1 << pass.index());
        let (first, last) = pass.queries();
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: beginning.then_some(first),
            end_of_pass_write_index: end.then_some(last),
        })
    }

    pub fn compute_writes(&self, pass: GpuPass) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        if self.in_flight || self.is_broken() {
            return None;
        }
        self.active_mask
            .set(self.active_mask.get() | 1 << pass.index());
        let (beginning, end) = pass.queries();
        Some(wgpu::ComputePassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(beginning),
            end_of_pass_write_index: Some(end),
        })
    }

    /// Resolve only query pairs that were written. This is required for optional passes.
    pub fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if self.in_flight || self.is_broken() {
            return;
        }
        let mask = self.active_mask.replace(0);
        if mask == 0 {
            return;
        }
        for pass in GpuPass::ALL {
            if mask & (1 << pass.index()) == 0 {
                continue;
            }
            let (beginning, end) = pass.queries();
            let resolved_offset = pass.index() as u64 * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
            let readback_offset = pass.index() as u64 * 16;
            encoder.resolve_query_set(
                &self.queries,
                beginning..end + 1,
                &self.resolved,
                resolved_offset,
            );
            encoder.copy_buffer_to_buffer(
                &self.resolved,
                resolved_offset,
                &self.readback,
                readback_offset,
                16,
            );
        }
        self.readback_mask = mask;
        self.in_flight = true;
    }

    pub fn request_readback(&self) {
        if !self.in_flight {
            return;
        }
        let ready = self.ready.clone();
        let broken = self.broken.clone();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| match result {
                Ok(()) => ready.store(true, Ordering::Release),
                Err(_) => broken.store(true, Ordering::Release),
            });
    }

    /// Take a completed measurement without waiting for it.
    pub fn collect(&mut self) {
        if self.is_broken() {
            self.in_flight = false;
            self.last = GpuPassTimings::default();
            return;
        }
        if !self.in_flight || !self.ready.swap(false, Ordering::Acquire) {
            return;
        }
        {
            let slice = self.readback.slice(..);
            if let Ok(mapped) = slice.get_mapped_range() {
                self.last = decode_timings(&mapped, self.readback_mask, self.period_nanos);
                self.sample_id = self.sample_id.wrapping_add(1);
            }
        }
        self.readback.unmap();
        self.in_flight = false;
        self.readback_mask = 0;
    }

    pub fn timings(&self) -> GpuPassTimings {
        self.last
    }

    pub fn sample_id(&self) -> u64 {
        self.sample_id
    }

    fn is_broken(&self) -> bool {
        self.broken.load(Ordering::Acquire)
    }
}

fn decode_timings(bytes: &[u8], mask: u8, period_nanos: f32) -> GpuPassTimings {
    let mut timings = GpuPassTimings::default();
    for pass in GpuPass::ALL {
        if mask & (1 << pass.index()) == 0 {
            continue;
        }
        let (beginning, end) = pass.queries();
        let tick = |index: u32| {
            let start = index as usize * 8;
            u64::from_le_bytes(bytes[start..start + 8].try_into().expect("timestamp bytes"))
        };
        let nanos = tick(end).saturating_sub(tick(beginning)) as f32 * period_nanos;
        timings.set(pass, (nanos / 1000.0) as u64);
    }
    timings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_only_passes_that_ran_and_sums_them() {
        let mut bytes = [0_u8; TIMESTAMP_BYTES as usize];
        for (query, ticks) in [(4_u32, 100_u64), (5, 350), (12, 500), (13, 650)] {
            let offset = query as usize * 8;
            bytes[offset..offset + 8].copy_from_slice(&ticks.to_le_bytes());
        }
        let mask = (1 << GpuPass::Opaque.index()) | (1 << GpuPass::Composite.index());
        let timings = decode_timings(&bytes, mask, 2.0);
        assert_eq!(timings.opaque_micros, Some(0));
        assert_eq!(timings.composite_micros, Some(0));
        assert_eq!(timings.beams_micros, None);
        assert_eq!(timings.total_micros(), Some(0));
    }

    #[test]
    fn converts_timestamp_ticks_to_microseconds() {
        let mut bytes = [0_u8; TIMESTAMP_BYTES as usize];
        let (beginning, end) = GpuPass::Beams.queries();
        bytes[beginning as usize * 8..beginning as usize * 8 + 8]
            .copy_from_slice(&1_000_u64.to_le_bytes());
        bytes[end as usize * 8..end as usize * 8 + 8].copy_from_slice(&6_000_u64.to_le_bytes());
        let timings = decode_timings(&bytes, 1 << GpuPass::Beams.index(), 2.0);
        assert_eq!(timings.beams_micros, Some(10));
        assert_eq!(timings.total_micros(), Some(10));
    }
}
