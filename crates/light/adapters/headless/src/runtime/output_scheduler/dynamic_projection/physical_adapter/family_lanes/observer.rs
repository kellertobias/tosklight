//! The all-family hybrid observer. Position keeps its complete-cohort observer
//! (`PositionFrameObserver`): every rich hook is forwarded to it with re-wrapped sidecars.
//! Color and Focus/Zoom observe one head at a time in their own lanes. Every family's writes are
//! installed natively in one engine call per frame (TL-548 C3, `native.rs`).
use super::super::position::PositionFrameObserver;
use super::super::programming_projection::hybrid::{
    HybridFamilyProgram, HybridFrameObserver, HybridPositionBatchComposer,
    HybridPositionBatchResult, HybridProgramComposer, OwnedHybridProjection,
};
use super::super::retained_preload_hybrid::RetainedHybridFrameObserver;
use super::bridge::{
    PositionBatchBridge, PositionProgramBridge, SplitRow, family_row, position_row,
};
use super::native::project_family_native_rows;
use super::*;
use rustc_hash::FxHashSet;

/// Observer of one complete Live frame or one retained branch over one [`FamilyLanes`] set.
pub(in crate::runtime) struct FamilyFrameObserver<'a> {
    lanes: &'a FamilyLanes,
    position: PositionFrameObserver<'a>,
}

impl<'a> FamilyFrameObserver<'a> {
    pub fn new(lanes: &'a FamilyLanes) -> Self {
        Self {
            lanes,
            position: PositionFrameObserver::new(&lanes.position),
        }
    }
}

impl HybridFrameObserver<FamilySidecar> for FamilyFrameObserver<'_> {
    /// Every family's rows in one `project_family_native` installation.
    fn project_native(
        &mut self,
        capture: &PreparedOutputFrame,
        frame_token: &CapturedFrameToken,
        token: &mut light_engine::PreparedStaticFamilyFrame,
        sidecars: &[FamilySidecar],
    ) -> Result<(), TransitionError> {
        project_family_native_rows(capture, frame_token, token, sidecars)
    }

    fn begin_frame(&mut self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.position.begin_frame(token)
    }

    fn prepare_current(
        &mut self,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
        protected: &[FixtureId],
    ) -> Result<(), TransitionError> {
        self.position.prepare_current(frame, baseline, protected)
    }

    fn static_program_targets(
        &mut self,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
    ) -> Result<Vec<(FixtureId, ProgrammingOwner)>, TransitionError> {
        let mut targets = self.position.static_program_targets(frame, baseline)?;
        // TL-596: a set for membership; scanning `targets` per head was quadratic at full-rig
        // size. `targets` keeps its order.
        let mut seen = targets.iter().copied().collect::<FxHashSet<_>>();
        static_color_targets(self.lanes, frame, baseline, &mut targets, &mut seen)?;
        static_zoom_targets(self.lanes, frame, baseline, &mut targets, &mut seen)?;
        Ok(targets)
    }

    fn prepare_programs(
        &mut self,
        frame: HybridFrameContext<'_>,
        programs: &[HybridFamilyProgram<'_>],
    ) -> Result<(), TransitionError> {
        self.position.prepare_programs(frame, programs)
    }

    fn compose_position_batch(
        &mut self,
        frame: HybridFrameContext<'_>,
        composer: &mut dyn HybridPositionBatchComposer<FamilySidecar>,
    ) -> Result<Option<HybridPositionBatchResult<FamilySidecar>>, TransitionError> {
        let mut bridge = PositionBatchBridge { inner: composer };
        Ok(self
            .position
            .compose_position_batch(frame, &mut bridge)?
            .map(|batch| HybridPositionBatchResult {
                handled: batch.handled,
                projections: batch.projections.into_iter().map(family_row).collect(),
                requirements: batch.requirements,
            }))
    }

    fn compose_program(
        &mut self,
        program: HybridFamilyProgram<'_>,
        composer: &mut dyn HybridProgramComposer<FamilySidecar>,
    ) -> Result<Option<OwnedHybridProjection<FamilySidecar>>, TransitionError> {
        let mut bridge = PositionProgramBridge { inner: composer };
        Ok(self
            .position
            .compose_program(program, &mut bridge)?
            .map(family_row))
    }

    fn observe(
        &mut self,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, FamilySidecar), TransitionError> {
        match owner_family(observation.owner) {
            PhysicalFamily::Position => self
                .position
                .observe(observation)
                .map(|(metadata, row)| (metadata, FamilySidecar::Position(row))),
            PhysicalFamily::Color | PhysicalFamily::Optics => self.lanes.observe(observation),
        }
    }

    /// The Position cohort is fitted over its own rows only; every other row keeps its place.
    fn finish(
        &mut self,
        frame: HybridFrameContext<'_>,
        projections: &mut Vec<OwnedHybridProjection<FamilySidecar>>,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        let mut slots = Vec::with_capacity(projections.len());
        let mut positions = Vec::new();
        for row in projections.drain(..) {
            match position_row(row) {
                SplitRow::Position(position) => {
                    slots.push(None);
                    positions.push(position);
                }
                SplitRow::Other(other) => slots.push(Some(other)),
            }
        }
        let expected = positions.len();
        let finished = self.position.finish(frame, &mut positions, requirements);
        let unchanged = positions.len() == expected;
        let mut positions = positions.into_iter().map(family_row);
        if unchanged {
            projections.extend(slots.into_iter().map(|slot| {
                slot.unwrap_or_else(|| positions.next().expect("one Position row per slot"))
            }));
        } else {
            // The cohort removed rows: keep the other families in order, then Position.
            projections.extend(slots.into_iter().flatten());
            projections.extend(positions);
        }
        finished
    }
}

