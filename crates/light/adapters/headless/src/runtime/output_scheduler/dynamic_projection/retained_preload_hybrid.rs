//! Detached paired hybrid evaluation over exactly one retained input. The paired coordinator
//! owns both runtime journals and COW source catalogues; this adapter never opens a transaction.
//! Both branches stay warm from their common episode seed, even without visible Color Release.
//! Physical adoption and output metadata policies remain explicit injected dependencies.
//! A physical resolver (see `physical_adapter::PhysicalPreloadLanes`) keeps one independent
//! adapter lane per branch: its lifecycle hooks are verified before the engine finalizer and
//! accepted only after it succeeded, so a failed pair advances neither branch's continuity.
pub(in crate::runtime) use super::programming_projection::hybrid::{
    HybridFamilyObservation, HybridFamilyRequirement, HybridFrameContext, HybridFrameResolver,
};
use super::programming_projection::hybrid::{
    HybridFamilyProgram, HybridFrameObserver, HybridFrameScratch, HybridPositionBatchComposer,
    HybridPositionBatchResult, HybridProgramComposer, OwnedHybridProjection, PreparedHybridFrame,
    prepare_captured_preload_hybrid_frame_with_observer,
};
use super::*;
use crate::runtime::dynamic_snapshot_publication::RetainedInputCapture;
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use crate::runtime::preload::retained_history::paired::PendingPairEvaluator;
use light_core::ProgrammerId;
use light_core::programming::{ProgrammingOwner, TransitionError};
use light_dynamics::DynamicRuntime;
use light_engine::{
    CapturedFrameToken, FamilyProjectionMetadata, PreloadBranch, PreloadFrameState,
    PreparedPreloadFrame, RenderedPreloadFrame,
};

pub(in crate::runtime) mod family_episode;
pub(in crate::runtime) mod pending_episode;
pub(in crate::runtime) mod pending_executor;
pub(in crate::runtime) mod position_episode;
mod state;
use state::EvaluatorState;
pub(in crate::runtime) use state::RetainedPreloadHybridState;

/// Observe each retained branch independently, then complete its physical cohort before any
/// family projection. Loans and callbacks remain synchronous; retained evidence must be owned.
pub(in crate::runtime) trait RetainedHybridFrameObserver<S> {
    fn project_native(
        &mut self,
        _branch: PreloadBranch,
        _capture: &light_engine::PreparedOutputFrame,
        _frame_token: &CapturedFrameToken,
        _token: &mut light_engine::PreparedStaticFamilyFrame,
        _sidecars: &[S],
    ) -> Result<(), TransitionError> {
        Ok(())
    }

    /// Start every attempt, including retries of the same captured token.
    fn begin_frame(
        &mut self,
        _branch: PreloadBranch,
        _token: &CapturedFrameToken,
    ) -> Result<(), TransitionError> {
        Ok(())
    }

    fn prepare_current(
        &mut self,
        _branch: PreloadBranch,
        _frame: HybridFrameContext<'_>,
        _baseline: &light_engine::PreparedStaticFamilyFrame,
        _protected: &[FixtureId],
    ) -> Result<(), TransitionError> {
        Ok(())
    }

    fn static_program_targets(
        &mut self,
        _branch: PreloadBranch,
        _frame: HybridFrameContext<'_>,
        _baseline: &light_engine::PreparedStaticFamilyFrame,
    ) -> Result<Vec<(FixtureId, ProgrammingOwner)>, TransitionError> {
        Ok(Vec::new())
    }
    fn prepare_programs(
        &mut self,
        _branch: PreloadBranch,
        _frame: HybridFrameContext<'_>,
        _programs: &[HybridFamilyProgram<'_>],
    ) -> Result<(), TransitionError> {
        Ok(())
    }

    fn compose_position_batch(
        &mut self,
        _branch: PreloadBranch,
        _frame: HybridFrameContext<'_>,
        _composer: &mut dyn HybridPositionBatchComposer<S>,
    ) -> Result<Option<HybridPositionBatchResult<S>>, TransitionError> {
        Ok(None)
    }

    fn compose_program(
        &mut self,
        _branch: PreloadBranch,
        _program: HybridFamilyProgram<'_>,
        _composer: &mut dyn HybridProgramComposer<S>,
    ) -> Result<Option<OwnedHybridProjection<S>>, TransitionError> {
        Ok(None)
    }

    fn observe(
        &mut self,
        branch: PreloadBranch,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, S), TransitionError>;

    fn finish(
        &mut self,
        _branch: PreloadBranch,
        _frame: HybridFrameContext<'_>,
        _projections: &mut Vec<OwnedHybridProjection<S>>,
        _requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        Ok(())
    }
}

impl<S, F> RetainedHybridFrameObserver<S> for F
where
    F: FnMut(
        PreloadBranch,
        HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, S), TransitionError>,
{
    fn observe(
        &mut self,
        branch: PreloadBranch,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, S), TransitionError> {
        self(branch, observation)
    }
}

/// Borrow only this evaluation's selected branch. The paired coordinator still owns all
/// journals and both retained lanes; this bridge changes no orchestration or history.
struct BranchObserver<'a, O> {
    branch: PreloadBranch,
    observer: &'a mut O,
}

