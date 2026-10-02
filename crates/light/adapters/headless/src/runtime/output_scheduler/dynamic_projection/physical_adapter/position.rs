//! Calibrated Position binding to one immutable captured frame (TL-556).
//! All requested emitters of a physical root/copy are solved together. Requested values,
//! captured mounts/Points and accepted lane continuity remain distinct from fitted output.
use super::*;
use light_core::programming::{PositionIntent, ScalarIntent, TargetReference};
use light_core::spatial::RigidTransform;
use light_engine::{CapturedNativeRaw, profile_head_destinations};
use light_fixture::{
    CompiledPositionFitting, PatchedFixture, PositionAxisRole, PositionFitInput,
    PositionFitRequest, PositionFitResult, PositionFitStatus, PositionFitWorkspace,
    forward::PositionInstallation,
};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use uuid::Uuid;
mod current_cohort;
mod cut_coordinator;
mod destination;
mod fit_cache;
pub(in crate::runtime) mod native_rows;
#[cfg(test)]
pub(in crate::runtime) mod tests;
mod tracking;

pub(in crate::runtime) struct PositionDescriptor {
    pub root: FixtureId,
    pub instances: Vec<Arc<PositionInstance>>,
    pub emitters: Box<[usize]>,
    pub footprint: Box<[NativeControlSlot]>,
}
pub(in crate::runtime) struct PositionInstance {
    pub destination: FixtureId,
    pub model: CompiledPositionFitting,
    mount_reference: Option<Uuid>,
    compatibility: [u8; 32],
    scratch: parking_lot::Mutex<PositionScratch>,
}
struct PositionScratch {
    workspace: PositionFitWorkspace,
    output: Vec<PositionFitResult>,
    raw: Vec<u32>,
    available: Vec<bool>,
    previous: Vec<Option<f64>>,
    requests: Vec<Option<PositionFitRequest>>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::runtime) struct PositionContinuity {
    pub instances: Vec<PositionInstanceContinuity>,
}
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct PositionInstanceContinuity {
    pub destination: FixtureId,
    pub joints: Vec<(Uuid, Option<f64>)>,
    pub compatibility: [u8; 32],
    /// index, channel identity, captured baseline, accepted write.
    pub controls: Vec<(u32, Uuid, u32, u32)>,
    fit_memo: Option<Arc<fit_cache::PositionFitMemo>>,
}
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct PositionOutcome {
    pub destination: FixtureId,
    pub result: PositionFitResult,
    pub missing_mount: bool,
    pub input_requirement: bool,
}
/// The original logical program stays distinct from its destination-specific calculations.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) enum PositionRequest {
    Intent(PositionIntent),
    Program(Arc<PositionProgram>),
}
impl PartialEq<PositionIntent> for PositionRequest {
    fn eq(&self, other: &PositionIntent) -> bool {
        matches!(self, Self::Intent(intent) if intent == other)
    }
}
pub(in crate::runtime) struct PositionProgram {
    pub base: AttributeValue,
    pub samples: Arc<[light_dynamics::FamilyCompositionSample]>,
}
impl std::fmt::Debug for PositionProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PositionProgram")
            .field("base", &self.base)
            .field("sample_count", &self.samples.len())
            .finish()
    }
}
impl PartialEq for PositionProgram {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
#[derive(Clone, Debug)]
pub(in crate::runtime) struct PositionProgramDestination {
    pub destination: FixtureId,
    /// A frame calculation, never authored or persisted programming.
    pub value: AttributeValue,
    pub provenance: PhysicalProvenance,
}

#[derive(Clone, Debug, Default)]
pub(in crate::runtime) struct AchievedPosition {
    pub outcomes: Vec<PositionOutcome>,
    /// Temporary destination calculations and their owned evidence, separate from requested
    /// original programming. Consumers must never use these as recording input.
    pub destinations: Vec<PositionProgramDestination>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::runtime) struct PositionQuality {
    pub held: bool,
    pub candidate_evaluations: usize,
    pub reused_fits: usize,
    pub geometry_dirty: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::runtime) struct PositionAdapterCounters {
    pub compiles: u64,
    pub fits: u64,
    pub candidate_evaluations: u64,
    pub fit_cache_hits: u64,
}
type InstanceCache = (
    Arc<Vec<PatchedFixture>>,
    FxHashMap<FixtureId, Option<Vec<Arc<PositionInstance>>>>,
);
#[derive(Default)]
pub(in crate::runtime) struct PositionAdapter {
    cache: RefCell<Option<InstanceCache>>,
    counters: Cell<PositionAdapterCounters>,
    tracking: RefCell<tracking::TrackingState>,
}
fn invalid(message: impl Into<String>) -> TransitionError {
    IntentError(message.into()).into()
}
fn intent(value: &AttributeValue) -> Result<&PositionIntent, TransitionError> {
    match value {
        AttributeValue::Position(value) => Ok(value),
        _ => Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        )),
    }
}
fn scalar(value: &ScalarIntent) -> Option<f64> {
    match value {
        ScalarIntent::Value(value) if value.is_finite() => Some(f64::from(*value)),
        _ => None,
    }
}
fn request(
    value: &PositionIntent,
    frame: HybridFrameContext<'_>,
    mount_known: bool,
) -> Result<PositionFitRequest, TransitionError> {
    match value {
        PositionIntent::Angles {
            pan_degrees,
            tilt_degrees,
        } => Ok(PositionFitRequest::Angles {
            pan: scalar(pan_degrees).ok_or_else(|| invalid("Position spread is not resolved"))?,
            tilt: scalar(tilt_degrees).ok_or_else(|| invalid("Position spread is not resolved"))?,
        }),
        PositionIntent::Target {
            reference,
            offset_metres,
        } => {
            let offset = offset_metres
                .iter()
                .map(scalar)
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| invalid("Position spread is not resolved"))?;
            let offset = [offset[0], offset[1], offset[2]];
            let world = if !mount_known {
                None
            } else {
                match reference {
                    TargetReference::Origin => Some(offset),
                    TargetReference::Point { point_id } => {
                        frame.geometry.point(FixtureId(*point_id)).and_then(|p| {
                            let rotation =
                                RigidTransform::euler_xyz(p.rotation_degrees.map(f64::from))?;
                            let local = rotation.direction(offset);
                            Some(std::array::from_fn(|i| {
                                f64::from(p.origin_metres[i])
                                    + f64::from(p.offset_metres[i])
                                    + local[i]
                            }))
                        })
                    }
                }
            };
            Ok(PositionFitRequest::Target {
                world: world.map(|p| RigidTransform::DESK_TO_PROFILE.point(p)),
            })
        }
    }
}
impl PositionAdapter {
    pub fn counters(&self) -> PositionAdapterCounters {
        self.counters.get()
    }
    fn instances(
        &self,
        snapshot: &EngineSnapshot,
        fixture: &PatchedFixture,
    ) -> Result<Option<Vec<Arc<PositionInstance>>>, TransitionError> {
        let mut cache = self.cache.borrow_mut();
        if !cache
            .as_ref()
            .is_some_and(|(fixtures, _)| Arc::ptr_eq(fixtures, &snapshot.fixtures))
        {
            *cache = Some((snapshot.fixtures.clone(), FxHashMap::default()));
        }
        let entries = &mut cache.as_mut().unwrap().1;
        if let Some(found) = entries.get(&fixture.fixture_id) {
            return Ok(found.clone());
        }
        let (Some(profile), Some(mode)) = (
            fixture.definition.profile_snapshot.as_deref(),
            fixture.definition.mode_id,
        ) else {
            return Ok(None);
        };
        let mut instances = Vec::new();
        for copy in std::iter::once(None).chain(fixture.multipatch.iter().map(Some)) {
            let (destination, installed, mount_reference) = match copy {
                None => (
                    fixture.fixture_id,
                    PositionInstallation {
                        calibration: fixture.position_calibration.as_ref(),
                        invert_pan: fixture.invert_pan,
                        invert_tilt: fixture.invert_tilt,
                        bracket_degrees: f64::from(fixture.bracket_angle),
                    },
                    fixture.position_master,
                ),
                Some(copy) => (
                    FixtureId(copy.id),
                    PositionInstallation {
                        calibration: copy.position_calibration.as_ref(),
                        invert_pan: copy.invert_pan,
                        invert_tilt: copy.invert_tilt,
                        bracket_degrees: f64::from(copy.bracket_angle),
                    },
                    fixture.position_master,
                ),
            };
            let model = match CompiledPositionFitting::compile(profile, mode, installed) {
                Ok(Some(model)) => model,
                _ => {
                    entries.insert(fixture.fixture_id, None);
                    return Ok(None);
                }
            };
            // Conservative Position-only compatibility: unrelated profile labels and Color
            // functions cannot reset an accepted unwrapped mechanical branch. The calibration
            // identity covers physical interpretation; compiled ownership supplements its
            // native layout and head bindings. Fresh native baselines remain checked per frame.
            let position_identity = profile
                .position_calibration_identity(mode)
                .map_err(|e| invalid(e.to_string()))?;
            let controls = |controls: &[light_fixture::PositionFitControlMetadata]| {
                controls
                    .iter()
                    .map(|c| (c.channel_index, c.channel_id, c.split, c.raw_max))
                    .collect::<Vec<_>>()
            };
            let axes = model
                .axes()
                .iter()
                .map(|axis| {
                    (
                        axis.command_index,
                        axis.node_id,
                        axis.role,
                        controls(&axis.controls),
                    )
                })
                .collect::<Vec<_>>();
            let emitters = model
                .emitters()
                .map(|emitter| {
                    (
                        emitter.emitter_index,
                        emitter.emitter_id,
                        emitter.head_id,
                        emitter.command_indices,
                        emitter.ancestor_axes,
                        controls(emitter.controls),
                    )
                })
                .collect::<Vec<_>>();
            let compatibility = Sha256::digest(
                serde_json::to_vec(&(
                    position_identity,
                    axes,
                    emitters,
                    installed.calibration,
                    installed.invert_pan,
                    installed.invert_tilt,
                    installed.bracket_degrees,
                ))
                .map_err(|e| invalid(e.to_string()))?,
            )
            .into();
            let scratch = PositionScratch {
                workspace: model.create_workspace(),
                output: model.create_output(),
                raw: vec![0; profile.mode(mode).unwrap().channels.len()],
                available: vec![true; profile.mode(mode).unwrap().channels.len()],
                previous: vec![None; model.axes().len()],
                requests: vec![None; model.emitters().len()],
            };
            instances.push(Arc::new(PositionInstance {
                destination,
                model,
                mount_reference,
                compatibility,
                scratch: parking_lot::Mutex::new(scratch),
            }));
        }
        entries.insert(fixture.fixture_id, Some(instances.clone()));
        Ok(Some(instances))
    }

