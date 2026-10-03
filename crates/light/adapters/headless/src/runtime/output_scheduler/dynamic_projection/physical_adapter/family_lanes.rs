//! TL-548 C1: every physical family of one evaluating lane behind ONE hybrid resolver, observer
//! and Live finalizer, so Position, Color (lamp and Media) and Focus/Zoom are produced from the
//! same captured frame and commit or abandon together.
//!
//! - [`FamilyLanes`] owns one Position lane, one routed Color lane and one Focus/Zoom pair of
//!   exactly one evaluating lane (Live, Preload Before Release or Preload After Release). The
//!   lanes stay separate: no lane reads another lane's continuity, descriptors or counters.
//! - Routing. Adoption routes by the address owner (`DynamicValueAddress::owner`, derived from
//!   its representation). A transition carries no owner: [`transition_family`] derives it from
//!   the requirement AND both endpoint variants, so only `ZoomConvention` between two Zoom
//!   values reaches optics. Focus never needs a frame transition (scalars interpolate without
//!   one), so a `Normalized` pair is never routed to optics. Anything else stays a passive
//!   `Requires` with the original requirement.
//! - Lifecycle. `begin`, `hold` and `verify` reach every lane in a fixed order (Position, Color,
//!   optics). A rejection by the first lane changes nothing (a foreign token cannot discard
//!   another attempt); a later rejection abandons every lane. `accept` commits only when every
//!   lane staged exactly that token, so the set never commits half a frame.
//! - Holds stay family-local: each lane keeps only requirement rows of the owners it owns, so a
//!   Zoom hold never holds Color or Position.
//!
//! Nothing here has a production caller: C3 wires it behind a separate opt-in.
use super::color_router::RoutingColorAdapter;
use super::lane::PhysicalLaneKind;
use super::live_frame::{
    LiveFrameLanes, LiveFrameSidecar, PublishedLiveFrame, finalize_live_lanes_frame,
};
use super::position::PositionAdapter;
use super::*;
use light_engine::PreloadBranch;

mod bridge;
mod native;
mod observer;
#[cfg(test)]
pub(in crate::runtime) mod tests;
#[allow(unused_imports)]
pub(in crate::runtime) use observer::{FamilyFrameObserver, FamilyPreloadObserver};

pub(in crate::runtime) type PositionSidecar = PhysicalHeadResult<PositionAdapter>;
pub(in crate::runtime) type ColorSidecar = PhysicalHeadResult<RoutingColorAdapter>;
pub(in crate::runtime) type OpticsSidecar = PhysicalHeadResult<OpticsAdapter>;
/// A finalized all-family Live frame. Only this value may be published.
pub(in crate::runtime) type PublishedFamilyFrame = PublishedLiveFrame<FamilySidecar>;

/// One owned per-head result of whichever family produced it. Fully owned like its payload.
/// Boxed (TL-639 round 2): a row moves through the cohort, its finish and its projection
/// several times per frame, and an inline result is several hundred bytes.
pub(in crate::runtime) enum FamilySidecar {
    Position(Box<PositionSidecar>),
    Color(Box<ColorSidecar>),
    Optics(Box<OpticsSidecar>),
}

macro_rules! each_sidecar {
    ($self:expr, $row:ident => $body:expr) => {
        match $self {
            FamilySidecar::Position($row) => $body,
            FamilySidecar::Color($row) => $body,
            FamilySidecar::Optics($row) => $body,
        }
    };
}

impl FamilySidecar {
    pub fn token(&self) -> &CapturedFrameToken {
        each_sidecar!(self, row => &row.token)
    }

    pub fn target(&self) -> FixtureId {
        each_sidecar!(self, row => row.target)
    }

    pub fn owner(&self) -> ProgrammingOwner {
        each_sidecar!(self, row => row.owner)
    }

