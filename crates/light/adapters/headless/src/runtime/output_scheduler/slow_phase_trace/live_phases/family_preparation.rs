//! Fixed-size, initiating-thread observations for already-admitted output frames.
//! Timers are inclusive wall time, not CPU or worker wait attribution. Counts use actual
//! selected branches; zero phase calls distinguish not-run from a zero-duration observation.
use super::{ACTIVE, Duration, Instant};
mod cpu;
pub(in crate::runtime::output_scheduler) use cpu::{Subspan, Substage, WorkerStage, covered};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy)]
pub(in crate::runtime::output_scheduler) enum Stage {
    InitialStatic,
    MissingFixedBases,
    RepeatStatic,
    BindAndControl,
    SamplingBeforeCallback,
    ScalarStatic,
    FinalGeometry,
    CompleteDeferred,
    PrepareFamilies,
    CohortSetup,
    PositionCompose,
    OwnerCompose,
    OrderedFinish,
    NativeProjection,
}
const STAGES: usize = 14;

#[derive(Clone, Copy)]
pub(in crate::runtime::output_scheduler) enum Count {
    StaticCalls,
    MissingBaseBatches,
    RuntimeInstances,
    ScalarSamples,
    TypedLanes,
    FamilySamples,
    Groups,
    StaticTargets,
    PositionGroups,
    PositionHandled,
    DeferredSerial,
    DeferredParallel,
    PreparationSerial,
    PreparationParallel,
    OwnerSerial,
    OwnerParallel,
    OwnerFallback,
    OwnerWorker,
    ForkInvocations,
    ForkChunks,
    ScalarReuseStatic,
    ScalarSeparateStatic,
}
const COUNTS: usize = 22;
static NEXT: AtomicU64 = AtomicU64::new(1);

pub(super) struct Summary {
    epoch: u64,
    pub(super) cpu: Option<cpu::Summary>,
    durations: [Duration; STAGES],
    pub(super) calls: [u64; STAGES],
    pub(super) counts: [u64; COUNTS],
}
impl Default for Summary {
    fn default() -> Self {
        Self {
            epoch: NEXT.fetch_add(1, Ordering::Relaxed),
            cpu: super::full_frequency_enabled().then(cpu::Summary::default),
            durations: [Duration::ZERO; STAGES],
            calls: [0; STAGES],
            counts: [0; COUNTS],
        }
    }
}
impl Summary {
    pub(super) fn emit_cpu(&self) {
        if let Some(cpu) = &self.cpu {
            cpu.emit();
        }
    }
    pub(super) fn micros(&self, stage: Stage) -> u64 {
        super::super::duration_micros(self.durations[stage as usize])
    }
}