    /// Full requested cohort under one token, before any family projection. Generic input
    /// requirements protect a physical root conservatively; they are not fabricated Targets.
    pub fn resolve_cohort(
        &self,
        requests: &[PhysicalRequest<'_, Self>],
        protected_roots: &[FixtureId],
    ) -> Result<Vec<PhysicalResolution<Self>>, TransitionError> {
        self.resolve_cohort_programs(requests, protected_roots, &[])
    }

    fn resolve_cohort_programs(
        &self,
        requests: &[PhysicalRequest<'_, Self>],
        protected_roots: &[FixtureId],
        programs: &[(FixtureId, &[PositionProgramDestination])],
    ) -> Result<Vec<PhysicalResolution<Self>>, TransitionError> {
        let Some(first) = requests.first() else {
            return Ok(Vec::new());
        };
        for r in requests {
            if r.frame.token != first.frame.token
                || r.owner != ProgrammingOwner::Position
                || !r.frame.token.matches_geometry(r.frame.geometry)
                || !r.frame.token.matches_static_frame(r.frame.scalar)
                || !std::ptr::eq(r.frame.scalar, first.frame.scalar)
                || !std::ptr::eq(r.frame.geometry, first.frame.geometry)
            {
                return Err(invalid("Position cohort uses foreign frame or owner"));
            }
            intent(r.value)?.validate()?;
        }
        let mut results: Vec<PhysicalResolution<Self>> = requests
            .iter()
            .map(|r| PhysicalResolution {
                writes: Vec::new(),
                requested: PositionRequest::Intent(intent(r.value).unwrap().clone()),
                achieved: AchievedPosition::default(),
                quality: PositionQuality::default(),
                continuity: PositionContinuity::default(),
            })
            .collect();
        let mut groups: Vec<(FixtureId, Vec<(usize, &PhysicalRequest<'_, Self>)>)> = Vec::new();
        let mut root_indices = FxHashMap::default();
        for (index, r) in requests.iter().enumerate() {
            let group = *root_indices.entry(r.descriptor.root).or_insert_with(|| {
                groups.push((r.descriptor.root, Vec::new()));
                groups.len() - 1
            });
            groups[group].1.push((index, r));
        }
        for (root, mut group) in groups {
            group.sort_by_key(|(_, request)| request.target.0);
            let cohort_owners = group
                .iter()
                .map(|(_, request)| (request.target, request.descriptor.emitters.as_ref()))
                .collect::<Vec<_>>();
            let mut native = CapturedNativeRaw::default();
            for instance in &group[0].1.descriptor.instances {
                first
                    .frame
                    .native_position_raw_into(root, instance.destination.0, &mut native)?;
                if native.token() != Some(first.frame.token)
                    || native.destination() != Some(root)
                    || native.instance_id() != Some(instance.destination.0)
                {
                    return Err(invalid(
                        "Position native capture does not match cohort instance",
                    ));
                }
                let mut scratch = instance.scratch.lock();
                scratch.raw.copy_from_slice(native.raw());
                scratch.previous.fill(None);
                scratch.requests.fill(None);
                let protected = protected_roots.contains(&root);
                let mut compatible_previous = Vec::new();
                for (_, r) in &group {
                    if let Some(previous) = r.previous.and_then(|p| {
                        p.instances.iter().find(|p| {
                            p.destination == instance.destination
                                && p.compatibility == instance.compatibility
                        })
                    }) {
                        // A scene/native edit wins over previous fitted output. Reuse branch
                        // anchors and writes only while the complete captured footprint agrees.
                        let compatible =
                            previous.controls.iter().all(|&(index, id, baseline, raw)| {
                                instance
                                    .model
                                    .axes()
                                    .iter()
                                    .flat_map(|a| a.controls.iter())
                                    .any(|c| {
                                        c.channel_index == index
                                            && c.channel_id == id
                                            && raw <= c.raw_max
                                    })
                                    && native.raw().get(index as usize) == Some(&baseline)
                            });
                        if compatible {
                            compatible_previous.push(previous);
                        }
                    }
                }
                // Keep an accepted hold only when it covers every mechanical control
                // and comes from one complete accepted fit. Otherwise use the complete
                // captured baseline; never seed only a surviving peer's old controls.
                let complete_previous = protected
                    && instance
                        .model
                        .axes()
                        .iter()
                        .filter(|axis| axis.role.is_some())
                        .flat_map(|axis| axis.controls.iter())
                        .all(|control| {
                            compatible_previous.iter().any(|previous| {
                                previous.controls.iter().any(|&(index, id, _, _)| {
                                    index == control.channel_index && id == control.channel_id
                                })
                            })
                        })
                    && (compatible_previous.len() == 1
                        || compatible_previous
                            .first()
                            .and_then(|previous| previous.fit_memo.as_ref())
                            .is_some_and(|memo| {
                                compatible_previous.iter().all(|previous| {
                                    previous
                                        .fit_memo
                                        .as_ref()
                                        .is_some_and(|other| Arc::ptr_eq(memo, other))
                                })
                            }));
                let mut seeded = FxHashMap::default();
                for previous in compatible_previous
                    .iter()
                    .filter(|_| !protected || complete_previous)
                {
                    for (id, value) in &previous.joints {
                        if let Some(axis) = instance.model.axes().iter().find(|a| a.node_id == *id)
                        {
                            scratch.previous[axis.command_index] = *value;
                        }
                    }
                    for &(index, _id, _baseline, raw) in &previous.controls {
                        if seeded.insert(index, raw).is_some_and(|old| old != raw) {
                            return Err(invalid(
                                "accepted Position continuity disagrees on shared control",
                            ));
                        }
                        scratch.raw[index as usize] = raw;
                    }
                }
                let mount = first
                    .frame
                    .geometry
                    .mounts()
                    .mount(instance.destination.0)
                    .and_then(|m| m.world_from_fixture);
                let reference_known = instance
                    .mount_reference
                    .is_none_or(|id| first.frame.geometry.point(FixtureId(id)).is_some());
                let missing_mount = mount.is_none() || !reference_known;
                if !protected {
                    for (_, r) in &group {
                        let program = programs
                            .iter()
                            .find(|(target, _)| *target == r.target)
                            .map(|(_, program)| program);
                        let value = match program {
                            Some(destinations) => {
                                &destinations
                                    .iter()
                                    .find(|d| d.destination == instance.destination)
                                    .ok_or_else(|| {
                                        invalid("Position program omitted a physical copy")
                                    })?
                                    .value
                            }
                            None => r.value,
                        };
                        let desired = request(intent(value)?, r.frame, !missing_mount)?;
                        for &index in r.descriptor.emitters.iter() {
                            if scratch.requests[index].replace(desired).is_some() {
                                return Err(invalid("Position emitter has two requested owners"));
                            }
                        }
                    }
                }
                let PositionScratch {
                    workspace,
                    output,
                    raw,
                    available,
                    previous,
                    requests: goals,
                } = &mut *scratch;
                let geometry_dirty = self
                    .tracking
                    .borrow()
                    .dirty_instance(root, instance.destination);
                let fit_input = fit_cache::PositionFitMemoInput {
                    root,
                    destination: instance.destination,
                    generation: first.frame.token.generation(),
                    lane: first.frame.token.lane(),
                    compatibility: &instance.compatibility,
                    owners: &cohort_owners,
                    fit: PositionFitInput {
                        current_raw: raw,
                        available,
                        requests: goals,
                        previous,
                        mount: mount
                            .unwrap_or(RigidTransform::IDENTITY)
                            .desk_pose_to_profile(),
                    },
                    native_baseline: native.raw(),
                    missing_mount,
                    protected,
                    geometry_dirty,
                };
                // A memo has authority only through every peer's accepted lane continuity.
                // Equal independently constructed cache values cannot certify one cohort.
                let accepted_memo = group[0]
                    .1
                    .previous
                    .and_then(|continuity| {
                        continuity
                            .instances
                            .iter()
                            .find(|prior| {
                                prior.destination == instance.destination
                                    && prior.compatibility == instance.compatibility
                            })
                            .and_then(|prior| prior.fit_memo.as_ref())
                            .cloned()
                    })
                    .filter(|memo| {
                        group.iter().all(|(_, request)| {
                            request
                                .previous
                                .and_then(|continuity| {
                                    continuity
                                        .instances
                                        .iter()
                                        .find(|prior| {
                                            prior.destination == instance.destination
                                                && prior.compatibility == instance.compatibility
                                        })
                                        .and_then(|prior| prior.fit_memo.as_ref())
                                })
                                .is_some_and(|other| Arc::ptr_eq(memo, other))
                        })
                    })
                    .filter(|memo| memo.matches(&fit_input));
                let reused = accepted_memo.is_some();
                let memo = if let Some(memo) = accepted_memo {
                    Some(memo)
                } else {
                    instance
                        .model
                        .fit(fit_input.fit, workspace, output)
                        .map_err(|e| {
                            invalid(format!("Position fitting capture rejected: {e:?}"))
                        })?;
                    fit_cache::PositionFitMemo::new(
                        &fit_input,
                        output,
                        workspace.proposed_raw(),
                        workspace.achieved_axes(),
                    )
                    .map(Arc::new)
                };
                let evaluations = if reused {
                    0
                } else {
                    workspace.candidate_evaluations()
                };
                let solved_output = memo
                    .as_ref()
                    .map_or_else(|| output.as_slice(), |memo| memo.output());
                let proposed_raw = memo
                    .as_ref()
                    .map_or_else(|| workspace.proposed_raw(), |memo| memo.proposed_raw());
                let achieved_axes = memo
                    .as_ref()
                    .map_or_else(|| workspace.achieved_axes(), |memo| memo.achieved_axes());
                let mut counters = self.counters.get();
                counters.fits += u64::from(!reused);
                counters.fit_cache_hits += u64::from(reused);
                counters.candidate_evaluations += evaluations as u64;
                self.counters.set(counters);
                for (result_index, r) in &group {
                    let resolved = &mut results[*result_index];
                    resolved.quality.candidate_evaluations += evaluations;
                    resolved.quality.reused_fits += usize::from(reused);
                    resolved.quality.geometry_dirty |= geometry_dirty;
                    for &index in r.descriptor.emitters.iter() {
                        let mut value = solved_output[index].clone();
                        if missing_mount {
                            value.pose = None;
                        }
                        resolved.quality.held |=
                            protected || value.status != PositionFitStatus::Fitted;
                        resolved.achieved.outcomes.push(PositionOutcome {
                            destination: instance.destination,
                            result: value,
                            missing_mount,
                            input_requirement: protected,
                        });
                    }
                    let mut controls = Vec::new();
                    for slot in r
                        .descriptor
                        .footprint
                        .iter()
                        .filter(|s| s.destination == instance.destination)
                    {
                        let metadata = instance
                            .model
                            .axes()
                            .iter()
                            .filter(|a| a.role.is_some())
                            .flat_map(|a| a.controls.iter())
                            .find(|c| c.channel_index == slot.channel_index)
                            .unwrap();
                        let write = solved_output
                            .iter()
                            .flat_map(|o| o.writes.iter().flatten())
                            .find(|w| w.channel_index == slot.channel_index);
                        let raw = proposed_raw[slot.channel_index as usize];
                        resolved.writes.push(NativeControlWrite {
                            slot: *slot,
                            channel_id: metadata.channel_id,
                            function_id: write.map(|w| w.function_id),
                            raw,
                            parked: write.is_none(),
                        });
                        controls.push((
                            slot.channel_index,
                            metadata.channel_id,
                            native.raw()[slot.channel_index as usize],
                            raw,
                        ));
                    }
                    resolved
                        .continuity
                        .instances
                        .push(PositionInstanceContinuity {
                            destination: instance.destination,
                            compatibility: instance.compatibility,
                            joints: instance
                                .model
                                .axes()
                                .iter()
                                .map(|a| (a.node_id, achieved_axes[a.command_index]))
                                .collect(),
                            controls,
                            fit_memo: memo.clone(),
                        });
                }
            }
        }
        Ok(results)
    }
}
impl PhysicalFamilyAdapter for PositionAdapter {
    type Descriptor = PositionDescriptor;
    type Continuity = PositionContinuity;
    type Requested = PositionRequest;
    type Achieved = AchievedPosition;
    type Quality = PositionQuality;
    fn owns(&self, owner: ProgrammingOwner) -> bool {
        owner == ProgrammingOwner::Position
    }
    fn begin_lane_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.tracking.borrow_mut().begin(token)
    }
    fn abandon_lane_frame(&self) {
        self.tracking.borrow_mut().abandon();
    }
    fn verify_lane_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.tracking.borrow().verify(token)
    }
    fn accept_lane_frame(&self, token: &CapturedFrameToken) {
        let accepted = self.tracking.borrow_mut().accept(token);
        debug_assert!(accepted, "verified Position dependency frame is accepted");
    }

    fn compile(
        &self,
        snapshot: &EngineSnapshot,
        target: FixtureId,
    ) -> Result<Option<PositionDescriptor>, TransitionError> {
        let mut counters = self.counters.get();
        counters.compiles += 1;
        self.counters.set(counters);
        let heads = profile_head_destinations(snapshot, target);
        // Root identity and emitter ownership are distinct from head ownership. An
        // unheaded emitter stays with its physical fixture even if every head is logical.
        let fixture_index = snapshot
            .fixtures
            .iter()
            .position(|fixture| fixture.fixture_id == target)
            .or_else(|| heads.first().map(|head| head.fixture_index));
        let Some(fixture_index) = fixture_index else {
            return Ok(None);
        };
        if heads.iter().any(|head| head.fixture_index != fixture_index) {
            return Err(invalid("Position target spans roots"));
        }
        let fixture = &snapshot.fixtures[fixture_index];
        let Some(instances) = self.instances(snapshot, fixture)? else {
            return Ok(None);
        };
        let emitters: Box<_> = instances[0]
            .model
            .emitters()
            .filter(|e| {
                e.head_id.map_or(target == fixture.fixture_id, |id| {
                    heads.iter().any(|h| h.head_id == id)
                })
            })
            .map(|e| e.emitter_index)
            .collect();
        if emitters.is_empty() {
            return Ok(None);
        }
        let mut footprint = Vec::new();
        for instance in &instances {
            for &index in emitters.iter() {
                let emitter = instance.model.emitter(index).unwrap();
                for &axis in emitter.ancestor_axes {
                    let axis = &instance.model.axes()[axis];
                    if !matches!(
                        axis.role,
                        Some(PositionAxisRole::Pan | PositionAxisRole::Tilt)
                    ) {
                        continue;
                    }
                    for control in axis.controls.iter() {
                        let slot = NativeControlSlot {
                            destination: instance.destination,
                            channel_index: control.channel_index,
                            split: control.split,
                        };
                        if !footprint.contains(&slot) {
                            footprint.push(slot);
                        }
                    }
                }
            }
        }
        Ok(Some(PositionDescriptor {
            root: fixture.fixture_id,
            instances,
            emitters,
            footprint: footprint.into_boxed_slice(),
        }))
    }
    fn footprint<'d>(&self, d: &'d PositionDescriptor) -> &'d [NativeControlSlot] {
        &d.footprint
    }
    fn adopt_with_continuity(
        &self,
        frame: HybridFrameContext<'_>,
        descriptor: &PositionDescriptor,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
        previous: Option<&PositionContinuity>,
    ) -> Result<AttributeValue, TransitionError> {
        if address.representation != light_dynamics::DynamicFamilyRepresentation::Angles {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        let requested = intent(original)?;
        if matches!(requested, PositionIntent::Angles { .. }) {
            return Ok(original.clone());
        }
        let result = self.resolve_cohort(
            &[PhysicalRequest {
                frame,
                target,
                owner: ProgrammingOwner::Position,
                descriptor,
                value: original,
                previous,
            }],
            &[],
        )?;
        let outcomes = &result[0].achieved.outcomes;
        let Some(first) = outcomes.first().and_then(|o| o.result.achieved) else {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        };
        // A single owner value cannot represent different copy/head joints. Retain the
        // expression's requirement until destination-bound forest evaluation is available.
        if outcomes.iter().any(|o| {
            o.result.status != PositionFitStatus::Fitted
                || o.result
                    .achieved
                    .is_none_or(|a| a.iter().zip(first).any(|(a, b)| (a - b).abs() > 1e-5))
        }) {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles,
            ));
        }
        Ok(AttributeValue::Position(Arc::new(PositionIntent::angles(
            first[0] as f32,
            first[1] as f32,
        ))))
    }
    fn transition(
        &self,
        frame: HybridFrameContext<'_>,
        _descriptor: &PositionDescriptor,
        _target: FixtureId,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        if requirement != TransitionRequirement::LiveTargetPoints
            || !frame.token.matches_geometry(frame.geometry)
        {
            return Err(TransitionError::Requires(requirement));
        }
        let world = |value| match request(intent(value)?, frame, true)? {
            PositionFitRequest::Target { world: Some(world) } => Ok(world),
            _ => Err(TransitionError::Requires(requirement)),
        };
        let (a, b) = (world(from)?, world(to)?);
        let factor = match operation {
            FamilyExpressionOperation::Transition { progress } => progress,
            FamilyExpressionOperation::Scale { factor } => factor,
        };
        if !factor.is_finite() {
            return Err(invalid("Position transition factor is nonfinite"));
        }
        let world = std::array::from_fn(|i| a[i] + f64::from(factor) * (b[i] - a[i]));
        let desk = RigidTransform::DESK_TO_PROFILE.inverse().point(world);
        Ok((
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Origin,
                desk.map(|v| v as f32),
            ))),
            None,
        ))
    }

    fn resolve(
        &self,
        r: PhysicalRequest<'_, Self>,
    ) -> Result<PhysicalResolution<Self>, TransitionError> {
        let _ = r;
        // This path cannot know which other logical targets requested a protected hold.
        // Position always uses PositionFrameObserver; unsupported consumers hold passively.
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles,
        ))
    }
}