    /// Composed requested value, identical to what the engine token received.
    pub fn value(&self) -> &AttributeValue {
        each_sidecar!(self, row => &row.value)
    }

    pub fn writes(&self) -> &[NativeControlWrite] {
        each_sidecar!(self, row => &row.writes)
    }

    pub fn provenance(&self) -> &PhysicalProvenance {
        each_sidecar!(self, row => &row.provenance)
    }

    pub fn position(&self) -> Option<&PositionSidecar> {
        match self {
            Self::Position(row) => Some(row.as_ref()),
            _ => None,
        }
    }

    pub fn color(&self) -> Option<&ColorSidecar> {
        match self {
            Self::Color(row) => Some(row.as_ref()),
            _ => None,
        }
    }

    pub fn optics(&self) -> Option<&OpticsSidecar> {
        match self {
            Self::Optics(row) => Some(row.as_ref()),
            _ => None,
        }
    }
}

impl LiveFrameSidecar for FamilySidecar {
    fn frame_token(&self) -> &CapturedFrameToken {
        self.token()
    }
}

/// The lane of a [`FamilyLanes`] set that owns a programming owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PhysicalFamily {
    Position,
    Color,
    /// Focus and Zoom: one [`OpticsLanes`] pair routes them to their own lanes.
    Optics,
}

pub(in crate::runtime) fn owner_family(owner: ProgrammingOwner) -> PhysicalFamily {
    match owner {
        ProgrammingOwner::Position => PhysicalFamily::Position,
        ProgrammingOwner::Color => PhysicalFamily::Color,
        ProgrammingOwner::Focus | ProgrammingOwner::Zoom => PhysicalFamily::Optics,
    }
}

/// The family that may solve a frame transition. `TransitionRequirement` names no owner, and the
/// value variants alone are ambiguous (`Normalized` is Focus as well as any legacy scalar), so
/// both must agree. Only these requirements reach a frame resolver (`light_dynamics`
/// endpoint/expression/activation composition): the two Position ones, `ColorAppearance` and
/// `ZoomConvention`. `NativeColorModel` is accepted for Color endpoints for completeness.
pub(in crate::runtime) fn transition_family(
    requirement: TransitionRequirement,
    from: &AttributeValue,
    to: &AttributeValue,
) -> Option<PhysicalFamily> {
    use AttributeValue as V;
    use TransitionRequirement as R;
    let color = |value: &AttributeValue| matches!(value, V::ColorProgram(_) | V::ColorXyz(_));
    match (requirement, from, to) {
        (R::LiveTargetPoints | R::LiveJointAngles, V::Position(_), V::Position(_)) => {
            Some(PhysicalFamily::Position)
        }
        (R::ColorAppearance | R::NativeColorModel, from, to) if color(from) && color(to) => {
            Some(PhysicalFamily::Color)
        }
        (R::ZoomConvention, V::Zoom(_), V::Zoom(_)) => Some(PhysicalFamily::Optics),
        _ => None,
    }
}

/// Every physical family lane of exactly one evaluating lane.
pub(in crate::runtime) struct FamilyLanes {
    position: PhysicalAdapterLane<PositionAdapter>,
    color: PhysicalAdapterLane<RoutingColorAdapter>,
    optics: OpticsLanes,
    /// The native write list being built and the last installation, kept so an unchanged
    /// collection installs again by reference (TL-639 round 4).
    native: std::cell::RefCell<(
        Vec<light_engine::FamilyNativeWrite>,
        light_engine::FamilyNativeMemo,
    )>,
}

fn lane<A: PhysicalFamilyAdapter>(adapter: A, kind: PhysicalLaneKind) -> PhysicalAdapterLane<A> {
    match kind {
        PhysicalLaneKind::Live => PhysicalAdapterLane::live(adapter),
        PhysicalLaneKind::Preload(branch) => PhysicalAdapterLane::preload(adapter, branch),
    }
}

