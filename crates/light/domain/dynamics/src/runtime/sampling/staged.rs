//! One frame plan separates legacy geometry output from typed Current evaluation. Both
//! stages use the same clocks, membership, controller ranks, addresses and Random decisions.
use super::*;
use crate::programming::CheckedCurrentSources;
use crate::{
    DynamicFamilyPreparationRequirement, DynamicFamilyPreparationRequirementReason,
    FamilySampleRank,
};
use light_core::programming::{ProgrammingOwner, TransitionError};
use std::{cell::Cell, collections::HashSet};

type SampleKey = (Uuid, FixtureId, Uuid);
type RandomKey = (Uuid, Uuid, FixtureId);

/// Reusable samples and Random decisions for one independently calculated output lane.
/// Plans retain only the current frame, never a second mutable runtime or historical copy.
#[derive(Default)]
pub struct DynamicSamplingScratch {
    plans: Vec<PinnedInstance>,
    completed_instances: Vec<Uuid>,
    scalar_samples: Vec<DynamicRuntimeSample>,
    samples: Vec<DynamicRuntimeSample>,
    work: SamplingWorkBuffers,
    requirements: Vec<DynamicFamilyPreparationRequirement>,
}

impl DynamicSamplingScratch {
    pub fn clear(&mut self) {
        for plan in self.plans.drain(..) {
            self.work.recycle(plan);
        }
        self.completed_instances.clear();
        self.scalar_samples.clear();
        self.samples.clear();
        self.requirements.clear();
        self.work.begin_frame();
    }
}

/// Nonsemantic storage. Recycling drops all captured expressions but retains nested lane,
/// retained-row and membership capacities for the next frame (also on the immediate path).
#[derive(Default)]
pub(in crate::runtime) struct SamplingWorkBuffers {
    controllers: HashMap<Uuid, Vec<PinnedController>>,
    random_envelopes: HashMap<RandomKey, f32>,
}

impl SamplingWorkBuffers {
    pub(super) fn begin_frame(&mut self) {
        self.random_envelopes.clear();
    }

    pub(super) fn retain_instances(&mut self, instances: &HashMap<Uuid, DynamicInstance>) {
        self.controllers.retain(|id, _| instances.contains_key(id));
    }

    pub(super) fn recycle(&mut self, mut plan: PinnedInstance) {
        for work in &mut plan.controllers {
            work.clear();
        }
        self.controllers.insert(plan.instance_id, plan.controllers);
    }
}

/// Successful completion proof. Only consuming this frame's deferred stage creates it.
/// Full expressions include legacy history; apply the scalar stage to physical output once,
/// and use these samples only for typed preparation/composition after that scalar stage.
pub struct CompletedDynamicSamples<'frame> {
    samples: &'frame [DynamicRuntimeSample],
    completion_identity: usize,
    requirements: &'frame [DynamicFamilyPreparationRequirement],
}

impl CompletedDynamicSamples<'_> {
    pub fn samples(&self) -> &[DynamicRuntimeSample] {
        self.samples
    }
    pub fn requirements(&self) -> &[DynamicFamilyPreparationRequirement] {
        self.requirements
    }
}

/// A single-use continuation borrowing the pinned runtime until typed sampling completes.
/// The caller first computes final Point poses from the supplied legacy fragments, then
/// supplies immutable Current/adoption sources built against those poses.
pub struct DeferredTypedSampling<'frame> {
    runtime: &'frame mut DynamicRuntime,
    plans: &'frame mut [PinnedInstance],
    completed_instances: &'frame [Uuid],
    samples: &'frame mut Vec<DynamicRuntimeSample>,
    completion: &'frame Cell<bool>,
    requirements: &'frame mut Vec<DynamicFamilyPreparationRequirement>,
}

