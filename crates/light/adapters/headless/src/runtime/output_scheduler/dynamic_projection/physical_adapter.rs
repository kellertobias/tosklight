//! Typed physical-adapter sidecars on the existing captured hybrid seam (TL-590).
//!
//! This module adds no orchestration. A family adapter plugs into the existing
//! `prepare_captured_hybrid_frame` / `RetainedPreloadHybridEvaluator` flow in two places:
//!
//! 1. As the `HybridFrameResolver` (through [`PhysicalAdapterLane`] or
//!    [`PhysicalPreloadLanes`]), for adoption and transition solves against the frame context.
//! 2. As the observer, returning `(FamilyProjectionMetadata, PhysicalHeadResult<A>)`; the
//!    engine token receives the composed family value and metadata exactly as before, and the
//!    owned result becomes the sidecar.
//!
//! Ownership and lifetimes:
//! - [`PhysicalHeadResult`] is fully owned: its `CapturedFrameToken`, native writes,
//!   requested/achieved/quality values and provenance (a cloned
//!   `DynamicFamilySourceProjection`, control contributions and metadata) never borrow the
//!   observation, the capture, the runtime or a source catalogue. It stays valid after later
//!   edits, pruning or catalogue publication.
//! - Destination descriptors are compiled per target from the captured frame's own snapshot and
//!   cached against its runtime generation. A different generation discards the whole cache.
//! - Continuity (for example previous commanded joints) is lane-owned. It is staged during the
//!   frame and committed only by `accept_frame` after the engine finalizer succeeded. Live and
//!   each Preload branch own separate lanes; no lane reads another lane's state.
//!
//! Production runs the semantic programming contract (contract 1, TL-552); the Live path reaches
//! these adapters through `output_transaction::family_frame` (TL-548 C3).
#![allow(dead_code)]

use super::programming_projection::hybrid::{
    HybridFamilyObservation, HybridFamilyRequirement, HybridFrameContext, HybridFrameResolver,
    PreparedHybridFrame,
};
use super::*;
use crate::runtime::dynamic_source_origins::DynamicFamilySourceProjection;
use light_core::programming::{
    IntentError, ProgrammingFieldScope, ProgrammingOwner, ProgrammingTransitionTrace,
    TransitionError, TransitionRequirement,
};
use light_dynamics::{
    DynamicRuntimeError, DynamicValueAddress, FamilyControlContribution, FamilyExpressionOperation,
};
use light_engine::{
    CapturedFrameToken, EngineSnapshot, FamilyProjectionEvidence, FamilyProjectionMetadata,
    PreparedFrameGeometry, PreparedOutputFrame,
};

pub(in crate::runtime) mod color;
pub(in crate::runtime) mod color_router;
// TL-548 C1: all-family Live/Preload lanes, engaged in production since C3/TL-552.
mod counters;
pub(in crate::runtime) mod family_lanes;
mod lane;
pub(in crate::runtime) mod live_frame;
pub(in crate::runtime) mod media_color;
pub(in crate::runtime) mod optics;
pub(in crate::runtime) mod position;
#[cfg(test)]
mod send_audit;
#[cfg(test)]
pub(in crate::runtime) mod test_adapter;
#[allow(unused_imports)]
// TL-592 Color/UV adapter re-exports; the family lanes use the adapter, tests use the rest.
pub(in crate::runtime) use color::{
    AchievedColor, ColorAdapter, ColorAdapterCounters, ColorContinuity, ColorDescriptor,
    ColorHeadContinuity, ColorHeadDescriptor, ColorHeadOutcome, ColorQuality, ColorSolveWork,
};
#[allow(unused_imports)]
// Preload lanes; some re-exports are consumed only by the Preload evaluator tests.
pub(in crate::runtime) use lane::{
    PhysicalAdapterLane, PhysicalLaneKind, PhysicalPreloadLanes, ReleasedPhysicalOwner,
};
pub(in crate::runtime) use live_frame::PublishedLiveFrame;
#[allow(unused_imports)]
// TL-593 Media Color adapter re-exports; the family lanes use the adapter, tests use the rest.
pub(in crate::runtime) use media_color::{
    MediaColorAdapter, MediaColorAdapterCounters, MediaColorDescriptor, MediaColorQuality,
};
#[allow(unused_imports)]
// TL-558 Focus/Zoom adapter re-exports; the family lanes use the adapter, tests use the rest.
pub(in crate::runtime) use optics::{
    OpticsAdapter, OpticsAdapterCounters, OpticsContinuity, OpticsDescriptor, OpticsLanes,
    OpticsPreloadLanes, OpticsQuality, OpticsRequested,
};