impl FamilyLanes {
    fn with(kind: PhysicalLaneKind) -> Self {
        Self {
            position: lane(PositionAdapter::default(), kind),
            color: lane(RoutingColorAdapter::default(), kind),
            optics: match kind {
                PhysicalLaneKind::Live => OpticsLanes::live(),
                PhysicalLaneKind::Preload(branch) => OpticsLanes::preload(branch),
            },
            native: Default::default(),
        }
    }

    pub fn live() -> Self {
        Self::with(PhysicalLaneKind::Live)
    }

    pub fn preload(branch: PreloadBranch) -> Self {
        Self::with(PhysicalLaneKind::Preload(branch))
    }

    pub fn kind(&self) -> PhysicalLaneKind {
        self.color.kind()
    }

    pub fn position(&self) -> &PhysicalAdapterLane<PositionAdapter> {
        &self.position
    }

    pub fn color(&self) -> &PhysicalAdapterLane<RoutingColorAdapter> {
        &self.color
    }

    pub fn optics(&self) -> &OpticsLanes {
        &self.optics
    }

    /// Owners released by the most recently accepted frame: Position, Color, Focus, Zoom.
    pub fn released(&self) -> Vec<ReleasedPhysicalOwner> {
        let mut released = self.position.released();
        released.extend(self.color.released());
        released.extend(self.optics.released());
        released
    }

    /// Discard every lane's staged attempt; committed continuity is unchanged.
    pub fn abandon(&self) {
        self.position.abandon();
        self.color.abandon();
        self.optics.abandon();
    }

    /// Last accepted token of each lane: Position, Color, Focus, Zoom.
    pub fn last_accepted(&self) -> [Option<CapturedFrameToken>; 4] {
        [
            self.position.last_accepted(),
            self.color.last_accepted(),
            self.optics
                .lane(light_fixture::OpticsFamily::Focus)
                .last_accepted(),
            self.optics
                .lane(light_fixture::OpticsFamily::Zoom)
                .last_accepted(),
        ]
    }

    fn stages(&self, token: &CapturedFrameToken) -> bool {
        self.position.stages(token)
            && self.color.stages(token)
            && [
                light_fixture::OpticsFamily::Focus,
                light_fixture::OpticsFamily::Zoom,
            ]
            .into_iter()
            .all(|family| self.optics.lane(family).stages(token))
    }

    /// Observe one composed head in its own family lane. The Position lane resolves one head
    /// at a time here; complete Position cohorts go through [`FamilyFrameObserver`].
    pub fn observe(
        &self,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, FamilySidecar), TransitionError> {
        match owner_family(observation.owner) {
            PhysicalFamily::Position => self
                .position
                .observe(observation)
                .map(|(metadata, row)| (metadata, FamilySidecar::Position(Box::new(row)))),
            PhysicalFamily::Color => self
                .color
                .observe(observation)
                .map(|(metadata, row)| (metadata, FamilySidecar::Color(Box::new(row)))),
            PhysicalFamily::Optics => self
                .optics
                .observe(observation)
                .map(|(metadata, row)| (metadata, FamilySidecar::Optics(Box::new(row)))),
        }
    }

    /// Run one lifecycle step on every lane in order. The first lane's rejection changes
    /// nothing; a later rejection abandons every lane so no partial attempt survives.
    fn each(
        &self,
        step: impl Fn(&dyn HybridFrameResolver) -> Result<(), TransitionError>,
    ) -> Result<(), TransitionError> {
        step(&self.position)?;
        step(&self.color)
            .and_then(|()| step(&self.optics))
            .inspect_err(|_| self.abandon())
    }
}

