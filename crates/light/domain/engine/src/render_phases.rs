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

/// The phases a render is divided into for measurement.
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
}

impl RenderPhase {
    const ALL: [Self; 12] = [
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
        }
    }
}

static NANOS: [AtomicU64; 12] = [
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

/// Time one phase of a render. Compiles to nothing observable when the counters are off.
pub(crate) fn timed<T>(phase: RenderPhase, work: impl FnOnce() -> T) -> T {
    if !*ENABLED {
        return work();
    }
    let started = Instant::now();
    let result = work();
    NANOS[phase as usize].fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
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
