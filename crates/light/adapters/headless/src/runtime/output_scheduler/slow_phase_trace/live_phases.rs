//! Opt-in scheduler-only inclusive phase diagnostics, bounded independently of startup logs.
//! `LIGHT_OUTPUT_PHASE_DIAGNOSTICS=1` (or `true`) samples at most 1024 frames, at least 1s apart.
//! Wall durations include preemption. Acquisition fields measure the lock call on this thread,
//! including contention and scheduling; they do not isolate parked time or other worker waits.
use std::{
    cell::RefCell,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

pub(in crate::runtime::output_scheduler) mod family_preparation;

use super::super::typed_cpu;

const PERIOD: Duration = Duration::from_secs(1);
const LIMIT: u16 = 1024;
static ENABLED: LazyLock<bool> = LazyLock::new(|| {
    enabled(
        std::env::var("LIGHT_OUTPUT_PHASE_DIAGNOSTICS")
            .ok()
            .as_deref(),
    )
});
static TYPED_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    enabled(
        std::env::var("LIGHT_OUTPUT_TYPED_CPU_DIAGNOSTICS")
            .ok()
            .as_deref(),
    )
});
static SAMPLER: Mutex<Sampler> = Mutex::new(Sampler {
    emitted: 0,
    last: None,
});
static ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);

fn enabled(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true"))
}

struct Sampler {
    emitted: u16,
    last: Option<Duration>,
}
impl Sampler {
    fn admit(&mut self, elapsed: Duration) -> bool {
        if self.emitted == LIMIT
            || self
                .last
                .is_some_and(|last| elapsed.saturating_sub(last) < PERIOD)
        {
            return false;
        }
        self.emitted += 1;
        self.last = Some(elapsed);
        true
    }
}

/// The early token uses the same admission budget as ordered wall sampling.
pub(in crate::runtime::output_scheduler) fn begin_typed_tick() -> Option<typed_cpu::Tick> {
    if !*ENABLED || !*TYPED_ENABLED {
        return None;
    }
    let mut sampler = SAMPLER.lock().unwrap_or_else(|e| e.into_inner());
    sampler
        .admit(Instant::now().saturating_duration_since(*ORIGIN))
        .then(|| typed_cpu::Tick::new(sampler.emitted))
}

#[derive(Clone, Copy)]
pub(in crate::runtime::output_scheduler) enum Phase {
    CaptureSettle,
    ProjectionInclusive,
    SpeedLockAcquire,
    DynamicsLockAcquire,
    ScratchLockAcquire,
    FamilyLockAcquire,
    FamilyPreparationInclusive,
    FamilyRetire,
    FamilyFinalize,
    Reconciliation,
}
const COUNT: usize = 10;
const PHASE_NAMES: [&str; COUNT] = [
    "capture_settle",
    "projection_inclusive",
    "speed_lock_acquire",
    "dynamics_lock_acquire",
    "scratch_lock_acquire",
    "family_lock_acquire",
    "family_preparation_inclusive",
    "family_retire",
    "family_finalize",
    "reconciliation",
];
struct Frame {
    typed_sample: Option<u16>,
    cpu: [typed_cpu::Observation; COUNT],
    family: family_preparation::Summary,
    phases: [Duration; COUNT],
    generation: Option<u64>,
    show_revision: Option<u64>,
    sampled_at_millis: Option<i64>,
    accepted: bool,
}
impl Frame {
    fn typed_payload(&self, sample: u16) -> serde_json::Value {
        serde_json::json!({"schema":1,"role":1,"kind":1,"sample":sample,
            "generation":self.generation,"show_revision":self.show_revision,
            "sampled_at_millis":self.sampled_at_millis.and_then(|value| u64::try_from(value).ok()),
            "accepted":u8::from(self.accepted),"phases":typed_cpu::named(&PHASE_NAMES,&self.cpu),
            "family_stages":self.family.cpu_json(),"family_stage_calls":self.family.calls,
            "family_counts":self.family.counts,"worker_cpu_coverage":0,"runqueue_availability":0})
    }
}
thread_local! { static ACTIVE: RefCell<Option<Frame>> = const { RefCell::new(None) }; }

