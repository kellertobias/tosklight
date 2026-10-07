//! Branch observers that capture Position evidence for one Live or retained frame and compose
//! the frame's Position programs from it.

use super::*;

pub(in crate::runtime) struct PendingPosition {
    pub(super) target: FixtureId,
    pub(super) descriptor: Arc<PositionDescriptor>,
    pub(super) previous: Option<PositionContinuity>,
    pub(super) program: Option<Arc<PositionProgram>>,
    pub(super) destinations: Vec<PositionProgramDestination>,
}
pub(super) struct CapturedPositionPeer {
    pub(super) target: FixtureId,
    pub(super) requested: Arc<PositionProgram>,
    pub(super) captured: Arc<HybridCapturedPositionProgram>,
    pub(super) has_requirements: bool,
}
/// Short-lived observer for one complete Live or retained branch. It owns evidence only;
/// no borrowed trace or application state survives an observation callback.
pub(in crate::runtime) struct PositionFrameObserver<'a> {
    pub(super) lane: &'a PhysicalAdapterLane<PositionAdapter>,
    pub(super) pending: Vec<PendingPosition>,
    pub(super) current: current_cohort::CapturedCurrentCohorts,
    pub(super) programs: Vec<CapturedPositionPeer>,
    /// TL-639 round 2: each program's position in `programs` (targets are unique).
    pub(super) program_index: rustc_hash::FxHashMap<FixtureId, usize>,
    pub(super) active_programs: Arc<[FixtureId]>,
    /// Physical ownership in this observer's capture, including roots without DMX heads.
    pub(super) roots: BTreeMap<Uuid, usize>,
    /// The frame's output pool, for per-target work whose results merge in program order
    /// (TL-639 round 6). `None` keeps everything on the frame's thread.
    pub(in crate::runtime) pool: Option<Arc<light_engine::parallel::OutputPool>>,
}
impl<'a> PositionFrameObserver<'a> {
    /// The collected program of `target`, if any.
    pub(super) fn program(&self, target: FixtureId) -> Option<&CapturedPositionPeer> {
        self.program_index
            .get(&target)
            .map(|index| &self.programs[*index])
    }

    pub fn new(lane: &'a PhysicalAdapterLane<PositionAdapter>) -> Self {
        Self {
            lane,
            pending: Vec::new(),
            current: Default::default(),
            programs: Vec::new(),
            program_index: Default::default(),
            active_programs: Default::default(),
            roots: Default::default(),
            pool: None,
        }
    }
}
impl<'a> frame_worker::PositionComposition<'a> for PositionFrameObserver<'a> {
    fn adapter(&self) -> &'a PositionAdapter {
        self.lane.adapter()
    }

    fn descriptor(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Result<Arc<PositionDescriptor>, TransitionError> {
        self.lane.descriptor(frame, target, owner)
    }

    fn continuity(&self, target: FixtureId, owner: ProgrammingOwner) -> Option<PositionContinuity> {
        self.lane.continuity(target, owner)
    }

    fn program(
        &self,
        target: FixtureId,
    ) -> Option<(Arc<PositionProgram>, Arc<HybridCapturedPositionProgram>)> {
        PositionFrameObserver::program(self, target).map(|program| {
            (
                Arc::clone(&program.requested),
                Arc::clone(&program.captured),
            )
        })
    }

    fn current(&self) -> current_cohort::CapturedCurrentCohorts {
        self.current.clone()
    }

    fn active_programs(&self) -> Arc<[FixtureId]> {
        Arc::clone(&self.active_programs)
    }

    fn pending(&mut self) -> &mut Vec<PendingPosition> {
        &mut self.pending
    }
}

impl<'a> PositionFrameObserver<'a> {
    /// Lend this frame's Position composition state to a parallel section (TL-639 round 6).
    pub(in crate::runtime) fn with_shared<R>(
        &self,
        run: impl FnOnce(&frame_worker::PositionFrameShared<'_, '_>) -> R,
    ) -> R {
        self.lane.with_shared(|lane| {
            run(&frame_worker::PositionFrameShared {
                lane,
                programs: &self.programs,
                program_index: &self.program_index,
                current: &self.current,
                active_programs: &self.active_programs,
            })
        })
    }

