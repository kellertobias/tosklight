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

mod emit;
mod parallel;
mod pin;
mod resolve;
pub use parallel::CompletedChunk;

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
    /// Fx-hashed (TL-639): per-frame lookups only, never iterated.
    random_envelopes: rustc_hash::FxHashMap<RandomKey, f32>,
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
    samples: &'frame mut Vec<DynamicRuntimeSample>,
    completion_identity: usize,
    requirements: &'frame [DynamicFamilyPreparationRequirement],
}

impl CompletedDynamicSamples<'_> {
    pub fn samples(&self) -> &[DynamicRuntimeSample] {
        self.samples
    }
    /// Move the completed samples out (TL-639 round 6). The frame publishes them as its owned
    /// sample list instead of cloning every expression; the scratch keeps an empty buffer of
    /// the same capacity for the next frame. `samples()` is empty afterwards.
    pub fn take_samples(&mut self) -> Vec<DynamicRuntimeSample> {
        let capacity = self.samples.len();
        std::mem::replace(self.samples, Vec::with_capacity(capacity))
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
                &mut || None,
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
        let mut instances = self
            .instances
            .iter()
            .map(|(id, instance)| (*id, instance.definition.speed.clone()))
            .collect::<Vec<_>>();
        if self.derives_instance_ids() {
            // TL-639: reproducible runs also fix the otherwise hash-ordered sample order.
            instances.sort_unstable_by_key(|(id, _)| *id);
        }
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
    // Fx-hashed (TL-639): membership only, never iterated.
    ready_keys: rustc_hash::FxHashSet<SampleKey>,
    emitted_keys: rustc_hash::FxHashSet<SampleKey>,
    required_keys: rustc_hash::FxHashSet<SampleKey>,
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
    let pinning = pin::LanePinning {
        instance_id,
        cycle_duration_millis,
        output_interval_millis,
        sources,
        authored_sources,
        evaluator: &evaluator,
        frame: &frame,
        addresses: &addresses,
        random_phases: &random_phases,
        holding,
        retain_held,
    };
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
        let preserve_angle_targets = pin::preserve_angle_targets(instance, &frame, controller);
        pinning.pin_controller_lanes(
            instance,
            work,
            controller,
            preserve_angle_targets.as_ref(),
            random_envelopes,
            undo.as_deref_mut(),
        );
        if retain_held {
            pin::retain_held_samples(instance, &frame, controller, holding, work);
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

pub(super) fn complete_samples(
    instance: &mut DynamicInstance,
    plan: &mut PinnedInstance,
    sources: &dyn DynamicValueSourceResolver,
    samples: &mut Vec<DynamicRuntimeSample>,
    requirements: Option<&mut Vec<DynamicFamilyPreparationRequirement>>,
    mut undo: Option<&mut transaction::OutputFrameUndo>,
    recorded: &mut dyn FnMut() -> Option<resolve::Evaluated>,
) -> Result<(), DynamicRuntimeError> {
    let sources = super::super::preset_values::RetainedPresetSources {
        current: sources,
        instance_id: plan.instance_id,
        values: Arc::clone(&instance.preset_values.by_binding),
    };
    resolve::resolve_deferred(instance, plan, &sources, requirements, recorded)?;
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
        emit::emit_controller_samples(
            instance,
            controller,
            plan.instance_id,
            plan.holding,
            &plan.frame.definition,
            work,
            samples,
            undo.as_deref_mut(),
        )?;
    }
    emit::finish_synchronized_holds(instance, plan, undo);
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
                                Box::leak(Box::default())
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
