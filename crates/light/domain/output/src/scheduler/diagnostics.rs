//! Opt-in bounded wall observations of the actual scheduler, never a second deadline policy.
use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};
use tracing::Span;

const LIMIT: u64 = 16_384;
static ENABLED: LazyLock<bool> = LazyLock::new(|| {
    enabled(
        std::env::var("LIGHT_OUTPUT_PHASE_DIAGNOSTICS")
            .ok()
            .as_deref(),
    )
});
fn enabled(value: Option<&str>) -> bool {
    value == Some("all")
}

pub(super) struct Budget {
    enabled: bool,
    admitted: u64,
    cap_reported: bool,
    origin: Option<Instant>,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            enabled: *ENABLED,
            admitted: 0,
            cap_reported: false,
            origin: (*ENABLED).then(Instant::now),
        }
    }
}
impl Budget {
    pub(super) fn wait_origin(&self, span: Option<&Span>) -> Option<Instant> {
        span.filter(|span| !span.is_disabled()).and(self.origin)
    }
    pub(super) fn next(&mut self) -> Option<Span> {
        if !self.enabled {
            return None;
        }
        if self.admitted == LIMIT {
            if !self.cap_reported {
                self.cap_reported = true;
                tracing::info!(
                    schema_version = 1u8,
                    admitted_ticks = LIMIT,
                    "bounded output scheduler trace cap reached"
                );
            }
            return None;
        }
        self.admitted += 1;
        Some(tracing::info_span!("output_tick", tick_seq = self.admitted))
    }
}
fn micros(value: Duration) -> u64 {
    u64::try_from(value.as_micros()).unwrap_or(u64::MAX)
}

pub(super) fn tick(
    span: &Span,
    interval: Duration,
    duration: Duration,
    scheduled: bool,
    result_ok: bool,
    packets: u64,
) {
    if span.is_disabled() {
        return;
    }
    span.in_scope(|| {
        tracing::info!(
            schema_version = 1u8,
            interval_micros = micros(interval),
            tick_micros = micros(duration),
            scheduled_tick = u8::from(scheduled),
            result_ok = u8::from(result_ok),
            packets,
            "bounded output scheduler tick sample"
        )
    });
}

/// Called with the scheduler's original decision values, before logging can move the clock.
pub(super) fn deadline(interval: Duration, lateness: Duration, missed: bool, already_late: bool) {
    if !*ENABLED {
        return;
    }
    if !tracing::Span::current()
        .metadata()
        .is_some_and(|metadata| metadata.name() == "output_tick")
    {
        return;
    }
    tracing::info!(
        schema_version = 1u8,
        interval_micros = micros(interval),
        lateness_micros = micros(lateness),
        missed_deadline = u8::from(missed),
        decision_code = u8::from(!already_late),
        "bounded output scheduler deadline decision"
    );
}
/// One admitted tick's wait. All offsets use its scheduler budget's single monotonic origin.
#[derive(Clone, Copy)]
pub(super) struct WaitObservation {
    origin: Instant,
    requested: Instant,
    entered: Instant,
}
impl WaitObservation {
    pub(super) fn begin(
        origin: Option<Instant>,
        requested: Instant,
        entered: Instant,
    ) -> Option<Self> {
        origin.map(|origin| Self {
            origin,
            requested,
            entered,
        })
    }
}
fn signed_micros(later: Instant, earlier: Instant) -> i64 {
    if let Some(elapsed) = later.checked_duration_since(earlier) {
        i64::try_from(elapsed.as_micros()).unwrap_or(i64::MAX)
    } else {
        -i64::try_from(earlier.duration_since(later).as_micros()).unwrap_or(i64::MAX)
    }
}
/// Called first inside the selected branch. Disabled/capped waits do not read the clock.
pub(super) fn resumed(observation: Option<WaitObservation>, reason: u8) {
    if let Some(observation) = observation {
        resumed_at(observation, reason, Instant::now());
    }
}
/// Already-due waits use the decision's existing instant, before health locking or tracing.
pub(super) fn resumed_at(observation: WaitObservation, reason: u8, actual: Instant) {
    tracing::info!(
        schema_version = 1u8,
        resume_reason = reason,
        requested_deadline_micros = signed_micros(observation.requested, observation.origin),
        wait_entered_micros = signed_micros(observation.entered, observation.origin),
        remaining_to_deadline_micros = signed_micros(observation.requested, observation.entered),
        actual_resume_micros = signed_micros(actual, observation.origin),
        actual_wait_micros = micros(actual.saturating_duration_since(observation.entered)),
        resume_offset_micros = signed_micros(actual, observation.requested),
        "bounded output scheduler wait result"
    );
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