impl<'frame> DeferredTypedSampling<'frame> {
    pub fn complete(
        self,
        sources: &dyn DynamicValueSourceResolver,
    ) -> Result<CompletedDynamicSamples<'frame>, DynamicRuntimeError> {
        for plan in self.plans {
            let instance = self
                .runtime
                .instances
                .get_mut(&plan.instance_id)
                .ok_or(DynamicRuntimeError::MissingInstance)?;
            complete_samples(
                instance,
                plan,
                sources,
                self.samples,
                Some(self.requirements),
                self.runtime.output_frame_undo.as_mut(),
            )?;
        }
        for id in self.completed_instances {
            self.runtime.complete_one_shot(*id);
        }
        self.completion.set(true);
        Ok(CompletedDynamicSamples {
            samples: self.samples,
            completion_identity: std::ptr::from_ref(self.completion) as usize,
            requirements: self.requirements,
        })
    }
}

impl DynamicRuntime {
    /// Calculate legacy output, establish final Point geometry, then evaluate typed numeric
    /// Current without advancing a runtime clock or Random stream a second time.
    ///
    /// Must run inside `with_output_frame_transaction`; the callback must include all
    /// fallible family preparation/composition and return its completion proof. An error
    /// must escape that transaction so the complete frame, including stage one, rolls back.
    /// `authored_sources` is queried only for authored occurrence identities, never values.
    /// Scalar-stage expressions can contain retained typed branches; their
    /// `visit_legacy_contributions` projection is the only stage-one physical output.
    #[allow(clippy::too_many_arguments)]
    pub fn sample_all_programming_staged<T, E>(
        &mut self,
        now_millis: u64,
        output_interval_millis: u64,
        speed_groups: &[DynamicSpeedTransport; 5],
        scalar_sources: &dyn ScalarSourceResolver,
        authored_sources: &dyn DynamicValueSourceResolver,
        addresses: Option<&dyn FrameAddressResolver>,
        scratch: &mut DynamicSamplingScratch,
        operation: impl for<'frame> FnOnce(
            &'frame [DynamicRuntimeSample],
            DeferredTypedSampling<'frame>,
        ) -> Result<(CompletedDynamicSamples<'frame>, T), E>,
    ) -> Result<T, E>
    where
        E: From<DynamicRuntimeError>,
    {
        if self.output_frame_undo.is_none() {
            return Err(DynamicRuntimeError::InvalidSample(
                "staged sampling requires an output transaction".into(),
            )
            .into());
        }
        self.begin_sample_boundary();
        scratch.clear();
        self.remove_completed_releases(now_millis);
        let instances = self
            .instances
            .iter()
            .map(|(id, instance)| (*id, instance.definition.speed.clone()))
            .collect::<Vec<_>>();
        for (instance_id, speed) in instances {
            let instance = self
                .instances
                .get_mut(&instance_id)
                .expect("pinned instance");
            if let Some(undo) = &mut self.output_frame_undo {
                undo.sampling(instance);
            }
            let cycle = cycle_duration(&speed, speed_groups);
            let frame = match prepare_sampling(
                instance,
                now_millis,
                cycle,
                speed_group_transport(&speed, speed_groups),
            )? {
                SamplingPreparation::Idle => continue,
                SamplingPreparation::Complete => {
                    scratch.completed_instances.push(instance_id);
                    continue;
                }
                SamplingPreparation::Ready(frame) => frame,
            };
            let addresses = addresses.map_or_else(
                || Arc::from([]),
                |resolver| instance.frame_addresses(resolver),
            );
            let plan = pin_samples(
                instance,
                instance_id,
                now_millis,
                cycle,
                output_interval_millis,
                scalar_sources,
                authored_sources,
                frame,
                addresses,
                &mut scratch.work,
                self.output_frame_undo.as_mut(),
            )?;
            append_scalar_samples(&plan, &mut scratch.scalar_samples);
            scratch.plans.push(plan);
        }
        // Discard spare instance buffers after a membership reduction; retained plans own
        // exactly the work buffers needed by this frame.
        scratch.work.controllers.clear();
        let completion = Cell::new(false);
        let (completed, context) = operation(
            &scratch.scalar_samples,
            DeferredTypedSampling {
                runtime: self,
                plans: &mut scratch.plans,
                completed_instances: &scratch.completed_instances,
                samples: &mut scratch.samples,
                completion: &completion,
                requirements: &mut scratch.requirements,
            },
        )?;
        if !completion.get()
            || completed.completion_identity != std::ptr::from_ref(&completion) as usize
        {
            return Err(DynamicRuntimeError::InvalidSample(
                "staged sampling did not complete its own frame".into(),
            )
            .into());
        }
        self.finish_sample_boundary(now_millis, DynamicSampleScope::WholeRuntime);
        Ok(context)
    }
}