impl<S, O: RetainedHybridFrameObserver<S>> HybridFrameObserver<S> for BranchObserver<'_, O> {
    fn project_native(
        &mut self,
        capture: &light_engine::PreparedOutputFrame,
        frame_token: &CapturedFrameToken,
        token: &mut light_engine::PreparedStaticFamilyFrame,
        sidecars: &[S],
    ) -> Result<(), TransitionError> {
        self.observer
            .project_native(self.branch, capture, frame_token, token, sidecars)
    }

    fn begin_frame(&mut self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.observer.begin_frame(self.branch, token)
    }

    fn prepare_current(
        &mut self,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
        protected: &[FixtureId],
    ) -> Result<(), TransitionError> {
        self.observer
            .prepare_current(self.branch, frame, baseline, protected)
    }

    fn static_program_targets(
        &mut self,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
    ) -> Result<Vec<(FixtureId, ProgrammingOwner)>, TransitionError> {
        self.observer
            .static_program_targets(self.branch, frame, baseline)
    }
    fn prepare_programs(
        &mut self,
        frame: HybridFrameContext<'_>,
        programs: &[HybridFamilyProgram<'_>],
    ) -> Result<(), TransitionError> {
        self.observer.prepare_programs(self.branch, frame, programs)
    }

    fn compose_position_batch(
        &mut self,
        frame: HybridFrameContext<'_>,
        composer: &mut dyn HybridPositionBatchComposer<S>,
    ) -> Result<Option<HybridPositionBatchResult<S>>, TransitionError> {
        self.observer
            .compose_position_batch(self.branch, frame, composer)
    }

    fn compose_program(
        &mut self,
        program: HybridFamilyProgram<'_>,
        composer: &mut dyn HybridProgramComposer<S>,
    ) -> Result<Option<OwnedHybridProjection<S>>, TransitionError> {
        self.observer
            .compose_program(self.branch, program, composer)
    }

    fn observe(
        &mut self,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, S), TransitionError> {
        self.observer.observe(self.branch, observation)
    }

    fn finish(
        &mut self,
        frame: HybridFrameContext<'_>,
        projections: &mut Vec<OwnedHybridProjection<S>>,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        self.observer
            .finish(self.branch, frame, projections, requirements)
    }
}

/// Owned samples, original-model view, exact observer evidence and passive requirements from
/// one branch. No static token escapes, and no subsequent reader reconstructs its source rows.
pub(in crate::runtime) struct PendingHybridBranch<S> {
    /// Owned identity of this branch's capture; every physical sidecar carries the same token.
    pub frame_token: CapturedFrameToken,
    /// Includes exact pre/post runtime snapshots, samples, model handle and captured controls.
    pub sampled: CapturedDynamicSample,
    /// Immutable COW catalogue matching the samples, not a later coordinator publication.
    pub origins: DynamicSourceOrigins,
    pub sidecars: Vec<S>,
    pub requirements: Vec<HybridFamilyRequirement>,
}

pub(in crate::runtime) struct PendingHybridResult<S> {
    pub rendered: RenderedPreloadFrame,
    pub before: PendingHybridBranch<S>,
    pub after: PendingHybridBranch<S>,
}