pub(in crate::runtime::output_scheduler) struct Scope {
    started: Instant,
}
impl Scope {
    pub(in crate::runtime::output_scheduler) fn begin(
        tick: Option<&typed_cpu::Tick>,
    ) -> Option<Self> {
        if !*ENABLED || ACTIVE.with(|active| active.borrow().is_some()) {
            return None;
        }
        let now = Instant::now();
        if (*TYPED_ENABLED && tick.is_none())
            || (tick.is_none()
                && !SAMPLER
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .admit(now.saturating_duration_since(*ORIGIN)))
        {
            return None;
        }
        ACTIVE.with(|active| {
            *active.borrow_mut() = Some(Frame {
                typed_sample: tick.map(typed_cpu::Tick::sample),
                cpu: [typed_cpu::Observation::default(); COUNT],
                phases: [Duration::ZERO; COUNT],
                family: family_preparation::Summary::default(),
                generation: None,
                show_revision: None,
                sampled_at_millis: None,
                accepted: false,
            })
        });
        Some(Self { started: now })
    }
    pub(in crate::runtime::output_scheduler) fn capture(
        &self,
        token: &light_engine::CapturedFrameToken,
    ) {
        ACTIVE.with(|active| {
            if let Some(frame) = active.borrow_mut().as_mut() {
                frame.generation = Some(token.generation());
                frame.show_revision = Some(token.show_revision());
                frame.sampled_at_millis = Some(token.sampled_at().timestamp_millis());
            }
        });
    }
    pub(in crate::runtime::output_scheduler) fn complete(&self, accepted: bool) {
        ACTIVE.with(|active| {
            if let Some(frame) = active.borrow_mut().as_mut() {
                frame.accepted = accepted;
            }
        });
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let Some(frame) = ACTIVE.with(|active| active.borrow_mut().take()) else {
            return;
        };
        if let Some(sample) = frame.typed_sample {
            tracing::info!(typed_cpu_payload=%frame.typed_payload(sample),
                "bounded live output typed nested CPU sample");
        }
        let micros = |phase: Phase| super::duration_micros(frame.phases[phase as usize]);
        tracing::info!(
            generation = ?frame.generation, show_revision = ?frame.show_revision,
            sampled_at_millis = ?frame.sampled_at_millis, accepted = frame.accepted,
            ordered_total_micros = super::duration_micros(self.started.elapsed()),
            capture_settle_micros = micros(Phase::CaptureSettle),
            projection_inclusive_micros = micros(Phase::ProjectionInclusive),
            speed_lock_acquire_micros = micros(Phase::SpeedLockAcquire),
            dynamics_lock_acquire_micros = micros(Phase::DynamicsLockAcquire),
            scratch_lock_acquire_micros = micros(Phase::ScratchLockAcquire),
            family_lock_acquire_micros = micros(Phase::FamilyLockAcquire),
            family_preparation_inclusive_micros = micros(Phase::FamilyPreparationInclusive),
            family_retire_micros = micros(Phase::FamilyRetire),
            family_finalize_micros = micros(Phase::FamilyFinalize),
            reconciliation_micros = micros(Phase::Reconciliation),
            family_initial_static_micros = frame.family.micros(family_preparation::Stage::InitialStatic),
            family_missing_fixed_bases_micros = frame.family.micros(family_preparation::Stage::MissingFixedBases),
            family_repeat_static_micros = frame.family.micros(family_preparation::Stage::RepeatStatic),
            family_bind_and_control_micros = frame.family.micros(family_preparation::Stage::BindAndControl),
            family_sampling_before_callback_micros = frame.family.micros(family_preparation::Stage::SamplingBeforeCallback),
            family_scalar_static_micros = frame.family.micros(family_preparation::Stage::ScalarStatic),
            family_final_geometry_micros = frame.family.micros(family_preparation::Stage::FinalGeometry),
            family_complete_deferred_micros = frame.family.micros(family_preparation::Stage::CompleteDeferred),
            family_prepare_families_micros = frame.family.micros(family_preparation::Stage::PrepareFamilies),
            family_cohort_setup_micros = frame.family.micros(family_preparation::Stage::CohortSetup),
            family_position_compose_micros = frame.family.micros(family_preparation::Stage::PositionCompose),
            family_owner_compose_micros = frame.family.micros(family_preparation::Stage::OwnerCompose),
            family_ordered_finish_micros = frame.family.micros(family_preparation::Stage::OrderedFinish),
            family_native_projection_micros = frame.family.micros(family_preparation::Stage::NativeProjection),
            family_diagnostics_schema = 1u8,
            family_stage_calls = ?frame.family.calls,
            family_counts = ?frame.family.counts,
            "bounded live output nested phase sample"
        );
    }
}

