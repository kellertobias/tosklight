//! Branch observers that capture Position evidence for one Live or retained frame and compose
//! the frame's Position programs from it.

use super::*;

pub(super) struct PendingPosition {
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
    pub(super) active_programs: Arc<[FixtureId]>,
    /// Physical ownership in this observer's capture, including roots without DMX heads.
    pub(super) roots: BTreeMap<Uuid, usize>,
}
impl<'a> PositionFrameObserver<'a> {
    pub fn new(lane: &'a PhysicalAdapterLane<PositionAdapter>) -> Self {
        Self {
            lane,
            pending: Vec::new(),
            current: Default::default(),
            programs: Vec::new(),
            active_programs: Default::default(),
            roots: Default::default(),
        }
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
        for fixture in frame.capture.snapshot().fixtures.iter() {
            for target in std::iter::once(fixture.fixture_id)
                .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
            {
                if !matches!(
                    baseline.value(target, &ProgrammingOwner::Position.key()),
                    Some(AttributeValue::Position(_))
                ) {
                    continue;
                }
                match self
                    .lane
                    .descriptor(frame, target, ProgrammingOwner::Position)
                {
                    Ok(_) => {
                        if !targets.contains(&(target, ProgrammingOwner::Position)) {
                            targets.push((target, ProgrammingOwner::Position));
                        }
                    }
                    Err(TransitionError::Requires(_)) => {} // No owned compiled emitter: preserve ordinary baseline output.
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(targets)
    }
    fn prepare_programs(
        &mut self,
        frame: HybridFrameContext<'_>,
        programs: &[super::super::super::programming_projection::hybrid::HybridFamilyProgram<'_>],
    ) -> Result<(), TransitionError> {
        self.programs.clear();
        let mut active_programs = Vec::new();
        for p in programs
            .iter()
            .filter(|p| p.owner == ProgrammingOwner::Position)
        {
            if p.frame.token != frame.token
                || self
                    .programs
                    .iter()
                    .any(|program| program.target == p.target)
            {
                return Err(invalid(
                    "Position registry contains a foreign or duplicate program",
                ));
            }
            if !p.samples.is_empty() || p.has_requirements {
                active_programs.push(p.target);
            }
            self.programs.push(CapturedPositionPeer {
                target: p.target,
                requested: Arc::new(PositionProgram {
                    base: p.base.clone(),
                    samples: p.samples.to_vec().into(),
                }),
                has_requirements: p.has_requirements,
                captured: Arc::new(HybridCapturedPositionProgram::new(
                    frame.token,
                    p.target,
                    p.base,
                    p.samples,
                )?),
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
        if p.owner != ProgrammingOwner::Position {
            return Ok(None);
        }
        let (program, captured) = self
            .programs
            .iter()
            .find(|program| program.target == p.target)
            .map(|program| {
                (
                    Arc::clone(&program.requested),
                    Arc::clone(&program.captured),
                )
            })
            .ok_or_else(|| invalid("Position program was not collected before composition"))?;
        let descriptor = self.lane.descriptor(p.frame, p.target, p.owner)?;
        let previous = self.lane.continuity(p.target, p.owner);
        let mut destinations = Vec::with_capacity(descriptor.instances.len());
        let mut representative = None;
        let current = self.current.clone();
        let active_programs = Arc::clone(&self.active_programs);
        for instance in &descriptor.instances {
            let bound = destination::PositionDestinationFrame {
                adapter: self.lane.adapter(),
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
            let pending_start = self.pending.len();
            let mut evaluation =
                composer.begin_position(&captured, instance.destination, &adoption)?;
            let result = match composer.advance_position(&mut evaluation, &bound, &adoption) {
                Ok(light_dynamics::PositionCompositionProgress::Complete(_)) => composer
                    .observe_position(&mut evaluation, &mut |observation| {
                        self.observe(observation)
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
            self.pending.truncate(pending_start);
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
        let mut row = representative
            .ok_or_else(|| invalid("Position program has no physical destinations"))?;
        let program = if p.samples.is_empty() {
            None
        } else {
            Some(program)
        };
        if let Some(program) = &program {
            row.sidecar.requested = PositionRequest::Program(Arc::clone(program));
        }
        self.pending.push(PendingPosition {
            target: p.target,
            descriptor,
            previous,
            program,
            destinations,
        });
        Ok(Some(row))
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
        let descriptor = self.lane.descriptor(o.frame, o.target, o.owner)?;
        let requested = intent(o.value)?.clone();
        let fields = self.lane.adapter().consumed_fields(o.owner, o.value)?;
        let mut sources = DynamicFamilySourceProjection::default();
        o.project_fields(&fields, &mut sources)?;
        let provenance = PhysicalProvenance {
            controls: o.controls_for_fields(&fields),
            fields,
            sources,
        };
        let metadata = self
            .lane
            .adapter()
            .projection_metadata(o.owner, &provenance);
        self.pending.push(PendingPosition {
            target: o.target,
            descriptor,
            previous: self.lane.continuity(o.target, o.owner),
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
    fn finish(
        &mut self,
        frame: HybridFrameContext<'_>,
        rows: &mut Vec<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        let requests = rows
            .iter()
            .map(|row| {
                let pending = self
                    .pending
                    .iter()
                    .find(|p| p.target == row.target)
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
        let programs = self
            .pending
            .iter()
            .filter_map(|p| {
                p.program
                    .as_ref()
                    .map(|_| (p.target, p.destinations.as_slice()))
            })
            .collect::<Vec<_>>();
        let resolved = self
            .lane
            .adapter()
            .resolve_cohort_programs(&requests, &protected, &programs)?;
        drop(requests);
        for (row, mut resolution) in rows.iter_mut().zip(resolved) {
            let pending = self
                .pending
                .iter()
                .find(|p| p.target == row.target)
                .unwrap();
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
        self.pending.clear();
        self.current.clear();
        Ok(())
    }
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