pub(super) struct PinnedInstance {
    instance_id: Uuid,
    frame: SamplingFrame,
    cycle_duration_millis: u64,
    controllers: Vec<PinnedController>,
    holding: bool,
}

#[derive(Default)]
struct PinnedController {
    controller_index: usize,
    /// One genuine witness per pinned instance/controller emission, reused by deferred
    /// completion for every target. Recycled work never carries it into another frame.
    emission: Option<Arc<crate::DynamicEmissionWitness>>,
    activation_mix: f32,
    lanes: Vec<PinnedLane>,
    typed_indices: Vec<usize>,
    retained: Vec<(SampleKey, DynamicSampleExpression)>,
    ready_keys: HashSet<SampleKey>,
    emitted_keys: HashSet<SampleKey>,
    required_keys: HashSet<SampleKey>,
    required_last: Vec<(SampleKey, DynamicSampleExpression)>,
}

impl PinnedController {
    fn clear(&mut self) {
        self.emission = None;
        self.lanes.clear();
        self.typed_indices.clear();
        self.retained.clear();
        self.ready_keys.clear();
        self.emitted_keys.clear();
        self.required_keys.clear();
        self.required_last.clear();
    }
}

struct PinnedLane {
    target: FixtureId,
    lane_index: usize,
    address: Option<FrameAddress>,
    preserve_angle_branches: bool,
    fresh: bool,
    value: PinnedValue,
}

enum PinnedValue {
    Absent,
    Required,
    Ready(DynamicSampleExpression),
    Typed {
        phase: f32,
        random_envelope: Option<f32>,
        authored_occurrence: Option<crate::DynamicSourceOccurrenceId>,
    },
}