impl HybridFrameResolver for FamilyLanes {
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        match owner_family(address.owner()) {
            PhysicalFamily::Position => self.position.adopt(frame, target, original, address),
            PhysicalFamily::Color => self.color.adopt(frame, target, original, address),
            PhysicalFamily::Optics => self.optics.adopt(frame, target, original, address),
        }
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
        let lane: &dyn HybridFrameResolver = match transition_family(requirement, from, to) {
            Some(PhysicalFamily::Position) => &self.position,
            Some(PhysicalFamily::Color) => &self.color,
            Some(PhysicalFamily::Optics) => &self.optics,
            None => return Err(TransitionError::Requires(requirement)),
        };
        lane.resolve(frame, target, requirement, from, to, operation)
    }

    fn begin_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.each(|lane| lane.begin_frame(token))
    }

    /// Each lane keeps only the rows of owners it owns (TL-602/TL-598), so a passive Zoom
    /// requirement holds Zoom only.
    fn hold_frame(
        &self,
        token: &CapturedFrameToken,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        self.each(|lane| lane.hold_frame(token, requirements))
    }

    fn verify_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.each(|lane| lane.verify_frame(token))
    }

    fn accept_frame(&self, token: &CapturedFrameToken) -> bool {
        // Infallible after verify; still commit nothing unless every lane staged this token.
        if !self.stages(token) {
            return false;
        }
        let position = self.position.accept_frame(token);
        let color = self.color.accept_frame(token);
        let optics = self.optics.accept_frame(token);
        position && color && optics
    }
}

impl LiveFrameLanes for FamilyLanes {
    fn abandon_frame(&self) {
        self.abandon();
    }

    fn released_owners(&self) -> Vec<ReleasedPhysicalOwner> {
        self.released()
    }
}

/// [`finalize_live_physical_frame`] for every family of one Live lane set: all token checks and
/// every lane's shared-control verification run before the engine render; all lanes commit only
/// after it succeeded. On any error nothing is published and every lane keeps its state.
pub(in crate::runtime::output_scheduler::dynamic_projection) fn finalize_live_family_frame(
    engine: &Engine,
    capture: &PreparedOutputFrame,
    lanes: &FamilyLanes,
    prepared: PreparedHybridFrame<FamilySidecar>,
) -> Result<PublishedFamilyFrame, DynamicRuntimeError> {
    finalize_live_lanes_frame(engine, capture, lanes, prepared)
}

/// Independent Before/After Release family lanes of one retained Preload episode.
pub(in crate::runtime) struct FamilyPreloadLanes {
    before: FamilyLanes,
    after: FamilyLanes,
}

impl Default for FamilyPreloadLanes {
    fn default() -> Self {
        Self {
            before: FamilyLanes::preload(PreloadBranch::BeforeRelease),
            after: FamilyLanes::preload(PreloadBranch::AfterRelease),
        }
    }
}

impl FamilyPreloadLanes {
    pub fn lanes(&self, branch: PreloadBranch) -> &FamilyLanes {
        match branch {
            PreloadBranch::BeforeRelease => &self.before,
            PreloadBranch::AfterRelease => &self.after,
        }
    }

    fn lanes_for(&self, token: &CapturedFrameToken) -> Result<&FamilyLanes, TransitionError> {
        match token.lane().preload_branch() {
            Some(branch) => Ok(self.lanes(branch)),
            None => {
                Err(IntentError("a Live token cannot address a Preload family lane".into()).into())
            }
        }
    }
}

impl HybridFrameResolver for FamilyPreloadLanes {
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        self.lanes_for(frame.token)?
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
        self.lanes_for(frame.token)?
            .resolve(frame, target, requirement, from, to, operation)
    }

    fn begin_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.lanes_for(token)?.begin_frame(token)
    }

    fn hold_frame(
        &self,
        token: &CapturedFrameToken,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        self.lanes_for(token)?.hold_frame(token, requirements)
    }

    fn verify_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.lanes_for(token)?.verify_frame(token)
    }

    fn accept_frame(&self, token: &CapturedFrameToken) -> bool {
        self.lanes_for(token)
            .is_ok_and(|lanes| lanes.accept_frame(token))
    }
}
