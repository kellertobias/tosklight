//! Shared staged orchestration for the gated typed producer. This builds a token, never renders
//! or publishes it. The caller owns the runtime/catalogue transaction and renders only after
//! this function returns, when the sampler has checked its completion proof.
//!
//! Ownership and lifetimes (TL-590):
//! - `HybridFrameContext<'a>` is a borrow of one capture, its final scalar-resolved geometry, the
//!   original native-model view and the owned `CapturedFrameToken` of the evaluating lane. It
//!   lives only for one `prepare_*_hybrid_frame` call and must never be stored.
//! - `HybridFrameResolver::adopt/resolve` receive that context synchronously. The lifecycle hooks
//!   (`begin_frame`, `verify_frame`, `accept_frame`) let a resolver own lane-local continuity:
//!   begin before any runtime mutation, verify before the engine finalizer, accept only after
//!   the finalizer succeeded. The default hooks are no-ops, so existing resolvers are unchanged.
//! - `HybridFamilyObservation<'a>` is a synchronous loan. Its callbacks borrow the composer's
//!   trace; a sidecar returned by the observer must own every piece of evidence it keeps.
//! - `PreparedHybridFrame<T>` owns the engine token, geometry, sampled bundle, the owned
//!   sidecars and the lane's `CapturedFrameToken`. See `physical_adapter` for the typed sidecar.

use super::super::{
    endpoint_controls::CapturedFamilyEndpointControls,
    family_inputs::{
        CapturedFamilyInputScratch, CapturedFamilyRequirement, assemble_captured_family_inputs,
    },
    fixed_masks::{
        CapturedFixedMaskRows, FixedMaskCompilationScratch, compile_captured_fixed_masks,
    },
    scalar_projection::{HybridScalarProjectionScratch, project_hybrid_scalar_samples},
};
use super::current_native::CurrentNativeVerificationCache;
use super::*;
use light_core::programming::{
    ProgrammingFieldScope, ProgrammingTransitionTrace, independent_programming_component,
};
use light_dynamics::{
    DynamicFamilyPreparationScratch, DynamicNativeModelResolver, DynamicRuntime,
    DynamicRuntimeError, DynamicSamplingScratch, FamilyCompositionContext,
    FamilyEndpointOutputContext, FamilyExpressionOperation, RetainedFamilyCompositionScratch,
    WholeFamilyExpressionFrameResolver, prepare_dynamic_family_samples_with_requirements,
};
use light_engine::{
    CapturedFrameToken, FamilyProjectionMetadata, PreloadBranch, PreloadFrameState,
    PreparedFrameGeometry, PreparedOutputFrame, PreparedPreloadFrame, PreparedStaticFamilyFrame,
};
use rustc_hash::FxHashSet;

mod fixed_bases;
mod position_batch;
mod position_program;
pub(in crate::runtime) use position_batch::{
    HybridPositionBatchComposer, HybridPositionBatchResult,
};
pub(in crate::runtime) use position_program::{
    HybridCapturedPositionProgram, HybridPositionDiscoveryEvaluation, HybridPositionEvaluation,
    HybridPositionGraphEvaluation, HybridPositionResumeEvaluation, HybridPositionStageEvaluation,
};

/// All frame-dependent work receives the already resolved final scalar geometry and the exact
/// original-model view captured before staged sampling. No callback must recapture Live sources.
/// `token` is the owned identity of this capture and lane; clone it into any retained result.
#[derive(Clone, Copy)]
pub(in crate::runtime) struct HybridFrameContext<'a> {
    pub capture: &'a PreparedOutputFrame,
    pub geometry: &'a PreparedFrameGeometry,
    pub native_models: &'a dyn DynamicNativeModelResolver,
    pub token: &'a CapturedFrameToken,
    /// The scalar-resolved static token of this frame (TL-592). Read-only: family projections
    /// are applied only after every observation of the frame, so it stays the immutable
    /// baseline that also produced `geometry`.
    pub scalar: &'a PreparedStaticFamilyFrame,
}

impl HybridFrameContext<'_> {
    /// Pre-master native raw values of the destination fixture that owns `target`, from this
    /// frame's scalar-resolved baseline and bound to `token`. Family-agnostic; see
    /// `light_engine::CapturedNativeRaw`.
    pub fn native_raw_into(
        &self,
        target: FixtureId,
        out: &mut light_engine::CapturedNativeRaw,
    ) -> Result<(), TransitionError> {
        self.scalar
            .native_raw_into(self.capture, self.token, target, out)
            .map_err(|error| IntentError(error.to_string()).into())
    }
    /// Position baseline including the selected physical installation's axis inversion.
    pub fn native_position_raw_into(
        &self,
        target: FixtureId,
        instance_id: uuid::Uuid,
        out: &mut light_engine::CapturedNativeRaw,
    ) -> Result<(), TransitionError> {
        self.scalar
            .native_position_raw_into(self.capture, self.token, target, instance_id, out)
            .map_err(|error| IntentError(error.to_string()).into())
    }
}

pub(in crate::runtime) trait HybridFrameResolver {
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError>;

    /// Preserve unknown field transfer when a successful frame solver cannot supply evidence.
    fn resolve(
        &self,
        _frame: HybridFrameContext<'_>,
        _target: FixtureId,
        requirement: TransitionRequirement,
        _from: &AttributeValue,
        _to: &AttributeValue,
        _operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        Err(TransitionError::Requires(requirement))
    }

    /// Called once per prepared frame before reconciliation or sampling can mutate the caller's
    /// runtime. A lane rejects foreign, stale or already accepted tokens here. Resetting staged
    /// continuity here also discards anything left by an earlier failed or unwound attempt.
    fn begin_frame(&self, _token: &CapturedFrameToken) -> Result<(), TransitionError> {
        Ok(())
    }

    /// Stage passive requirements from a successfully prepared frame. They retain prior
    /// continuity without claiming a new physical solve or publishing an old sidecar.
    /// Produced target/owners take precedence over incidental requirements.
    fn hold_frame(
        &self,
        _token: &CapturedFrameToken,
        _requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        Ok(())
    }

