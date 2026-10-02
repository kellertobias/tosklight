//! Original Current is an explicit evaluation cut shared by every physical owner. Capture
//! all static Position peers before composition, then lazily fit that one cut per root/copy.
//! Generic transition endpoints deliberately cannot use this cache: equal values alone do
//! not prove that another owner's outgoing/incoming endpoint is the same evaluation cut.
use super::*;

struct CurrentOwner {
    target: FixtureId,
    descriptor: Arc<PositionDescriptor>,
    value: AttributeValue,
    previous: Option<PositionContinuity>,
}
struct CurrentCohort {
    root: FixtureId,
    owners: Vec<CurrentOwner>,
    protected: bool,
    resolved: RefCell<Option<Vec<PhysicalResolution<PositionAdapter>>>>,
}
#[derive(Default, Clone)]
pub(super) struct CapturedCurrentCohorts {
    token: Option<CapturedFrameToken>,
    groups: Arc<[CurrentCohort]>,
    owners: Arc<FxHashMap<FixtureId, (usize, usize)>>,
}
impl CapturedCurrentCohorts {
    pub fn clear(&mut self) {
        self.token = None;
        self.groups = Default::default();
        self.owners = Default::default();
    }
    pub fn capture(
        &mut self,
        lane: &PhysicalAdapterLane<PositionAdapter>,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
        protected: &[FixtureId],
    ) -> Result<(), TransitionError> {
        self.clear();
        if !frame.token.matches_static_frame(baseline) {
            return Err(invalid(
                "Position Current baseline belongs to another frame",
            ));
        }
        let snapshot = frame.capture.snapshot();
        let protected_targets = protected
            .iter()
            .copied()
            .collect::<rustc_hash::FxHashSet<_>>();
        let mut protected_roots = rustc_hash::FxHashSet::default();
        let mut root_indices = FxHashMap::default();
        let mut owner_indices = FxHashMap::default();
        let mut groups: Vec<CurrentCohort> = Vec::new();
        for fixture in snapshot.fixtures.iter() {
            for target in std::iter::once(fixture.fixture_id)
                .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
            {
                if protected_targets.contains(&target) {
                    protected_roots.insert(fixture.fixture_id);
                }
                let Some(value @ AttributeValue::Position(_)) =
                    baseline.value(target, &ProgrammingOwner::Position.key())
                else {
                    continue;
                };
                value.validate_programming_address(&ProgrammingOwner::Position.key())?;
                let descriptor = match lane.descriptor(frame, target, ProgrammingOwner::Position) {
                    Ok(descriptor) => descriptor,
                    Err(TransitionError::Requires(_)) => {
                        protected_roots.insert(fixture.fixture_id);
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                let index = *root_indices.entry(descriptor.root).or_insert_with(|| {
                    groups.push(CurrentCohort {
                        root: descriptor.root,
                        owners: Vec::new(),
                        protected: false,
                        resolved: RefCell::new(None),
                    });
                    groups.len() - 1
                });
                owner_indices.insert(target, (index, groups[index].owners.len()));
                groups[index].owners.push(CurrentOwner {
                    target,
                    descriptor,
                    value: value.clone(),
                    previous: lane.continuity(target, ProgrammingOwner::Position),
                });
            }
        }
        for group in &mut groups {
            group.protected = protected_roots.contains(&group.root);
        }
        self.groups = groups.into();
        self.owners = Arc::new(owner_indices);
        self.token = Some(frame.token.clone());
        Ok(())
    }
    /// A scoped endpoint may use only a complete captured mechanical membership. Missing
    /// semantic owners and unowned emitters are not evidence of a static peer.
    pub fn complete_members(
        &self,
        frame: HybridFrameContext<'_>,
        root: FixtureId,
    ) -> Result<Option<Vec<FixtureId>>, TransitionError> {
        if self.token.as_ref() != Some(frame.token) {
            return Err(invalid("Position cut membership belongs to another frame"));
        }
        let Some(cohort) = self.groups.iter().find(|cohort| cohort.root == root) else {
            return Ok(None);
        };
        if cohort.protected || cohort.owners.is_empty() {
            return Ok(None);
        }
        let reference = &cohort.owners[0].descriptor;
        if reference.instances.is_empty() {
            return Ok(None);
        }
        let mut covered = rustc_hash::FxHashSet::default();
        for owner in &cohort.owners {
            if owner.descriptor.root != root
                || owner.descriptor.instances.len() != reference.instances.len()
                || owner.descriptor.emitters.is_empty()
                || owner
                    .descriptor
                    .instances
                    .iter()
                    .zip(&reference.instances)
                    .any(|(a, b)| !Arc::ptr_eq(a, b))
            {
                return Ok(None);
            }
            for &emitter in owner.descriptor.emitters.iter() {
                if !covered.insert(emitter)
                    || reference
                        .instances
                        .iter()
                        .any(|instance| instance.model.emitter(emitter).is_none())
                {
                    return Ok(None);
                }
            }
        }
        if reference.instances.iter().any(|instance| {
            instance
                .model
                .emitters()
                .any(|emitter| !covered.contains(&emitter.emitter_index))
        }) {
            return Ok(None);
        }
        Ok(Some(
            cohort.owners.iter().map(|owner| owner.target).collect(),
        ))
    }

    /// A program-local endpoint is a complete cut only when every other participating owner
    /// is proven static. No graph node/progress/value equality establishes that property.
    /// Requirements-only Dynamic groups count as active, so their static underlay cannot be
    /// substituted for an unavailable endpoint. The solve is speculative and never staged.
    pub fn adopt_single_program_endpoint(
        &self,
        adapter: &PositionAdapter,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        destination: FixtureId,
        original: &AttributeValue,
        active_programs: &[FixtureId],
    ) -> Result<Option<AttributeValue>, TransitionError> {
        if self.token.as_ref() != Some(frame.token) {
            return Err(invalid("Position endpoint cohort belongs to another frame"));
        }
        let Some(&(group_index, index)) = self.owners.get(&target) else {
            return Ok(None);
        };
        let cohort = &self.groups[group_index];
        if cohort.protected
            || cohort
                .owners
                .iter()
                .any(|owner| owner.target != target && active_programs.contains(&owner.target))
        {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        let requests = cohort
            .owners
            .iter()
            .map(|owner| PhysicalRequest {
                frame,
                target: owner.target,
                owner: ProgrammingOwner::Position,
                descriptor: owner.descriptor.as_ref(),
                value: if owner.target == target {
                    original
                } else {
                    &owner.value
                },
                previous: owner.previous.as_ref(),
            })
            .collect::<Vec<_>>();
        let resolved = adapter.resolve_cohort(&requests, &[])?;
        let mut outcomes = resolved[index]
            .achieved
            .outcomes
            .iter()
            .filter(|outcome| outcome.destination == destination);
        let Some(first) = outcomes.next() else {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        };
        let Some(angles) = first.result.achieved else {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        };
        let usable = |outcome: &PositionOutcome| {
            !outcome.missing_mount
                && !outcome.input_requirement
                && outcome.result.status == PositionFitStatus::Fitted
                && outcome
                    .result
                    .achieved
                    .is_some_and(|pair| pair.iter().zip(angles).all(|(a, b)| (a - b).abs() <= 1e-5))
        };
        if !usable(first) || !outcomes.all(usable) {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        Ok(Some(AttributeValue::Position(Arc::new(
            PositionIntent::angles(angles[0] as f32, angles[1] as f32),
        ))))
    }

    /// Called only by the deferred Angle-pair's original Current callback. None means this
    /// is not the captured cut; the existing destination resolver retains its usual policy.
    pub fn adopt(
        &self,
        adapter: &PositionAdapter,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        destination: FixtureId,
        original: &AttributeValue,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        if self.token.as_ref() != Some(frame.token) {
            return Err(invalid("Position Current cohort belongs to another frame"));
        }
        let Some(&(group_index, index)) = self.owners.get(&target) else {
            return Ok(None);
        };
        let cohort = &self.groups[group_index];
        if &cohort.owners[index].value != original {
            return Ok(None);
        }
        if cohort.protected {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        let mut resolved = cohort.resolved.borrow_mut();
        if resolved.is_none() {
            let requests = cohort
                .owners
                .iter()
                .map(|owner| PhysicalRequest {
                    frame,
                    target: owner.target,
                    owner: ProgrammingOwner::Position,
                    descriptor: owner.descriptor.as_ref(),
                    value: &owner.value,
                    previous: owner.previous.as_ref(),
                })
                .collect::<Vec<_>>();
            // This speculative solve owns no accepted continuity or writes. Final composition
            // still fits its own complete cohort and only the finalizer may accept its result.
            *resolved = Some(adapter.resolve_cohort(&requests, &[])?);
        }
        let outcomes = resolved.as_ref().unwrap()[index]
            .achieved
            .outcomes
            .iter()
            .filter(|outcome| outcome.destination == destination)
            .collect::<Vec<_>>();
        let Some(first) = outcomes.first().and_then(|outcome| outcome.result.achieved) else {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        };
        if outcomes.iter().any(|outcome| {
            outcome.missing_mount
                || outcome.input_requirement
                || outcome.result.status != PositionFitStatus::Fitted
                || outcome.result.achieved.is_none_or(|angles| {
                    angles.iter().zip(first).any(|(a, b)| (a - b).abs() > 1e-5)
                })
        }) {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        Ok(Some(AttributeValue::Position(Arc::new(
            PositionIntent::angles(first[0] as f32, first[1] as f32),
        ))))
    }
}
