//! TL-659: Dynamic starts waiting for the first output frame that carries them.
//!
//! A start is due at its activation instant plus its activation delay. A start that waits for
//! the next beat or bar boundary is marked when sampling chooses that boundary instead, since
//! only then is its scheduled instant known. The Live output lane claims due starts under the
//! Dynamics lock once its frame has committed.
use super::*;

/// Application-time milliseconds as the ledger's microseconds.
pub(super) fn ledger_micros(millis: u64) -> i64 {
    i64::try_from(millis)
        .unwrap_or(i64::MAX / 1_000)
        .saturating_mul(1_000)
}

impl DynamicRuntime {
    /// Marks a controller start that is due at its activation, not at a later boundary.
    pub(super) fn mark_start_change_lead(
        &mut self,
        request: &DynamicStartRequest,
        waits_for_boundary: bool,
    ) {
        if !waits_for_boundary {
            let due = request
                .now_millis
                .saturating_add(request.activation_delay_millis);
            self.change_lead.mark(ledger_micros(due));
        }
    }

    /// The earliest start due by `now_millis`, in application-time microseconds, taken by the
    /// output frame sampled then. Call only from the Live output lane.
    pub fn claim_change_lead_start(&mut self, now_millis: u64) -> Option<i64> {
        self.change_lead.claim(ledger_micros(now_millis))
    }
}