/// One state belongs to one paired episode. An evaluator may own it for convenience or
/// borrow it for this synchronous call; dropping a borrowing evaluator does not reset it.
/// Its mount state is private, and its scratch may only cache immutable computations. Callbacks must not mutate semantic external state.
/// Metadata is deliberately not inferred from the static underlay or the resulting value.
/// The final engine render is the last fallible operation. Its mount-cache commit follows all
/// projection errors; after it succeeds only owned moves remain before both journals commit.
/// Error/unwind rollback covers callbacks before that commit tail, not arbitrary internal
/// panics after a nested journal has already committed. No output or Live state is published.
pub(in crate::runtime) struct RetainedPreloadHybridEvaluator<'a, R, O> {
    engine: &'a Engine,
    programmer: ProgrammerId,
    state: EvaluatorState<'a>,
    resolver: R,
    observe: O,
    #[cfg(test)]
    pub(in crate::runtime::output_scheduler::dynamic_projection) swap_finalization_tokens: bool,
}

impl<'a, R, O> RetainedPreloadHybridEvaluator<'a, R, O> {
    pub(in crate::runtime) fn new<S>(
        engine: &'a Engine,
        programmer: ProgrammerId,
        resolver: R,
        observe: O,
    ) -> Self
    where
        O: FnMut(
            PreloadBranch,
            HybridFamilyObservation<'_>,
        ) -> Result<(FamilyProjectionMetadata, S), TransitionError>,
    {
        Self::new_with_observer(engine, programmer, resolver, observe)
    }

    pub(in crate::runtime) fn new_with_observer(
        engine: &'a Engine,
        programmer: ProgrammerId,
        resolver: R,
        observe: O,
    ) -> Self {
        Self {
            engine,
            programmer,
            state: EvaluatorState::Owned(Box::default()),
            resolver,
            observe,
            #[cfg(test)]
            swap_finalization_tokens: false,
        }
    }

    pub(in crate::runtime) fn with_state<S>(
        engine: &'a Engine,
        programmer: ProgrammerId,
        state: &'a mut RetainedPreloadHybridState,
        resolver: R,
        observe: O,
    ) -> Self
    where
        O: FnMut(
            PreloadBranch,
            HybridFamilyObservation<'_>,
        ) -> Result<(FamilyProjectionMetadata, S), TransitionError>,
    {
        Self::with_state_and_observer(engine, programmer, state, resolver, observe)
    }

