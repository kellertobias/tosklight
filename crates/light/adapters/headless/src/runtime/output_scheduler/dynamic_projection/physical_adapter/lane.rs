//! Lane-owned physical adapter state: generation-bound destination descriptors, staged and
//! committed continuity. One lane belongs to exactly one evaluating lane (Live, Preload Before
//! Release or Preload After Release). Nothing is shared between lanes.

use super::*;
use light_engine::PreloadBranch;
use rustc_hash::FxHashSet;
use std::cell::RefCell;

mod worker;
pub(in crate::runtime) use worker::{LaneShared, LaneStaging, LaneWorker, StagingMark};
pub(super) use worker::{adopt_in, resolve_in};

type OwnerKey = (FixtureId, ProgrammingOwner);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PhysicalLaneKind {
    Live,
    Preload(PreloadBranch),
}

impl PhysicalLaneKind {
    fn accepts(self, token: &CapturedFrameToken) -> bool {
        match self {
            Self::Live => {
                token.lane().preload_branch().is_none()
                    && *token.lane() == light_engine::CapturedFrameLane::Live
            }
            Self::Preload(branch) => token.lane().preload_branch() == Some(branch),
        }
    }
}

/// An owner this lane published in its previous accepted frame but not in the latest one.
/// It names the source evidence that last owned the physical output, so a consumer can tell
/// a source Release from an unknown drop.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct ReleasedPhysicalOwner {
    pub target: FixtureId,
    pub owner: ProgrammingOwner,
    pub last_token: CapturedFrameToken,
    pub last_provenance: PhysicalProvenance,
}

struct Committed<C> {
    continuity: C,
    provenance: PhysicalProvenance,
    token: CapturedFrameToken,
    /// The accept that last produced or held this entry; anything older is released.
    stamp: u64,
}

struct Staged<C> {
    token: CapturedFrameToken,
    entries: Vec<(OwnerKey, C, PhysicalProvenance)>,
    writes: Vec<NativeControlWrite>,
    held: FxHashSet<OwnerKey>,
    observed: FxHashSet<OwnerKey>,
}

/// Descriptors compiled against exactly one runtime generation.
struct DestinationCache<D> {
    generation: Option<u64>,
    entries: FxHashMap<FixtureId, Option<Arc<D>>>,
}

struct LaneState<A: PhysicalFamilyAdapter> {
    descriptors: DestinationCache<A::Descriptor>,
    committed: FxHashMap<OwnerKey, Committed<A::Continuity>>,
    staged: Option<Staged<A::Continuity>>,
    last_accepted: Option<CapturedFrameToken>,
    released: Vec<ReleasedPhysicalOwner>,
    /// Reused by `verify`: the first raw value staged for each shared native slot.
    shared_slots: FxHashMap<NativeControlSlot, u32>,
    /// Number of accepted frames; stamps the committed entries each accept keeps.
    accepts: u64,
    /// The last accepted frame's staging storage, emptied, for the next frame (TL-639 round 4).
    spare: Option<Staged<A::Continuity>>,
    /// Entries the last accept replaced, until [`PhysicalAdapterLane::retire_on`] frees them on
    /// a pool thread (TL-639 round 5); the next accept frees any nobody took. Kept only once a
    /// pool has taken them (`retiring`); otherwise the accept frees them in place.
    retired: Vec<Committed<A::Continuity>>,
    retiring: bool,
}

/// One lane of one family adapter. Interior mutability lets the same lane serve as the hybrid
/// resolver and the observer of one synchronous frame; it is not shared across threads.
///
/// Lifecycle per frame: `begin` (via `begin_frame`) → descriptor lookups/`observe` stage
/// continuity → `verify` → engine finalizer → `accept`. A failure at any point leaves the
/// committed state unchanged; the next `begin` or `abandon` discards the staged attempt.
pub(in crate::runtime) struct PhysicalAdapterLane<A: PhysicalFamilyAdapter> {
    adapter: A,
    kind: PhysicalLaneKind,
    state: RefCell<LaneState<A>>,
}

fn invalid<T>(message: &str) -> Result<T, TransitionError> {
    Err(IntentError(message.into()).into())
}

impl<A: PhysicalFamilyAdapter> PhysicalAdapterLane<A> {
    pub fn live(adapter: A) -> Self {
        Self::new(adapter, PhysicalLaneKind::Live)
    }

    pub fn preload(adapter: A, branch: PreloadBranch) -> Self {
        Self::new(adapter, PhysicalLaneKind::Preload(branch))
    }

