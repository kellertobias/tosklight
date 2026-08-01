//! What the GPU itself spent on a frame.
//!
//! The frame time an application can measure on its own is the time between two presents, which on
//! a display-limited surface is the refresh interval and says nothing about the headroom. The
//! answer to "can this rig afford another quality tier" is on the GPU, and only the GPU can time
//! it: two timestamps written around the frame's passes, resolved into a buffer, and read back a
//! frame later so nothing ever waits for the device.
//!
//! An adapter without timestamp queries simply has no timer, and every reading is `None`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Timestamps written around one frame, and the readback that follows a frame behind.
pub struct GpuTimer {
    queries: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    readback: wgpu::Buffer,
    /// Nanoseconds per timestamp tick, from the queue.
    period_nanos: f32,
    /// Set by the map callback, cleared when the value has been taken.
    ready: Arc<AtomicBool>,
    /// Set when a mapping failed. The device has bigger problems than a diagnostic number, so the
    /// timer stops rather than reporting a frozen one for the rest of the session.
    broken: Arc<AtomicBool>,
    /// Whether a frame's readback is currently in flight.
    in_flight: bool,
    /// The most recent completed measurement.
    last_micros: Option<u64>,
}

impl GpuTimer {
    /// Build a timer, or `None` when this adapter cannot time a pass.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, supported: bool) -> Option<Self> {
        if !supported {
            return None;
        }
        let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("viz frame timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        let resolved = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viz timestamps resolved"),
            size: 16,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viz timestamps readback"),
            size: 16,
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
            last_micros: None,
        })
    }

    /// The timestamp writes for the frame's first pass, or `None` while a readback is still out.
    ///
    /// Only one frame is ever measured at a time. Timing every frame would need a ring of buffers
    /// for a number nobody reads more than a few times a second.
    pub fn opening_writes(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        (!self.in_flight && !self.is_broken()).then_some(wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: None,
        })
    }

    /// The timestamp writes for the frame's last pass.
    pub fn closing_writes(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        (!self.in_flight && !self.is_broken()).then_some(wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: None,
            end_of_pass_write_index: Some(1),
        })
    }

    /// Resolve this frame's timestamps into the readback buffer. Call once, after the last pass.
    pub fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if self.in_flight || self.is_broken() {
            return;
        }
        encoder.resolve_query_set(&self.queries, 0..2, &self.resolved, 0);
        encoder.copy_buffer_to_buffer(&self.resolved, 0, &self.readback, 0, 16);
        self.in_flight = true;
    }

    /// Ask for the mapping. Call after the frame has been submitted.
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

    /// Take a completed measurement, if the device has finished with the last one.
    ///
    /// Never waits: an unfinished readback simply leaves the previous reading in place.
    pub fn collect(&mut self) {
        if self.is_broken() {
            // Nothing was mapped, so there is nothing to unmap: the timer simply stops.
            self.in_flight = false;
            self.last_micros = None;
            return;
        }
        if !self.in_flight || !self.ready.swap(false, Ordering::Acquire) {
            return;
        }
        {
            let slice = self.readback.slice(..);
            if let Ok(mapped) = slice.get_mapped_range() {
                let mut ticks = [0_u64; 2];
                for (index, tick) in ticks.iter_mut().enumerate() {
                    let start = index * 8;
                    *tick = u64::from_le_bytes(
                        mapped[start..start + 8]
                            .try_into()
                            .expect("eight bytes per timestamp"),
                    );
                }
                let elapsed = ticks[1].saturating_sub(ticks[0]) as f32 * self.period_nanos;
                self.last_micros = Some((elapsed / 1000.0) as u64);
            }
        }
        self.readback.unmap();
        self.in_flight = false;
    }

    /// The last completed measurement, in microseconds.
    pub fn micros(&self) -> Option<u64> {
        self.last_micros
    }

    fn is_broken(&self) -> bool {
        self.broken.load(Ordering::Acquire)
    }
}