    /// Take a worker's pending cohort members of one group, in order.
    pub(in crate::runtime) fn take_pending(
        &mut self,
        pending: impl Iterator<Item = PendingPosition>,
    ) {
        self.pending.extend(pending);
    }
}

impl HybridFrameObserver<PhysicalHeadResult<PositionAdapter>> for PositionFrameObserver<'_> {
    fn project_native(
        &mut self,
        capture: &light_engine::PreparedOutputFrame,
        frame_token: &CapturedFrameToken,
        token: &mut light_engine::PreparedStaticFamilyFrame,
        sidecars: &[PhysicalHeadResult<PositionAdapter>],
    ) -> Result<(), TransitionError> {
        native_rows::project_position_native_rows(capture, frame_token, token, sidecars.iter())
    }

    fn begin_frame(&mut self, _token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.pending.clear();
        self.current.clear();
        self.programs.clear();
        self.program_index.clear();
        self.active_programs = Default::default();
        Ok(())
    }

    fn prepare_current(
        &mut self,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
        protected: &[FixtureId],
    ) -> Result<(), TransitionError> {
        self.current.capture(self.lane, frame, baseline, protected)
    }

    fn static_program_targets(
        &mut self,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
    ) -> Result<Vec<(FixtureId, ProgrammingOwner)>, TransitionError> {
        if !frame.token.matches_static_frame(baseline) {
            return Err(invalid("Position static registry belongs to another frame"));
        }
        let mut targets = Vec::new();
        // TL-639 round 2: a set for the duplicate check; scanning `targets` was quadratic.
        let mut seen = rustc_hash::FxHashSet::default();
        // TL-639 round 6: scanned on the pool; no owned compiled emitter (`Requires`) preserves
        // ordinary baseline output.
        self.lane.static_targets(
            frame,
            baseline,
            ProgrammingOwner::Position,
            |value| matches!(value, AttributeValue::Position(_)),
            &mut targets,
            &mut seen,
            self.pool.as_deref(),
        )?;
        Ok(targets)
    }
    fn prepare_programs(
        &mut self,
        frame: HybridFrameContext<'_>,
        programs: &[super::super::super::programming_projection::hybrid::HybridFamilyProgram<'_>],
    ) -> Result<(), TransitionError> {
        self.programs.clear();
        self.program_index.clear();
        let mut active_programs = Vec::new();
        let position = programs
            .iter()
            .filter(|p| p.owner == ProgrammingOwner::Position)
            .collect::<Vec<_>>();
        let captured = capture_programs(frame.token, &position, self.pool.as_deref());
        // TL-596: a set for the duplicate check; scanning every earlier program was quadratic.
        let mut seen = rustc_hash::FxHashSet::default();
        for (p, (samples, captured)) in position.into_iter().zip(captured) {
            if p.frame.token != frame.token || !seen.insert(p.target) {
                return Err(invalid(
                    "Position registry contains a foreign or duplicate program",
                ));
            }
            if !p.samples.is_empty() || p.has_requirements {
                active_programs.push(p.target);
            }
            self.program_index.insert(p.target, self.programs.len());
            self.programs.push(CapturedPositionPeer {
                target: p.target,
                requested: Arc::new(PositionProgram {
                    base: p.base.clone(),
                    samples,
                }),
                has_requirements: p.has_requirements,
                captured: Arc::new(captured?),
            });
        }
        self.active_programs = active_programs.into();
        let snapshot = frame.capture.snapshot();
        self.roots.clear();
        for (index, fixture) in snapshot.fixtures.iter().enumerate() {
            self.roots.insert(fixture.fixture_id.0, index);
            for head in &fixture.logical_heads {
                self.roots.insert(head.fixture_id.0, index);
            }
        }
        let mut owners = Vec::with_capacity(self.programs.len());
        for program in &self.programs {
            let census = program.captured.registry().point_dependencies();
            // Root/copy mounting identity is independent of logical emitter ownership.
            // Keep requirements-only targets and missing Point references in the census.
            let Some(fixture) = self
                .roots
                .get(&program.target.0)
                .and_then(|index| snapshot.fixtures.get(*index))
            else {
                continue;
            };
            let destinations = std::iter::once(fixture.fixture_id)
                .chain(fixture.multipatch.iter().map(|copy| FixtureId(copy.id)))
                .collect::<Vec<_>>();
            owners.push(tracking::TrackingOwner {
                target: program.target,
                root: fixture.fixture_id,
                mount_references: destinations
                    .iter()
                    .map(|id| (*id, fixture.position_master.map(FixtureId)))
                    .collect(),
                destinations,
                points: census.point_ids().iter().copied().map(FixtureId).collect(),
                incomplete: census.incomplete(),
            });
        }
        self.lane
            .adapter()
            .tracking
            .borrow_mut()
            .prepare(frame, &owners)?;
        Ok(())
    }

    fn compose_position_batch(
        &mut self,
        frame: HybridFrameContext<'_>,
        composer: &mut dyn super::super::super::programming_projection::hybrid::HybridPositionBatchComposer<PhysicalHeadResult<PositionAdapter>>,
    ) -> Result<
        Option<
            super::super::super::programming_projection::hybrid::HybridPositionBatchResult<
                PhysicalHeadResult<PositionAdapter>,
            >,
        >,
        TransitionError,
    > {
        cut_coordinator::compose(self, frame, composer)
    }

    fn compose_program(
        &mut self,
        p: super::super::super::programming_projection::hybrid::HybridFamilyProgram<'_>,
        composer: &mut dyn super::super::super::programming_projection::hybrid::HybridProgramComposer<
            PhysicalHeadResult<PositionAdapter>,
        >,
    ) -> Result<Option<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>, TransitionError>
    {
        frame_worker::compose_program(self, p, composer)
    }

    fn observe(
        &mut self,
        o: HybridFamilyObservation<'_>,
    ) -> Result<
        (
            FamilyProjectionMetadata,
            PhysicalHeadResult<PositionAdapter>,
        ),
        TransitionError,
    > {
        frame_worker::observe(self, o)
    }
    fn finish(
        &mut self,
        frame: HybridFrameContext<'_>,
        rows: &mut Vec<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        // TL-639 round 6: the first pending entry of each target, as `find` returned it, without
        // scanning every pending entry per row.
        let mut first_pending =
            rustc_hash::FxHashMap::with_capacity_and_hasher(self.pending.len(), Default::default());
        for (index, pending) in self.pending.iter().enumerate() {
            first_pending.entry(pending.target).or_insert(index);
        }
        let requests = rows
            .iter()
            .map(|row| {
                let pending = first_pending
                    .get(&row.target)
                    .map(|index| &self.pending[*index])
                    .ok_or_else(|| invalid("missing Position cohort member"))?;
                Ok(PhysicalRequest {
                    frame,
                    target: row.target,
                    owner: row.owner,
                    descriptor: pending.descriptor.as_ref(),
                    value: &row.value,
                    previous: pending.previous.as_ref(),
                })
            })
            .collect::<Result<Vec<_>, TransitionError>>()?;
        let snapshot = frame.capture.snapshot();
        let protected: Vec<_> = requirements
            .iter()
            .filter(|r| r.owner == ProgrammingOwner::Position)
            .filter_map(|r| {
                self.roots
                    .get(&r.target.0)
                    .and_then(|index| snapshot.fixtures.get(*index))
                    .map(|fixture| fixture.fixture_id)
            })
            .collect();
        // TL-544 G1: static Cue/Programmer crossings move through this capture's solved joints.
        let crossings = super::static_crossing::static_crossings(
            self.lane.adapter(),
            frame,
            rows,
            |target| first_pending.get(&target).copied(),
            &self.pending,
            &self.current,
            &self.active_programs,
        )?;
        let programs = self
            .pending
            .iter()
            .filter_map(|p| {
                p.program
                    .as_ref()
                    .map(|_| (p.target, p.destinations.as_slice()))
            })
            .chain(
                crossings
                    .iter()
                    .map(|(target, destinations)| (*target, destinations.as_slice())),
            )
            .collect::<Vec<_>>();
        let resolved = self.lane.adapter().resolve_cohort_programs_on(
            &requests,
            &protected,
            &programs,
            self.pool.as_deref(),
        )?;
        drop(requests);
        for (row, mut resolution) in rows.iter_mut().zip(resolved) {
            let pending = &self.pending[first_pending[&row.target]];
            if let Some(program) = &pending.program {
                resolution.requested = PositionRequest::Program(Arc::clone(program));
            }
            resolution.achieved.destinations = pending.destinations.clone();
            validate_complete_writes(&pending.descriptor.footprint, &resolution.writes)?;
            // Unresolved mechanical input protects every peer of this root. A parked
            // peer is diagnostic output, not a newly accepted fitted pose.
            let stage = if resolution
                .achieved
                .outcomes
                .iter()
                .any(|outcome| outcome.input_requirement)
            {
                PhysicalAdapterLane::stage_held_resolution
            } else {
                PhysicalAdapterLane::stage_resolution
            };
            let (metadata, sidecar) = stage(
                self.lane,
                frame.token,
                row.target,
                row.owner,
                row.value.clone(),
                row.sidecar.provenance.clone(),
                row.metadata.clone(),
                resolution,
            )?;
            row.metadata = metadata;
            row.sidecar = sidecar;
        }
        self.retire(false);
        Ok(())
    }
}