    /// Final fallible check before the engine finalizer: every staged result must belong to
    /// `token` and describe one consistent set of native writes.
    fn verify_frame(&self, _token: &CapturedFrameToken) -> Result<(), TransitionError> {
        Ok(())
    }

    /// Infallible commit after the finalizer accepted exactly this frame. A mismatched token
    /// commits nothing and returns false.
    fn accept_frame(&self, _token: &CapturedFrameToken) -> bool {
        true
    }
}

impl<T: HybridFrameResolver + ?Sized> HybridFrameResolver for &T {
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        (**self).adopt(frame, target, original, address)
    }

    fn resolve(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        (**self).resolve(frame, target, requirement, from, to, operation)
    }

    fn begin_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        (**self).begin_frame(token)
    }

    fn hold_frame(
        &self,
        token: &CapturedFrameToken,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        (**self).hold_frame(token, requirements)
    }

    fn verify_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        (**self).verify_frame(token)
    }

    fn accept_frame(&self, token: &CapturedFrameToken) -> bool {
        (**self).accept_frame(token)
    }
}

struct BoundFrameResolver<'a, R> {
    resolver: &'a R,
    frame: HybridFrameContext<'a>,
    target: FixtureId,
}

impl<R: HybridFrameResolver> WholeFamilyExpressionFrameResolver for BoundFrameResolver<'_, R> {
    fn adopt_position_angles(
        &self,
        original: &AttributeValue,
    ) -> Result<AttributeValue, TransitionError> {
        self.resolver.adopt(
            self.frame,
            self.target,
            original,
            &DynamicValueAddress {
                representation: light_dynamics::DynamicFamilyRepresentation::Angles,
                component: None,
            },
        )
    }

    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.resolve_with_trace(requirement, from, to, operation)
            .map(|(value, _)| value)
    }

    fn resolve_with_trace(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        self.resolver
            .resolve(self.frame, self.target, requirement, from, to, operation)
    }
}

type ProjectFields<'a> = dyn Fn(&ProgrammingFieldScope, &mut DynamicFamilySourceProjection) -> Result<(), TransitionError>
    + 'a;
type ControlFields<'a> =
    dyn Fn(&ProgrammingFieldScope) -> Option<Vec<light_dynamics::FamilyControlContribution>> + 'a;

/// A synchronous trace loan. The returned sidecar must own any retained field/source evidence.
/// The observer chooses metadata explicitly; the helper cannot infer mixed Color master or
/// source ownership from a value, rank, or the original baseline's master.
pub(in crate::runtime) struct HybridFamilyObservation<'a> {
    pub target: FixtureId,
    pub owner: ProgrammingOwner,
    pub value: &'a AttributeValue,
    pub static_baseline: &'a PreparedStaticFamilyFrame,
    /// The same frame context used by adoption/transition solves for this composition.
    pub frame: HybridFrameContext<'a>,
    project: &'a ProjectFields<'a>,
    controls: &'a ControlFields<'a>,
}

/// An owned composition result waiting for the complete physical cohort. Trace callbacks
/// remain confined to `observe`; sidecars must capture their source evidence synchronously.
pub(in crate::runtime) struct OwnedHybridProjection<T> {
    pub target: FixtureId,
    pub owner: ProgrammingOwner,
    pub value: AttributeValue,
    pub metadata: FamilyProjectionMetadata,
    pub sidecar: T,
}

/// Original prepared program before any destination has converted or composed Position.
/// Samples retain the whole eligible stack, original ranks, masks and compiled expressions.
/// The loan cannot escape this callback; observers may retain owned copies of the program.
pub(in crate::runtime) struct HybridFamilyProgram<'a> {
    pub target: FixtureId,
    pub owner: ProgrammingOwner,
    pub base: &'a AttributeValue,
    pub samples: &'a [light_dynamics::FamilyCompositionSample],
    /// Requirements-only groups are not proven static peers of an endpoint conversion.
    pub has_requirements: bool,
    pub frame: HybridFrameContext<'a>,
}

