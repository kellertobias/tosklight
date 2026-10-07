//! Where a frame's time actually goes, when someone asks.
//!
//! A sampling profile of a render says it spends its time hashing, allocating, and cloning
//! strings. It does not say whether that is the build side or the read side of a structure, and
//! that distinction decides which change is worth making. These counters answer the question the
//! profile cannot.
//!
//! Off unless `LIGHT_RENDER_PHASES` is set, and the decision is read once rather than per call:
//! asking the environment on every frame costs more than the clock read it would be guarding.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var_os("LIGHT_RENDER_PHASES").is_some());

/// Inclusive phase timings; child phases are already included in their enclosing total.
/// PreparedCapture measures source/continuity acquisition separately from RenderTotal, which
/// measures evaluation and projection of an already captured frame. Capture includes Playback
/// sampling and can also measure an observer capture or a busy capture attempt without a render.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderPhase {
    ResolveTotal,
    RenderTotal,
    PlaybackResolution,
    ProgrammerContributions,
    GroupContributions,
    MoveInBlack,
    ContributionMerge,
    ResolverFinish,
    FixtureFreezes,
    ValueIndexBuild,
    FixtureProjection,
    Encoding,
    PreparedCapture,
}

impl RenderPhase {
    const ALL: [Self; 13] = [
        Self::ResolveTotal,
        Self::RenderTotal,
        Self::PlaybackResolution,
        Self::ProgrammerContributions,
        Self::GroupContributions,
        Self::MoveInBlack,
        Self::ContributionMerge,
        Self::ResolverFinish,
        Self::FixtureFreezes,
        Self::ValueIndexBuild,
        Self::FixtureProjection,
        Self::Encoding,
        Self::PreparedCapture,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::ResolveTotal => "= resolve total",
            Self::RenderTotal => "= render total",
            Self::PlaybackResolution => "playback resolution",
            Self::ProgrammerContributions => "programmer contributions",
            Self::GroupContributions => "group contributions",
            Self::MoveInBlack => "move in black",
            Self::ContributionMerge => "contribution merge",
            Self::ResolverFinish => "resolver finish",
            Self::FixtureFreezes => "fixture freezes",
            Self::ValueIndexBuild => "value index build",
            Self::FixtureProjection => "fixture projection",
            Self::Encoding => "encoding",
            Self::PreparedCapture => "= prepared capture",
        }
    }
}

static NANOS: [AtomicU64; 13] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];

// A per-test-thread observer proves wrapper placement without changing process environment or
// sharing/resetting global counters with other concurrently running engine tests.
#[cfg(test)]
thread_local! {
    static TEST_PHASES: std::cell::RefCell<Option<Vec<(RenderPhase, std::time::Duration)>>> = const { std::cell::RefCell::new(None) };
}

/// Time one phase of a render. Compiles to nothing observable when the counters are off.
pub(crate) fn timed<T>(phase: RenderPhase, work: impl FnOnce() -> T) -> T {
    #[cfg(test)]
    let test_observed = TEST_PHASES.with(|phases| phases.borrow().is_some());
    #[cfg(not(test))]
    let test_observed = false;
    if !*ENABLED && !test_observed {
        return work();
    }
    let started = Instant::now();
    let result = work();
    let elapsed = started.elapsed();
    if *ENABLED {
        NANOS[phase as usize].fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);
    }
    #[cfg(test)]
    TEST_PHASES.with(|phases| {
        if let Some(phases) = phases.borrow_mut().as_mut() {
            phases.push((phase, elapsed));
        }
    });
    result
}

/// What every phase has cost since the counters were last read, in microseconds.
///
/// These accumulate across every render the process has performed, so a caller comparing
/// scenarios reads them between scenarios rather than at the end.
pub fn accumulated_microseconds() -> Vec<(&'static str, u64)> {
    RenderPhase::ALL
        .iter()
        .map(|phase| {
            (
                phase.name(),
                NANOS[*phase as usize].load(Ordering::Relaxed) / 1_000,
            )
        })
        .collect()
}

/// Start the next scenario from zero.
pub fn reset() {
    for counter in &NANOS {
        counter.store(0, Ordering::Relaxed);
    }
}

/// Whether anyone asked for these counters.
pub fn enabled() -> bool {
    *ENABLED
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PhaseObservation;
    impl PhaseObservation {
        fn start() -> Self {
            TEST_PHASES.with(|phases| *phases.borrow_mut() = Some(Vec::new()));
            Self
        }
        fn take(&self) -> Vec<(RenderPhase, std::time::Duration)> {
            TEST_PHASES.with(|phases| std::mem::take(phases.borrow_mut().as_mut().unwrap()))
        }
    }
    impl Drop for PhaseObservation {
        fn drop(&mut self) {
            TEST_PHASES.with(|phases| *phases.borrow_mut() = None);
        }
    }

    #[test]
    fn every_phase_has_a_distinct_counter_and_report_name() {
        let names: std::collections::HashSet<_> =
            RenderPhase::ALL.iter().map(|phase| phase.name()).collect();
        assert_eq!(names.len(), RenderPhase::ALL.len());
        assert_eq!(NANOS.len(), RenderPhase::ALL.len());
        for (index, phase) in RenderPhase::ALL.iter().enumerate() {
            assert_eq!(*phase as usize, index);
        }
        assert!(
            accumulated_microseconds()
                .iter()
                .any(|(name, _)| *name == "= prepared capture")
        );
    }

    #[test]
    fn preparation_and_playback_capture_are_measured_separately_from_render() {
        let engine = crate::Engine::new(Default::default());
        let observed = PhaseObservation::start();
        let frame = engine.prepare_output_frame(Default::default());
        let capture = observed.take();
        let prepare: Vec<_> = capture
            .iter()
            .filter(|(phase, _)| *phase == RenderPhase::PreparedCapture)
            .collect();
        let playback: Vec<_> = capture
            .iter()
            .filter(|(phase, _)| *phase == RenderPhase::PlaybackResolution)
            .collect();
        assert_eq!(prepare.len(), 1);
        assert_eq!(playback.len(), 1);
        assert!(prepare[0].1 >= playback[0].1);
        assert!(
            !capture
                .iter()
                .any(|(phase, _)| *phase == RenderPhase::RenderTotal)
        );

        engine.render_prepared(&frame, &[]).unwrap();
        let render = observed.take();
        assert_eq!(
            render
                .iter()
                .filter(|(phase, _)| *phase == RenderPhase::RenderTotal)
                .count(),
            1
        );
        assert!(!render.iter().any(|(phase, _)| matches!(
            phase,
            RenderPhase::PreparedCapture | RenderPhase::PlaybackResolution
        )));
    }
}
