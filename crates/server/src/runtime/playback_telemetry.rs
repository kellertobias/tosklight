//! Sampled playback runtime telemetry: a render-frame divider nearest ~10 Hz drives
//! delta-only pushes of the volatile playback values (fade progress, master, current cue,
//! button state) onto the v2 events lane.

use chrono::{DateTime, Utc};
use light_application::{
    EventDraft, PlaybackShowScope, PlaybackTelemetryDeltas, PlaybackTelemetryTick,
    telemetry_frame_divider,
};
use light_engine::Engine;
use parking_lot::Mutex;
use std::sync::{
    Arc,
    atomic::{AtomicU16, Ordering},
};
use uuid::Uuid;

/// Frame-coherent telemetry sampling state shared by the output scheduler and the test bench.
///
/// Deriving the tick from the completed-frame counter — never a wall-clock timer — keeps
/// samples aligned with the output clock, adds no timer wakeups, and keeps the telemetry rate
/// stable across output-rate configurations.
pub(super) struct PlaybackTelemetrySampler {
    rate: Arc<AtomicU16>,
    state: Mutex<SamplerState>,
}

#[derive(Default)]
struct SamplerState {
    frame: u64,
    show_id: Option<Uuid>,
    deltas: PlaybackTelemetryDeltas,
}

impl PlaybackTelemetrySampler {
    pub(super) fn new(rate: Arc<AtomicU16>) -> Self {
        Self {
            rate,
            state: Mutex::new(SamplerState::default()),
        }
    }

    /// Advances the completed-frame counter and, on sampling ticks, returns the delta event
    /// draft — or `None` between ticks and when no sampled value changed.
    ///
    /// Reads only already-published engine state; the timing-critical render path itself is
    /// never touched.
    pub(super) fn completed_frame(
        &self,
        engine: &Engine,
        show_id: Uuid,
        show_revision: u64,
        at: DateTime<Utc>,
    ) -> Option<EventDraft> {
        let rate = self.rate.load(Ordering::Relaxed).max(1);
        let divider = telemetry_frame_divider(rate);
        let mut state = self.state.lock();
        state.frame += 1;
        if state.show_id != Some(show_id) {
            state.deltas.reset();
            state.show_id = Some(show_id);
        }
        if !state.frame.is_multiple_of(divider) {
            return None;
        }
        let (samples, released) = state.deltas.advance(engine.playback_telemetry_at(at));
        if samples.is_empty() && released.is_empty() {
            return None;
        }
        Some(EventDraft::playback_telemetry_sampled(
            PlaybackTelemetryTick {
                scope: PlaybackShowScope {
                    show_id,
                    show_revision,
                },
                frame: state.frame,
                sample_rate_hz: f32::from(rate) / divider as f32,
                samples,
                released,
            },
        ))
    }
}
