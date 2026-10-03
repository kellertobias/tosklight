//! One body for observing, adopting and transitioning a head, over the lane itself or over a
//! parallel worker's view of it (TL-639 round 5).
//!
//! A frame's non-Position family groups are independent across targets: each reads only the
//! lane's descriptors and committed continuity, which no group of the frame changes, and stages
//! its own result. A worker therefore borrows those two maps read-only ([`LaneShared`]) and
//! stages into its own [`LaneStaging`]; the frame merges the stagings in group order
//! ([`PhysicalAdapterLane::merge_staging`]), which leaves the lane exactly as the
//! single-threaded loop would. A worker never compiles: a descriptor the lane has not cached for
//! this generation marks the worker `missed`, and the frame reruns that group on the lane.

use super::*;
use std::cell::Cell;

/// What observing, adopting and transitioning a head read and write on a lane.
pub(in crate::runtime::output_scheduler::dynamic_projection::physical_adapter) trait LaneAccess<
    A: PhysicalFamilyAdapter,
>
{
    fn adapter(&self) -> &A;

    fn descriptor(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Result<Arc<A::Descriptor>, TransitionError>;

    /// The last accepted continuity of `(target, owner)`, copied.
    fn continuity(&self, target: FixtureId, owner: ProgrammingOwner) -> Option<A::Continuity>;

    /// Read the last accepted continuity of `key` without copying it.
    fn with_previous<R>(&self, key: OwnerKey, read: impl FnOnce(Option<&A::Continuity>) -> R) -> R;

    #[allow(clippy::too_many_arguments)]
    fn stage(
        &self,
        token: &CapturedFrameToken,
        target: FixtureId,
        owner: ProgrammingOwner,
        value: AttributeValue,
        provenance: PhysicalProvenance,
        metadata: FamilyProjectionMetadata,
        resolution: PhysicalResolution<A>,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError>;
}

/// Resolve and stage one composed head.
pub(in crate::runtime::output_scheduler::dynamic_projection::physical_adapter) fn observe_in<
    A: PhysicalFamilyAdapter,
>(
    lane: &impl LaneAccess<A>,
    observation: HybridFamilyObservation<'_>,
) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError> {
    let frame = observation.frame;
    let (target, owner) = (observation.target, observation.owner);
    let descriptor = lane.descriptor(frame, target, owner)?;
    if !frame.token.matches_geometry(frame.geometry) {
        return invalid("physical adapter geometry belongs to another frame");
    }
    let adapter = lane.adapter();
    let fields =
        observation.consumed_fields(|| adapter.consumed_fields(owner, observation.value))?;
    let mut sources = DynamicFamilySourceProjection::default();
    observation.project_fields(&fields, &mut sources)?;
    let controls = observation.controls_for_fields(&fields);
    let provenance = PhysicalProvenance {
        fields,
        sources,
        controls,
    };
    // Borrowed, not cloned (TL-639 round 2): the adapter never reaches this lane's state.
    let resolution = lane.with_previous((target, owner), |previous| {
        adapter.resolve(PhysicalRequest {
            frame,
            target,
            owner,
            descriptor: &descriptor,
            value: observation.value,
            previous,
        })
    })?;
    validate_complete_writes(adapter.footprint(&descriptor), &resolution.writes)?;
    let metadata = adapter.projection_metadata(owner, &provenance);
    lane.stage(
        frame.token,
        target,
        owner,
        observation.value.clone(),
        provenance,
        metadata,
        resolution,
    )
}

/// First-edit / Current adoption through the lane's destination model.
pub(in crate::runtime::output_scheduler::dynamic_projection::physical_adapter) fn adopt_in<
    A: PhysicalFamilyAdapter,
>(
    lane: &impl LaneAccess<A>,
    frame: HybridFrameContext<'_>,
    target: FixtureId,
    original: &AttributeValue,
    address: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    let descriptor = lane.descriptor(frame, target, address.owner())?;
    let previous = lane.continuity(target, address.owner());
    lane.adapter().adopt_with_continuity(
        frame,
        &descriptor,
        target,
        original,
        address,
        previous.as_ref(),
    )
}

/// A cross-representation transition through the lane's destination model.
pub(in crate::runtime::output_scheduler::dynamic_projection::physical_adapter) fn resolve_in<
    A: PhysicalFamilyAdapter,
>(
    lane: &impl LaneAccess<A>,
    frame: HybridFrameContext<'_>,
    target: FixtureId,
    requirement: TransitionRequirement,
    from: &AttributeValue,
    to: &AttributeValue,
    operation: FamilyExpressionOperation,
) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
    let owner = transition_owner(lane.adapter(), requirement, from, to)?;
    let descriptor = lane.descriptor(frame, target, owner)?;
    lane.adapter()
        .transition(frame, &descriptor, target, requirement, from, to, operation)
}

impl<A: PhysicalFamilyAdapter> LaneAccess<A> for PhysicalAdapterLane<A> {
    fn adapter(&self) -> &A {
        &self.adapter
    }

    fn descriptor(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Result<Arc<A::Descriptor>, TransitionError> {
        PhysicalAdapterLane::descriptor(self, frame, target, owner)
    }

    fn continuity(&self, target: FixtureId, owner: ProgrammingOwner) -> Option<A::Continuity> {
        PhysicalAdapterLane::continuity(self, target, owner)
    }

    fn with_previous<R>(&self, key: OwnerKey, read: impl FnOnce(Option<&A::Continuity>) -> R) -> R {
        let state = self.state.borrow();
        read(state.committed.get(&key).map(|entry| &entry.continuity))
    }

    fn stage(
        &self,
        token: &CapturedFrameToken,
        target: FixtureId,
        owner: ProgrammingOwner,
        value: AttributeValue,
        provenance: PhysicalProvenance,
        metadata: FamilyProjectionMetadata,
        resolution: PhysicalResolution<A>,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError> {
        self.stage_resolution(
            token, target, owner, value, provenance, metadata, resolution,
        )
    }
}

/// The read-only part of one lane a parallel section's workers share: the adapter, the staged
/// token, this generation's descriptors and the committed continuity. Borrowed from the lane's
/// state for the whole section; nothing writes either map until the section ends.
/// A lane's cached descriptors.
type Descriptors<A> = FxHashMap<FixtureId, Option<Arc<<A as PhysicalFamilyAdapter>::Descriptor>>>;

pub(in crate::runtime) struct LaneShared<'l, A: PhysicalFamilyAdapter> {
    adapter: &'l A,
    token: Option<&'l CapturedFrameToken>,
    /// Descriptors with the generation they were compiled for.
    descriptors: (Option<u64>, &'l Descriptors<A>),
    committed: &'l FxHashMap<OwnerKey, Committed<A::Continuity>>,
    /// Heads staged before the section, for the duplicate check.
    observed: Option<&'l FxHashSet<OwnerKey>>,
}

/// One worker's staged heads, in the order it staged them.
pub(in crate::runtime) struct LaneStaging<C> {
    entries: Vec<(OwnerKey, C, PhysicalProvenance)>,
    writes: Vec<NativeControlWrite>,
    observed: Vec<OwnerKey>,
}

impl<C> Default for LaneStaging<C> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            writes: Vec::new(),
            observed: Vec::new(),
        }
    }
}