    pub(in crate::runtime) fn with_state_and_observer(
        engine: &'a Engine,
        programmer: ProgrammerId,
        state: &'a mut RetainedPreloadHybridState,
        resolver: R,
        observe: O,
    ) -> Self {
        Self {
            engine,
            programmer,
            state: EvaluatorState::Borrowed(state),
            resolver,
            observe,
            #[cfg(test)]
            swap_finalization_tokens: false,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_branch<S>(
    engine: &Engine,
    retained: &RetainedInputCapture,
    input: &PreparedPreloadFrame<'_>,
    state: &PreloadFrameState,
    branch: PreloadBranch,
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    scratch: &mut HybridFrameScratch,
    resolver: &impl HybridFrameResolver,
    observe: &mut impl RetainedHybridFrameObserver<S>,
) -> Result<PreparedHybridFrame<S>, String> {
    let frame = input.frame();
    let snapshot = frame.snapshot();
    let addresser = frame.frame_addresser();
    let (values, rows) = match branch {
        PreloadBranch::BeforeRelease => (
            &input.sources().dynamic_values_before,
            &input.sources().dynamic_rows_before,
        ),
        PreloadBranch::AfterRelease => (
            &input.sources().dynamic_values_after,
            &input.sources().dynamic_rows_after,
        ),
    };
    let inputs = CapturedDynamicInputs {
        now: frame.sampled_at(),
        speed_transports: &retained.speed_transports,
        rate: retained.rate,
        snapshot: &snapshot,
        programmer_values: values,
        programmer_rows: Some(rows.as_slice()),
        cue_values: input.cue_dynamic_values(),
        dynamic_playbacks: input.dynamic_playbacks(),
        playback_paused: input.playback_dynamics_paused(),
        addresser: &addresser,
        extra_programmer_values: &[],
        programmer_reconciliation_cache: None,
        force_source_reconciliation: false,
    };
    prepare_captured_preload_hybrid_frame_with_observer(
        engine,
        input,
        state,
        branch,
        &retained.baseline,
        runtime,
        origins,
        &inputs,
        scratch,
        resolver,
        None, // Exact retained per-instance Preset tables; no external current resolver.
        &mut BranchObserver {
            branch,
            observer: observe,
        },
    )
    .map_err(|error| error.to_string())
}

impl<R, O, S> PendingPairEvaluator<PendingHybridResult<S>>
    for RetainedPreloadHybridEvaluator<'_, R, O>
where
    R: HybridFrameResolver,
    O: RetainedHybridFrameObserver<S>,
{
    fn evaluate(
        &mut self,
        capture: &RetainedInputCapture,
        before: &mut DynamicRuntime,
        before_origins: &mut DynamicSourceOrigins,
        after: &mut DynamicRuntime,
        after_origins: &mut DynamicSourceOrigins,
    ) -> Result<PendingHybridResult<S>, String> {
        if capture.frame.programmer().identity != Some(self.programmer) {
            return Err("retained hybrid input belongs to another Programmer".into());
        }
        if !capture
            .frame
            .programmer()
            .preload_playback_actions
            .is_empty()
        {
            return Err("retained hybrid queue context is unavailable".into());
        }
        let state = self.state.as_mut();
        let input = self.engine.prepare_preload_frame(&capture.frame, None);
        let before = prepare_branch(
            self.engine,
            capture,
            &input,
            &state.state,
            PreloadBranch::BeforeRelease,
            before,
            before_origins,
            &mut state.before_scratch,
            &self.resolver,
            &mut self.observe,
        )?;
        let after = prepare_branch(
            self.engine,
            capture,
            &input,
            &state.state,
            PreloadBranch::AfterRelease,
            after,
            after_origins,
            &mut state.after_scratch,
            &self.resolver,
            &mut self.observe,
        )?;
        let PreparedHybridFrame {
            token: before_token,
            frame_token: before_frame,
            sampled: before_sampled,
            family_sidecars: before_sidecars,
            requirements: before_requirements,
            ..
        } = before;
        let PreparedHybridFrame {
            token: after_token,
            frame_token: after_frame,
            sampled: after_sampled,
            family_sidecars: after_sidecars,
            requirements: after_requirements,
            ..
        } = after;
        // Last fallible adapter checks: each branch's staged physical results must belong to
        // its own token before the finalizer may commit mount continuity.
        for frame in [&before_frame, &after_frame] {
            self.resolver
                .verify_frame(frame)
                .map_err(|error| error.to_string())?;
        }
        let before_catalogue = before_origins.clone();
        let after_catalogue = after_origins.clone();
        // The engine uses this SAME input bundle for both tokens. It suppresses unnecessary
        // Before output; that does not suppress either branch's Dynamic sampling above.
        // Its hidden Before mount cache is not committed by this engine API. That cache only
        // reuses exact current layout/Point poses, so discarding it costs recomputation, not history.
        // Mount continuity commits only after all fallible projection has succeeded. No
        // observer, source query, or validation may be added after this call.
        #[cfg(test)]
        let (before_token, after_token) = if self.swap_finalization_tokens {
            (after_token, before_token)
        } else {
            (before_token, after_token)
        };
        let rendered = self
            .engine
            .render_prepared_preload_families(
                &input,
                Some(before_token),
                after_token,
                &mut state.state,
            )
            .map_err(|error| error.to_string())?;
        // Infallible moves only: tokens were verified above against the same staged results.
        let accepted = self.resolver.accept_frame(&before_frame);
        let accepted = self.resolver.accept_frame(&after_frame) && accepted;
        debug_assert!(accepted, "verified branch tokens are accepted");
        Ok(PendingHybridResult {
            rendered,
            before: PendingHybridBranch {
                frame_token: before_frame,
                sampled: before_sampled,
                origins: before_catalogue,
                sidecars: before_sidecars,
                requirements: before_requirements,
            },
            after: PendingHybridBranch {
                frame_token: after_frame,
                sampled: after_sampled,
                origins: after_catalogue,
                sidecars: after_sidecars,
                requirements: after_requirements,
            },
        })
    }
}

#[cfg(test)]
pub(in crate::runtime) mod tests;
