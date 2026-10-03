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
mod frame_observer;
pub(in crate::runtime) mod native_rows;
#[cfg(test)]
pub(in crate::runtime) mod tests;
mod tracking;
use frame_observer::PendingPosition;
pub(in crate::runtime) use frame_observer::{PositionFrameObserver, PositionPreloadObserver};

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
    /// TL-596: the last accepted tracking census as (capture time, changed Points, dirty
    /// instances). `None` until a frame with registered Point dependencies was accepted.
    pub fn tracking_census(&self) -> Option<(chrono::DateTime<chrono::Utc>, usize, usize)> {
        self.tracking.borrow().snapshot().map(|snapshot| {
            (
                snapshot.token().sampled_at(),
                snapshot.changed_points().len(),
                snapshot.dirty_instances().len(),
            )
        })
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
        validate_cohort(requests, first)?;
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
        for (root, mut group) in cohort_groups(requests) {
            group.sort_by_key(|(_, request)| request.target.0);
            let cohort_owners = group
                .iter()
                .map(|(_, request)| (request.target, request.descriptor.emitters.as_ref()))
                .collect::<Vec<_>>();
            let mut native = CapturedNativeRaw::default();
            for instance in &group[0].1.descriptor.instances {
                capture_instance_native(first, root, instance, &mut native)?;
                let mut scratch = instance.scratch.lock();
                scratch.raw.copy_from_slice(native.raw());
                scratch.previous.fill(None);
                scratch.requests.fill(None);
                let protected = protected_roots.contains(&root);
                seed_accepted_continuity(instance, &group, &native, protected, &mut scratch)?;
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
                    request_cohort_goals(&group, programs, instance, missing_mount, &mut scratch)?;
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
                let (memo, reused, evaluations) =
                    fit_or_reuse(&group, instance, &fit_input, workspace, output)?;
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
                publish_fitted_instance(
                    &mut results,
                    &group,
                    &FittedInstance {
                        instance,
                        native_raw: native.raw(),
                        solved_output,
                        proposed_raw,
                        achieved_axes,
                        memo: &memo,
                        evaluations,
                        reused,
                        geometry_dirty,
                        missing_mount,
                        protected,
                    },
                );
            }
        }
        Ok(results)
    }
}
/// Requests grouped by physical root, in first-seen root order, each with its result index.
#[allow(clippy::type_complexity)]
fn cohort_groups<'r, 'a>(
    requests: &'r [PhysicalRequest<'a, PositionAdapter>],
) -> Vec<(
    FixtureId,
    Vec<(usize, &'r PhysicalRequest<'a, PositionAdapter>)>,
)> {
    let mut groups: Vec<(
        FixtureId,
        Vec<(usize, &PhysicalRequest<'_, PositionAdapter>)>,
    )> = Vec::new();
    let mut root_indices = FxHashMap::default();
    for (index, r) in requests.iter().enumerate() {
        let group = *root_indices.entry(r.descriptor.root).or_insert_with(|| {
            groups.push((r.descriptor.root, Vec::new()));
            groups.len() - 1
        });
        groups[group].1.push((index, r));
    }
    groups
}

/// Captures one instance's native raw values from the cohort frame and checks their provenance.
fn capture_instance_native(
    first: &PhysicalRequest<'_, PositionAdapter>,
    root: FixtureId,
    instance: &PositionInstance,
    native: &mut CapturedNativeRaw,
) -> Result<(), TransitionError> {
    first
        .frame
        .native_position_raw_into(root, instance.destination.0, native)?;
    if native.token() != Some(first.frame.token)
        || native.destination() != Some(root)
        || native.instance_id() != Some(instance.destination.0)
    {
        return Err(invalid(
            "Position native capture does not match cohort instance",
        ));
    }
    Ok(())
}

/// Reuses the cohort's accepted fit memo, or fits this instance afresh and memoizes the fit.
/// Returns the memo, whether it was reused, and the fit's candidate evaluations.
fn fit_or_reuse(
    group: &[(usize, &PhysicalRequest<'_, PositionAdapter>)],
    instance: &PositionInstance,
    fit_input: &fit_cache::PositionFitMemoInput<'_>,
    workspace: &mut PositionFitWorkspace,
    output: &mut [PositionFitResult],
) -> Result<(Option<Arc<fit_cache::PositionFitMemo>>, bool, usize), TransitionError> {
    let accepted_memo = accepted_fit_memo(group, instance, fit_input);
    let reused = accepted_memo.is_some();
    let memo = if let Some(memo) = accepted_memo {
        Some(memo)
    } else {
        instance
            .model
            .fit(fit_input.fit, workspace, output)
            .map_err(|e| invalid(format!("Position fitting capture rejected: {e:?}")))?;
        fit_cache::PositionFitMemo::new(
            fit_input,
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
    Ok((memo, reused, evaluations))
}

/// Every request in one Position cohort shares one captured frame token, geometry and static
/// scalar frame, and carries a valid Position intent.
fn validate_cohort(
    requests: &[PhysicalRequest<'_, PositionAdapter>],
    first: &PhysicalRequest<'_, PositionAdapter>,
) -> Result<(), TransitionError> {
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
    Ok(())
}

/// Seeds one instance's fit scratch with the accepted continuity its cohort may reuse.
fn seed_accepted_continuity(
    instance: &PositionInstance,
    group: &[(usize, &PhysicalRequest<'_, PositionAdapter>)],
    native: &CapturedNativeRaw,
    protected: bool,
    scratch: &mut PositionScratch,
) -> Result<(), TransitionError> {
    let mut compatible_previous = Vec::new();
    for (_, r) in group {
        if let Some(previous) = r.previous.and_then(|p| {
            p.instances.iter().find(|p| {
                p.destination == instance.destination && p.compatibility == instance.compatibility
            })
        }) {
            // A scene/native edit wins over previous fitted output. Reuse branch
            // anchors and writes only while the complete captured footprint agrees.
            let compatible = previous.controls.iter().all(|&(index, id, baseline, raw)| {
                instance
                    .model
                    .axes()
                    .iter()
                    .flat_map(|a| a.controls.iter())
                    .any(|c| c.channel_index == index && c.channel_id == id && raw <= c.raw_max)
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
            if let Some(axis) = instance.model.axes().iter().find(|a| a.node_id == *id) {
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
    Ok(())
}

/// Writes each cohort request's goal (or its program's copy for this instance) to its emitters.
fn request_cohort_goals(
    group: &[(usize, &PhysicalRequest<'_, PositionAdapter>)],
    programs: &[(FixtureId, &[PositionProgramDestination])],
    instance: &PositionInstance,
    missing_mount: bool,
    scratch: &mut PositionScratch,
) -> Result<(), TransitionError> {
    for (_, r) in group {
        let program = programs
            .iter()
            .find(|(target, _)| *target == r.target)
            .map(|(_, program)| program);
        let value = match program {
            Some(destinations) => {
                &destinations
                    .iter()
                    .find(|d| d.destination == instance.destination)
                    .ok_or_else(|| invalid("Position program omitted a physical copy"))?
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
    Ok(())
}

/// A memo has authority only through every peer's accepted lane continuity.
fn accepted_fit_memo(
    group: &[(usize, &PhysicalRequest<'_, PositionAdapter>)],
    instance: &PositionInstance,
    fit_input: &fit_cache::PositionFitMemoInput<'_>,
) -> Option<Arc<fit_cache::PositionFitMemo>> {
    group[0]
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
        .filter(|memo| memo.matches(fit_input))
}

/// One instance's fitted output, shared by every peer request in its cohort.
struct FittedInstance<'a> {
    instance: &'a PositionInstance,
    native_raw: &'a [u32],
    solved_output: &'a [PositionFitResult],
    proposed_raw: &'a [u32],
    achieved_axes: &'a [Option<f64>],
    memo: &'a Option<Arc<fit_cache::PositionFitMemo>>,
    evaluations: usize,
    reused: bool,
    geometry_dirty: bool,
    missing_mount: bool,
    protected: bool,
}

/// Records one fitted instance's outcomes, writes and continuity on each peer's resolution.
fn publish_fitted_instance(
    results: &mut [PhysicalResolution<PositionAdapter>],
    group: &[(usize, &PhysicalRequest<'_, PositionAdapter>)],
    fitted: &FittedInstance<'_>,
) {
    for (result_index, r) in group {
        let resolved = &mut results[*result_index];
        resolved.quality.candidate_evaluations += fitted.evaluations;
        resolved.quality.reused_fits += usize::from(fitted.reused);
        resolved.quality.geometry_dirty |= fitted.geometry_dirty;
        for &index in r.descriptor.emitters.iter() {
            let mut value = fitted.solved_output[index].clone();
            if fitted.missing_mount {
                value.pose = None;
            }
            resolved.quality.held |= fitted.protected || value.status != PositionFitStatus::Fitted;
            resolved.achieved.outcomes.push(PositionOutcome {
                destination: fitted.instance.destination,
                result: value,
                missing_mount: fitted.missing_mount,
                input_requirement: fitted.protected,
            });
        }
        let mut controls = Vec::new();
        for slot in r
            .descriptor
            .footprint
            .iter()
            .filter(|s| s.destination == fitted.instance.destination)
        {
            let metadata = fitted
                .instance
                .model
                .axes()
                .iter()
                .filter(|a| a.role.is_some())
                .flat_map(|a| a.controls.iter())
                .find(|c| c.channel_index == slot.channel_index)
                .unwrap();
            let write = fitted
                .solved_output
                .iter()
                .flat_map(|o| o.writes.iter().flatten())
                .find(|w| w.channel_index == slot.channel_index);
            let raw = fitted.proposed_raw[slot.channel_index as usize];
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
                fitted.native_raw[slot.channel_index as usize],
                raw,
            ));
        }
        resolved
            .continuity
            .instances
            .push(PositionInstanceContinuity {
                destination: fitted.instance.destination,
                compatibility: fitted.instance.compatibility,
                joints: fitted
                    .instance
                    .model
                    .axes()
                    .iter()
                    .map(|a| (a.node_id, fitted.achieved_axes[a.command_index]))
                    .collect(),
                controls,
                fit_memo: fitted.memo.clone(),
            });
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
