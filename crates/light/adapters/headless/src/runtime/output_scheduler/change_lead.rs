//! TL-659: the output lane's change lead time measurement.
//!
//! A committed frame carries the earliest Cue or Dynamic start it claimed
//! (`CommittedDynamicOutput::change_lead_start`). Once the frame's delivery is known, the lead is
//! recorded against the application clock the starts were scheduled on. The lane holds its own
//! handle on the recorder, so recording never takes the output health lock; the work is a few
//! atomic operations (`light_output::ChangeLeadTime`).
use super::*;
use light_output::ChangeLeadTime;

/// The recorder every `OutputHealth` reader shares; the lane takes it once, at start.
pub(in crate::runtime) fn change_lead_recorder(
    health: &std::sync::Mutex<OutputHealth>,
) -> Arc<ChangeLeadTime> {
    Arc::clone(
        &health
            .lock()
            .expect("output health mutex poisoned")
            .change_lead,
    )
}

/// A sent frame measures the starts it carries; a frame that never reached the wire hands them
/// to the next sent frame.
pub(in crate::runtime) fn record_change_lead(
    recorder: &ChangeLeadTime,
    engine: &Engine,
    start: Option<i64>,
    delivered: bool,
) {
    if delivered {
        recorder.frame_sent(start, engine.application_time().timestamp_micros());
    } else {
        recorder.frame_not_sent(start);
    }
}

/// A capture whose frame failed before it committed still owes its Cue starts a measurement.
pub(in crate::runtime) fn carry_uncommitted_capture(
    recorder: &ChangeLeadTime,
    frame: &light_engine::PreparedOutputFrame,
) {
    recorder.frame_not_sent(frame.claim_change_lead_start());
}