/// How far a staging had grown; [`LaneStaging::truncate`] returns it there.
#[derive(Clone, Copy)]
pub(in crate::runtime) struct StagingMark(usize, usize, usize);

impl<C> LaneStaging<C> {
    pub fn mark(&self) -> StagingMark {
        StagingMark(self.entries.len(), self.writes.len(), self.observed.len())
    }

    /// Drop everything staged after `mark` (a group the frame reruns on the lane).
    pub fn truncate(&mut self, mark: StagingMark) {
        self.entries.truncate(mark.0);
        self.writes.truncate(mark.1);
        self.observed.truncate(mark.2);
    }

    pub fn is_empty(&self) -> bool {
        self.observed.is_empty()
    }
}

/// One worker's view of a lane: the shared read-only state and its own staging.
pub(in crate::runtime) struct LaneWorker<'s, 'l, A: PhysicalFamilyAdapter> {
    shared: &'s LaneShared<'l, A>,
    staging: RefCell<LaneStaging<A::Continuity>>,
    missed: &'s Cell<bool>,
}

impl<A: PhysicalFamilyAdapter> PhysicalAdapterLane<A> {
    /// Lend this lane's read-only frame state to a parallel section.
    pub(in crate::runtime) fn with_shared<R>(
        &self,
        run: impl FnOnce(&LaneShared<'_, A>) -> R,
    ) -> R {
        let state = self.state.borrow();
        let shared = LaneShared {
            adapter: &self.adapter,
            token: state.staged.as_ref().map(|staged| &staged.token),
            descriptors: (state.descriptors.generation, &state.descriptors.entries),
            committed: &state.committed,
            observed: state.staged.as_ref().map(|staged| &staged.observed),
        };
        run(&shared)
    }

    /// Stage a worker's heads as if they had been staged here, in order: the same token and
    /// duplicate checks, the same entries and writes.
    /// `staging` is left empty, with its room, for a later section.
    pub(in crate::runtime) fn merge_staging(
        &self,
        token: &CapturedFrameToken,
        staging: &mut LaneStaging<A::Continuity>,
    ) -> Result<(), TransitionError> {
        if staging.is_empty() {
            return Ok(());
        }
        let mut state = self.state.borrow_mut();
        let Some(staged) = state
            .staged
            .as_mut()
            .filter(|staged| staged.token == *token)
        else {
            return invalid("physical adapter result uses a mixed or stale frame token");
        };
        for key in staging.observed.drain(..) {
            if !staged.observed.insert(key) {
                return invalid("physical adapter resolved one head owner twice in a frame");
            }
        }
        staged.entries.append(&mut staging.entries);
        staged.writes.append(&mut staging.writes);
        Ok(())
    }
}

impl<'s, 'l, A: PhysicalFamilyAdapter> LaneWorker<'s, 'l, A> {
    /// A worker staging into `staging` (an emptied staging of an earlier frame keeps its room).
    pub fn new(
        shared: &'s LaneShared<'l, A>,
        missed: &'s Cell<bool>,
        staging: LaneStaging<A::Continuity>,
    ) -> Self {
        Self {
            shared,
            staging: RefCell::new(staging),
            missed,
        }
    }

