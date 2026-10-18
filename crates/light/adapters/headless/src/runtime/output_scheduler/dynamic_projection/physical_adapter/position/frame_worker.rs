//! One body for composing a Position program and observing its rows, over the frame's own
//! Position observer or over a parallel worker's view of it (TL-639 round 6).
//!
//! Composing one Position group reads the frame's captured programs, Current cohorts and the
//! lane's descriptors and committed continuity, none of which another group of the frame
//! changes, and records one pending cohort member. A worker therefore reads them through a
//! shared view ([`PositionFrameShared`]) and records its pending members privately; the frame
//! takes them back in group order. A descriptor the lane has not compiled marks the worker
//! `missed` and the frame composes that group itself.
use super::super::super::programming_projection::hybrid::{
    HybridCapturedPositionProgram, HybridFamilyProgram, HybridProgramComposer,
    OwnedHybridProjection,
};
use super::super::lane::{LaneAccess, LaneShared, LaneWorker};
use super::frame_observer::{CapturedPositionPeer, PendingPosition};
use super::*;
use std::cell::Cell;

type PositionRow = PhysicalHeadResult<PositionAdapter>;

/// What composing a Position group reads and records.
pub(super) trait PositionComposition<'l> {
    fn adapter(&self) -> &'l PositionAdapter;
    fn descriptor(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Result<Arc<PositionDescriptor>, TransitionError>;
    fn continuity(&self, target: FixtureId, owner: ProgrammingOwner) -> Option<PositionContinuity>;
    /// The requested and captured program of `target`.
    fn program(
        &self,
        target: FixtureId,
    ) -> Option<(Arc<PositionProgram>, Arc<HybridCapturedPositionProgram>)>;
    fn current(&self) -> current_cohort::CapturedCurrentCohorts;
    fn active_programs(&self) -> Arc<[FixtureId]>;
    fn pending(&mut self) -> &mut Vec<PendingPosition>;
}

/// Compose one Position program at each of its physical destinations.
pub(super) fn compose_program<'l>(
    access: &mut impl PositionComposition<'l>,
    p: HybridFamilyProgram<'_>,
    composer: &mut dyn HybridProgramComposer<PositionRow>,
) -> Result<Option<OwnedHybridProjection<PositionRow>>, TransitionError> {
    if p.owner != ProgrammingOwner::Position {
        return Ok(None);
    }
    let (program, captured) = access
        .program(p.target)
        .ok_or_else(|| invalid("Position program was not collected before composition"))?;
    let descriptor = access.descriptor(p.frame, p.target, p.owner)?;
    let previous = access.continuity(p.target, p.owner);
    let mut destinations = Vec::with_capacity(descriptor.instances.len());
    let mut representative = None;
    let current = access.current();
    let active_programs = access.active_programs();
    let adapter = access.adapter();
    for instance in &descriptor.instances {
        let bound = destination::PositionDestinationFrame {
            adapter,
            frame: p.frame,
            descriptor: &descriptor,
            target: p.target,
            instance,
            previous: previous.as_ref(),
            current: &current,
            active_programs: &active_programs,
        };
        let adoption = |original: &AttributeValue, address: &DynamicValueAddress| {
            bound.adopt(original, address)
        };
        let pending_start = access.pending().len();
        let mut evaluation = composer.begin_position(&captured, instance.destination, &adoption)?;
        let result = match composer.advance_position(&mut evaluation, &bound, &adoption) {
            Ok(light_dynamics::PositionCompositionProgress::Complete(_)) => composer
                .observe_position(&mut evaluation, &mut |observation| {
                    observe(access, observation)
                }),
            Ok(light_dynamics::PositionCompositionProgress::NeedsMaterialization(request)) => {
                // The owned request retains its exact original registry node. Complete
                // changing-peer environments must be established by the batch coordinator;
                // this bridge never replaces a missing peer with its static underlay.
                Err(TransitionError::Requires(request.requirement))
            }
            Err(error) => Err(error),
        };
        composer.recycle_position(evaluation);
        access.pending().truncate(pending_start);
        let row = result?;
        destinations.push(PositionProgramDestination {
            destination: instance.destination,
            value: row.value.clone(),
            provenance: row.sidecar.provenance.clone(),
        });
        if representative.is_none() {
            representative = Some(row);
        }
    }
    let mut row =
        representative.ok_or_else(|| invalid("Position program has no physical destinations"))?;
    let program = if p.samples.is_empty() {
        None
    } else {
        Some(program)
    };
    if let Some(program) = &program {
        row.sidecar.requested = PositionRequest::Program(Arc::clone(program));
    }
    access.pending().push(PendingPosition {
        target: p.target,
        descriptor,
        previous,
        program,
        destinations,
    });
    Ok(Some(row))
}

