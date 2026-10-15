//! Fixed-size, initiating-thread observations for already-admitted output frames.
//! Timers are inclusive wall time, not CPU or worker wait attribution. Counts use actual
//! selected branches; zero phase calls distinguish not-run from a zero-duration observation.
use super::{ACTIVE, Duration, Instant};
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
    // Append so schema 1's original stage indices keep their meaning in schema 2.
    StaticDiscovery,
    InputAssembly,
    PrepareCurrent,
    PreparePrograms,
    // Schema 3 appends disjoint children inside the unchanged PreparePrograms parent.
    ProgramInput,
    PositionCapture,
    PositionTracking,
}
const STAGES: usize = 21;

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
    // Submitted Position rows; empty includes requirements-only, not a partition.
    PositionPrograms,
    PositionSamples,
    PositionEmptyPrograms,
    PositionRequirementsOnly,
}
const COUNTS: usize = 26;
static NEXT: AtomicU64 = AtomicU64::new(1);

pub(super) struct Summary {
    epoch: u64,
    durations: [Duration; STAGES],
    pub(super) calls: [u64; STAGES],
    pub(super) counts: [u64; COUNTS],
}
impl Default for Summary {
    fn default() -> Self {
        Self {
            epoch: NEXT.fetch_add(1, Ordering::Relaxed),
            durations: [Duration::ZERO; STAGES],
            calls: [0; STAGES],
            counts: [0; COUNTS],
        }
    }
}
impl Summary {
    pub(super) fn micros(&self, stage: Stage) -> u64 {
        super::super::duration_micros(self.durations[stage as usize])
    }
}

/// No clock, allocation or counter creation unless the existing sampler admitted this frame.
pub(in crate::runtime::output_scheduler) struct Span {
    stage: Stage,
    started: Option<(Instant, u64)>,
}
impl Span {
    pub(in crate::runtime::output_scheduler) fn begin(stage: Stage) -> Self {
        let started = ACTIVE.with(|active| {
            active.borrow_mut().as_mut().map(|frame| {
                let summary = &mut frame.family;
                summary.calls[stage as usize] = summary.calls[stage as usize].saturating_add(1);
                (Instant::now(), summary.epoch)
            })
        });
        Self { stage, started }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some((started, epoch)) = self.started {
            ACTIVE.with(|active| {
                if let Some(frame) = active.borrow_mut().as_mut()
                    && frame.family.epoch == epoch
                {
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
        for stage in [
            Stage::InitialStatic,
            Stage::StaticDiscovery,
            Stage::InputAssembly,
            Stage::PrepareCurrent,
            Stage::PreparePrograms,
            Stage::ProgramInput,
            Stage::PositionCapture,
            Stage::PositionTracking,
        ] {
            let span = Span::begin(stage);
            assert!(span.started.is_none());
            drop(span);
        }
        count(Count::Groups, usize::MAX);
        for kind in [
            Count::Groups,
            Count::PositionPrograms,
            Count::PositionSamples,
            Count::PositionEmptyPrograms,
            Count::PositionRequirementsOnly,
        ] {
            count_with(kind, || panic!("inactive count evaluated"));
        }
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
    fn failed_cohort_retains_only_entered_children_in_its_admitted_frame() {
        fn failing_cohort(fail_current: bool) -> Result<(), &'static str> {
            let _parent = Span::begin(Stage::CohortSetup);
            {
                let _discovery = Span::begin(Stage::StaticDiscovery);
                count(Count::StaticTargets, 3);
            }
            {
                let _assembly = Span::begin(Stage::InputAssembly);
                count(Count::Groups, 2);
            }
            let _current = Span::begin(Stage::PrepareCurrent);
            if fail_current {
                return Err("Current preparation rejected the cohort");
            }
            drop(_current);
            let _programs = Span::begin(Stage::PreparePrograms);
            Ok(())
        }
        install();
        assert!(failing_cohort(true).is_err());
        let frame = ACTIVE.with(|a| a.borrow_mut().take()).unwrap();
        for stage in [
            Stage::CohortSetup,
            Stage::StaticDiscovery,
            Stage::InputAssembly,
            Stage::PrepareCurrent,
        ] {
            assert_eq!(frame.family.calls[stage as usize], 1);
        }
        assert_eq!(frame.family.calls[Stage::PreparePrograms as usize], 0);
        assert_eq!(frame.family.counts[Count::StaticTargets as usize], 3);
        assert_eq!(frame.family.counts[Count::Groups as usize], 2);
        let children = [
            Stage::StaticDiscovery,
            Stage::InputAssembly,
            Stage::PrepareCurrent,
        ]
        .into_iter()
        .map(|stage| frame.family.durations[stage as usize])
        .sum::<Duration>();
        assert!(frame.family.durations[Stage::CohortSetup as usize] >= children);
        assert_eq!(frame.phases, [Duration::ZERO; super::super::COUNT]);
        // The next admitted frame owns independent rows, even after an early return.
        install();
        let next = ACTIVE.with(|a| a.borrow_mut().take()).unwrap();
        assert_eq!(next.family.calls, [0; STAGES]);
        assert_eq!(next.family.durations, [Duration::ZERO; STAGES]);
        assert_eq!(next.family.counts, [0; COUNTS]);
    }

    #[test]
    fn program_children_remain_disjoint_on_capture_and_tracking_errors() {
        fn prepare(fail: Option<Stage>) -> Result<(), &'static str> {
            let _parent = Span::begin(Stage::PreparePrograms);
            {
                let _input = Span::begin(Stage::ProgramInput);
            }
            {
                let _capture = Span::begin(Stage::PositionCapture);
                count(Count::PositionPrograms, 1);
                count_with(Count::PositionEmptyPrograms, || 1);
                if matches!(fail, Some(Stage::PositionCapture)) {
                    return Err("capture rejected");
                }
            }
            let _tracking = Span::begin(Stage::PositionTracking);
            if matches!(fail, Some(Stage::PositionTracking)) {
                return Err("tracking rejected");
            }
            Ok(())
        }
        for fail in [
            None,
            Some(Stage::PositionCapture),
            Some(Stage::PositionTracking),
        ] {
            install();
            let result = prepare(fail);
            assert_eq!(result.is_err(), fail.is_some());
            let frame = ACTIVE.with(|a| a.borrow_mut().take()).unwrap();
            for stage in [
                Stage::PreparePrograms,
                Stage::ProgramInput,
                Stage::PositionCapture,
            ] {
                assert_eq!(frame.family.calls[stage as usize], 1);
            }
            assert_eq!(
                frame.family.calls[Stage::PositionTracking as usize],
                u64::from(!matches!(fail, Some(Stage::PositionCapture)))
            );
            let children = [
                Stage::ProgramInput,
                Stage::PositionCapture,
                Stage::PositionTracking,
            ]
            .into_iter()
            .map(|stage| frame.family.durations[stage as usize])
            .sum::<Duration>();
            assert!(frame.family.durations[Stage::PreparePrograms as usize] >= children);
            assert_eq!(frame.family.counts[Count::PositionPrograms as usize], 1);
            assert_eq!(
                frame.family.counts[Count::PositionEmptyPrograms as usize],
                1
            );
            assert_eq!(frame.family.counts[Count::PositionSamples as usize], 0);
            assert_eq!(
                frame.family.counts[Count::PositionRequirementsOnly as usize],
                0
            );
        }
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
        let old = [
            Span::begin(Stage::StaticDiscovery),
            Span::begin(Stage::ProgramInput),
            Span::begin(Stage::PositionCapture),
            Span::begin(Stage::PositionTracking),
        ];
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
