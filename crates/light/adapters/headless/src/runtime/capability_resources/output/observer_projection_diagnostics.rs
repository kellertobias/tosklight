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
const SUBSTAGES: usize = 7;
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

/// Only the synchronous observational projection installs this thread-local scope.
/// Outer and inner timings overlap; neither is process CPU or a worker CPU total.
#[derive(Clone, Copy)]
pub(in crate::runtime) enum Substage {
    InputCapture,
    BeforeSnapshot,
    Reconcile,
    SampleCallback,
    AfterSnapshot,
    TransitionEvents,
    CandidateProjection,
}
#[derive(Clone)]
struct InnerObservation {
    calls: [u64; SUBSTAGES],
    finished: [bool; SUBSTAGES],
    samples: [CpuObservation; SUBSTAGES],
    // 2 means the call never reached its idle/active decision.
    idle: u8,
}
impl Default for InnerObservation {
    fn default() -> Self {
        Self {
            calls: [0; SUBSTAGES],
            finished: [false; SUBSTAGES],
            samples: [CpuObservation::default(); SUBSTAGES],
            idle: 2,
        }
    }
}
impl InnerObservation {
    fn complete(&self) -> bool {
        self.idle < 2
            && (0..SUBSTAGES).all(|i| {
                let expected = u64::from(self.idle == 0 || matches!(i, 0 | 1 | 3));
                self.calls[i] == expected && self.finished[i] == (expected == 1)
            })
    }
}
thread_local! {
    static INNER: std::cell::RefCell<Option<InnerObservation>> = const { std::cell::RefCell::new(None) };
}
struct InnerScope {
    previous: Option<InnerObservation>,
    active: bool,
}
impl InnerScope {
    fn begin() -> Self {
        Self {
            previous: INNER.with(|slot| slot.replace(Some(InnerObservation::default()))),
            active: true,
        }
    }
    fn take(&mut self) -> InnerObservation {
        self.active = false;
        INNER.with(|slot| {
            slot.replace(self.previous.take())
                .expect("active observer scope")
        })
    }
}
impl Drop for InnerScope {
    fn drop(&mut self) {
        if self.active {
            INNER.with(|slot| {
                slot.replace(self.previous.take());
            });
        }
    }
}

pub(in crate::runtime) struct InnerSpan {
    stage: Substage,
    timer: Option<CpuTimer>,
}
impl InnerSpan {
    pub(in crate::runtime) fn begin(stage: Substage) -> Self {
        let timer = INNER.with(|slot| {
            slot.borrow_mut().as_mut().and_then(|inner| {
                inner.calls[stage as usize] += 1;
                CpuTimer::start_if(true)
            })
        });
        Self { stage, timer }
    }
    pub(in crate::runtime) fn finish(mut self) {
        if let Some(timer) = self.timer.take() {
            let sample = timer.finish();
            INNER.with(|slot| {
                if let Some(inner) = slot.borrow_mut().as_mut() {
                    inner.samples[self.stage as usize] = sample;
                    inner.finished[self.stage as usize] = true;
                }
            });
        }
    }
}

/// No clock, counters, allocation or environment read outside an admitted observer scope.
pub(in crate::runtime) fn measure_inner<T>(stage: Substage, run: impl FnOnce() -> T) -> T {
    let timer = INNER.with(|slot| {
        let mut slot = slot.borrow_mut();
        slot.as_mut().and_then(|inner| {
            inner.calls[stage as usize] += 1;
            CpuTimer::start_if(true)
        })
    });
    let result = run();
    if let Some(timer) = timer {
        let sample = timer.finish();
        INNER.with(|slot| {
            if let Some(inner) = slot.borrow_mut().as_mut() {
                inner.samples[stage as usize] = sample;
                inner.finished[stage as usize] = true;
            }
        });
    }
    result
}
pub(in crate::runtime) fn note_idle(idle: bool) {
    INNER.with(|slot| {
        if let Some(inner) = slot.borrow_mut().as_mut() {
            inner.idle = u8::from(idle);
        }
    });
}

pub(super) struct ProjectionObservation {
    total: Option<CpuTimer>,
    calls: [u64; STAGES],
    finished: [bool; STAGES],
    stages: [CpuObservation; STAGES],
    inner: Option<InnerObservation>,
}
impl ProjectionObservation {
    pub(super) fn begin() -> Self {
        let (observation, capped) = Self::admit(*ENABLED, &SAMPLES);
        if capped && !CAP_REPORTED.swap(true, Ordering::Relaxed) {
            tracing::info!(
                schema_version = 2,
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
                inner: None,
            },
            enabled && !admitted,
        )
    }

