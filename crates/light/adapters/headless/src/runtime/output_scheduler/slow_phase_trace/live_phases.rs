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

const PERIOD: Duration = Duration::from_secs(1);
const LIMIT: u16 = 1024;
static ENABLED: LazyLock<bool> = LazyLock::new(|| {
    enabled(
        std::env::var("LIGHT_OUTPUT_PHASE_DIAGNOSTICS")
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
struct Frame {
    family: family_preparation::Summary,
    phases: [Duration; COUNT],
    generation: Option<u64>,
    show_revision: Option<u64>,
    sampled_at_millis: Option<i64>,
    accepted: bool,
}
thread_local! { static ACTIVE: RefCell<Option<Frame>> = const { RefCell::new(None) }; }

pub(in crate::runtime::output_scheduler) struct Scope {
    started: Instant,
}
impl Scope {
    pub(in crate::runtime::output_scheduler) fn begin() -> Option<Self> {
        if !*ENABLED || ACTIVE.with(|active| active.borrow().is_some()) {
            return None;
        }
        let now = Instant::now();
        if !SAMPLER
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .admit(now.saturating_duration_since(*ORIGIN))
        {
            return None;
        }
        ACTIVE.with(|active| {
            *active.borrow_mut() = Some(Frame {
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
            family_static_discovery_micros = frame.family.micros(family_preparation::Stage::StaticDiscovery),
            family_input_assembly_micros = frame.family.micros(family_preparation::Stage::InputAssembly),
            family_prepare_current_micros = frame.family.micros(family_preparation::Stage::PrepareCurrent),
            family_prepare_programs_micros = frame.family.micros(family_preparation::Stage::PreparePrograms),
            // Schema 2 appends four child calls after the original 14 stage indices.
            // Counter indices and the sampler's opt-in, interval and limit are unchanged.
            family_diagnostics_schema = 2u8,
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
}
impl Span {
    pub(in crate::runtime::output_scheduler) fn begin(phase: Phase) -> Self {
        Self {
            phase,
            started: ACTIVE.with(|active| active.borrow().as_ref().map(|_| Instant::now())),
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            add(self.phase, started.elapsed());
        }
    }
}
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