    fn new(adapter: A, kind: PhysicalLaneKind) -> Self {
        Self {
            adapter,
            kind,
            state: RefCell::new(LaneState {
                descriptors: DestinationCache {
                    generation: None,
                    entries: FxHashMap::default(),
                },
                committed: FxHashMap::default(),
                staged: None,
                last_accepted: None,
                released: Vec::new(),
                shared_slots: FxHashMap::default(),
                accepts: 0,
                spare: None,
                retired: Vec::new(),
                retiring: false,
            }),
        }
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub fn kind(&self) -> PhysicalLaneKind {
        self.kind
    }

    /// Last committed continuity of one head/owner. Diagnostics and tests only.
    pub fn continuity(&self, target: FixtureId, owner: ProgrammingOwner) -> Option<A::Continuity> {
        let state = self.state.borrow();
        state
            .committed
            .get(&(target, owner))
            .map(|entry| entry.continuity.clone())
    }

    pub fn last_accepted(&self) -> Option<CapturedFrameToken> {
        self.state.borrow().last_accepted.clone()
    }

    /// Owners released by the most recently accepted frame.
    pub fn released(&self) -> Vec<ReleasedPhysicalOwner> {
        self.state.borrow().released.clone()
    }

    /// Descriptor generation currently cached, if any.
    pub fn descriptor_generation(&self) -> Option<u64> {
        self.state.borrow().descriptors.generation
    }

    pub(super) fn begin(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        if !self.kind.accepts(token) {
            return invalid("physical adapter token belongs to another lane");
        }
        self.adapter.abandon_lane_frame();
        let mut state = self.state.borrow_mut();
        state.staged = None;
        if let Some(last) = &state.last_accepted {
            if last == token {
                return invalid("physical adapter frame was already accepted");
            }
            if token.sampled_at() < last.sampled_at() {
                return invalid("physical adapter frame is older than the accepted frame");
            }
        }
        self.adapter.begin_lane_frame(token)?;
        state.staged = Some(match state.spare.take() {
            Some(mut spare) => {
                spare.token = token.clone();
                spare
            }
            None => Staged {
                token: token.clone(),
                entries: Vec::new(),
                writes: Vec::new(),
                held: FxHashSet::default(),
                observed: FxHashSet::default(),
            },
        });
        Ok(())
    }

    /// Discard a staged attempt without touching committed continuity.
    pub fn abandon(&self) {
        self.state.borrow_mut().staged = None;
        self.adapter.abandon_lane_frame();
    }

    /// Whether exactly `token` is staged. A lane composite checks every lane before it commits
    /// any of them (TL-548 C1).
    pub(super) fn stages(&self, token: &CapturedFrameToken) -> bool {
        Self::staged_token_matches(&self.state.borrow(), token)
    }

    fn staged_token_matches(state: &LaneState<A>, token: &CapturedFrameToken) -> bool {
        state
            .staged
            .as_ref()
            .is_some_and(|staged| staged.token == *token)
    }

    /// Destination lookup against the frame's own captured generation. Any generation change
    /// discards every cached descriptor before the first lookup of the new frame.
    pub(super) fn descriptor(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Result<Arc<A::Descriptor>, TransitionError> {
        if !self.adapter.owns(owner) {
            return Err(TransitionError::Requires(owner_requirement(owner)));
        }
        let mut state = self.state.borrow_mut();
        if !Self::staged_token_matches(&state, frame.token) {
            return invalid("physical adapter lookup uses a mixed or stale frame token");
        }
        let generation = frame.token.generation();
        if frame.capture.generation() != generation {
            return invalid("physical adapter frame token does not match its capture generation");
        }
        let cache = &mut state.descriptors;
        if cache.generation != Some(generation) {
            cache.entries.clear();
            cache.generation = Some(generation);
        }
        let compiled = match cache.entries.entry(target) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(
                self.adapter
                    .compile(&frame.capture.snapshot(), target)?
                    .map(Arc::new),
            ),
        };
        compiled
            .clone()
            .ok_or(TransitionError::Requires(owner_requirement(owner)))
    }

    /// Resolve and stage one composed head. Returns the engine metadata and the owned sidecar.
    pub fn observe(
        &self,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError> {
        worker::observe_in(self, observation)
    }

    /// Stage one result only after its complete physical cohort has been fitted.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn stage_resolution(
        &self,
        token: &CapturedFrameToken,
        target: FixtureId,
        owner: ProgrammingOwner,
        value: AttributeValue,
        provenance: PhysicalProvenance,
        metadata: FamilyProjectionMetadata,
        resolution: PhysicalResolution<A>,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError> {
        self.stage_result(
            token, target, owner, value, provenance, metadata, resolution, true,
        )
    }

    /// A protected mechanical peer may publish a parked diagnostic row, but cannot replace
    /// its last accepted continuity, source evidence or owner token. Never creates a prior
    /// entry for a peer that has not yet produced a complete physical result.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn stage_held_resolution(
        &self,
        token: &CapturedFrameToken,
        target: FixtureId,
        owner: ProgrammingOwner,
        value: AttributeValue,
        provenance: PhysicalProvenance,
        metadata: FamilyProjectionMetadata,
        resolution: PhysicalResolution<A>,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError> {
        if resolution.writes.iter().any(|write| !write.parked) {
            return invalid("held physical adapter result contains an active native write");
        }
        self.stage_result(
            token, target, owner, value, provenance, metadata, resolution, false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn stage_result(
        &self,
        token: &CapturedFrameToken,
        target: FixtureId,
        owner: ProgrammingOwner,
        value: AttributeValue,
        provenance: PhysicalProvenance,
        metadata: FamilyProjectionMetadata,
        resolution: PhysicalResolution<A>,
        accept_continuity: bool,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError> {
        let mut state = self.state.borrow_mut();
        let Some(staged) = state
            .staged
            .as_mut()
            .filter(|staged| staged.token == *token)
        else {
            return invalid("physical adapter result uses a mixed or stale frame token");
        };
        if !staged.observed.insert((target, owner)) {
            return invalid("physical adapter resolved one head owner twice in a frame");
        }
        if accept_continuity {
            staged
                .entries
                .push(((target, owner), resolution.continuity, provenance.clone()));
        } else {
            staged.held.insert((target, owner));
        }
        staged.writes.extend_from_slice(&resolution.writes);
        Ok((
            metadata.clone(),
            PhysicalHeadResult {
                token: token.clone(),
                target,
                owner,
                value: value,
                writes: resolution.writes,
                requested: resolution.requested,
                achieved: resolution.achieved,
                quality: resolution.quality,
                provenance,
                metadata,
            },
        ))
    }

    /// Mark unresolved owners in this attempt. This does not change their accepted token,
    /// provenance or continuity, and creates no entry for an owner that was never resolved.
    pub(super) fn hold(
        &self,
        token: &CapturedFrameToken,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        let mut state = self.state.borrow_mut();
        let Some(staged) = state
            .staged
            .as_mut()
            .filter(|staged| staged.token == *token)
        else {
            return invalid("physical adapter hold uses a mixed or stale frame token");
        };
        staged.held.extend(
            requirements
                .iter()
                .filter(|row| self.adapter.owns(row.owner))
                .map(|row| (row.target, row.owner)),
        );
        Ok(())
    }

    /// Last fallible check: the staged frame is exactly `token` and its heads agree on every
    /// shared native control. Different raw values for one slot are an ownership conflict.
    pub(super) fn verify(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        {
            let mut state = self.state.borrow_mut();
            let LaneState {
                staged,
                shared_slots,
                ..
            } = &mut *state;
            let Some(staged) = staged.as_ref().filter(|staged| staged.token == *token) else {
                return invalid("physical adapter verification uses a mixed or stale frame token");
            };
            // TL-596: one pass over the staged writes. Comparing every write with every earlier
            // one was quadratic in the frame's write count (about 27 ms per frame for 2,520
            // Color heads). Any disagreement on a slot differs from that slot's first value.
            shared_slots.clear();
            for write in &staged.writes {
                if *shared_slots.entry(write.slot).or_insert(write.raw) != write.raw {
                    shared_slots.clear();
                    return invalid("physical adapter heads disagree on a shared native control");
                }
            }
            shared_slots.clear();
        }
        self.adapter.verify_lane_frame(token)
    }

    /// Infallible commit after the finalizer accepted `token`. Produced results win over
    /// incidental holds. Held owners keep their last accepted entry; only genuine removals
    /// release with their last provenance and retire continuity.
    pub(super) fn accept(&self, token: &CapturedFrameToken) -> bool {
        let mut state = self.state.borrow_mut();
        if !Self::staged_token_matches(&state, token) {
            return false;
        }
        let mut staged = state.staged.take().expect("matched staged frame");
        // TL-639 round 2: updated in place. Produced entries replace theirs, held owners keep
        // theirs, and every entry neither touched is released with its last provenance.
        state.accepts += 1;
        let stamp = state.accepts;
        let LaneState {
            committed,
            retired,
            retiring,
            ..
        } = &mut *state;
        retired.clear();
        if *retiring {
            retired.reserve(staged.entries.len());
        }
        for (key, continuity, provenance) in staged.entries.drain(..) {
            let replaced = committed.insert(
                key,
                Committed {
                    continuity,
                    provenance,
                    token: staged.token.clone(),
                    stamp,
                },
            );
            if *retiring {
                retired.extend(replaced);
            }
        }
        for key in staged.held.drain() {
            if let Some(entry) = state.committed.get_mut(&key) {
                entry.stamp = stamp;
            }
        }
        let LaneState {
            committed,
            released,
            ..
        } = &mut *state;
        released.clear();
        released.extend(committed.extract_if(|_, entry| entry.stamp != stamp).map(
            |((target, owner), entry)| ReleasedPhysicalOwner {
                target,
                owner,
                last_token: entry.token,
                last_provenance: entry.provenance,
            },
        ));
        staged.writes.clear();
        staged.observed.clear();
        state.last_accepted = Some(staged.token.clone());
        state.spare = Some(staged);
        self.adapter.accept_lane_frame(token);
        true
    }
}

impl<A: PhysicalFamilyAdapter> PhysicalAdapterLane<A>
where
    A::Continuity: Send + 'static,
{
    /// Free the entries the last accept replaced on `pool`, off the frame's thread.
    pub(in crate::runtime) fn retire_on(&self, pool: &light_engine::parallel::OutputPool) {
        let mut state = self.state.borrow_mut();
        state.retiring = true;
        let retired = std::mem::take(&mut state.retired);
        drop(state);
        if !retired.is_empty() {
            pool.drop_later(retired);
        }
    }
}

/// Independent Before/After Release lanes for one retained Preload episode. Use `&lanes` as the
/// `RetainedPreloadHybridEvaluator` resolver and `|branch, o| lanes.observe(branch, o)` as its
/// observer. Neither lane ever reads Live state or the other branch's continuity.
pub(in crate::runtime) struct PhysicalPreloadLanes<A: PhysicalFamilyAdapter> {
    before: PhysicalAdapterLane<A>,
    after: PhysicalAdapterLane<A>,
}

impl<A: PhysicalFamilyAdapter> PhysicalPreloadLanes<A> {
    pub fn new(before: A, after: A) -> Self {
        Self {
            before: PhysicalAdapterLane::preload(before, PreloadBranch::BeforeRelease),
            after: PhysicalAdapterLane::preload(after, PreloadBranch::AfterRelease),
        }
    }

    pub fn lane(&self, branch: PreloadBranch) -> &PhysicalAdapterLane<A> {
        match branch {
            PreloadBranch::BeforeRelease => &self.before,
            PreloadBranch::AfterRelease => &self.after,
        }
    }

    fn lane_for(
        &self,
        token: &CapturedFrameToken,
    ) -> Result<&PhysicalAdapterLane<A>, TransitionError> {
        match token.lane().preload_branch() {
            Some(branch) => Ok(self.lane(branch)),
            None => invalid("a Live token cannot address a Preload physical lane"),
        }
    }

    pub fn observe(
        &self,
        branch: PreloadBranch,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<A>), TransitionError> {
        if observation.frame.token.lane().preload_branch() != Some(branch) {
            return invalid("Preload observation token belongs to another branch");
        }
        self.lane(branch).observe(observation)
    }
}

impl<A: PhysicalFamilyAdapter> HybridFrameResolver for PhysicalPreloadLanes<A> {
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        self.lane_for(frame.token)?
            .adopt(frame, target, original, address)
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
        self.lane_for(frame.token)?
            .resolve(frame, target, requirement, from, to, operation)
    }

    fn begin_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.lane_for(token)?.begin(token)
    }

    fn hold_frame(
        &self,
        token: &CapturedFrameToken,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        self.lane_for(token)?.hold(token, requirements)
    }

    fn verify_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.lane_for(token)?.verify(token)
    }

    fn accept_frame(&self, token: &CapturedFrameToken) -> bool {
        self.lane_for(token).is_ok_and(|lane| lane.accept(token))
    }
}
