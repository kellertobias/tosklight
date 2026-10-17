//! Sampled tracing of output ticks whose phases exceed one slow-frame threshold.

pub(in crate::runtime::output_scheduler) mod live_phases;

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const SLOW_OUTPUT_PHASE_THRESHOLD: Duration = Duration::from_millis(20);
const SLOW_OUTPUT_PHASE_SAMPLE_LIMIT: u64 = 128;
static SLOW_OUTPUT_PHASE_SAMPLES: AtomicU64 = AtomicU64::new(0);

pub(super) fn trace_slow_output_phases(
    total: Duration,
    dynamic: Duration,
    engine: Duration,
    publish: Duration,
    send: Duration,
) {
    if total < SLOW_OUTPUT_PHASE_THRESHOLD {
        return;
    }
    let sample = SLOW_OUTPUT_PHASE_SAMPLES.fetch_add(1, Ordering::Relaxed);
    if sample >= SLOW_OUTPUT_PHASE_SAMPLE_LIMIT {
        return;
    }
    tracing::info!(
        sample = sample + 1,
        total_micros = duration_micros(total),
        dynamic_micros = duration_micros(dynamic),
        engine_micros = duration_micros(engine),
        publish_micros = duration_micros(publish),
        send_micros = duration_micros(send),
        "slow output tick phase sample"
    );
}

fn duration_micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

const APPLICATION_SECTION_LIMIT: u64 = 16_384;
static APPLICATION_SECTION_SAMPLES: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
pub(super) enum ApplicationPhase {
    OrderedOperation,
    AudioReconcile,
    PublicationPayload,
    Delivery,
}

/// Owned timers remain valid across delivery awaits; no thread-local or entered-span guard.
pub(super) struct ApplicationSections {
    started: Option<Instant>,
    phases: [Duration; 4],
    retained: bool,
    path_code: u8,
}
impl ApplicationSections {
    pub(super) fn begin() -> Self {
        Self::admit(
            live_phases::full_frequency_enabled(),
            &APPLICATION_SECTION_SAMPLES,
        )
    }
    fn admit(enabled: bool, samples: &AtomicU64) -> Self {
        let admitted = enabled
            && samples
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                    if count < APPLICATION_SECTION_LIMIT {
                        Some(count + 1)
                    } else {
                        None
                    }
                })
                .is_ok();
        Self {
            started: admitted.then(Instant::now),
            phases: [Duration::ZERO; 4],
            retained: false,
            path_code: 0,
        }
    }
    pub(super) fn measure<T>(&mut self, phase: ApplicationPhase, work: impl FnOnce() -> T) -> T {
        let started = self.started.map(|_| Instant::now());
        let result = work();
        if let Some(started) = started {
            self.record(phase, started.elapsed());
        }
        result
    }
    pub(super) fn record(&mut self, phase: ApplicationPhase, elapsed: Duration) {
        if self.started.is_some() {
            self.phases[phase as usize] += elapsed;
        }
    }
    pub(super) fn record_since(&mut self, phase: ApplicationPhase, started: Instant) {
        if self.started.is_some() {
            self.record(phase, started.elapsed());
        }
    }
    pub(super) fn retain(&mut self, path_code: u8) {
        self.retained = true;
        self.path_code = path_code;
    }
    pub(super) fn finish(&self, failed: bool) {
        let Some(started) = self.started else {
            return;
        };
        tracing::info!(
            application_total_micros = duration_micros(started.elapsed()),
            ordered_operation_micros =
                duration_micros(self.phases[ApplicationPhase::OrderedOperation as usize]),
            audio_reconcile_micros =
                duration_micros(self.phases[ApplicationPhase::AudioReconcile as usize]),
            publication_payload_micros =
                duration_micros(self.phases[ApplicationPhase::PublicationPayload as usize]),
            delivery_micros = duration_micros(self.phases[ApplicationPhase::Delivery as usize]),
            retained = self.retained,
            path_code = if failed { 3u8 } else { self.path_code },
            "output tick application sections"
        );
    }
}

#[cfg(test)]
mod application_tests {
    use super::*;
    #[test]
    fn only_explicitly_enabled_sections_consume_the_hard_budget() {
        let samples = AtomicU64::new(0);
        let mut inactive = ApplicationSections::admit(false, &samples);
        assert_eq!(
            inactive.measure(ApplicationPhase::OrderedOperation, || 42),
            42
        );
        inactive.record(ApplicationPhase::Delivery, Duration::from_secs(1));
        assert!(inactive.started.is_none());
        assert_eq!(inactive.phases, [Duration::ZERO; 4]);
        assert_eq!(samples.load(Ordering::Relaxed), 0);
        for _ in 0..APPLICATION_SECTION_LIMIT {
            assert!(ApplicationSections::admit(true, &samples).started.is_some());
        }
        assert!(ApplicationSections::admit(true, &samples).started.is_none());
        assert_eq!(samples.load(Ordering::Relaxed), APPLICATION_SECTION_LIMIT);
    }
    #[test]
    fn named_sections_accumulate_independently_and_retain_paths_are_explicit() {
        let mut trace = ApplicationSections::admit(true, &AtomicU64::new(0));
        trace.record(ApplicationPhase::OrderedOperation, Duration::from_millis(8));
        trace.record(
            ApplicationPhase::PublicationPayload,
            Duration::from_millis(2),
        );
        trace.record(ApplicationPhase::Delivery, Duration::from_millis(1));
        assert_eq!(
            trace.phases,
            [
                Duration::from_millis(8),
                Duration::ZERO,
                Duration::from_millis(2),
                Duration::from_millis(1)
            ]
        );
        trace.retain(2);
        assert!(trace.retained);
        assert_eq!(trace.path_code, 2);
    }
}