use super::super::programming_projection::hybrid::{
    HybridCapturedPositionProgram, HybridFrameObserver, OwnedHybridProjection,
};
struct PendingPosition {
    target: FixtureId,
    descriptor: Arc<PositionDescriptor>,
    previous: Option<PositionContinuity>,
    program: Option<Arc<PositionProgram>>,
    destinations: Vec<PositionProgramDestination>,
}
struct CapturedPositionPeer {
    target: FixtureId,
    requested: Arc<PositionProgram>,
    captured: Arc<HybridCapturedPositionProgram>,
    has_requirements: bool,
}
/// Short-lived observer for one complete Live or retained branch. It owns evidence only;
/// no borrowed trace or application state survives an observation callback.
pub(in crate::runtime) struct PositionFrameObserver<'a> {
    lane: &'a PhysicalAdapterLane<PositionAdapter>,
    pending: Vec<PendingPosition>,
    current: current_cohort::CapturedCurrentCohorts,
    programs: Vec<CapturedPositionPeer>,
    active_programs: Arc<[FixtureId]>,
    /// Physical ownership in this observer's capture, including roots without DMX heads.
    roots: BTreeMap<Uuid, usize>,
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
        programs: &[super::super::programming_projection::hybrid::HybridFamilyProgram<'_>],
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
        composer: &mut dyn super::super::programming_projection::hybrid::HybridPositionBatchComposer<PhysicalHeadResult<PositionAdapter>>,
    ) -> Result<
        Option<
            super::super::programming_projection::hybrid::HybridPositionBatchResult<
                PhysicalHeadResult<PositionAdapter>,
            >,
        >,
        TransitionError,
    > {
        cut_coordinator::compose(self, frame, composer)
    }

    fn compose_program(
        &mut self,
        p: super::super::programming_projection::hybrid::HybridFamilyProgram<'_>,
        composer: &mut dyn super::super::programming_projection::hybrid::HybridProgramComposer<
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
    before: PositionFrameObserver<'a>,
    after: PositionFrameObserver<'a>,
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
    super::super::retained_preload_hybrid::RetainedHybridFrameObserver<
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
        programs: &[super::super::programming_projection::hybrid::HybridFamilyProgram<'_>],
    ) -> Result<(), TransitionError> {
        self.observer(branch).prepare_programs(frame, programs)
    }

    fn compose_position_batch(
        &mut self,
        branch: light_engine::PreloadBranch,
        frame: HybridFrameContext<'_>,
        composer: &mut dyn super::super::programming_projection::hybrid::HybridPositionBatchComposer<PhysicalHeadResult<PositionAdapter>>,
    ) -> Result<
        Option<
            super::super::programming_projection::hybrid::HybridPositionBatchResult<
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
        program: super::super::programming_projection::hybrid::HybridFamilyProgram<'_>,
        composer: &mut dyn super::super::programming_projection::hybrid::HybridProgramComposer<
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