    pub(super) fn measure<T>(&mut self, stage: Stage, run: impl FnOnce() -> T) -> T {
        let timer = CpuTimer::start_if(self.total.is_some());
        if timer.is_some() {
            self.calls[stage as usize] = self.calls[stage as usize].saturating_add(1);
        }
        let mut scope =
            (timer.is_some() && matches!(stage, Stage::DynamicProjection)).then(InnerScope::begin);
        let value = run();
        if let Some(scope) = scope.as_mut() {
            self.inner = Some(scope.take());
        }
        drop(scope);
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
        let inner = self.inner.take().unwrap_or_default();
        let complete = self.calls.iter().all(|count| *count == 1)
            && self.finished.iter().all(|finished| *finished)
            && inner.complete();
        tracing::info!(
            schema_version = 2,
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
            idle = inner.idle,
            inner_complete = u8::from(inner.complete()),
            substage_calls = ?inner.calls,
            substage_finished = ?inner.finished.map(u8::from),
            substage_wall_nanos = ?inner.samples.map(|sample| sample.wall_nanos),
            substage_thread_cpu_nanos = ?inner.samples.map(|sample| sample.thread_cpu_nanos),
            substage_cpu_status = ?inner.samples.map(|sample| sample.cpu_status),
            substage_counter_read_nanos = ?inner.samples.map(|sample| sample.counter_read_nanos),
            "bounded observational dynamic projection sample"
        );
    }
}

#[cfg(test)]
pub(in crate::runtime) fn observe_for_test<T>(
    run: impl FnOnce() -> T,
) -> (T, [u64; SUBSTAGES], u8, bool) {
    let mut scope = InnerScope::begin();
    let result = run();
    let inner = scope.take();
    (result, inner.calls, inner.idle, inner.complete())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_observation_does_not_consume_budget_or_change_the_operation() {
        let samples = AtomicU64::new(0);
        let (mut observation, capped) = ProjectionObservation::admit(false, &samples);
        let mut calls = 0;
        let returned = observation.measure(Stage::DynamicProjection, || {
            calls += 1;
            note_idle(false);
            measure_inner(Substage::SampleCallback, || "unchanged")
        });
        assert_eq!(returned, "unchanged");
        assert_eq!(calls, 1);
        assert!(!capped);
        assert_eq!(samples.load(Ordering::Relaxed), 0);
        assert!(observation.total.is_none());
        assert_eq!(observation.calls, [0; STAGES]);
        assert!(observation.inner.is_none());
        assert!(INNER.with(|slot| slot.borrow().is_none()));
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
    fn observed_call(idle: bool) {
        for stage in [
            Substage::InputCapture,
            Substage::BeforeSnapshot,
            Substage::SampleCallback,
        ] {
            assert_eq!(measure_inner(stage, || 42), 42);
        }
        if !idle {
            for stage in [
                Substage::Reconcile,
                Substage::AfterSnapshot,
                Substage::TransitionEvents,
                Substage::CandidateProjection,
            ] {
                measure_inner(stage, || ());
            }
        }
        note_idle(idle);
    }
    #[test]
    fn observer_inner_scopes_distinguish_active_idle_and_disabled() {
        for idle in [false, true] {
            let (_, calls, recorded_idle, complete) = observe_for_test(|| observed_call(idle));
            assert!(complete);
            assert_eq!(recorded_idle, u8::from(idle));
            assert_eq!(
                calls,
                if idle {
                    [1, 1, 0, 1, 0, 0, 0]
                } else {
                    [1; SUBSTAGES]
                }
            );
        }
        assert!(INNER.with(|slot| slot.borrow().is_none()));
        assert_eq!(measure_inner(Substage::SampleCallback, || 73), 73);
        note_idle(false);
        assert!(INNER.with(|slot| slot.borrow().is_none()));
    }
    #[test]
    fn failed_inner_operation_is_unfinished_and_scope_unwinds_without_leaking() {
        let (_, calls, idle, complete) = observe_for_test(|| {
            note_idle(false);
            let result = std::panic::catch_unwind(|| {
                measure_inner(Substage::SampleCallback, || panic!("sample error"))
            });
            assert!(result.is_err());
            INNER.with(|slot| {
                let slot = slot.borrow();
                let inner = slot.as_ref().unwrap();
                assert_eq!(inner.calls[3], 1);
                assert!(!inner.finished[3]);
            });
        });
        assert_eq!(calls[3], 1);
        assert_eq!(idle, 0);
        assert!(!complete);
        let result = std::panic::catch_unwind(|| observe_for_test(|| panic!("scope error")));
        assert!(result.is_err());
        assert!(INNER.with(|slot| slot.borrow().is_none()));
    }
    #[test]
    fn nested_inner_scope_restores_outer_scope_and_unfinished_span_cannot_pass() {
        let (_, calls, _, complete) = observe_for_test(|| {
            measure_inner(Substage::InputCapture, || ());
            let (_, _, _, nested_complete) = observe_for_test(|| observed_call(true));
            assert!(nested_complete);
            let _unfinished = InnerSpan::begin(Substage::BeforeSnapshot);
            note_idle(false);
        });
        assert_eq!(calls, [1, 1, 0, 0, 0, 0, 0]);
        assert!(!complete);
        assert!(INNER.with(|slot| slot.borrow().is_none()));
    }
}