/// One native control addressed on its physical destination (root fixture or patched copy).
/// `channel_index`/`split` follow `FixtureModeEncodingPlan::encode_split_by_index`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(in crate::runtime) struct NativeControlSlot {
    pub destination: FixtureId,
    pub channel_index: u32,
    pub split: u16,
}

/// One complete native write. A parked control is still written: `parked` records that its
/// value is the owner's neutral/park value, not a fitted drive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct NativeControlWrite {
    pub slot: NativeControlSlot,
    pub channel_id: uuid::Uuid,
    pub function_id: Option<uuid::Uuid>,
    pub raw: u32,
    pub parked: bool,
}

impl NativeControlWrite {
    /// Reuse the Focus/Zoom fitter's write without re-deriving its address.
    pub fn from_optics(destination: FixtureId, write: &light_fixture::OpticsControlWrite) -> Self {
        Self {
            slot: NativeControlSlot {
                destination,
                channel_index: write.channel_index,
                split: write.split,
            },
            channel_id: write.channel_id,
            function_id: None,
            raw: write.raw,
            parked: false,
        }
    }

    /// Reuse the Color fitter's write, including its function and park role.
    pub fn from_color(
        destination: FixtureId,
        write: &light_fixture::forward::ColorControlWrite,
    ) -> Self {
        Self {
            slot: NativeControlSlot {
                destination,
                channel_index: write.channel_index,
                split: write.split,
            },
            channel_id: write.channel_id,
            function_id: (!write.function_id.is_nil()).then_some(write.function_id),
            raw: write.raw,
            parked: matches!(
                write.role,
                light_fixture::forward::ColorWriteRole::ParkedEmitter { .. }
            ),
        }
    }
}

/// What a family resolver sees for one composed head. Everything is borrowed for the duration
/// of one synchronous `resolve` call.
pub(in crate::runtime) struct PhysicalRequest<'a, A: PhysicalFamilyAdapter + ?Sized> {
    pub frame: HybridFrameContext<'a>,
    pub target: FixtureId,
    pub owner: ProgrammingOwner,
    pub descriptor: &'a A::Descriptor,
    /// The complete composed family value (requested intent), never fitted DMX.
    pub value: &'a AttributeValue,
    /// This lane's last accepted continuity for the same target/owner, if any.
    pub previous: Option<&'a A::Continuity>,
}

/// A family resolver's owned answer for one head.
pub(in crate::runtime) struct PhysicalResolution<A: PhysicalFamilyAdapter + ?Sized> {
    /// Every slot in the descriptor's footprint exactly once (emit or park), nothing else.
    pub writes: Vec<NativeControlWrite>,
    pub requested: A::Requested,
    pub achieved: A::Achieved,
    pub quality: A::Quality,
    pub continuity: A::Continuity,
}

/// Family-specific physical adapter. Implementations wrap existing compiled fitters
/// (`CompiledOpticsFitting`, `CompiledColorFitting`, position forward/calibration); this trait
/// only fixes how they are compiled, looked up, called and published.
pub(in crate::runtime) trait PhysicalFamilyAdapter {
    /// Compiled per-target destination model (share immutable fitters through `Arc`).
    type Descriptor;
    /// Lane-owned continuity carried from one accepted frame to the next.
    type Continuity: Clone;
    type Requested: Clone;
    type Achieved: Clone;
    /// Passive capability/clipping/unknown status. Never a notification.
    type Quality: Clone;

    fn owns(&self, owner: ProgrammingOwner) -> bool;

    /// Optional adapter-owned frame metadata follows the same staged/accepted lane boundary
    /// as native writes. It never commits from speculative observation or fitting.
    fn begin_lane_frame(&self, _token: &CapturedFrameToken) -> Result<(), TransitionError> {
        Ok(())
    }
    fn abandon_lane_frame(&self) {}
    fn verify_lane_frame(&self, _token: &CapturedFrameToken) -> Result<(), TransitionError> {
        Ok(())
    }
    fn accept_lane_frame(&self, _token: &CapturedFrameToken) {}