/// Dropping adds only to this named inclusive span, never to enclosing totals.
pub(in crate::runtime::output_scheduler) struct Span {
    phase: Phase,
    started: Option<Instant>,
    cpu: Option<typed_cpu::Measurement>,
    epoch: Option<u64>,
}
impl Span {
    pub(in crate::runtime::output_scheduler) fn begin(phase: Phase) -> Self {
        Self {
            phase,
            cpu: ACTIVE.with(|a| {
                a.borrow()
                    .as_ref()
                    .filter(|f| f.typed_sample.is_some())
                    .map(|_| typed_cpu::Measurement::begin())
            }),
            epoch: ACTIVE.with(|a| a.borrow().as_ref().map(|f| f.family.epoch)),
            started: ACTIVE.with(|active| active.borrow().as_ref().map(|_| Instant::now())),
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            let elapsed = started.elapsed();
            let cpu = self.cpu.take().map(typed_cpu::Measurement::finish);
            ACTIVE.with(|a| {
                if let Some(frame) = a.borrow_mut().as_mut()
                    && Some(frame.family.epoch) == self.epoch
                {
                    frame.phases[self.phase as usize] += elapsed;
                    if let Some(cpu) = cpu {
                        frame.cpu[self.phase as usize].add(cpu);
                    }
                }
            });
        }
    }
}
#[cfg(test)]
fn add(phase: Phase, elapsed: Duration) {
    ACTIVE.with(|active| {
        if let Some(frame) = active.borrow_mut().as_mut() {
            frame.phases[phase as usize] += elapsed;
        }
    });
}

