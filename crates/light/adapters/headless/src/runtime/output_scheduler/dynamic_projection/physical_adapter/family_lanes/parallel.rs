//! The Color and Focus/Zoom lanes of one [`FamilyLanes`] set as seen by a parallel section's
//! workers (TL-639 round 5). A worker resolves, adopts and stages exactly as the lanes would,
//! into its own [`FamilyStaging`]; the frame merges the stagings in group order. Position
//! groups compose on a worker through the frame's shared Position state (round 6,
//! [`FamilyPositionWorker`]); their cohort is still fitted and staged by the frame.
use super::super::lane::{LaneShared, LaneStaging, LaneWorker, StagingMark};
use super::super::position::{PositionFrameShared, PositionPendingLog, PositionWorker};
use super::super::programming_projection::hybrid::{
    HybridFamilyProgram, HybridProgramComposer, OwnedHybridProjection,
};
use super::bridge::{PositionProgramBridge, family_row};
use super::*;
use std::cell::Cell;

type Staging<A> = LaneStaging<<A as PhysicalFamilyAdapter>::Continuity>;

/// Read-only Color, Focus and Zoom lane state shared by a section's workers.
pub(in crate::runtime) struct FamilyLanesShared<'s, 'l> {
    color: &'s LaneShared<'l, RoutingColorAdapter>,
    focus: &'s LaneShared<'l, OpticsAdapter>,
    zoom: &'s LaneShared<'l, OpticsAdapter>,
    /// Emptied stagings of earlier sections, so a worker's staging is grown once.
    spare: &'s parking_lot::Mutex<Vec<FamilyStaging>>,
}

/// The heads one worker staged in the Color, Focus and Zoom lanes.
#[derive(Default)]
pub(in crate::runtime) struct FamilyStaging {
    color: Staging<RoutingColorAdapter>,
    focus: Staging<OpticsAdapter>,
    zoom: Staging<OpticsAdapter>,
}

/// Where a worker's stagings stood before a group.
#[derive(Clone, Copy)]
pub(in crate::runtime) struct FamilyStagingMark([StagingMark; 3]);

impl FamilyLanes {
    /// Lend the Color, Focus and Zoom lanes' read-only frame state to a parallel section.
    pub fn with_shared<R>(&self, run: impl FnOnce(&FamilyLanesShared<'_, '_>) -> R) -> R {
        let focus = self.optics.lane(light_fixture::OpticsFamily::Focus);
        let zoom = self.optics.lane(light_fixture::OpticsFamily::Zoom);
        let spare = parking_lot::Mutex::new(std::mem::take(&mut *self.spare_stagings.borrow_mut()));
        let result = self.color.with_shared(|color| {
            focus.with_shared(|focus| {
                zoom.with_shared(|zoom| {
                    run(&FamilyLanesShared {
                        color,
                        focus,
                        zoom,
                        spare: &spare,
                    })
                })
            })
        });
        self.spare_stagings.borrow_mut().extend(spare.into_inner());
        result
    }

    /// Stage a worker's heads in each lane, as if they had been staged there in order.
    pub fn merge_staging(
        &self,
        token: &CapturedFrameToken,
        mut staging: FamilyStaging,
    ) -> Result<(), TransitionError> {
        self.color.merge_staging(token, &mut staging.color)?;
        self.optics
            .lane(light_fixture::OpticsFamily::Focus)
            .merge_staging(token, &mut staging.focus)?;
        self.optics
            .lane(light_fixture::OpticsFamily::Zoom)
            .merge_staging(token, &mut staging.zoom)?;
        self.spare_stagings.borrow_mut().push(staging);
        Ok(())
    }
}

/// One worker's Color, Focus and Zoom lanes. `missed` is set when a group needs anything a
/// worker cannot do (a descriptor compile, a Position lane); the frame then reruns that group.
pub(in crate::runtime) struct FamilyLanesWorker<'s, 'l> {
    color: LaneWorker<'s, 'l, RoutingColorAdapter>,
    focus: LaneWorker<'s, 'l, OpticsAdapter>,
    zoom: LaneWorker<'s, 'l, OpticsAdapter>,
    missed: &'s Cell<bool>,
}

impl<'s, 'l> FamilyLanesWorker<'s, 'l> {
    pub fn new(shared: &'s FamilyLanesShared<'s, 'l>, missed: &'s Cell<bool>) -> Self {
        let FamilyStaging { color, focus, zoom } = shared.spare.lock().pop().unwrap_or_default();
        Self {
            color: LaneWorker::new(shared.color, missed, color),
            focus: LaneWorker::new(shared.focus, missed, focus),
            zoom: LaneWorker::new(shared.zoom, missed, zoom),
            missed,
        }
    }