    /// Compile against the captured snapshot. `Ok(None)` means this target has no physical model
    /// for the family (the scalar owner stays, as a passive requirement); `Err` is corruption.
    fn compile(
        &self,
        snapshot: &EngineSnapshot,
        target: FixtureId,
    ) -> Result<Option<Self::Descriptor>, TransitionError>;

    /// Complete native footprint owned by this family on this head.
    fn footprint<'d>(&self, descriptor: &'d Self::Descriptor) -> &'d [NativeControlSlot];

    fn resolve(
        &self,
        request: PhysicalRequest<'_, Self>,
    ) -> Result<PhysicalResolution<Self>, TransitionError>;

    /// First-edit / Current adoption of a foreign representation for this family.
    fn adopt(
        &self,
        _frame: HybridFrameContext<'_>,
        _descriptor: &Self::Descriptor,
        _target: FixtureId,
        _original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        Err(TransitionError::Requires(owner_requirement(
            address.owner(),
        )))
    }

    /// Adoption receives only this lane's last accepted continuity, never another branch.
    fn adopt_with_continuity(
        &self,
        frame: HybridFrameContext<'_>,
        descriptor: &Self::Descriptor,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
        _previous: Option<&Self::Continuity>,
    ) -> Result<AttributeValue, TransitionError> {
        self.adopt(frame, descriptor, target, original, address)
    }

    /// Cross-representation transition/scale inside one frame.
    #[allow(clippy::too_many_arguments)]
    fn transition(
        &self,
        _frame: HybridFrameContext<'_>,
        _descriptor: &Self::Descriptor,
        _target: FixtureId,
        requirement: TransitionRequirement,
        _from: &AttributeValue,
        _to: &AttributeValue,
        _operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        Err(TransitionError::Requires(requirement))
    }

    /// Fields actually consumed for this value. Provenance is projected over exactly these.
    fn consumed_fields(
        &self,
        owner: ProgrammingOwner,
        value: &AttributeValue,
    ) -> Result<ProgrammingFieldScope, TransitionError> {
        Ok(ProgrammingFieldScope::for_value(owner, value)?)
    }

    /// Engine metadata for the composed value. The default is an explicit unknown replacement:
    /// mixed sources stay in the owned provenance, and the baseline master is not reinterpreted.
    fn projection_metadata(
        &self,
        _owner: ProgrammingOwner,
        _provenance: &PhysicalProvenance,
    ) -> FamilyProjectionMetadata {
        FamilyProjectionMetadata {
            changed_at: None,
            evidence: FamilyProjectionEvidence::Replace {
                origin: None,
                family_evidence: None,
            },
        }
    }
}

/// The owned family addressed by a transition's endpoints. A Focus/Zoom adapter that owns both
/// scalar families disambiguates only through `ZoomConvention`; otherwise ambiguity stays a
/// passive requirement instead of choosing the first owner.
fn transition_owner<A: PhysicalFamilyAdapter + ?Sized>(
    adapter: &A,
    requirement: TransitionRequirement,
    from: &AttributeValue,
    to: &AttributeValue,
) -> Result<ProgrammingOwner, TransitionError> {
    let owners: Vec<_> = [
        ProgrammingOwner::Position,
        ProgrammingOwner::Color,
        ProgrammingOwner::Focus,
        ProgrammingOwner::Zoom,
    ]
    .into_iter()
    .filter(|owner| {
        adapter.owns(*owner)
            && ProgrammingFieldScope::for_value(*owner, from).is_ok()
            && ProgrammingFieldScope::for_value(*owner, to).is_ok()
    })
    .collect();
    match owners.as_slice() {
        [owner] => Ok(*owner),
        _ if requirement == TransitionRequirement::ZoomConvention
            && owners.contains(&ProgrammingOwner::Zoom) =>
        {
            Ok(ProgrammingOwner::Zoom)
        }
        _ => Err(TransitionError::Requires(requirement)),
    }
}

