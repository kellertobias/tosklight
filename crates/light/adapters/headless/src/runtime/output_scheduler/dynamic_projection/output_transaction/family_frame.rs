//! TL-548 C3: gated Live injection of the all-family physical adapters.
//!
//! Both Live paths (the scheduler's `render_tick` and `OutputResource`'s
//! `render_with_playback_events_timed_with_capture`) enter `dynamic_output_frame`, which hands
//! `finish` an [`OutputRenderSource`]. The legacy source is the scalar batch list the caller
//! renders; the hybrid source is a frame already rendered by `finalize_live_family_frame`.
//!
//! **Gate.** The hybrid path runs only when [`LiveFamilyAdapters::engaged`]: the shared resource
//! was built opted in AND the Engine supports `PROGRAMMING_CONTRACT_VERSION`. Production builds
//! the resource with `opted_in = false` (and keeps `SUPPORTED_PROGRAMMING_CONTRACT = 0`), and the
//! default headless test state (contract 1) is not opted in either, so neither moves implicitly.
//!
//! **Ownership.** One `LiveFamilyAdapters` is created with the scheduler's `SharedResources` and
//! the same `Arc` is installed on `OutputResource`, so both Live paths drive one `FamilyLanes`
//! set and one `HybridFrameScratch`. Show activation replaces both ([`LiveFamilyAdapters::reset`]).
//!
//! **Lock order.** activation → Playback operation → `dynamics` → `cache.transaction` →
//! `family_lanes`. The lanes lock is taken only inside the Dynamics transaction of
//! `dynamic_output_frame`, or by `reset` after the activation install released `dynamics`.
//!
//! **Presets.** `presets` is `None`, which is parity with the legacy sampler: its only resolver,
//! `AuthoredDynamicSources::preset`, also returns `None`. Semantic Preset values come from each
//! instance's cold-compiled table (`cold_preset_materialization`, run by the hybrid preparation
//! through `prepare_captured_preset_dependencies`), which the staged sampler consults through
//! `RetainedPresetSources` whenever the frame-local override returns nothing.
use super::super::physical_adapter::family_lanes::{
    FamilyFrameObserver, FamilyLanes, finalize_live_family_frame,
};
use super::super::programming_projection::hybrid::{
    HybridFrameScratch, prepare_captured_hybrid_frame_with_observer,
};
use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use light_core::programming::PROGRAMMING_CONTRACT_VERSION;

mod accepted_color;
#[cfg(test)]
pub(in crate::runtime) use accepted_color::lamp_quality;
pub(in crate::runtime) use accepted_color::{AcceptedColorFrame, AcceptedColorFrames};

/// What `dynamic_output_frame` hands its `finish` step.
#[allow(clippy::large_enum_variant)] // Built and consumed once per frame; boxing adds an allocation.
pub(in crate::runtime) enum OutputRenderSource<'a> {
    /// Legacy scalar Dynamic batches plus the baseline; `finish` renders them.
    Legacy(&'a [ContributionBatch]),
    /// A finalized all-family frame: rendered, and its lanes already accepted the token.
    Hybrid(RenderResult),
}

impl OutputRenderSource<'_> {
    /// The ordered Playback operation of either source. Legacy renders through
    /// `prepared_playback_operation`; a hybrid frame passes its finished render through
    /// `completed_prepared_playback_operation`, so captured events are claimed identically.
    pub(in crate::runtime) fn playback_operation(
        self,
        engine: &Engine,
        active_show: &ActiveShowProjection,
        playback: &PlaybackRenderCapability,
        frame: &light_engine::PreparedOutputFrame,
        persistence: Option<&OutputPersistenceResource>,
    ) -> PlaybackOperation<Result<RenderResult, EngineError>> {
        match self {
            Self::Legacy(sampled) => prepared_playback_operation(
                engine,
                active_show,
                playback,
                frame,
                sampled,
                persistence,
            ),
            Self::Hybrid(rendered) => completed_prepared_playback_operation(
                engine,
                active_show,
                playback,
                frame,
                Ok(rendered),
                persistence,
            ),
        }
    }
}

struct LiveFamilyState {
    lanes: FamilyLanes,
    scratch: HybridFrameScratch,
}

impl LiveFamilyState {
    fn new() -> Self {
        Self {
            lanes: FamilyLanes::live(),
            scratch: HybridFrameScratch::default(),
        }
    }
}

/// The Live family lanes shared by both Live output paths, plus the explicit opt-in.
pub(in crate::runtime) struct LiveFamilyAdapters {
    opted_in: bool,
    state: Mutex<LiveFamilyState>,
    /// TL-594: Color results of the last accepted frames, for the accepted-frame colour report.
    accepted_color: AcceptedColorFrames,
    /// Test observation of the last published frame: its token and every native write.
    #[cfg(test)]
    published: Mutex<Option<PublishedFamilyWrites>>,
}

/// `(owner, target, write)` of every sidecar of one published hybrid frame.
#[cfg(test)]
pub(in crate::runtime) struct PublishedFamilyWrites {
    pub token: light_engine::CapturedFrameToken,
    pub writes: Vec<(
        light_core::programming::ProgrammingOwner,
        light_core::FixtureId,
        super::super::physical_adapter::NativeControlWrite,
    )>,
}

impl Default for LiveFamilyAdapters {
    /// Not opted in: the legacy path, whatever the Engine's contract.
    fn default() -> Self {
        Self::new(false)
    }
}