pub(in crate::runtime::output_scheduler) fn measure<T>(
    phase: Phase,
    work: impl FnOnce() -> T,
) -> T {
    let _span = Span::begin(phase);
    work()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn install_typed(sample: Option<u16>) {
        ACTIVE.with(|a| {
            *a.borrow_mut() = Some(Frame {
                typed_sample: sample,
                cpu: [typed_cpu::Observation::default(); COUNT],
                phases: [Duration::ZERO; COUNT],
                family: family_preparation::Summary::default(),
                generation: None,
                show_revision: None,
                sampled_at_millis: None,
                accepted: false,
            })
        });
    }
    #[test]
    fn nested_display_payload_preserves_nullable_numeric_capture_binding() {
        install_typed(Some(7));
        let mut frame = ACTIVE.with(|active| active.borrow_mut().take()).unwrap();
        frame.generation = Some(11);
        frame.show_revision = Some(13);
        frame.sampled_at_millis = Some(1234);
        frame.accepted = true;
        let encoded = format!("{}", frame.typed_payload(7));
        let parsed: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        typed_cpu::assert_numeric_payload(&parsed);
        assert_eq!(parsed["schema"], 1);
        assert_eq!(parsed["role"], 1);
        assert_eq!(parsed["kind"], 1);
        assert_eq!(parsed["sample"], 7);
        assert_eq!(parsed["sampled_at_millis"], 1234);
        assert_eq!(parsed["accepted"], 1);
        assert_eq!(parsed["generation"], 11);
        assert_eq!(parsed["show_revision"], 13);
        assert!(parsed["phases"]["capture_settle"]["thread_cpu_ns"].is_null());
        assert_eq!(parsed.as_object().unwrap().len(), 14);
        frame.sampled_at_millis = Some(-1);
        assert!(frame.typed_payload(7)["sampled_at_millis"].is_null());
        frame.sampled_at_millis = None;
        assert!(frame.typed_payload(7)["sampled_at_millis"].is_null());
    }
    #[test]
    fn wall_only_and_inactive_spans_never_read_thread_counters() {
        let before = typed_cpu::reads_for_test();
        measure(Phase::CaptureSettle, || ());
        install_typed(None);
        measure(Phase::CaptureSettle, || {
            let _family = family_preparation::Span::begin(family_preparation::Stage::InitialStatic);
        });
        ACTIVE.with(|a| a.borrow_mut().take());
        assert_eq!(typed_cpu::reads_for_test(), before);
    }
    #[test]
    fn typed_synchronous_error_and_replacement_epoch_are_closed() {
        install_typed(Some(1));
        let result: Result<(), ()> = measure(Phase::CaptureSettle, || {
            let _family = family_preparation::Span::begin(family_preparation::Stage::InitialStatic);
            Err(())
        });
        assert!(result.is_err());
        ACTIVE.with(|a| assert_eq!(a.borrow().as_ref().unwrap().cpu[0].json()["calls"], 1));
        let late = Span::begin(Phase::Reconciliation);
        install_typed(Some(2));
        drop(late);
        let frame = ACTIVE.with(|a| a.borrow_mut().take()).unwrap();
        assert_eq!(frame.cpu[Phase::Reconciliation as usize].json()["calls"], 0);
        assert_eq!(frame.phases, [Duration::ZERO; COUNT]);
    }
    #[test]
    fn malformed_or_absent_opt_in_is_disabled() {
        for value in [
            None,
            Some(""),
            Some("false"),
            Some("yes"),
            Some("-1"),
            Some("256"),
            Some(" true "),
        ] {
            assert!(!enabled(value));
        }
        assert!(enabled(Some("1")));
        assert!(enabled(Some("true")));
    }
    #[test]
    fn startup_burst_cannot_exhaust_later_periodic_samples() {
        let mut sampler = Sampler {
            emitted: 0,
            last: None,
        };
        assert!(sampler.admit(Duration::ZERO));
        for _ in 0..10_000 {
            assert!(!sampler.admit(Duration::from_millis(1)));
        }
        assert!(sampler.admit(PERIOD));
        assert_eq!(sampler.emitted, 2);
        assert!(!sampler.admit(Duration::from_millis(999))); // non-monotonic input is conservative
    }
    #[test]
    fn preparation_and_measurement_window_remain_inside_budget() {
        let mut sampler = Sampler {
            emitted: 0,
            last: None,
        };
        // 480s preparation, 60s measurement and 120s margin, fresh process per case.
        for second in 0..=660 {
            assert!(sampler.admit(Duration::from_secs(second)));
        }
        assert_eq!(sampler.emitted, 661);
        assert!(sampler.emitted < LIMIT);
    }
    #[test]
    fn actual_nested_spans_accumulate_without_parent_double_add() {
        ACTIVE.with(|active| {
            *active.borrow_mut() = Some(Frame {
                typed_sample: None,
                cpu: [typed_cpu::Observation::default(); COUNT],
                phases: [Duration::ZERO; COUNT],
                family: family_preparation::Summary::default(),
                generation: None,
                show_revision: None,
                sampled_at_millis: None,
                accepted: false,
            })
        });
        let parent = Span::begin(Phase::ProjectionInclusive);
        let child = Span::begin(Phase::FamilyPreparationInclusive);
        assert!(parent.started.is_some());
        assert!(child.started.is_some());
        drop(child);
        let child_duration = ACTIVE.with(|active| {
            active.borrow().as_ref().unwrap().phases[Phase::FamilyPreparationInclusive as usize]
        });
        drop(parent);
        let frame = ACTIVE.with(|active| active.borrow_mut().take()).unwrap();
        assert_eq!(
            frame.phases[Phase::FamilyPreparationInclusive as usize],
            child_duration
        );
        assert!(frame.phases[Phase::ProjectionInclusive as usize] >= child_duration);
        assert_eq!(
            frame.phases[Phase::FamilyLockAcquire as usize],
            Duration::ZERO
        );
    }
    #[test]
    fn periodic_logging_has_a_hard_total_budget() {
        let mut sampler = Sampler {
            emitted: 0,
            last: None,
        };
        for index in 0..LIMIT {
            assert!(sampler.admit(PERIOD * u32::from(index)));
        }
        assert!(!sampler.admit(PERIOD * u32::from(LIMIT)));
        assert_eq!(sampler.emitted, LIMIT);
    }
    #[test]
    fn inclusive_child_is_not_added_to_parent_or_unattributed_as_wait() {
        ACTIVE.with(|active| {
            *active.borrow_mut() = Some(Frame {
                typed_sample: None,
                cpu: [typed_cpu::Observation::default(); COUNT],
                phases: [Duration::ZERO; COUNT],
                family: family_preparation::Summary::default(),
                generation: None,
                show_revision: None,
                sampled_at_millis: None,
                accepted: false,
            })
        });
        add(Phase::ProjectionInclusive, Duration::from_millis(30));
        add(Phase::FamilyPreparationInclusive, Duration::from_millis(20));
        add(Phase::Reconciliation, Duration::from_millis(2));
        let frame = ACTIVE.with(|active| active.borrow_mut().take()).unwrap();
        assert_eq!(
            frame.phases[Phase::ProjectionInclusive as usize],
            Duration::from_millis(30)
        );
        assert_eq!(
            frame.phases[Phase::FamilyPreparationInclusive as usize],
            Duration::from_millis(20)
        );
        assert_eq!(
            frame.phases[Phase::DynamicsLockAcquire as usize],
            Duration::ZERO
        );
    }
    #[test]
    fn inactive_spans_do_not_create_an_observation() {
        assert!(ACTIVE.with(|active| active.borrow().is_none()));
        measure(Phase::FamilyFinalize, || 42);
        assert!(ACTIVE.with(|active| active.borrow().is_none()));
    }
}