    pub fn mark(&self) -> FamilyStagingMark {
        FamilyStagingMark([
            self.color.staging().mark(),
            self.focus.staging().mark(),
            self.zoom.staging().mark(),
        ])
    }

    /// Drop the heads staged since `mark`.
    pub fn truncate(&self, FamilyStagingMark([color, focus, zoom]): FamilyStagingMark) {
        self.color.staging().truncate(color);
        self.focus.staging().truncate(focus);
        self.zoom.staging().truncate(zoom);
    }

    pub fn into_staging(self) -> FamilyStaging {
        FamilyStaging {
            color: self.color.into_staging(),
            focus: self.focus.into_staging(),
            zoom: self.zoom.into_staging(),
        }
    }

    fn unsupported<T>(&self) -> Result<T, TransitionError> {
        self.missed.set(true);
        Err(IntentError("a parallel worker cannot reach the Position lane".into()).into())
    }

    fn optics(
        &self,
        owner: ProgrammingOwner,
    ) -> Result<&LaneWorker<'s, 'l, OpticsAdapter>, TransitionError> {
        match owner {
            ProgrammingOwner::Focus => Ok(&self.focus),
            ProgrammingOwner::Zoom => Ok(&self.zoom),
            other => Err(TransitionError::Requires(owner_requirement(other))),
        }
    }

    /// [`FamilyLanes::observe`] for Color and Focus/Zoom.
    pub fn observe(
        &self,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, FamilySidecar), TransitionError> {
        match owner_family(observation.owner) {
            PhysicalFamily::Position => self.unsupported(),
            PhysicalFamily::Color => self
                .color
                .observe(observation)
                .map(|(metadata, row)| (metadata, FamilySidecar::Color(Box::new(row)))),
            PhysicalFamily::Optics => self
                .optics(observation.owner)?
                .observe(observation)
                .map(|(metadata, row)| (metadata, FamilySidecar::Optics(Box::new(row)))),
        }
    }
}

impl HybridFrameResolver for FamilyLanesWorker<'_, '_> {
    /// [`FamilyLanes`]' adoption routing.
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        match owner_family(address.owner()) {
            PhysicalFamily::Position => self.unsupported(),
            PhysicalFamily::Color => self.color.adopt(frame, target, original, address),
            PhysicalFamily::Optics => self
                .optics(address.owner())?
                .adopt(frame, target, original, address),
        }
    }

    /// [`FamilyLanes`]' transition routing, with [`OpticsLanes`]' endpoint routing.
    fn resolve(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        match transition_family(requirement, from, to) {
            Some(PhysicalFamily::Position) => self.unsupported(),
            Some(PhysicalFamily::Color) => {
                self.color
                    .resolve(frame, target, requirement, from, to, operation)
            }
            Some(PhysicalFamily::Optics) => {
                let lane = match (from, to) {
                    (AttributeValue::Zoom(_), AttributeValue::Zoom(_)) => &self.zoom,
                    (AttributeValue::Normalized(_), AttributeValue::Normalized(_)) => &self.focus,
                    _ => return Err(TransitionError::Requires(requirement)),
                };
                lane.resolve(frame, target, requirement, from, to, operation)
            }
            None => Err(TransitionError::Requires(requirement)),
        }
    }
}

/// One worker's Position composer, with the all-family sidecar (TL-639 round 6).
pub(in crate::runtime) struct FamilyPositionWorker<'s, 'l>(PositionWorker<'s, 'l>);

impl<'s, 'l> FamilyPositionWorker<'s, 'l> {
    pub fn new(shared: &'s PositionFrameShared<'s, 'l>, missed: &'s Cell<bool>) -> Self {
        Self(PositionWorker::new(shared, missed))
    }

    pub fn mark(&self) -> usize {
        self.0.mark()
    }

    pub fn truncate(&mut self, mark: usize) {
        self.0.truncate(mark);
    }

    pub fn into_pending(self) -> PositionPendingLog {
        self.0.into_pending()
    }

    /// [`FamilyFrameObserver`]'s Position composition, on this worker.
    pub fn compose_program(
        &mut self,
        program: HybridFamilyProgram<'_>,
        composer: &mut dyn HybridProgramComposer<FamilySidecar>,
    ) -> Result<Option<OwnedHybridProjection<FamilySidecar>>, TransitionError> {
        let mut bridge = PositionProgramBridge { inner: composer };
        Ok(self
            .0
            .compose_program(program, &mut bridge)?
            .map(family_row))
    }
}