/// Two isolated branch observers of one retained Preload episode, the all-family counterpart of
/// `PositionPreloadObserver` (which pairs two `PositionFrameObserver`s the same way). No Live
/// cache, held value or accepted continuity enters either branch.
pub(in crate::runtime) struct FamilyPreloadObserver<'a> {
    before: FamilyFrameObserver<'a>,
    after: FamilyFrameObserver<'a>,
}

impl<'a> FamilyPreloadObserver<'a> {
    pub fn new(lanes: &'a FamilyPreloadLanes) -> Self {
        Self {
            before: FamilyFrameObserver::new(lanes.lanes(PreloadBranch::BeforeRelease)),
            after: FamilyFrameObserver::new(lanes.lanes(PreloadBranch::AfterRelease)),
        }
    }

    fn observer(&mut self, branch: PreloadBranch) -> &mut FamilyFrameObserver<'a> {
        match branch {
            PreloadBranch::BeforeRelease => &mut self.before,
            PreloadBranch::AfterRelease => &mut self.after,
        }
    }
}

impl RetainedHybridFrameObserver<FamilySidecar> for FamilyPreloadObserver<'_> {
    fn project_native(
        &mut self,
        branch: PreloadBranch,
        capture: &PreparedOutputFrame,
        frame_token: &CapturedFrameToken,
        token: &mut light_engine::PreparedStaticFamilyFrame,
        sidecars: &[FamilySidecar],
    ) -> Result<(), TransitionError> {
        self.observer(branch)
            .project_native(capture, frame_token, token, sidecars)
    }

    fn begin_frame(
        &mut self,
        branch: PreloadBranch,
        token: &CapturedFrameToken,
    ) -> Result<(), TransitionError> {
        self.observer(branch).begin_frame(token)
    }

    fn prepare_current(
        &mut self,
        branch: PreloadBranch,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
        protected: &[FixtureId],
    ) -> Result<(), TransitionError> {
        self.observer(branch)
            .prepare_current(frame, baseline, protected)
    }

    fn static_program_targets(
        &mut self,
        branch: PreloadBranch,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
    ) -> Result<Vec<(FixtureId, ProgrammingOwner)>, TransitionError> {
        self.observer(branch)
            .static_program_targets(frame, baseline)
    }

    fn prepare_programs(
        &mut self,
        branch: PreloadBranch,
        frame: HybridFrameContext<'_>,
        programs: &[HybridFamilyProgram<'_>],
    ) -> Result<(), TransitionError> {
        self.observer(branch).prepare_programs(frame, programs)
    }

    fn compose_position_batch(
        &mut self,
        branch: PreloadBranch,
        frame: HybridFrameContext<'_>,
        composer: &mut dyn HybridPositionBatchComposer<FamilySidecar>,
    ) -> Result<Option<HybridPositionBatchResult<FamilySidecar>>, TransitionError> {
        self.observer(branch)
            .compose_position_batch(frame, composer)
    }

    fn compose_program(
        &mut self,
        branch: PreloadBranch,
        program: HybridFamilyProgram<'_>,
        composer: &mut dyn HybridProgramComposer<FamilySidecar>,
    ) -> Result<Option<OwnedHybridProjection<FamilySidecar>>, TransitionError> {
        self.observer(branch).compose_program(program, composer)
    }

    fn observe(
        &mut self,
        branch: PreloadBranch,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, FamilySidecar), TransitionError> {
        if observation.frame.token.lane().preload_branch() != Some(branch) {
            return Err(
                IntentError("Preload observation token belongs to another branch".into()).into(),
            );
        }
        self.observer(branch).observe(observation)
    }

    fn finish(
        &mut self,
        branch: PreloadBranch,
        frame: HybridFrameContext<'_>,
        projections: &mut Vec<OwnedHybridProjection<FamilySidecar>>,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        self.observer(branch)
            .finish(frame, projections, requirements)
    }
}