    pub fn staging(&self) -> std::cell::RefMut<'_, LaneStaging<A::Continuity>> {
        self.staging.borrow_mut()
    }

    pub fn into_staging(self) -> LaneStaging<A::Continuity> {
        self.staging.into_inner()
    }

    pub fn observe(
        &self,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError> {
        observe_in(self, observation)
    }

    pub fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        adopt_in(self, frame, target, original, address)
    }

    pub fn resolve(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        resolve_in(self, frame, target, requirement, from, to, operation)
    }

    fn miss(&self) -> TransitionError {
        self.missed.set(true);
        IntentError("a parallel worker needs a descriptor its lane has not compiled".into()).into()
    }
}

impl<A: PhysicalFamilyAdapter> LaneAccess<A> for LaneWorker<'_, '_, A> {
    fn adapter(&self) -> &A {
        self.shared.adapter
    }

    /// The lane's `descriptor` without the compile: the same checks in the same order, and a
    /// miss wherever the lane would compile.
    fn descriptor(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Result<Arc<A::Descriptor>, TransitionError> {
        if !self.shared.adapter.owns(owner) {
            return Err(TransitionError::Requires(owner_requirement(owner)));
        }
        if self.shared.token != Some(frame.token) {
            return invalid("physical adapter lookup uses a mixed or stale frame token");
        }
        let generation = frame.token.generation();
        if frame.capture.generation() != generation {
            return invalid("physical adapter frame token does not match its capture generation");
        }
        let (cached, entries) = self.shared.descriptors;
        if cached != Some(generation) {
            return Err(self.miss());
        }
        match entries.get(&target) {
            Some(compiled) => compiled
                .clone()
                .ok_or(TransitionError::Requires(owner_requirement(owner))),
            None => Err(self.miss()),
        }
    }

    fn continuity(&self, target: FixtureId, owner: ProgrammingOwner) -> Option<A::Continuity> {
        self.shared
            .committed
            .get(&(target, owner))
            .map(|entry| entry.continuity.clone())
    }

    fn with_previous<R>(&self, key: OwnerKey, read: impl FnOnce(Option<&A::Continuity>) -> R) -> R {
        read(
            self.shared
                .committed
                .get(&key)
                .map(|entry| &entry.continuity),
        )
    }

    fn stage(
        &self,
        token: &CapturedFrameToken,
        target: FixtureId,
        owner: ProgrammingOwner,
        value: AttributeValue,
        provenance: PhysicalProvenance,
        metadata: FamilyProjectionMetadata,
        resolution: PhysicalResolution<A>,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError> {
        if self.shared.token != Some(token) {
            return invalid("physical adapter result uses a mixed or stale frame token");
        }
        let key = (target, owner);
        let mut staging = self.staging.borrow_mut();
        // A worker stages its targets in group order, so an earlier head of this target is at
        // the tail; the merge checks the lane's complete set again.
        if self
            .shared
            .observed
            .is_some_and(|observed| observed.contains(&key))
            || staging
                .observed
                .iter()
                .rev()
                .take_while(|staged| staged.0 == target)
                .any(|staged| *staged == key)
        {
            return invalid("physical adapter resolved one head owner twice in a frame");
        }
        staging.observed.push(key);
        staging
            .entries
            .push((key, resolution.continuity, provenance.clone()));
        staging.writes.extend_from_slice(&resolution.writes);
        Ok((
            metadata.clone(),
            PhysicalHeadResult {
                token: token.clone(),
                target,
                owner,
                value,
                writes: resolution.writes,
                requested: resolution.requested,
                achieved: resolution.achieved,
                quality: resolution.quality,
                provenance,
                metadata,
            },
        ))
    }
}
