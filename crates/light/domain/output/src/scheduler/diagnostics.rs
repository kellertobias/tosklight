//! Opt-in bounded wall observations of the actual scheduler, never a second deadline policy.
use std::{sync::LazyLock, time::Duration};
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
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            enabled: *ENABLED,
            admitted: 0,
            cap_reported: false,
        }
    }
}
impl Budget {
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
pub(super) fn resumed(reason: u8) {
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
        resume_reason = reason,
        "bounded output scheduler wait result"
    );
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