#[allow(clippy::too_many_arguments)]
pub(super) fn pin_samples(
    instance: &mut DynamicInstance,
    instance_id: Uuid,
    now_millis: u64,
    cycle_duration_millis: u64,
    output_interval_millis: u64,
    sources: &dyn ScalarSourceResolver,
    authored_sources: &dyn DynamicValueSourceResolver,
    frame: SamplingFrame,
    addresses: Arc<[Option<FrameAddress>]>,
    buffers: &mut SamplingWorkBuffers,
    mut undo: Option<&mut transaction::OutputFrameUndo>,
) -> Result<PinnedInstance, DynamicRuntimeError> {
    let evaluator = DynamicEvaluator::new(&frame.definition);
    let random_phases = random_phase_by_lane_target(
        &frame.definition,
        &frame.targets,
        frame.elapsed,
        cycle_duration_millis,
    );
    let holding = instance.paused_at_millis.is_some()
        && instance.activation_policy == crate::ActivationPolicy::JoinSyncNow;
    let mut controllers = buffers.controllers.remove(&instance_id).unwrap_or_default();
    let mut controller_count = 0;
    let retain_held = holding || frame.synchronized_resume_mix.is_some_and(|mix| mix < 1.0);
    let random_envelopes = &mut buffers.random_envelopes;
    for (controller_index, controller) in frame.controllers.iter().enumerate() {
        if controller.size == 0.0 {
            continue;
        }
        let transition = instance
            .controller_transitions
            .get(&controller.id)
            .copied()
            .unwrap_or(DynamicControllerTransitionSnapshot {
                controller_id: controller.id,
                activation_started_at_millis: controller.activated_at_millis,
                ..Default::default()
            });
        let activation_mix = transition_mix(transition, now_millis)
            * transition
                .output_gate
                .map_or(1.0, |gate| gate.mix_at(now_millis));
        if controllers.len() == controller_count {
            controllers.push(PinnedController::default());
        }
        let work = &mut controllers[controller_count];
        controller_count += 1;
        work.clear();
        work.controller_index = controller_index;
        work.emission = Some(crate::DynamicEmissionWitness::pin(
            instance_id,
            &frame.definition,
            controller,
            &frame.targets,
            frame.elapsed,
            cycle_duration_millis,
        ));
        work.activation_mix = activation_mix;
        let preserve_angle_targets = frame.synchronized_resume_mix.map(|_| {
            let mut held_by_target =
                HashMap::<FixtureId, Vec<(Uuid, &DynamicSampleExpression)>>::new();
            for (key, expression) in &instance.synchronized_hold_values {
                if key.0 == controller.id && instance.synchronized_hold_angle_sources.contains(key)
                {
                    held_by_target
                        .entry(key.1)
                        .or_default()
                        .push((key.2, expression));
                }
            }
            frame
                .targets
                .iter()
                .copied()
                .filter(|target| {
                    !same_angle_sources(
                        instance,
                        &frame.definition,
                        controller.id,
                        *target,
                        held_by_target.get(target).map_or(&[], Vec::as_slice),
                    )
                })
                .collect::<HashSet<_>>()
        });
        for (target_index, target) in frame.targets.iter().copied().enumerate() {
            let preserve_angle_branches = preserve_angle_targets
                .as_ref()
                .is_some_and(|targets| targets.contains(&target));
            for (lane_index, lane) in frame.definition.lanes.iter().enumerate() {
                if !instance.lane_is_active(controller.id, target, lane.id) {
                    continue;
                }
                let key = (controller.id, target, lane.id);
                if instance.unavailable_samples.contains_key(&key)
                    && (holding || frame.synchronized_resume_mix.is_some_and(|mix| mix < 1.0))
                {
                    continue;
                }
                let cached_address = addresses
                    .get(target_index * frame.definition.lanes.len() + lane_index)
                    .copied()
                    .flatten();
                if holding && let Some(held) = instance.synchronized_hold_values.get(&key) {
                    work.lanes.push(PinnedLane {
                        target,
                        lane_index,
                        address: expression_address(held, lane, cached_address),
                        preserve_angle_branches,
                        fresh: false,
                        value: PinnedValue::Ready(held.clone()),
                    });
                    if retain_held {
                        work.ready_keys.insert(key);
                    }
                    continue;
                }
                if holding && instance.synchronized_hold_captured {
                    continue;
                }
                if instance
                    .programming_lanes
                    .get(&lane.id)
                    .is_some_and(|lane| lane.unavailable_native_source().is_some())
                {
                    continue;
                }
                let phase = random_phases
                    .get(&lane.id)
                    .and_then(|phases| phases.get(&target))
                    .or_else(|| instance.phase_by_lane_target.get(&(lane.id, target)))
                    .copied()
                    .unwrap_or(0.0)
                    + controller.phase_offset_degrees;
                // Only a reached eligible live lane requests a decision. Shared group/target
                // lanes and controllers receive that same cached envelope in both stages.
                let random = lane.random_group_id.and_then(|group_id| {
                    let key = (instance_id, group_id, target);
                    if let Some(value) = random_envelopes.get(&key) {
                        return Some(*value);
                    }
                    let group = frame
                        .definition
                        .random_groups
                        .iter()
                        .find(|group| group.id == group_id)?;
                    if let Some(undo) = undo.as_deref_mut() {
                        undo.random(instance, (group_id, target));
                    }
                    let envelope = random_envelope(
                        instance
                            .random_streams
                            .entry((group_id, target))
                            .or_default(),
                        group,
                        instance_id,
                        target,
                        frame.elapsed,
                        random_group_speed_factor(&frame.definition, group_id),
                        output_interval_millis,
                    );
                    random_envelopes.insert(key, envelope);
                    Some(envelope)
                });
                let authored_occurrence = authored_sources.authored_occurrence(
                    instance_id,
                    controller.id,
                    target,
                    lane.id,
                );
                let mut expression = if let Some(address) = angle_current_address(lane) {
                    DynamicSampleExpression::AngleCurrent {
                        address: Arc::new(address.clone()),
                    }
                } else if instance.programming_lanes.get(&lane.id).is_some() {
                    work.typed_indices.push(work.lanes.len());
                    work.lanes.push(PinnedLane {
                        target,
                        lane_index,
                        address: cached_address,
                        preserve_angle_branches,
                        fresh: true,
                        value: PinnedValue::Typed {
                            phase,
                            random_envelope: random,
                            authored_occurrence,
                        },
                    });
                    continue;
                } else {
                    let observed = ObservedScalarSources {
                        inner: sources,
                        used_current: std::cell::Cell::new(false),
                    };
                    let Some(value) = evaluator.sample_lane(
                        lane,
                        DynamicEvaluationContext {
                            instance_id,
                            target,
                            elapsed_millis: frame.elapsed,
                            cycle_duration_millis,
                            phase_degrees: phase,
                            output_interval_millis,
                            random_envelope: random,
                            sources: &observed,
                        },
                    ) else {
                        continue;
                    };
                    let value = if controller.size == 1.0 {
                        value
                    } else {
                        observed
                            .current(target, &lane.output_owner())
                            .map_or(value, |base| base + (value - base) * controller.size)
                    };
                    DynamicSampleExpression::LegacyScalar {
                        attribute: lane.output_owner(),
                        value,
                        occurrence: None,
                        dependency_occurrence: observed.used_current.get().then(|| {
                            crate::DynamicSourceDependency::unknown(
                                observed.current_occurrence(target, &lane.output_owner()),
                            )
                        }),
                    }
                };
                expression.bind_fresh_authored_occurrence(authored_occurrence);
                if let Some(expression) = held_or_live_value(
                    instance,
                    key,
                    expression,
                    frame.synchronized_resume_mix,
                    preserve_angle_branches,
                ) {
                    let address = expression_address(&expression, lane, cached_address);
                    work.lanes.push(PinnedLane {
                        target,
                        lane_index,
                        address,
                        preserve_angle_branches,
                        fresh: true,
                        value: PinnedValue::Ready(expression),
                    });
                }
                if retain_held {
                    work.ready_keys.insert(key);
                }
            }
        }
        if retain_held {
            for (key, held) in &instance.synchronized_hold_values {
                if key.0 != controller.id
                    || instance.unavailable_samples.contains_key(key)
                    || work.ready_keys.contains(key)
                {
                    continue;
                }
                let expression = if holding {
                    held.clone()
                } else {
                    DynamicSampleExpression::Transition {
                        from: Some(Arc::new(held.clone())),
                        to: None,
                        progress: frame.synchronized_resume_mix.unwrap_or(0.0),
                        reason: crate::DynamicTransitionReason::Resume {
                            occurrence_id: instance
                                .synchronized_resume_transition
                                .expect("resume transition")
                                .occurrence_id,
                        },
                    }
                };
                work.retained.push((*key, expression));
            }
        }
    }
    controllers.truncate(controller_count);
    Ok(PinnedInstance {
        instance_id,
        frame,
        cycle_duration_millis,
        controllers,
        holding,
    })
}