impl PositionFrameObserver<'_> {
    /// Free this frame's pending cohort members and Current cohorts (and, at the end of the
    /// frame, its captured programs) on the pool when there is one (TL-639 round 6).
    pub(in crate::runtime) fn retire(&mut self, programs: bool) {
        let pending = std::mem::take(&mut self.pending);
        let current = std::mem::take(&mut self.current);
        let programs = if programs {
            self.program_index.clear();
            self.active_programs = Default::default();
            std::mem::take(&mut self.programs)
        } else {
            Vec::new()
        };
        match &self.pool {
            Some(pool) => pool.drop_later((pending, current, programs)),
            None => drop((pending, current, programs)),
        }
    }
}

/// Below this many Position programs a frame captures them on its own thread.
const MIN_PARALLEL_PROGRAMS: usize = if cfg!(test) { 2 } else { 64 };

type CapturedProgramResult = (
    Arc<[light_dynamics::FamilyCompositionSample]>,
    Result<HybridCapturedPositionProgram, TransitionError>,
);

/// Each Position program's shared sample list and captured registry, in program order. The
/// registries are independent per target; capture identities are drawn in program order on the
/// frame's thread first, so the result does not depend on which thread built which registry.
fn capture_programs(
    token: &CapturedFrameToken,
    programs: &[&super::super::super::programming_projection::hybrid::HybridFamilyProgram<'_>],
    pool: Option<&light_engine::parallel::OutputPool>,
) -> Vec<CapturedProgramResult> {
    let identities = programs
        .iter()
        .map(|_| super::super::super::programming_projection::hybrid::position_capture_identity())
        .collect::<Vec<_>>();
    let inputs = programs
        .iter()
        .zip(identities)
        .map(|(p, identity)| (p.target, p.base, p.samples, identity))
        .collect::<Vec<_>>();
    let capture = |range: std::ops::Range<usize>| {
        inputs[range]
            .iter()
            .map(|(target, base, samples, identity)| {
                let samples: Arc<[light_dynamics::FamilyCompositionSample]> = (*samples).into();
                let captured = HybridCapturedPositionProgram::new_shared(
                    token,
                    *target,
                    base,
                    Arc::clone(&samples),
                    *identity,
                );
                (samples, captured)
            })
            .collect::<Vec<_>>()
    };
    let pool = pool.filter(|_| inputs.len() >= MIN_PARALLEL_PROGRAMS);
    let Some(pool) = pool else {
        return capture(0..inputs.len());
    };
    let chunks = light_engine::parallel::chunk_count(inputs.len(), pool.workers(), 16);
    let mut slots = vec![(); pool.workers()];
    light_engine::parallel::run_ordered(Some(pool), &mut slots, chunks, |_, chunk| {
        capture(light_engine::parallel::chunk_range(
            inputs.len(),
            chunks,
            chunk,
        ))
    })
    .into_iter()
    .flatten()
    .collect()
}

/// The existing retained evaluator supplies two isolated branch observers. No Live cache,
/// held value or accepted continuity enters either retained branch.
pub(in crate::runtime) struct PositionPreloadObserver<'a> {
    pub(super) before: PositionFrameObserver<'a>,
    pub(super) after: PositionFrameObserver<'a>,
}
impl<'a> PositionPreloadObserver<'a> {
    pub fn new(lanes: &'a PhysicalPreloadLanes<PositionAdapter>) -> Self {
        Self {
            before: PositionFrameObserver::new(
                lanes.lane(light_engine::PreloadBranch::BeforeRelease),
            ),
            after: PositionFrameObserver::new(
                lanes.lane(light_engine::PreloadBranch::AfterRelease),
            ),
        }
    }
    fn observer(&mut self, branch: light_engine::PreloadBranch) -> &mut PositionFrameObserver<'a> {
        match branch {
            light_engine::PreloadBranch::BeforeRelease => &mut self.before,
            light_engine::PreloadBranch::AfterRelease => &mut self.after,
        }
    }
}
impl
    super::super::super::retained_preload_hybrid::RetainedHybridFrameObserver<
        PhysicalHeadResult<PositionAdapter>,
    > for PositionPreloadObserver<'_>
{
    fn project_native(
        &mut self,
        branch: light_engine::PreloadBranch,
        capture: &light_engine::PreparedOutputFrame,
        frame_token: &CapturedFrameToken,
        token: &mut light_engine::PreparedStaticFamilyFrame,
        sidecars: &[PhysicalHeadResult<PositionAdapter>],
    ) -> Result<(), TransitionError> {
        self.observer(branch)
            .project_native(capture, frame_token, token, sidecars)
    }

    fn begin_frame(
        &mut self,
        branch: light_engine::PreloadBranch,
        token: &CapturedFrameToken,
    ) -> Result<(), TransitionError> {
        self.observer(branch).begin_frame(token)
    }
    fn prepare_current(
        &mut self,
        branch: light_engine::PreloadBranch,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
        protected: &[FixtureId],
    ) -> Result<(), TransitionError> {
        self.observer(branch)
            .prepare_current(frame, baseline, protected)
    }
    fn static_program_targets(
        &mut self,
        branch: light_engine::PreloadBranch,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
    ) -> Result<Vec<(FixtureId, ProgrammingOwner)>, TransitionError> {
        self.observer(branch)
            .static_program_targets(frame, baseline)
    }
    fn prepare_programs(
        &mut self,
        branch: light_engine::PreloadBranch,
        frame: HybridFrameContext<'_>,
        programs: &[super::super::super::programming_projection::hybrid::HybridFamilyProgram<'_>],
    ) -> Result<(), TransitionError> {
        self.observer(branch).prepare_programs(frame, programs)
    }

    fn compose_position_batch(
        &mut self,
        branch: light_engine::PreloadBranch,
        frame: HybridFrameContext<'_>,
        composer: &mut dyn super::super::super::programming_projection::hybrid::HybridPositionBatchComposer<PhysicalHeadResult<PositionAdapter>>,
    ) -> Result<
        Option<
            super::super::super::programming_projection::hybrid::HybridPositionBatchResult<
                PhysicalHeadResult<PositionAdapter>,
            >,
        >,
        TransitionError,
    > {
        self.observer(branch)
            .compose_position_batch(frame, composer)
    }

    fn compose_program(
        &mut self,
        branch: light_engine::PreloadBranch,
        program: super::super::super::programming_projection::hybrid::HybridFamilyProgram<'_>,
        composer: &mut dyn super::super::super::programming_projection::hybrid::HybridProgramComposer<
            PhysicalHeadResult<PositionAdapter>,
        >,
    ) -> Result<Option<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>, TransitionError>
    {
        self.observer(branch).compose_program(program, composer)
    }

    fn observe(
        &mut self,
        branch: light_engine::PreloadBranch,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<
        (
            FamilyProjectionMetadata,
            PhysicalHeadResult<PositionAdapter>,
        ),
        TransitionError,
    > {
        self.observer(branch).observe(observation)
    }
    fn finish(
        &mut self,
        branch: light_engine::PreloadBranch,
        frame: HybridFrameContext<'_>,
        rows: &mut Vec<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        self.observer(branch).finish(frame, rows, requirements)
    }
}