/// TL-554: a static Color program (Semantic or Direct) is resolved by the routed Color adapter
/// like any other Color owner, so the published frame carries its complete native output, the
/// premaster values a first native edit adopts, and the per-head Direct replay status. Targets
/// without a compiled Color destination keep their ordinary baseline output.
fn static_color_targets(
    lanes: &FamilyLanes,
    frame: HybridFrameContext<'_>,
    baseline: &light_engine::PreparedStaticFamilyFrame,
    targets: &mut Vec<(FixtureId, ProgrammingOwner)>,
    seen: &mut FxHashSet<(FixtureId, ProgrammingOwner)>,
) -> Result<(), TransitionError> {
    let owner = ProgrammingOwner::Color;
    for fixture in frame.capture.snapshot().fixtures.iter() {
        for target in std::iter::once(fixture.fixture_id)
            .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
        {
            if !matches!(
                baseline.value(target, &owner.key()),
                Some(AttributeValue::ColorProgram(_))
            ) || seen.contains(&(target, owner))
            {
                continue;
            }
            match lanes.color.descriptor(frame, target, owner) {
                Ok(_) => {
                    seen.insert((target, owner));
                    targets.push((target, owner));
                }
                Err(TransitionError::Requires(_)) => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
}

/// TL-560: a static typed Zoom (Programmer, played Cue, committed Preload) is owned by the Zoom
/// adapter like a static Color program; the scalar path cannot render a typed opening, so
/// without this the Zoom control kept its default whenever no Dynamic sampled the owner. Focus
/// stays a Normalized scalar on the ordinary path. Targets without a compiled Zoom destination
/// keep their ordinary baseline output.
fn static_zoom_targets(
    lanes: &FamilyLanes,
    frame: HybridFrameContext<'_>,
    baseline: &light_engine::PreparedStaticFamilyFrame,
    targets: &mut Vec<(FixtureId, ProgrammingOwner)>,
    seen: &mut FxHashSet<(FixtureId, ProgrammingOwner)>,
) -> Result<(), TransitionError> {
    let owner = ProgrammingOwner::Zoom;
    let lane = lanes.optics.lane(light_fixture::OpticsFamily::Zoom);
    for fixture in frame.capture.snapshot().fixtures.iter() {
        for target in std::iter::once(fixture.fixture_id)
            .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
        {
            if !matches!(
                baseline.value(target, &owner.key()),
                Some(AttributeValue::Zoom(_))
            ) || seen.contains(&(target, owner))
            {
                continue;
            }
            match lane.descriptor(frame, target, owner) {
                Ok(_) => {
                    seen.insert((target, owner));
                    targets.push((target, owner));
                }
                Err(TransitionError::Requires(_)) => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
}