impl LiveFamilyAdapters {
    pub(in crate::runtime) fn new(opted_in: bool) -> Self {
        Self {
            opted_in,
            state: Mutex::new(LiveFamilyState::new()),
            accepted_color: AcceptedColorFrames::default(),
            #[cfg(test)]
            published: Mutex::new(None),
        }
    }

    /// Both conditions are required; contract support alone never selects the hybrid path.
    pub(in crate::runtime) fn engaged(&self, engine: &Engine) -> bool {
        self.opted_in && engine.supported_programming_contract() >= PROGRAMMING_CONTRACT_VERSION
    }

    /// Show activation: new lanes and scratch, so no continuity crosses into another show.
    /// Callers must not hold `dynamics` (see the module lock order).
    pub(in crate::runtime) fn reset(&self) {
        *self.state.lock() = LiveFamilyState::new();
        self.accepted_color.clear();
    }

    /// The Color results of the accepted frame with this identity, while it is retained.
    pub(in crate::runtime) fn accepted_color(
        &self,
        generation: u64,
        sampled_at: chrono::DateTime<chrono::Utc>,
    ) -> Option<Arc<AcceptedColorFrame>> {
        self.accepted_color.find(generation, sampled_at)
    }

    /// TL-554: the newest retained accepted frame's Color results.
    pub(in crate::runtime) fn latest_accepted_color(&self) -> Option<Arc<AcceptedColorFrame>> {
        self.accepted_color.latest()
    }

    /// Read the Live lanes (tests, diagnostics and the TL-596 output benchmark seam). Never call
    /// while holding `dynamics`.
    pub(in crate::runtime) fn with_lanes<R>(&self, read: impl FnOnce(&FamilyLanes) -> R) -> R {
        read(&self.state.lock().lanes)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn take_published(&self) -> Option<PublishedFamilyWrites> {
        self.published.lock().take()
    }
}

/// The hybrid body of the Dynamics transaction. The caller holds `dynamics` and
/// `cache.transaction`; this takes the lanes last. Any failure abandons every lane and returns
/// Err, so the enclosing transaction rolls sampling and the source catalogue back and no lane
/// advances. `finish` runs only after the lanes accepted, and must not fail on a rendered frame.
#[allow(clippy::too_many_arguments)]
pub(super) fn family_output_frame<T>(
    engine: &Engine,
    frame: &light_engine::PreparedOutputFrame,
    baseline_samples: &[ContributionBatch],
    runtime: &mut light_dynamics::DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    inputs: &CapturedDynamicInputs<'_>,
    family: &LiveFamilyAdapters,
    finish: impl FnOnce(OutputRenderSource<'_>) -> Result<T, EngineError>,
) -> Result<(T, CapturedDynamicSample), EngineError> {
    let mut state = family.state.lock();
    let LiveFamilyState { lanes, scratch } = &mut *state;
    let lanes = &*lanes;
    let mut observer = FamilyFrameObserver::new(lanes).with_pool(engine.output_pool());
    let published = prepare_captured_hybrid_frame_with_observer(
        engine,
        frame,
        baseline_samples,
        runtime,
        origins,
        inputs,
        scratch,
        lanes,
        None,
        &mut observer,
    );
    observer.retire();
    let published =
        published.and_then(|prepared| finalize_live_family_frame(engine, frame, lanes, prepared));
    let published = match published {
        Ok(published) => published,
        Err(error) => {
            // A preparation error can leave a staged attempt; the finalizer already abandoned.
            lanes.abandon();
            return Err(engine_error(error));
        }
    };
    drop(state);
    #[cfg(test)]
    {
        *family.published.lock() = Some(PublishedFamilyWrites {
            token: published.token.clone(),
            writes: published
                .results
                .iter()
                .flat_map(|row| {
                    row.writes()
                        .iter()
                        .map(|write| (row.owner(), row.target(), *write))
                })
                .collect(),
        });
    }
    let held = published
        .requirements
        .iter()
        .filter(|required| required.owner == light_core::programming::ProgrammingOwner::Color)
        .map(|required| required.target)
        .collect();
    family.accepted_color.record(
        &published.token,
        published.results,
        held,
        engine.output_pool().as_deref(),
    );
    let output = finish(OutputRenderSource::Hybrid(published.rendered))?;
    Ok((output, published.sampled))
}

/// Keep the legacy error contract: a superseded capture stays `StalePreparedFrame`, so both Live
/// callers resend retained output instead of failing. The hybrid seam carries engine errors as
/// their exact message (`invalid(error.to_string())`), so only an exact match is mapped back.
fn engine_error(error: light_dynamics::DynamicRuntimeError) -> EngineError {
    match error {
        light_dynamics::DynamicRuntimeError::InvalidSample(message)
            if message == EngineError::StalePreparedFrame.to_string() =>
        {
            EngineError::StalePreparedFrame
        }
        error => EngineError::Invalid(error.to_string()),
    }
}

/// Adapt a legacy-only `finish` closure (tests that predate the render source).
#[cfg(test)]
pub(in crate::runtime) fn legacy<R>(
    finish: impl FnOnce(&[ContributionBatch]) -> R,
) -> impl for<'a> FnOnce(OutputRenderSource<'a>) -> R {
    move |source: OutputRenderSource<'_>| match source {
        OutputRenderSource::Legacy(batches) => finish(batches),
        OutputRenderSource::Hybrid(_) => panic!("a legacy test frame took the family path"),
    }
}