/// Compose the same prepared program with a destination-bound frame/adoption resolver. This
/// reuses the existing retained compositor and original Current/source catalogue, never the
/// Dynamic sampler. Every trace query must be consumed inside `observe` before scratch reuse.
pub(in crate::runtime) trait HybridProgramComposer<T> {
    fn begin_position(
        &mut self,
        _program: &HybridCapturedPositionProgram,
        _destination: FixtureId,
        _adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionEvaluation, TransitionError> {
        Err(IntentError("composer does not support owned Position evaluation".into()).into())
    }

    fn begin_position_branch(
        &mut self,
        _program: &HybridCapturedPositionProgram,
        _branch: &light_dynamics::PositionProgramBranch,
        _destination: FixtureId,
        _adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionEvaluation, TransitionError> {
        Err(IntentError("composer does not support conditioned Position evaluation".into()).into())
    }

    fn advance_position(
        &mut self,
        _evaluation: &mut HybridPositionEvaluation,
        _frame: &dyn WholeFamilyExpressionFrameResolver,
        _adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<light_dynamics::PositionCompositionProgress, TransitionError> {
        Err(IntentError("composer does not support owned Position evaluation".into()).into())
    }

    fn observe_position(
        &mut self,
        _evaluation: &mut HybridPositionEvaluation,
        _observe: &mut dyn FnMut(
            HybridFamilyObservation<'_>,
        ) -> Result<(FamilyProjectionMetadata, T), TransitionError>,
    ) -> Result<OwnedHybridProjection<T>, TransitionError> {
        Err(IntentError("composer does not support owned Position observation".into()).into())
    }

    fn resume_position(
        &mut self,
        _evaluation: &mut HybridPositionEvaluation,
        _request_id: uuid::Uuid,
        _value: AttributeValue,
        _transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        Err(IntentError("composer does not support owned Position responses".into()).into())
    }

    fn recycle_position(&mut self, _evaluation: HybridPositionEvaluation) {}

    fn compose(
        &mut self,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
        observe: &mut dyn FnMut(
            HybridFamilyObservation<'_>,
        ) -> Result<(FamilyProjectionMetadata, T), TransitionError>,
    ) -> Result<OwnedHybridProjection<T>, TransitionError>;
}

struct CapturedHybridProgramComposer<'a, 'sources, S> {
    typed: &'a CapturedProgrammingSources<'sources, S>,
    group: &'a light_dynamics::DynamicFamilySampleGroup,
    frame: HybridFrameContext<'a>,
    baseline: &'a PreparedStaticFamilyFrame,
    control:
        &'a dyn Fn(light_dynamics::FamilySampleRank) -> light_dynamics::FamilyEndpointOutputControl,
    scratch: &'a mut RetainedFamilyCompositionScratch,
}
impl<S: DynamicTickSource, T> HybridProgramComposer<T>
    for CapturedHybridProgramComposer<'_, '_, S>
{
    fn begin_position(
        &mut self,
        program: &HybridCapturedPositionProgram,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionEvaluation, TransitionError> {
        position_program::begin(self, program, destination, adoption)
    }

    fn begin_position_branch(
        &mut self,
        program: &HybridCapturedPositionProgram,
        branch: &light_dynamics::PositionProgramBranch,
        destination: FixtureId,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<HybridPositionEvaluation, TransitionError> {
        position_program::begin_branch(self, program, branch, destination, adoption)
    }

    fn advance_position(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        frame: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
    ) -> Result<light_dynamics::PositionCompositionProgress, TransitionError> {
        position_program::advance(self, evaluation, frame, adoption)
    }

    fn observe_position(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        observe: &mut dyn FnMut(
            HybridFamilyObservation<'_>,
        ) -> Result<(FamilyProjectionMetadata, T), TransitionError>,
    ) -> Result<OwnedHybridProjection<T>, TransitionError> {
        position_program::observe(self, evaluation, observe)
    }

    fn resume_position(
        &mut self,
        evaluation: &mut HybridPositionEvaluation,
        request_id: uuid::Uuid,
        value: AttributeValue,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> Result<(), TransitionError> {
        position_program::resume(self, evaluation, request_id, value, transfer)
    }

    fn recycle_position(&mut self, evaluation: HybridPositionEvaluation) {
        *self.scratch = evaluation.into_scratch();
    }

    fn compose(
        &mut self,
        frame_resolver: &dyn WholeFamilyExpressionFrameResolver,
        adoption: &light_dynamics::FamilyAdoptionResolver<'_>,
        observe: &mut dyn FnMut(
            HybridFamilyObservation<'_>,
        ) -> Result<(FamilyProjectionMetadata, T), TransitionError>,
    ) -> Result<OwnedHybridProjection<T>, TransitionError> {
        let group = self.group;
        let context = FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            resolve_adoption: Some(adoption),
            endpoint_output: Some(FamilyEndpointOutputContext {
                control: self.control,
                target: group.target,
                current: self.typed,
                native_models: Some(self.frame.native_models),
            }),
            ..Default::default()
        };
        self.typed.compose_family(
            group,
            &context,
            frame_resolver,
            self.scratch,
            |observation| {
                let project =
                    |fields: &ProgrammingFieldScope,
                     projection: &mut DynamicFamilySourceProjection| {
                        observation.project_fields(fields, projection)
                    };
                let controls = |fields: &ProgrammingFieldScope| {
                    fields.validate(group.owner).ok()?;
                    observation
                        .trace
                        .root()
                        .and_then(|root| observation.trace.control_sources_for_fields(root, fields))
                };
                let (metadata, sidecar) = observe(HybridFamilyObservation {
                    target: group.target,
                    owner: group.owner,
                    value: observation.value(),
                    static_baseline: self.baseline,
                    frame: self.frame,
                    project: &project,
                    controls: &controls,
                })?;
                Ok(OwnedHybridProjection {
                    target: group.target,
                    owner: group.owner,
                    value: observation.value().clone(),
                    metadata,
                    sidecar,
                })
            },
        )
    }
}

/// Observe individual compositions, then fit the complete frame before any family is projected.
/// `finish` sees all input, composition and Current requirements, including held owners with
/// no successful observation. Removing a projection also removes its sidecar from the result.
pub(in crate::runtime) trait HybridFrameObserver<T> {
    /// Install derived native inputs on the same completed semantic token, before the staged
    /// sampler completion proof is accepted. A rejection rolls back the complete attempt.
    fn project_native(
        &mut self,
        _capture: &PreparedOutputFrame,
        _frame_token: &CapturedFrameToken,
        _token: &mut PreparedStaticFamilyFrame,
        _sidecars: &[T],
    ) -> Result<(), TransitionError> {
        Ok(())
    }

    /// Begin one attempt, including retries of the same capture after a failure.
    fn begin_frame(&mut self, _token: &CapturedFrameToken) -> Result<(), TransitionError> {
        Ok(())
    }

    /// Capture every original pre-Dynamic Current owner before composing any group. This
    /// includes static peers with no Dynamic samples. Implementations own copied evidence;
    /// they must not retain the baseline/frame loan or stage accepted continuity here.
    fn prepare_current(
        &mut self,
        _frame: HybridFrameContext<'_>,
        _baseline: &PreparedStaticFamilyFrame,
        _protected: &[FixtureId],
    ) -> Result<(), TransitionError> {
        Ok(())
    }

    /// Physical owners that need final fitting even without Dynamic samples. Copies remain
    /// destination instances of their logical owner, never separately authored targets.
    fn static_program_targets(
        &mut self,
        _frame: HybridFrameContext<'_>,
        _baseline: &PreparedStaticFamilyFrame,
    ) -> Result<Vec<(FixtureId, ProgrammingOwner)>, TransitionError> {
        Ok(Vec::new())
    }

    /// Capture the complete original program registry before any destination composition.
    /// The loan cannot escape; retained programs must own their values/samples, not this frame.
    fn prepare_programs(
        &mut self,
        _frame: HybridFrameContext<'_>,
        _programs: &[HybridFamilyProgram<'_>],
    ) -> Result<(), TransitionError> {
        Ok(())
    }

    /// Complete Position batch inside the pinned typed-source/control lifetime. A returned
    /// handled target is removed from the ordinary per-owner loop, never composed a second time.
    fn compose_position_batch(
        &mut self,
        _frame: HybridFrameContext<'_>,
        _composer: &mut dyn HybridPositionBatchComposer<T>,
    ) -> Result<Option<HybridPositionBatchResult<T>>, TransitionError> {
        Ok(None)
    }

    /// Optional physical-destination binding BEFORE generic root composition. Returning None
    /// keeps the existing one-composition path; Some already owns its complete observed row.
    fn compose_program(
        &mut self,
        _program: HybridFamilyProgram<'_>,
        _composer: &mut dyn HybridProgramComposer<T>,
    ) -> Result<Option<OwnedHybridProjection<T>>, TransitionError> {
        Ok(None)
    }

    fn observe(
        &mut self,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, T), TransitionError>;

    fn finish(
        &mut self,
        _frame: HybridFrameContext<'_>,
        _projections: &mut Vec<OwnedHybridProjection<T>>,
        _requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        Ok(())
    }
}

impl<T, F> HybridFrameObserver<T> for F
where
    F: FnMut(HybridFamilyObservation<'_>) -> Result<(FamilyProjectionMetadata, T), TransitionError>,
{
    fn observe(
        &mut self,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, T), TransitionError> {
        self(observation)
    }
}

impl HybridFamilyObservation<'_> {
    pub fn project_fields(
        &self,
        fields: &ProgrammingFieldScope,
        projection: &mut DynamicFamilySourceProjection,
    ) -> Result<(), TransitionError> {
        (self.project)(fields, projection)
    }

    /// Control coverage is independent of authored appearance. Current partners and a master
    /// at zero can keep controller ownership even when their authored appearance is absent.
    pub fn controls_for_fields(
        &self,
        fields: &ProgrammingFieldScope,
    ) -> Option<Vec<light_dynamics::FamilyControlContribution>> {
        (self.controls)(fields)
    }
}

#[derive(Clone)]
pub(in crate::runtime::output_scheduler::dynamic_projection) enum HybridFamilyRequirementReason {
    Input(CapturedFamilyRequirement),
    Current {
        address: DynamicValueAddress,
        requirement: TransitionRequirement,
    },
    Composition(TransitionRequirement),
    /// Legacy scalar candidates have not yet been imported into the typed rank/source graph.
    /// Keep the already resolved scalar owner instead of replacing it with a guessed underlay.
    LegacyOwnerOverlap,
    /// Scalar output can change another family's effective baseline (for example Move-in-Black
    /// depends on Intensity). Current keeps its original input, but composition must not erase
    /// the changed final underlay or borrow the original input's source identity for it.
    ScalarBaselineChanged,
}

#[derive(Clone)]
pub(in crate::runtime) struct HybridFamilyRequirement {
    pub target: FixtureId,
    pub owner: ProgrammingOwner,
    pub(in crate::runtime::output_scheduler::dynamic_projection) reason:
        HybridFamilyRequirementReason,
}

pub(in crate::runtime::output_scheduler::dynamic_projection) struct PreparedHybridFrame<T> {
    pub token: PreparedStaticFamilyFrame,
    /// Owned identity of the capture and lane that produced `token` and every sidecar.
    pub frame_token: CapturedFrameToken,
    pub geometry: PreparedFrameGeometry,
    pub sampled: CapturedDynamicSample,
    pub family_sidecars: Vec<T>,
    /// Passive facts, not filtered UI notices. Exact visibility/coverage remains a producer gate.
    pub requirements: Vec<HybridFamilyRequirement>,
}

#[derive(Default)]
pub(in crate::runtime::output_scheduler::dynamic_projection) struct HybridFrameScratch {
    sampling: DynamicSamplingScratch,
    preparation: DynamicFamilyPreparationScratch,
    fixed: FixedMaskCompilationScratch,
    scalar: HybridScalarProjectionScratch,
    families: CapturedFamilyInputScratch,
    composition: RetainedFamilyCompositionScratch,
    position_batch_scratch: Vec<RetainedFamilyCompositionScratch>,
    legacy_owners: FxHashSet<(FixtureId, ProgrammingOwner)>,
    native_current: RefCell<CurrentNativeVerificationCache>,
}

fn invalid(error: impl std::fmt::Display) -> DynamicRuntimeError {
    DynamicRuntimeError::InvalidSample(error.to_string())
}

fn same_static_baseline(
    original: &PreparedStaticFamilyFrame,
    scalar: &PreparedStaticFamilyFrame,
    target: FixtureId,
    owner: ProgrammingOwner,
) -> bool {
    let key = owner.key();
    if original.value(target, &key) != scalar.value(target, &key)
        || original.changed_at(target, &key) != scalar.changed_at(target, &key)
        || original.sequence_master(target, &key) != scalar.sequence_master(target, &key)
    {
        return false;
    }
    let same_stamp = |a: light_core::ProgrammerEditStamp, b: light_core::ProgrammerEditStamp| {
        a.changed_at == b.changed_at && a.programmer_order == b.programmer_order
    };
    let same_origin = match (
        original.contribution_origin(target, &key),
        scalar.contribution_origin(target, &key),
    ) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.source() == b.source()
                && same_stamp(a.stamp(), b.stamp())
                && a.transition_ordinal() == b.transition_ordinal()
        }
        _ => false,
    };
    same_origin
        && match (
            original.contribution_family_evidence(target, &key),
            scalar.contribution_family_evidence(target, &key),
        ) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                Arc::ptr_eq(a, b)
                    || (a.entries().len() == b.entries().len()
                        && a.entries().iter().zip(b.entries()).all(|(a, b)| {
                            a.source() == b.source()
                                && same_stamp(a.stamp(), b.stamp())
                                && a.transition_ordinal() == b.transition_ordinal()
                                && a.authored_cue_id() == b.authored_cue_id()
                                && a.footprint() == b.footprint()
                                && a.role() == b.role()
                                && a.effective_fields() == b.effective_fields()
                        }))
            }
            _ => false,
        }
}

/// Prepare one coherent hybrid frame inside the caller's existing runtime/source transaction.
/// Expected Current/composition requirements retain the scalar owner's baseline while unrelated
/// owners progress. Unavailable Fixed masks conservatively hold their entire owner until exact
/// mask coverage is implemented; they are never silently dropped to expose a lower source.
/// Invalid inputs abort. This helper does not enable physical programming contract 1.
#[allow(clippy::too_many_arguments)]
pub(in crate::runtime::output_scheduler::dynamic_projection) fn prepare_captured_hybrid_frame<T>(
    engine: &Engine,
    capture: &PreparedOutputFrame,
    baseline_samples: &[ContributionBatch],
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    inputs: &CapturedDynamicInputs<'_>,
    scratch: &mut HybridFrameScratch,
    resolver: &impl HybridFrameResolver,
    presets: Option<&dyn DynamicValueSourceResolver>,
    mut observe: impl FnMut(
        HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, T), TransitionError>,
) -> Result<PreparedHybridFrame<T>, DynamicRuntimeError> {
    prepare_captured_hybrid_frame_with_observer(
        engine,
        capture,
        baseline_samples,
        runtime,
        origins,
        inputs,
        scratch,
        resolver,
        presets,
        &mut observe,
    )
}

/// Prepare Live with an observer that can fit complete physical cohorts before projection.
#[allow(clippy::too_many_arguments)]
pub(in crate::runtime::output_scheduler::dynamic_projection) fn prepare_captured_hybrid_frame_with_observer<
    T,
>(
    engine: &Engine,
    capture: &PreparedOutputFrame,
    baseline_samples: &[ContributionBatch],
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    inputs: &CapturedDynamicInputs<'_>,
    scratch: &mut HybridFrameScratch,
    resolver: &impl HybridFrameResolver,
    presets: Option<&dyn DynamicValueSourceResolver>,
    observer: &mut impl HybridFrameObserver<T>,
) -> Result<PreparedHybridFrame<T>, DynamicRuntimeError> {
    // Reject a substituted lane before reconciliation or sampling can mutate the caller's
    // runtime. Pending lanes enter through the explicitly branch-bound constructor below.
    let snapshot = capture.snapshot();
    if inputs.now != capture.sampled_at()
        || !Arc::ptr_eq(inputs.snapshot, &snapshot)
        || !Arc::ptr_eq(
            inputs.programmer_values,
            capture.dynamic_programmer_values(),
        )
        || !inputs
            .programmer_rows
            .is_some_and(|rows| std::ptr::eq(rows, capture.dynamic_programmer_rows().as_slice()))
        || !std::ptr::eq(inputs.cue_values, capture.cue_dynamic_values())
        || !std::ptr::eq(inputs.dynamic_playbacks, capture.dynamic_playbacks())
        || inputs.playback_paused != capture.playback_dynamics_paused()
        || !inputs.extra_programmer_values.is_empty()
    {
        return Err(invalid(
            "hybrid source inputs do not belong to this Live capture",
        ));
    }
    let scalar_sources = TickSources::prepared(engine, capture, baseline_samples);
    prepare_hybrid_frame(
        engine,
        capture,
        capture.frame_token(),
        baseline_samples,
        runtime,
        origins,
        inputs,
        scratch,
        resolver,
        presets,
        observer,
        &scalar_sources,
        |samples| engine.prepare_static_family_frame(capture, samples),
    )
}

/// Prepare one isolated pending branch with the same staged algorithm as Live. The caller
/// supplies that branch's retained runtime, catalogue and scratch, and owns their transaction.
/// Before/after Release use separate state; neither may borrow Live rows or a Live reconciliation
/// acknowledgment. Returned tokens can only be consumed by paired Preload finalization.
#[allow(clippy::too_many_arguments)]
pub(in crate::runtime::output_scheduler::dynamic_projection) fn prepare_captured_preload_hybrid_frame<
    T,
>(
    engine: &Engine,
    input: &PreparedPreloadFrame<'_>,
    state: &PreloadFrameState,
    branch: PreloadBranch,
    baseline_samples: &[ContributionBatch],
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    inputs: &CapturedDynamicInputs<'_>,
    scratch: &mut HybridFrameScratch,
    resolver: &impl HybridFrameResolver,
    presets: Option<&dyn DynamicValueSourceResolver>,
    mut observe: impl FnMut(
        HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, T), TransitionError>,
) -> Result<PreparedHybridFrame<T>, DynamicRuntimeError> {
    prepare_captured_preload_hybrid_frame_with_observer(
        engine,
        input,
        state,
        branch,
        baseline_samples,
        runtime,
        origins,
        inputs,
        scratch,
        resolver,
        presets,
        &mut observe,
    )
}

/// Prepare an isolated Preload branch with the same complete-cohort observation hook as Live.
#[allow(clippy::too_many_arguments)]
pub(in crate::runtime::output_scheduler::dynamic_projection) fn prepare_captured_preload_hybrid_frame_with_observer<
    T,
>(
    engine: &Engine,
    input: &PreparedPreloadFrame<'_>,
    state: &PreloadFrameState,
    branch: PreloadBranch,
    baseline_samples: &[ContributionBatch],
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    inputs: &CapturedDynamicInputs<'_>,
    scratch: &mut HybridFrameScratch,
    resolver: &impl HybridFrameResolver,
    presets: Option<&dyn DynamicValueSourceResolver>,
    observer: &mut impl HybridFrameObserver<T>,
) -> Result<PreparedHybridFrame<T>, DynamicRuntimeError> {
    let capture = input.frame();
    let snapshot = capture.snapshot();
    let before_release = branch == PreloadBranch::BeforeRelease;
    let (values, rows) = if before_release {
        (
            &input.sources().dynamic_values_before,
            &input.sources().dynamic_rows_before,
        )
    } else {
        (
            &input.sources().dynamic_values_after,
            &input.sources().dynamic_rows_after,
        )
    };
    if inputs.now != capture.sampled_at()
        || !Arc::ptr_eq(inputs.snapshot, &snapshot)
        || !Arc::ptr_eq(inputs.programmer_values, values)
        || !inputs
            .programmer_rows
            .is_some_and(|captured| std::ptr::eq(captured, rows.as_slice()))
        || !std::ptr::eq(inputs.cue_values, input.cue_dynamic_values())
        || !std::ptr::eq(inputs.dynamic_playbacks, input.dynamic_playbacks())
        || inputs.playback_paused != input.playback_dynamics_paused()
        || !inputs.extra_programmer_values.is_empty()
        || inputs.programmer_reconciliation_cache.is_some()
    {
        return Err(invalid(
            "hybrid source inputs do not belong to this Preload branch",
        ));
    }
    let scalar_sources = PreloadTickSources {
        engine,
        input,
        state,
        before_release,
        baseline_samples,
        values: OnceLock::new(),
    };
    prepare_hybrid_frame(
        engine,
        capture,
        input.frame_token(state, branch),
        baseline_samples,
        runtime,
        origins,
        inputs,
        scratch,
        resolver,
        presets,
        observer,
        &scalar_sources,
        |samples| engine.prepare_preload_static_family_frame(input, samples, state, branch),
    )
}

/// Branch constructors provide both scalar Current and static tokens from the same immutable
/// lane. Keeping the staged sampler/composer here prevents Live and Preload semantics drifting.
#[allow(clippy::too_many_arguments)]
fn prepare_hybrid_frame<T>(
    engine: &Engine,
    capture: &PreparedOutputFrame,
    frame_token: CapturedFrameToken,
    baseline_samples: &[ContributionBatch],
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    inputs: &CapturedDynamicInputs<'_>,
    scratch: &mut HybridFrameScratch,
    resolver: &impl HybridFrameResolver,
    presets: Option<&dyn DynamicValueSourceResolver>,
    observer: &mut impl HybridFrameObserver<T>,
    scalar_sources: &impl DynamicTickSource,
    prepare_static: impl Fn(&[ContributionBatch]) -> PreparedStaticFamilyFrame,
) -> Result<PreparedHybridFrame<T>, DynamicRuntimeError> {
    // A generation number alone cannot certify a custom resolver's slot mapping. Every lookup
    // in reconciliation, scalar projection and typed sampling uses this capture's index.
    let addresser = capture.frame_addresser();
    let inputs = &CapturedDynamicInputs {
        addresser: &addresser,
        ..*inputs
    };
    // Typed Current and whole Size always read this original pre-Freeze source. The later
    // scalar-resolved token is an output underlay and geometry input, never Current feedback.
    let mut static_token = prepare_static(baseline_samples);
    let with_fixed_bases;
    let baseline_samples = match fixed_bases::missing(engine, runtime, inputs, &static_token) {
        Some(bases) => {
            with_fixed_bases = [baseline_samples, &[bases]].concat();
            static_token = prepare_static(&with_fixed_bases);
            with_fixed_bases.as_slice()
        }
        None => baseline_samples,
    };
    begin_hybrid_frame(resolver, &frame_token, &static_token)?;
    observer.begin_frame(&frame_token).map_err(invalid)?;
    let static_sources = PreparedFamilySources(&static_token);
    let models = runtime.captured_native_color_models();
    let HybridFrameScratch {
        sampling,
        preparation,
        fixed,
        scalar,
        families,
        composition,
        position_batch_scratch,
        legacy_owners,
        native_current,
    } = scratch;
    native_current.borrow_mut().begin_frame();
    let result = sample_captured_dynamic_inputs_with_context(
        runtime,
        inputs,
        |runtime, now, interval, assignments, controls| {
            prepare_captured_preset_dependencies(runtime, inputs)?;
            source_bindings::bind_captured_sources(origins, runtime, inputs, assignments)
                .map_err(invalid)?;
            origins
                .reconcile_captured_programming_fixed_sources(
                    inputs.programmer_values,
                    inputs.programmer_rows,
                    inputs.cue_values,
                )
                .map_err(invalid)?;
            // Authored lookups occur during pinning, before Current bindings can be mutated.
            // This shares immutable catalogue storage; it never clones the runtime/history.
            let authored_origins = origins.clone();
            let authored = source_bindings::AuthoredDynamicSources(&authored_origins);
            let fixed = compile_captured_fixed_masks(
                &CapturedFixedMaskRows::from_inputs(inputs),
                Some(&authored_origins),
                Some(models.as_ref()),
                fixed,
            )
            .map_err(invalid)?;
            let endpoint_controls =
                CapturedFamilyEndpointControls::new(controls).map_err(invalid)?;
            runtime.sample_all_programming_staged(
                now,
                interval,
                inputs.speed_transports,
                scalar_sources,
                &authored,
                Some(inputs.addresser),
                sampling,
                |legacy, deferred| {
                    let mut batches = project_hybrid_scalar_samples(
                        inputs,
                        legacy,
                        controls,
                        scalar_sources,
                        fixed,
                        scalar,
                    );
                    collect_legacy_owners(&batches, legacy_owners);
                    batches.extend_from_slice(baseline_samples);
                    let mut token = prepare_static(&batches);
                    let geometry = final_geometry(engine, capture, &mut token, &frame_token)?;
                    let frame = HybridFrameContext {
                        capture,
                        geometry: &geometry,
                        native_models: models.as_ref(),
                        token: &frame_token,
                        scalar: &token,
                    };
                    let adopt =
                        |target, original: &AttributeValue, address: &DynamicValueAddress| {
                            resolver.adopt(frame, target, original, address)
                        };
                    let typed = CapturedProgrammingSources::new(&static_sources, &adopt, presets)
                        .with_source_transaction(origins)
                        .with_native_current_validation(models.as_ref(), native_current);
                    let completed = deferred.complete(&typed)?;
                    let prepared = prepare_dynamic_family_samples_with_requirements(
                        completed.samples(),
                        completed.requirements(),
                        &typed,
                        Some(models.as_ref()),
                        preparation,
                    )
                    .map_err(invalid)?;
                    typed.check().map_err(invalid)?;
                    let static_targets = observer.static_program_targets(frame, &static_token).map_err(invalid)?;
                    assemble_captured_family_inputs(&prepared, fixed, families);
                    let (groups, static_only) = families.with_static_targets(&static_targets);
                    let (mut requirements, mut projections) = (Vec::new(), Vec::new());
                    let mut protected_current = legacy_owners
                        .iter()
                        .filter_map(|(target, owner)| {
                            (*owner == ProgrammingOwner::Position).then_some(*target)
                        })
                        .collect::<Vec<_>>();
                    for entry in groups.iter() {
                        let group = &entry.group;
                        if group.owner == ProgrammingOwner::Position
                            && (scalar_owner_guard(
                                legacy_owners,
                                &static_token,
                                &token,
                                group.target,
                                group.owner,
                            )
                            .is_some()
                                || entry.requirements.iter().any(|requirement| {
                                    matches!(requirement, CapturedFamilyRequirement::Fixed { .. })
                                })
                                || static_sources
                                    .value(group.target, &group.owner.key())
                                    .is_none())
                        {
                            protected_current.push(group.target);
                        }
                    }
                    observer
                        .prepare_current(frame, &static_token, &protected_current)
                        .map_err(invalid)?;
                    let programs = groups.iter().filter_map(|entry| {
                        let group = &entry.group;
                        static_sources.value(group.target, &group.owner.key()).map(|base| HybridFamilyProgram {
                            target: group.target, owner: group.owner, base, samples: &group.samples,
                            has_requirements: !entry.requirements.is_empty(), frame,
                        })
                    }).collect::<Vec<_>>();
                    observer.prepare_programs(frame, &programs).map_err(invalid)?;
                    let control = |rank| endpoint_controls.control_for(rank);
                    let eligible_position_groups = groups.iter().filter_map(|entry| {
                        let group = &entry.group;
                        (group.owner == ProgrammingOwner::Position
                            && (!group.samples.is_empty() || static_only.contains(&(group.target, group.owner)))
                            && static_sources.value(group.target, &group.owner.key()).is_some()
                            && scalar_owner_guard(legacy_owners, &static_token, &token, group.target, group.owner).is_none()
                            && !entry.requirements.iter().any(|requirement| matches!(requirement, CapturedFamilyRequirement::Fixed { .. })))
                            .then_some(group)
                    }).collect::<Vec<_>>();
                    let mut batch_composer = position_batch::CapturedHybridPositionBatchComposer::new_with_pool(
                        &typed, &eligible_position_groups, frame, &static_token, &control,
                        std::mem::take(composition),
                        std::mem::take(position_batch_scratch),
                    );
                    let batch = observer.compose_position_batch(frame, &mut batch_composer).map_err(invalid)?;
                    let handled_position = match batch {
                        Some(batch) => {
                            position_batch::validate_result(&batch, &eligible_position_groups).map_err(invalid)?;
                            for mut row in batch.projections {
                                if static_only.contains(&(row.target, row.owner)) {
                                    row.metadata = FamilyProjectionMetadata {
                                        changed_at: static_token.changed_at(row.target, &row.owner.key()),
                                        evidence: light_engine::FamilyProjectionEvidence::PreserveBaseline,
                                        master: light_engine::FamilyProjectionMaster::PreserveBaseline,
                                    };
                                }
                                projections.push(row);
                            }
                            requirements.extend(batch.requirements);
                            batch.handled.into_iter().collect::<FxHashSet<_>>()
                        }
                        None => FxHashSet::default(),
                    };
                    let (local, pool) = batch_composer.into_workspaces();
                    *composition = local;
                    *position_batch_scratch = pool;
                    for entry in groups {
                        let group = &entry.group;
                        requirements.extend(entry.requirements.iter().cloned().map(
                            |requirement| HybridFamilyRequirement {
                                target: group.target,
                                owner: group.owner,
                                reason: HybridFamilyRequirementReason::Input(requirement),
                            },
                        ));
                        if let Some(reason) = scalar_owner_guard(
                            legacy_owners,
                            &static_token,
                            &token,
                            group.target,
                            group.owner,
                        ) {
                            requirements.push(HybridFamilyRequirement {
                                target: group.target,
                                owner: group.owner,
                                reason,
                            });
                            continue;
                        }
                        if group.owner == ProgrammingOwner::Position && handled_position.contains(&group.target) {
                            continue;
                        }
                        let static_only = static_only.contains(&(group.target, group.owner));
                        if (group.samples.is_empty() && !static_only)
                            || entry.requirements.iter().any(|requirement| {
                                matches!(requirement, CapturedFamilyRequirement::Fixed { .. })
                            })
                        {
                            continue;
                        }
                        let adoption =
                            |original: &AttributeValue, address: &DynamicValueAddress| {
                                resolver.adopt(frame, group.target, original, address)
                            };
                        let frame_resolver = BoundFrameResolver {
                            resolver,
                            frame,
                            target: group.target,
                        };
                        let mut composer = CapturedHybridProgramComposer {
                            typed: &typed,
                            group,
                            frame,
                            baseline: &static_token,
                            control: &control,
                            scratch: composition,
                        };
                        let base = static_sources.value(group.target, &group.owner.key());
                        let deferred = match base {
                            Some(base) => observer.compose_program(
                                HybridFamilyProgram {
                                    target: group.target,
                                    owner: group.owner,
                                    base,
                                    samples: &group.samples,
                                    has_requirements: !entry.requirements.is_empty(),
                                    frame,
                                },
                                &mut composer,
                            ),
                            None => Err(TransitionError::Requires(
                                TransitionRequirement::MaterializedEndpoints,
                            )),
                        };
                        let output = match deferred {
                            Ok(Some(row)) => Ok(row),
                            Ok(None) => {
                                composer.compose(&frame_resolver, &adoption, &mut |observation| {
                                    observer.observe(observation)
                                })
                            }
                            Err(error) => Err(error),
                        };
                        match output {
                            Ok(mut row) => {
                                if static_only {
                                    let metadata = FamilyProjectionMetadata {
                                        changed_at: static_token.changed_at(group.target, &group.owner.key()),
                                        evidence: light_engine::FamilyProjectionEvidence::PreserveBaseline,
                                        master: light_engine::FamilyProjectionMaster::PreserveBaseline,
                                    };
                                    row.metadata = metadata;
                                }
                                projections.push(row);
                            },
                            Err(TransitionError::Requires(requirement)) => {
                                requirements.push(HybridFamilyRequirement {
                                    target: group.target,
                                    owner: group.owner,
                                    reason: HybridFamilyRequirementReason::Composition(requirement),
                                })
                            }
                            Err(error) => return Err(invalid(error)),
                        }
                    }
                    requirements.extend(typed.requirements().into_iter().map(|required| {
                        HybridFamilyRequirement {
                            target: required.target,
                            owner: required.address.owner(),
                            reason: HybridFamilyRequirementReason::Current {
                                address: required.address,
                                requirement: required.requirement,
                            },
                        }
                    }));
                    let samples = completed.samples().to_vec();
                    // Complete the cohort while its scalar baseline is still unprojected.
                    observer
                        .finish(frame, &mut projections, &requirements)
                        .map_err(invalid)?;
                    typed.finish_source_bindings().map_err(invalid)?;
                    drop(typed);
                    let mut sidecars = Vec::with_capacity(projections.len());
                    for OwnedHybridProjection {
                        target,
                        owner,
                        value,
                        metadata,
                        sidecar,
                    } in projections
                    {
                        token
                            .project_family(target, owner, value, metadata)
                            .map_err(invalid)?;
                        sidecars.push(sidecar);
                    }
                    observer
                        .project_native(capture, &frame_token, &mut token, &sidecars)
                        .map_err(invalid)?;
                    Ok((
                        completed,
                        (samples, (token, geometry, sidecars, requirements)),
                    ))
                },
            )
        },
    );
    native_current.borrow_mut().finish_frame();
    let result = result?;
    resolver
        .hold_frame(&frame_token, &result.1.3)
        .map_err(invalid)?;
    Ok(finish_hybrid_frame(result, models, origins, frame_token))
}

type StagedHybridOutput<T> = (
    CapturedDynamicSample,
    (
        PreparedStaticFamilyFrame,
        PreparedFrameGeometry,
        Vec<T>,
        Vec<HybridFamilyRequirement>,
    ),
);

fn finish_hybrid_frame<T>(
    (mut sampled, (token, geometry, family_sidecars, requirements)): StagedHybridOutput<T>,
    models: Arc<dyn DynamicNativeModelResolver>,
    origins: &mut DynamicSourceOrigins,
    frame_token: CapturedFrameToken,
) -> PreparedHybridFrame<T> {
    sampled.native_models = models;
    // The outer staged call has now verified its own single-use proof. Retirement cannot run
    // while a deferred cohort could still add a held source. Publishing/rendering stays outside.
    source_bindings::retire_removed_controllers(
        origins,
        &sampled.before_runtime,
        &sampled.after_runtime,
    );
    PreparedHybridFrame {
        token,
        frame_token,
        geometry,
        sampled,
        family_sidecars,
        requirements,
    }
}

/// The resolver's lane rejects a foreign, stale or already accepted token before anything is
/// sampled, and the static lane must be the token's own capture/branch.
fn begin_hybrid_frame(
    resolver: &impl HybridFrameResolver,
    frame_token: &CapturedFrameToken,
    static_token: &PreparedStaticFamilyFrame,
) -> Result<(), DynamicRuntimeError> {
    if !frame_token.matches_static_frame(static_token) {
        return Err(invalid(
            "hybrid frame token does not belong to its static lane",
        ));
    }
    resolver.begin_frame(frame_token).map_err(invalid)
}

/// Final scalar-resolved geometry, checked against the frame token's generation and sample.
fn final_geometry(
    engine: &Engine,
    capture: &PreparedOutputFrame,
    token: &mut PreparedStaticFamilyFrame,
    frame_token: &CapturedFrameToken,
) -> Result<PreparedFrameGeometry, DynamicRuntimeError> {
    let geometry = engine
        .observe_static_family_geometry(capture, token)
        .map_err(invalid)?;
    if !frame_token.matches_geometry(&geometry) {
        return Err(invalid(
            "final geometry does not belong to the hybrid frame token",
        ));
    }
    Ok(geometry)
}

fn collect_legacy_owners(
    batches: &[ContributionBatch],
    legacy_owners: &mut FxHashSet<(FixtureId, ProgrammingOwner)>,
) {
    legacy_owners.clear();
    for sample in batches.iter().flat_map(|batch| batch.samples()) {
        for owner in [
            ProgrammingOwner::Position,
            ProgrammingOwner::Color,
            ProgrammingOwner::Focus,
            ProgrammingOwner::Zoom,
        ] {
            if sample.value().attribute == owner.key()
                || independent_programming_component(&sample.value().attribute, owner)
            {
                legacy_owners.insert((sample.value().fixture_id, owner));
            }
        }
    }
}

/// The LegacyOwnerOverlap and ScalarBaselineChanged guards: keep the scalar-resolved owner
/// until safe family adoption with imported scalar rank/source data exists.
fn scalar_owner_guard(
    legacy_owners: &FxHashSet<(FixtureId, ProgrammingOwner)>,
    original: &PreparedStaticFamilyFrame,
    scalar: &PreparedStaticFamilyFrame,
    target: FixtureId,
    owner: ProgrammingOwner,
) -> Option<HybridFamilyRequirementReason> {
    if legacy_owners.contains(&(target, owner)) {
        Some(HybridFamilyRequirementReason::LegacyOwnerOverlap)
    } else if !same_static_baseline(original, scalar, target, owner) {
        Some(HybridFamilyRequirementReason::ScalarBaselineChanged)
    } else {
        None
    }
}
