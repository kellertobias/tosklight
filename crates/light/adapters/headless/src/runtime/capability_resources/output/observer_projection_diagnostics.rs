//! Bounded, opt-in observations of fresh synchronous visualization projections.
//! Total includes its six stages. Thread CPU excludes other threads and is not process CPU;
//! counter-read duration is wall time and may include preemption. No observer result is reused.
use light_engine::parallel::diagnostics::{CpuObservation, CpuTimer};
use std::sync::{
    LazyLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

const LIMIT: u64 = 1_024;
const STAGES: usize = 6;
static ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("LIGHT_OUTPUT_PHASE_DIAGNOSTICS")
        .ok()
        .as_deref()
        == Some("all")
});
static SAMPLES: AtomicU64 = AtomicU64::new(0);
static CAP_REPORTED: AtomicBool = AtomicBool::new(false);

fn reserve(samples: &AtomicU64) -> bool {
    let mut count = samples.load(Ordering::Relaxed);
    loop {
        if count >= LIMIT {
            return false;
        }
        match samples.compare_exchange_weak(count, count + 1, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => return true,
            Err(observed) => count = observed,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Stage {
    InstallDefinitions,
    LiveCapture,
    RestoreSnapshot,
    DynamicProjection,
    ResolvedValues,
    Teardown,
}

pub(super) struct ProjectionObservation {
    total: Option<CpuTimer>,
    calls: [u64; STAGES],
    finished: [bool; STAGES],
    stages: [CpuObservation; STAGES],
}
impl ProjectionObservation {
    pub(super) fn begin() -> Self {
        let (observation, capped) = Self::admit(*ENABLED, &SAMPLES);
        if capped && !CAP_REPORTED.swap(true, Ordering::Relaxed) {
            tracing::info!(
                schema_version = 1,
                sample_limit = LIMIT,
                "observational dynamic projection sample cap reached"
            );
        }
        observation
    }

    fn admit(enabled: bool, samples: &AtomicU64) -> (Self, bool) {
        let admitted = enabled && reserve(samples);
        (
            Self {
                total: CpuTimer::start_if(admitted),
                calls: [0; STAGES],
                finished: [false; STAGES],
                stages: [CpuObservation::default(); STAGES],
            },
            enabled && !admitted,
        )
    }

    pub(super) fn measure<T>(&mut self, stage: Stage, run: impl FnOnce() -> T) -> T {
        let timer = CpuTimer::start_if(self.total.is_some());
        if timer.is_some() {
            self.calls[stage as usize] = self.calls[stage as usize].saturating_add(1);
        }
        let value = run();
        if let Some(timer) = timer {
            self.stages[stage as usize] = timer.finish();
            self.finished[stage as usize] = true;
        }
        value
    }
}
impl Drop for ProjectionObservation {
    fn drop(&mut self) {
        let Some(timer) = self.total.take() else {
            return;
        };
        let total = timer.finish();
        let complete = self.calls.iter().all(|count| *count == 1)
            && self.finished.iter().all(|finished| *finished);
        tracing::info!(
            schema_version = 1,
            complete = u8::from(complete),
            total_wall_nanos = total.wall_nanos,
            total_thread_cpu_nanos = total.thread_cpu_nanos,
            total_cpu_status = total.cpu_status,
            total_counter_read_nanos = total.counter_read_nanos,
            stage_calls = ?self.calls,
            stage_wall_nanos = ?self.stages.map(|sample| sample.wall_nanos),
            stage_thread_cpu_nanos = ?self.stages.map(|sample| sample.thread_cpu_nanos),
            stage_cpu_status = ?self.stages.map(|sample| sample.cpu_status),
            stage_counter_read_nanos = ?self.stages.map(|sample| sample.counter_read_nanos),
            "bounded observational dynamic projection sample"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_observation_does_not_consume_budget_or_change_the_operation() {
        let samples = AtomicU64::new(0);
        let (mut observation, capped) = ProjectionObservation::admit(false, &samples);
        let mut calls = 0;
        let returned = observation.measure(Stage::LiveCapture, || {
            calls += 1;
            "unchanged"
        });
        assert_eq!(returned, "unchanged");
        assert_eq!(calls, 1);
        assert!(!capped);
        assert_eq!(samples.load(Ordering::Relaxed), 0);
        assert!(observation.total.is_none());
        assert_eq!(observation.calls, [0; STAGES]);
    }

    #[test]
    fn admission_stops_without_overflow_at_the_fixed_budget() {
        let samples = AtomicU64::new(LIMIT - 1);
        let (last, capped) = ProjectionObservation::admit(true, &samples);
        assert!(last.total.is_some());
        assert!(!capped);
        let (extra, capped) = ProjectionObservation::admit(true, &samples);
        assert!(extra.total.is_none());
        assert!(capped);
        assert_eq!(samples.load(Ordering::Relaxed), LIMIT);
        let exhausted = AtomicU64::new(u64::MAX);
        let (extra, capped) = ProjectionObservation::admit(true, &exhausted);
        assert!(extra.total.is_none());
        assert!(capped);
        assert_eq!(exhausted.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn only_completed_operations_fill_stage_measurements() {
        let (mut observation, _) = ProjectionObservation::admit(true, &AtomicU64::new(0));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            observation.measure(Stage::RestoreSnapshot, || panic!("projection failure"));
        }));
        assert!(result.is_err());
        assert_eq!(observation.calls[Stage::RestoreSnapshot as usize], 1);
        assert!(!observation.finished[Stage::RestoreSnapshot as usize]);
        let returned = observation.measure(Stage::LiveCapture, || 42);
        assert_eq!(returned, 42);
        assert!(observation.finished[Stage::LiveCapture as usize]);
    }
}