/// Observe one composed Position row and record it as a pending cohort member.
pub(super) fn observe<'l>(
    access: &mut impl PositionComposition<'l>,
    o: HybridFamilyObservation<'_>,
) -> Result<(FamilyProjectionMetadata, PositionRow), TransitionError> {
    let descriptor = access.descriptor(o.frame, o.target, o.owner)?;
    let requested = intent(o.value)?.clone();
    let adapter = access.adapter();
    let fields = adapter.consumed_fields(o.owner, o.value)?;
    let mut sources = DynamicFamilySourceProjection::default();
    o.project_fields(&fields, &mut sources)?;
    let provenance = PhysicalProvenance {
        controls: o.controls_for_fields(&fields),
        fields,
        sources,
    };
    let metadata = adapter.projection_metadata(o.owner, &provenance);
    let previous = access.continuity(o.target, o.owner);
    access.pending().push(PendingPosition {
        target: o.target,
        descriptor,
        previous,
        program: None,
        destinations: Vec::new(),
    });
    Ok((
        metadata.clone(),
        PhysicalHeadResult {
            token: o.frame.token.clone(),
            target: o.target,
            owner: o.owner,
            value: o.value.clone(),
            writes: Vec::new(),
            requested: PositionRequest::Intent(requested),
            achieved: AchievedPosition::default(),
            quality: PositionQuality::default(),
            provenance,
            metadata,
        },
    ))
}

/// The frame's Position composition state a parallel section's workers read.
pub(in crate::runtime) struct PositionFrameShared<'a, 'l> {
    pub(super) lane: &'a LaneShared<'l, PositionAdapter>,
    pub(super) programs: &'a [CapturedPositionPeer],
    pub(super) program_index: &'a rustc_hash::FxHashMap<FixtureId, usize>,
    pub(super) current: &'a current_cohort::CapturedCurrentCohorts,
    pub(super) active_programs: &'a Arc<[FixtureId]>,
}

/// The pending cohort members one worker composed, in group order.
#[derive(Default)]
pub(in crate::runtime) struct PositionPendingLog(Vec<PendingPosition>);

impl PositionPendingLog {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn truncate(&mut self, len: usize) {
        self.0.truncate(len);
    }

    pub fn into_iter(self) -> std::vec::IntoIter<PendingPosition> {
        self.0.into_iter()
    }
}

/// One worker's Position composer over the frame's shared state.
pub(in crate::runtime) struct PositionWorker<'s, 'l> {
    shared: &'s PositionFrameShared<'s, 'l>,
    lane: LaneWorker<'s, 'l, PositionAdapter>,
    pending: Vec<PendingPosition>,
}

impl<'s, 'l> PositionWorker<'s, 'l> {
    pub fn new(shared: &'s PositionFrameShared<'s, 'l>, missed: &'s Cell<bool>) -> Self {
        Self {
            shared,
            lane: LaneWorker::new(shared.lane, missed, Default::default()),
            pending: Vec::new(),
        }
    }

    /// Where its pending log stands, and back there (a group the frame reruns).
    pub fn mark(&self) -> usize {
        self.pending.len()
    }

    pub fn truncate(&mut self, mark: usize) {
        self.pending.truncate(mark);
    }

    pub fn into_pending(self) -> PositionPendingLog {
        PositionPendingLog(self.pending)
    }

    pub fn compose_program(
        &mut self,
        program: HybridFamilyProgram<'_>,
        composer: &mut dyn HybridProgramComposer<PositionRow>,
    ) -> Result<Option<OwnedHybridProjection<PositionRow>>, TransitionError> {
        compose_program(self, program, composer)
    }
}

impl<'s, 'l> PositionComposition<'l> for PositionWorker<'s, 'l> {
    fn adapter(&self) -> &'l PositionAdapter {
        self.shared.lane.adapter_ref()
    }

    fn descriptor(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Result<Arc<PositionDescriptor>, TransitionError> {
        LaneAccess::descriptor(&self.lane, frame, target, owner)
    }

    fn continuity(&self, target: FixtureId, owner: ProgrammingOwner) -> Option<PositionContinuity> {
        LaneAccess::continuity(&self.lane, target, owner)
    }

    fn program(
        &self,
        target: FixtureId,
    ) -> Option<(Arc<PositionProgram>, Arc<HybridCapturedPositionProgram>)> {
        self.shared.program_index.get(&target).map(|index| {
            let program = &self.shared.programs[*index];
            (
                Arc::clone(&program.requested),
                Arc::clone(&program.captured),
            )
        })
    }

    fn current(&self) -> current_cohort::CapturedCurrentCohorts {
        self.shared.current.clone()
    }

    fn active_programs(&self) -> Arc<[FixtureId]> {
        Arc::clone(self.shared.active_programs)
    }

    fn pending(&mut self) -> &mut Vec<PendingPosition> {
        &mut self.pending
    }
}