/// Passive requirement reported when a family has no destination model on a head.
pub(in crate::runtime) fn owner_requirement(owner: ProgrammingOwner) -> TransitionRequirement {
    match owner {
        ProgrammingOwner::Position => TransitionRequirement::LiveJointAngles,
        ProgrammingOwner::Color => TransitionRequirement::ColorAppearance,
        ProgrammingOwner::Zoom => TransitionRequirement::ZoomConvention,
        ProgrammingOwner::Focus => TransitionRequirement::CompatibleOwners,
    }
}

/// Owned source/control evidence for exactly the consumed fields.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct PhysicalProvenance {
    pub fields: ProgrammingFieldScope,
    pub sources: DynamicFamilySourceProjection,
    /// None when the composer has no controller trace for these fields.
    pub controls: Option<Vec<FamilyControlContribution>>,
}

/// One owned per-head result: complete native writes, requested vs achieved physical values,
/// passive quality, provenance and the captured token. This is the sidecar type `T` of
/// `PreparedHybridFrame<T>` / `PendingHybridBranch<T>`.
pub(in crate::runtime) struct PhysicalHeadResult<A: PhysicalFamilyAdapter + ?Sized> {
    pub token: CapturedFrameToken,
    pub target: FixtureId,
    pub owner: ProgrammingOwner,
    /// Composed requested family value, identical to what the engine token received.
    pub value: AttributeValue,
    pub writes: Vec<NativeControlWrite>,
    pub requested: A::Requested,
    pub achieved: A::Achieved,
    pub quality: A::Quality,
    pub provenance: PhysicalProvenance,
    pub metadata: FamilyProjectionMetadata,
}

/// Validate an adapter's writes against its declared footprint: complete, unique, no foreign.
pub(in crate::runtime) fn validate_complete_writes(
    footprint: &[NativeControlSlot],
    writes: &[NativeControlWrite],
) -> Result<(), TransitionError> {
    let invalid = |message: &str| Err(IntentError(message.into()).into());
    if writes.len() != footprint.len() {
        return invalid("physical adapter must write its complete native footprint once");
    }
    for (index, write) in writes.iter().enumerate() {
        if !footprint.contains(&write.slot) {
            return invalid("physical adapter wrote outside its native footprint");
        }
        if writes[..index].iter().any(|other| other.slot == write.slot) {
            return invalid("physical adapter wrote one native control twice");
        }
    }
    Ok(())
}

/// Observe one composed head through a lane. Use as the hybrid observer body:
/// `|observation| lane.observe(observation)`.
pub(in crate::runtime) fn observe_physical_family<A: PhysicalFamilyAdapter>(
    lane: &PhysicalAdapterLane<A>,
    observation: HybridFamilyObservation<'_>,
) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError> {
    lane.observe(observation)
}

/// A successfully finalized Live frame of one family lane. Only this value may be published.
pub(in crate::runtime) type PublishedPhysicalFrame<A> = PublishedLiveFrame<PhysicalHeadResult<A>>;

/// Finalize a prepared Live frame of one family lane through the existing engine finalizer
/// (see [`live_frame::finalize_live_lanes_frame`]): every check precedes the render, continuity
/// commits only after it succeeded, and any error abandons the lane's staged attempt.
pub(in crate::runtime::output_scheduler::dynamic_projection) fn finalize_live_physical_frame<
    A: PhysicalFamilyAdapter,
>(
    engine: &Engine,
    capture: &PreparedOutputFrame,
    lane: &PhysicalAdapterLane<A>,
    prepared: PreparedHybridFrame<PhysicalHeadResult<A>>,
) -> Result<PublishedPhysicalFrame<A>, DynamicRuntimeError> {
    live_frame::finalize_live_lanes_frame(engine, capture, lane, prepared)
}

impl<A: PhysicalFamilyAdapter> HybridFrameResolver for PhysicalAdapterLane<A> {
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        lane::adopt_in(self, frame, target, original, address)
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
        lane::resolve_in(self, frame, target, requirement, from, to, operation)
    }

    fn begin_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.begin(token)
    }

    fn hold_frame(
        &self,
        token: &CapturedFrameToken,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        self.hold(token, requirements)
    }

    fn verify_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.verify(token)
    }

    fn accept_frame(&self, token: &CapturedFrameToken) -> bool {
        self.accept(token)
    }
}