fn append_scalar_samples(plan: &PinnedInstance, samples: &mut Vec<DynamicRuntimeSample>) {
    for work in &plan.controllers {
        let controller = &plan.frame.controllers[work.controller_index];
        let mut append = |target, lane_id, expression: &DynamicSampleExpression, address| {
            let mut legacy = false;
            expression.visit_legacy_contributions(|_, _, _| legacy = true);
            if legacy {
                samples.push(DynamicRuntimeSample {
                    instance_id: plan.instance_id,
                    controller_id: controller.id,
                    target,
                    lane_id,
                    expression: expression.clone(),
                    priority: controller.priority,
                    activated_at_millis: controller.activated_at_millis,
                    activation_mix: work.activation_mix,
                    address,
                });
            }
        };
        for lane in &work.lanes {
            if let PinnedValue::Ready(expression) = &lane.value {
                append(
                    lane.target,
                    plan.frame.definition.lanes[lane.lane_index].id,
                    expression,
                    lane.address,
                );
            }
        }
        for (key, expression) in &work.retained {
            append(key.1, key.2, expression, None);
        }
    }
}

fn resolve_deferred(
    instance: &mut DynamicInstance,
    plan: &mut PinnedInstance,
    sources: &dyn DynamicValueSourceResolver,
    mut requirements: Option<&mut Vec<DynamicFamilyPreparationRequirement>>,
) -> Result<(), DynamicRuntimeError> {
    for work in &mut plan.controllers {
        let controller = &plan.frame.controllers[work.controller_index];
        let mut position_required = HashSet::new();
        for index in &work.typed_indices {
            let pinned = &mut work.lanes[*index];
            let PinnedValue::Typed {
                phase,
                random_envelope,
                authored_occurrence,
            } = pinned.value
            else {
                continue;
            };
            let lane = &plan.frame.definition.lanes[pinned.lane_index];
            let key = (controller.id, pinned.target, lane.id);
            let checked = CheckedCurrentSources::new(sources);
            let input = if requirements.is_some() {
                &checked as &dyn DynamicValueSourceResolver
            } else {
                sources
            };
            let compiled = instance
                .programming_lanes
                .get_mut(&lane.id)
                .expect("pinned compiled lane");
            let operation = work
                .emission
                .as_ref()
                .map(|emission| crate::DynamicOperationContext {
                    emission,
                    target: pinned.target,
                    lane_id: lane.id,
                });
            let context = ProgrammingEvaluationContext {
                instance_id: plan.instance_id,
                controller_id: controller.id,
                authored_occurrence,
                target: pinned.target,
                elapsed_millis: plan.frame.elapsed,
                cycle_duration_millis: plan.cycle_duration_millis,
                phase_degrees: phase,
                random_envelope,
                sources: input,
            };
            let result = compiled
                .pin_angle_numeric_with_operations(&context, controller.size, operation)
                .and_then(|numeric| match numeric {
                    crate::AngleNumericSample::Program(program) => {
                        Ok(Some(DynamicSampleExpression::AngleNumeric { program }))
                    }
                    crate::AngleNumericSample::Absent => Ok(None),
                    crate::AngleNumericSample::NotApplicable => compiled
                        .sample_with_operations(context, operation)
                        .and_then(|value| {
                            value
                                .map(|value| {
                                    compiled
                                        .apply_controller_size_with_operations(
                                            value,
                                            controller.size,
                                            pinned.target,
                                            input,
                                            operation,
                                        )
                                        .map_err(TransitionError::from)
                                })
                                .transpose()
                        }),
                });
            let result = match (result, checked.take_error()) {
                (Err(error @ TransitionError::Invalid(_)), _)
                | (_, Some(error @ TransitionError::Invalid(_))) => Err(error),
                (_, Some(error)) => Err(error),
                (result, None) => result,
            };
            match result {
                Ok(Some(mut expression)) => {
                    expression.bind_fresh_authored_occurrence(authored_occurrence);
                    pinned.value = held_or_live_value(
                        instance,
                        key,
                        expression,
                        plan.frame.synchronized_resume_mix,
                        pinned.preserve_angle_branches,
                    )
                    .map_or(PinnedValue::Absent, PinnedValue::Ready);
                }
                Ok(None) => pinned.value = PinnedValue::Absent,
                Err(TransitionError::Requires(reason)) if requirements.is_some() => {
                    let crate::DynamicLaneBody::Programming(body) = &lane.body else {
                        unreachable!("typed work");
                    };
                    let owner = body.address.owner();
                    requirements.as_deref_mut().unwrap().push(
                        DynamicFamilyPreparationRequirement {
                            target: pinned.target,
                            owner,
                            rank: FamilySampleRank {
                                priority: controller.priority,
                                changed_at_millis: controller.activated_at_millis,
                                changed_at_submillis_nanos: 0,
                                stable_order: controller.id.as_u128(),
                                identity: crate::FamilySampleIdentity::Dynamic {
                                    instance_id: plan.instance_id,
                                    controller_id: controller.id,
                                    lane_id: lane.id,
                                },
                            },
                            reason: DynamicFamilyPreparationRequirementReason::Transition(reason),
                        },
                    );
                    if owner == ProgrammingOwner::Position {
                        position_required.insert(pinned.target);
                    }
                    work.required_keys.insert(key);
                    pinned.value = PinnedValue::Required;
                }
                Err(error) => return Err(DynamicRuntimeError::InvalidSample(error.to_string())),
            }
        }
        if work.required_keys.is_empty() {
            continue;
        }
        // Retain both sides of a correlated Position history. Independent owners and legacy
        // output still use this frame's prepared expressions; only retained records are held.
        for pinned in &work.lanes {
            let lane = &plan.frame.definition.lanes[pinned.lane_index];
            let key = (controller.id, pinned.target, lane.id);
            if position_required.contains(&pinned.target) {
                let mut position = lane.output_owner() == ProgrammingOwner::Position.key();
                if let PinnedValue::Ready(expression) = &pinned.value {
                    position |= expression.contains_angles();
                    expression
                        .visit_programming_values(&mut |address, _| {
                            position |= address.owner() == ProgrammingOwner::Position;
                            Ok(())
                        })
                        .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
                }
                if position {
                    work.required_keys.insert(key);
                }
            }
        }
        // A partner can already have been deleted from the definition. Its retained branch
        // still belongs to this unresolved cohort and must survive a completed Resume.
        if !position_required.is_empty() {
            for (key, held) in &instance.synchronized_hold_values {
                if key.0 != controller.id || !position_required.contains(&key.1) {
                    continue;
                }
                let mut position = held.contains_angles();
                held.visit_programming_values(&mut |address, _| {
                    position |= address.owner() == ProgrammingOwner::Position;
                    Ok(())
                })
                .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
                if position {
                    work.required_keys.insert(*key);
                }
            }
        }
        for key in &work.required_keys {
            if let Some(previous) = instance.last_sample_values.get(key) {
                work.required_last.push((*key, previous.clone()));
            }
        }
    }
    Ok(())
}