/// No clock, allocation or counter creation unless the existing sampler admitted this frame.
pub(in crate::runtime::output_scheduler) struct Span {
    stage: Stage,
    started: Option<(Instant, u64)>,
    cpu_timer: Option<light_engine::parallel::diagnostics::CpuTimer>,
}
impl Span {
    pub(in crate::runtime::output_scheduler) fn begin(stage: Stage) -> Self {
        let (started, cpu_timer) = ACTIVE.with(|active| {
            active.borrow_mut().as_mut().map_or((None, None), |frame| {
                let summary = &mut frame.family;
                summary.calls[stage as usize] = summary.calls[stage as usize].saturating_add(1);
                let started = Some((Instant::now(), summary.epoch));
                let timer =
                    light_engine::parallel::diagnostics::CpuTimer::start_if(summary.cpu.is_some());
                (started, timer)
            })
        });
        Self {
            stage,
            started,
            cpu_timer,
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        let cpu_sample = self.cpu_timer.take().map(|timer| timer.finish());
        if let Some((started, epoch)) = self.started {
            ACTIVE.with(|active| {
                if let Some(frame) = active.borrow_mut().as_mut()
                    && frame.family.epoch == epoch
                {
                    if let (Some(cpu), Some(sample)) = (frame.family.cpu.as_mut(), cpu_sample) {
                        cpu.stage(self.stage as usize, sample);
                    }
                    let duration = &mut frame.family.durations[self.stage as usize];
                    *duration = duration.saturating_add(started.elapsed());
                }
            });
        }
    }
}

pub(in crate::runtime::output_scheduler) fn count(kind: Count, value: usize) {
    ACTIVE.with(|active| {
        if let Some(frame) = active.borrow_mut().as_mut() {
            let counter = &mut frame.family.counts[kind as usize];
            *counter = counter.saturating_add(u64::try_from(value).unwrap_or(u64::MAX));
        }
    });
}

/// Expensive aggregate counts are evaluated only inside an admitted frame.
pub(in crate::runtime::output_scheduler) fn count_with(kind: Count, value: impl FnOnce() -> usize) {
    if ACTIVE.with(|active| active.borrow().is_some()) {
        count(kind, value());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn install() {
        ACTIVE.with(|active| {
            *active.borrow_mut() = Some(super::super::Frame {
                phases: [Duration::ZERO; super::super::COUNT],
                family: Summary::default(),
                generation: None,
                show_revision: None,
                sampled_at_millis: None,
                accepted: false,
            });
        });
    }
    #[test]
    fn inactive_observations_create_no_clock_or_summary() {
        assert!(ACTIVE.with(|active| active.borrow().is_none()));
        let span = Span::begin(Stage::InitialStatic);
        assert!(span.started.is_none());
        count(Count::Groups, usize::MAX);
        count_with(Count::Groups, || panic!("inactive count evaluated"));
        drop(span);
        assert!(ACTIVE.with(|active| active.borrow().is_none()));
    }
    #[test]
    fn nested_wall_spans_do_not_add_children_to_outer_phase() {
        install();
        let parent = super::super::Span::begin(super::super::Phase::FamilyPreparationInclusive);
        let first = Span::begin(Stage::InitialStatic);
        assert!(first.started.is_some());
        drop(first);
        let before = ACTIVE.with(|a| a.borrow().as_ref().unwrap().phases);
        assert_eq!(
            before[super::super::Phase::FamilyPreparationInclusive as usize],
            Duration::ZERO
        );
        drop(parent);
        let frame = ACTIVE.with(|a| a.borrow_mut().take()).unwrap();
        assert_eq!(frame.family.calls[Stage::InitialStatic as usize], 1);
        assert_eq!(frame.family.calls[Stage::RepeatStatic as usize], 0);
        assert!(
            frame.phases[super::super::Phase::FamilyPreparationInclusive as usize]
                >= frame.family.durations[Stage::InitialStatic as usize]
        );
    }
    #[test]
    fn counters_are_fixed_size_and_saturating() {
        install();
        ACTIVE.with(|active| {
            active.borrow_mut().as_mut().unwrap().family.counts[Count::OwnerFallback as usize] =
                u64::MAX - 1
        });
        count(Count::OwnerFallback, 1);
        count(Count::OwnerFallback, 1);
        let frame = ACTIVE.with(|a| a.borrow_mut().take()).unwrap();
        assert_eq!(frame.family.counts.len(), COUNTS);
        assert_eq!(frame.family.calls.len(), STAGES);
        assert_eq!(frame.family.counts[Count::OwnerFallback as usize], u64::MAX);
        assert_eq!(frame.family.counts[Count::OwnerWorker as usize], 0);
    }
    #[test]
    fn late_span_cannot_contaminate_replacement_frame() {
        install();
        let old = Span::begin(Stage::InitialStatic);
        install();
        drop(old);
        let frame = ACTIVE.with(|a| a.borrow_mut().take()).unwrap();
        assert_eq!(frame.family.calls, [0; STAGES]);
        assert_eq!(frame.family.durations, [Duration::ZERO; STAGES]);
    }
    #[test]
    fn panic_drop_closes_only_observed_stage() {
        install();
        let _ = std::panic::catch_unwind(|| {
            let _span = Span::begin(Stage::OrderedFinish);
            panic!("diagnostic unwind");
        });
        let frame = ACTIVE.with(|a| a.borrow_mut().take()).unwrap();
        assert_eq!(frame.family.calls[Stage::OrderedFinish as usize], 1);
        assert_eq!(frame.family.calls[Stage::NativeProjection as usize], 0);
    }
}

/// Tests admit one frame without mutating the environment or process-global sampler.
#[cfg(test)]
pub(in crate::runtime::output_scheduler) fn observe_for_test<T>(
    work: impl FnOnce() -> T,
) -> (T, [u64; STAGES], [u64; COUNTS]) {
    assert!(ACTIVE.with(|a| a.borrow().is_none()));
    ACTIVE.with(|a| {
        *a.borrow_mut() = Some(super::Frame {
            phases: [Duration::ZERO; super::COUNT],
            family: Summary::default(),
            generation: None,
            show_revision: None,
            sampled_at_millis: None,
            accepted: false,
        });
    });
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ACTIVE.with(|a| {
                a.borrow_mut().take();
            });
        }
    }
    let _reset = Reset;
    let result = work();
    let frame = ACTIVE.with(|a| a.borrow_mut().take()).unwrap();
    (result, frame.family.calls, frame.family.counts)
}