pub(super) fn complete_samples(
    instance: &mut DynamicInstance,
    plan: &mut PinnedInstance,
    sources: &dyn DynamicValueSourceResolver,
    samples: &mut Vec<DynamicRuntimeSample>,
    mut requirements: Option<&mut Vec<DynamicFamilyPreparationRequirement>>,
    mut undo: Option<&mut transaction::OutputFrameUndo>,
) -> Result<(), DynamicRuntimeError> {
    let sources = super::super::preset_values::RetainedPresetSources {
        current: sources,
        instance_id: plan.instance_id,
        values: Arc::clone(&instance.preset_values.by_binding),
    };
    resolve_deferred(instance, plan, &sources, requirements.as_deref_mut())?;
    let mut unavailable_last = if instance.unavailable_samples.is_empty() {
        Vec::new()
    } else {
        instance
            .last_sample_values
            .iter()
            .filter(|(key, _)| instance.unavailable_samples.contains_key(key))
            .map(|(key, value)| (*key, value.clone()))
            .collect::<Vec<_>>()
    };
    for work in &plan.controllers {
        unavailable_last.extend(work.required_last.iter().cloned());
    }
    if let Some(undo) = undo.as_deref_mut() {
        undo.begin_samples(instance);
    }
    instance.last_sample_values.clear();
    instance.last_sample_values.extend(unavailable_last);
    for work in &mut plan.controllers {
        let controller = &plan.frame.controllers[work.controller_index];
        let track_emitted = !work.retained.is_empty();
        work.emitted_keys.clear();
        for pinned in &work.lanes {
            let lane = &plan.frame.definition.lanes[pinned.lane_index];
            let key = (controller.id, pinned.target, lane.id);
            let expression = match &pinned.value {
                PinnedValue::Ready(expression) => expression.clone(),
                PinnedValue::Absent | PinnedValue::Required => continue,
                PinnedValue::Typed { .. } => {
                    unreachable!("all numeric work resolved before emission")
                }
            };
            if pinned.fresh && !work.required_keys.contains(&key) {
                if plan.holding && !instance.synchronized_hold_values.contains_key(&key) {
                    if let Some(undo) = undo.as_deref_mut() {
                        undo.held(instance);
                    }
                    if expression.contains_angles() {
                        instance.synchronized_hold_angle_sources.insert(key);
                    }
                    instance
                        .synchronized_hold_values
                        .insert(key, expression.clone());
                }
                if instance.unavailable_samples.contains_key(&key) {
                    if let Some(undo) = undo.as_deref_mut() {
                        undo.unavailable(instance, key);
                    }
                    instance.unavailable_samples.remove(&key);
                }
            }
            let address = expression_address(&expression, lane, pinned.address);
            if work.required_keys.contains(&key) {
                append_sample(
                    controller,
                    plan.instance_id,
                    pinned.target,
                    lane.id,
                    expression,
                    work.activation_mix,
                    address,
                    samples,
                )?;
            } else {
                emit_sample(
                    instance,
                    controller,
                    plan.instance_id,
                    pinned.target,
                    lane.id,
                    expression,
                    work.activation_mix,
                    address,
                    samples,
                )?;
            }
            if track_emitted {
                work.emitted_keys.insert(key);
            }
        }
        for (key, expression) in &work.retained {
            if !work.emitted_keys.contains(key) {
                if work.required_keys.contains(key) {
                    append_sample(
                        controller,
                        plan.instance_id,
                        key.1,
                        key.2,
                        expression.clone(),
                        work.activation_mix,
                        None,
                        samples,
                    )?;
                } else {
                    emit_sample(
                        instance,
                        controller,
                        plan.instance_id,
                        key.1,
                        key.2,
                        expression.clone(),
                        work.activation_mix,
                        None,
                        samples,
                    )?;
                }
            }
        }
    }
    if plan.holding {
        instance.synchronized_hold_captured = true;
    }
    if plan
        .frame
        .synchronized_resume_mix
        .is_some_and(|mix| mix >= 1.0)
        && let Some(undo) = undo.as_deref_mut()
    {
        undo.held(instance);
    }
    if plan
        .frame
        .synchronized_resume_mix
        .is_some_and(|mix| mix >= 1.0)
        && plan
            .controllers
            .iter()
            .any(|work| !work.required_keys.is_empty())
    {
        // Preserve the exact unresolved branch and clock through retry/restore. Valid lanes
        // retire their old holds now and continue sampling at the already-completed progress.
        let keep = |key: &SampleKey| {
            instance.unavailable_samples.contains_key(key)
                || plan
                    .controllers
                    .iter()
                    .any(|work| work.required_keys.contains(key))
        };
        instance.synchronized_hold_values.retain(|key, _| keep(key));
        instance.synchronized_hold_angle_sources.retain(keep);
    } else {
        finish_synchronized_resume(instance, plan.frame.synchronized_resume_mix);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EmptySources;
    impl ScalarSourceResolver for EmptySources {
        fn current(&self, _: FixtureId, _: &light_core::AttributeKey) -> Option<f32> {
            None
        }
        fn preset(&self, _: &str, _: FixtureId, _: &light_core::AttributeKey) -> Option<f32> {
            None
        }
    }

    #[test]
    fn foreign_completion_is_rejected_even_if_it_claims_success_or_this_frame_completed() {
        let mut runtime = DynamicRuntime::default();
        let mut transaction = DynamicOutputFrameScratch::default();
        let mut scratch = DynamicSamplingScratch::default();
        let transports = [DynamicSpeedTransport {
            effective_bpm: 60.0,
            phase_origin_millis: 0,
            phase_reference_millis: 0,
            beat_phase: 0.0,
            phase_advancing: true,
        }; 5];
        for finish_own_frame in [false, true] {
            let result: Result<(), DynamicRuntimeError> =
                runtime.with_output_frame_transaction(&mut transaction, |runtime| {
                    runtime.sample_all_programming_staged(
                        0,
                        10,
                        &transports,
                        &EmptySources,
                        &UnavailableProgrammingSources,
                        None,
                        &mut scratch,
                        |_, deferred| {
                            let samples = if finish_own_frame {
                                deferred.complete(&UnavailableProgrammingSources)?.samples
                            } else {
                                &[]
                            };
                            // Internal misuse: a private proof from another frame must not certify
                            // a dropped continuation, nor replace this frame's actual proof.
                            Ok((
                                CompletedDynamicSamples {
                                    samples,
                                    completion_identity: 0,
                                    requirements: &[],
                                },
                                (),
                            ))
                        },
                    )
                });
            assert!(
                matches!(result, Err(DynamicRuntimeError::InvalidSample(message)) if message == "staged sampling did not complete its own frame")
            );
        }
    }
}
