use super::*;
use crate::programming::UnavailableProgrammingSources;
use crate::{
    DynamicEvaluationContext, DynamicEvaluator, DynamicValueSourceResolver,
    ProgrammingEvaluationContext, project_phase,
};
use light_core::{FrameAddress, FrameAddressResolver};

struct ObservedScalarSources<'a> {
    inner: &'a dyn ScalarSourceResolver,
    used_current: std::cell::Cell<bool>,
}

impl ScalarSourceResolver for ObservedScalarSources<'_> {
    fn current(&self, target: FixtureId, attribute: &light_core::AttributeKey) -> Option<f32> {
        let value = self.inner.current(target, attribute);
        if value.is_some() {
            self.used_current.set(true);
        }
        value
    }

    fn current_occurrence(
        &self,
        target: FixtureId,
        attribute: &light_core::AttributeKey,
    ) -> Option<crate::DynamicSourceOccurrenceId> {
        self.inner.current_occurrence(target, attribute)
    }

    fn preset(
        &self,
        preset_id: &str,
        target: FixtureId,
        attribute: &light_core::AttributeKey,
    ) -> Option<f32> {
        self.inner.preset(preset_id, target, attribute)
    }
}

mod staged;
pub(super) use staged::SamplingWorkBuffers;
pub use staged::{
    CompletedChunk, CompletedDynamicSamples, DeferredTypedSampling, DynamicSamplingScratch,
    InstanceWorkers,
};

struct SamplingFrame {
    definition: Arc<DynamicDefinition>,
    controllers: Vec<DynamicController>,
    /// Shared by this frame's emission witnesses; one allocation per pinned frame.
    targets: Arc<[FixtureId]>,
    elapsed: u64,
    synchronized_resume_mix: Option<f32>,
}

enum SamplingPreparation {
    Idle,
    Complete,
    Ready(SamplingFrame),
}

impl DynamicRuntime {
    pub fn sample(
        &mut self,
        instance_id: Uuid,
        now_millis: u64,
        cycle_duration_millis: u64,
        output_interval_millis: u64,
        sources: &dyn ScalarSourceResolver,
    ) -> Result<Vec<DynamicRuntimeSample>, DynamicRuntimeError> {
        self.sample_programming(
            instance_id,
            now_millis,
            cycle_duration_millis,
            output_interval_millis,
            sources,
            &UnavailableProgrammingSources,
        )
    }

    /// Both source views belong to the same immutable pre-Dynamic frame.
    pub fn sample_programming(
        &mut self,
        instance_id: Uuid,
        now_millis: u64,
        cycle_duration_millis: u64,
        output_interval_millis: u64,
        sources: &dyn ScalarSourceResolver,
        programming_sources: &dyn DynamicValueSourceResolver,
    ) -> Result<Vec<DynamicRuntimeSample>, DynamicRuntimeError> {
        self.begin_sample_boundary();
        self.sampling_buffers.retain_instances(&self.instances);
        let (samples, completed) = self.sample_with_transport(
            instance_id,
            now_millis,
            cycle_duration_millis,
            output_interval_millis,
            None,
            sources,
            programming_sources,
            None,
        )?;
        if completed {
            self.complete_one_shot(instance_id);
        }
        self.finish_sample_boundary(now_millis, DynamicSampleScope::Instance(instance_id));
        Ok(samples)
    }

    #[allow(clippy::too_many_arguments)]
    fn sample_with_transport(
        &mut self,
        instance_id: Uuid,
        now_millis: u64,
        cycle_duration_millis: u64,
        output_interval_millis: u64,
        transport: Option<DynamicSpeedTransport>,
        sources: &dyn ScalarSourceResolver,
        programming_sources: &dyn DynamicValueSourceResolver,
        addresses: Option<&dyn FrameAddressResolver>,
    ) -> Result<(Vec<DynamicRuntimeSample>, bool), DynamicRuntimeError> {
        let instance = self
            .instances
            .get_mut(&instance_id)
            .ok_or(DynamicRuntimeError::MissingInstance)?;
        if let Some(undo) = &mut self.output_frame_undo {
            undo.sampling(instance);
        }
        let frame = match prepare_sampling(
            instance,
            now_millis,
            cycle_duration_millis,
            transport,
            &mut self.change_lead,
        )? {
            SamplingPreparation::Idle => return Ok((Vec::new(), false)),
            SamplingPreparation::Complete => return Ok((Vec::new(), true)),
            SamplingPreparation::Ready(frame) => frame,
        };
        let addresses = addresses.map_or_else(
            || Arc::from([]),
            |resolver| instance.frame_addresses(resolver),
        );
        self.sampling_buffers.begin_frame();
        let mut plan = staged::pin_samples(
            instance,
            instance_id,
            now_millis,
            cycle_duration_millis,
            output_interval_millis,
            sources,
            programming_sources,
            frame,
            addresses,
            &mut self.sampling_buffers,
            self.output_frame_undo.as_mut().map(transaction::journal),
        )?;
        let mut samples = Vec::new();
        let result = staged::complete_samples(
            instance,
            &mut plan,
            programming_sources,
            &mut samples,
            None,
            self.output_frame_undo.as_mut().map(transaction::journal),
            &mut || None,
        );
        self.sampling_buffers.recycle(plan);
        result?;
        Ok((samples, false))
    }

    /// Samples every active instance at one authoritative output timestamp.
    pub fn sample_all(
        &mut self,
        now_millis: u64,
        output_interval_millis: u64,
        speed_groups: &[DynamicSpeedTransport; 5],
        sources: &dyn ScalarSourceResolver,
    ) -> Vec<DynamicRuntimeSample> {
        self.sample_all_addressed(
            now_millis,
            output_interval_millis,
            speed_groups,
            sources,
            None,
        )
    }

    /// [`Self::sample_all`], with each sample told where the engine keeps its pair.
    ///
    /// The addresses are resolved once per instance and kept while the patch generation, the
    /// definition and the targets stand, so a running Dynamic costs the engine no lookup by name.
    pub fn sample_all_addressed(
        &mut self,
        now_millis: u64,
        output_interval_millis: u64,
        speed_groups: &[DynamicSpeedTransport; 5],
        sources: &dyn ScalarSourceResolver,
        addresses: Option<&dyn FrameAddressResolver>,
    ) -> Vec<DynamicRuntimeSample> {
        self.begin_sample_boundary();
        self.remove_completed_releases(now_millis);
        let mut complete = true;
        let mut samples = Vec::new();
        let instances = self
            .instances
            .iter()
            .map(|(id, instance)| (*id, instance.definition.speed.clone()))
            .collect::<Vec<_>>();
        for (instance_id, speed) in instances {
            let transport = speed_group_transport(&speed, speed_groups);
            let cycle_duration_millis = cycle_duration(&speed, speed_groups);
            if let Ok((mut instance_samples, completed)) = self.sample_with_transport(
                instance_id,
                now_millis,
                cycle_duration_millis,
                output_interval_millis,
                transport,
                sources,
                &UnavailableProgrammingSources,
                addresses,
            ) {
                samples.append(&mut instance_samples);
                if completed {
                    self.complete_one_shot(instance_id);
                }
            } else {
                complete = false;
            }
        }
        if complete {
            self.finish_sample_boundary(now_millis, DynamicSampleScope::WholeRuntime);
        }
        samples
    }

    /// Typed output preserves expressions for the authoritative family compositor.
    /// Arithmetic failures are returned to the caller; unavailable sources omit a lane.
    #[allow(clippy::too_many_arguments)]
    pub fn sample_all_programming_addressed(
        &mut self,
        now_millis: u64,
        output_interval_millis: u64,
        speed_groups: &[DynamicSpeedTransport; 5],
        sources: &dyn ScalarSourceResolver,
        programming_sources: &dyn DynamicValueSourceResolver,
        addresses: Option<&dyn FrameAddressResolver>,
    ) -> Result<Vec<DynamicRuntimeSample>, DynamicRuntimeError> {
        self.begin_sample_boundary();
        self.remove_completed_releases(now_millis);
        let instances = self
            .instances
            .iter()
            .map(|(id, instance)| (*id, instance.definition.speed.clone()))
            .collect::<Vec<_>>();
        let mut samples = Vec::new();
        for (instance_id, speed) in instances {
            let (mut instance_samples, completed) = self.sample_with_transport(
                instance_id,
                now_millis,
                cycle_duration(&speed, speed_groups),
                output_interval_millis,
                speed_group_transport(&speed, speed_groups),
                sources,
                programming_sources,
                addresses,
            )?;
            samples.append(&mut instance_samples);
            if completed {
                self.complete_one_shot(instance_id);
            }
        }
        self.finish_sample_boundary(now_millis, DynamicSampleScope::WholeRuntime);
        Ok(samples)
    }

    fn complete_one_shot(&mut self, instance_id: Uuid) {
        if let Some(instance) = self.instances.get_mut(&instance_id) {
            instance.completed = true;
        }
    }

    fn remove_completed_releases(&mut self, now_millis: u64) {
        let completed = self
            .instances
            .iter()
            .flat_map(|(instance_id, instance)| {
                instance
                    .controller_transitions
                    .values()
                    .filter(move |transition| {
                        transition.release_started_at_millis.is_some_and(|started| {
                            now_millis
                                >= started
                                    .saturating_add(transition.release_delay_millis)
                                    .saturating_add(transition.release_duration_millis)
                        })
                    })
                    .map(move |transition| (*instance_id, transition.controller_id))
            })
            .collect::<Vec<_>>();
        for (instance_id, controller_id) in completed {
            let _ = self.off_controller(instance_id, controller_id, now_millis, 0, 0);
        }
        self.sampling_buffers.retain_instances(&self.instances);
    }
}

fn prepare_sampling(
    instance: &mut DynamicInstance,
    now_millis: u64,
    cycle_duration_millis: u64,
    transport: Option<DynamicSpeedTransport>,
    change_lead: &mut light_core::ChangeLeadLedger,
) -> Result<SamplingPreparation, DynamicRuntimeError> {
    if instance.completed {
        return Ok(SamplingPreparation::Idle);
    }
    let winning = winning_controller(instance)
        .cloned()
        .ok_or(DynamicRuntimeError::MissingController)?;
    let mut controllers = instance.controllers.values().cloned().collect::<Vec<_>>();
    controllers.sort_by_key(|controller| {
        std::cmp::Reverse((
            controller.priority,
            controller.activated_at_millis,
            controller.id,
        ))
    });
    reconcile_speed_pause(instance, now_millis, transport);
    let effective_now = instance
        .paused_at_millis
        .or(instance.speed_paused_at_millis)
        .unwrap_or(now_millis);
    let Some(elapsed) =
        activation_elapsed(instance, now_millis, effective_now, transport, change_lead)
    else {
        return Ok(SamplingPreparation::Idle);
    };
    let definition = Arc::clone(&instance.definition);
    let speed = (f64::from(winning.speed_multiplier)
        * definition.overall_speed_multiplier.factor())
    .max(f64::EPSILON);
    let elapsed = (elapsed as f64 * speed).round() as u64;
    let lifecycle_elapsed =
        (lifecycle_elapsed(instance, effective_now) as f64 * speed).round() as u64;
    if definition.run_mode == crate::DynamicRunMode::OneShot
        && lifecycle_elapsed >= cycle_duration_millis
    {
        return Ok(SamplingPreparation::Complete);
    }
    Ok(SamplingPreparation::Ready(SamplingFrame {
        definition,
        controllers,
        targets: Arc::from(instance.targets.as_slice()),
        elapsed,
        synchronized_resume_mix: synchronized_resume_mix(instance, now_millis),
    }))
}

fn reconcile_speed_pause(
    instance: &mut DynamicInstance,
    now_millis: u64,
    transport: Option<DynamicSpeedTransport>,
) {
    let Some(transport) = transport else {
        return;
    };
    if transport.phase_advancing {
        if let Some(paused_at) = instance.speed_paused_at_millis.take() {
            instance.speed_paused_elapsed_millis = instance
                .speed_paused_elapsed_millis
                .saturating_add(now_millis.saturating_sub(paused_at));
        }
    } else if instance.speed_paused_at_millis.is_none() {
        instance.speed_paused_at_millis = Some(now_millis);
    }
}

fn activation_elapsed(
    instance: &mut DynamicInstance,
    now_millis: u64,
    effective_now: u64,
    transport: Option<DynamicSpeedTransport>,
    change_lead: &mut light_core::ChangeLeadLedger,
) -> Option<u64> {
    match (instance.activation_policy, transport) {
        (crate::ActivationPolicy::JoinSyncNow, Some(transport)) => {
            let live_elapsed = transport
                .phase_reference_millis
                .saturating_sub(transport.phase_origin_millis);
            if instance.paused_at_millis.is_some() {
                Some(
                    *instance
                        .synchronized_hold_elapsed_millis
                        .get_or_insert(live_elapsed),
                )
            } else {
                instance.last_synchronized_elapsed_millis = Some(live_elapsed);
                Some(live_elapsed)
            }
        }
        (crate::ActivationPolicy::NextBoundary, Some(transport)) => {
            if !transport.phase_advancing {
                return None;
            }
            let boundary = if let Some(boundary) = instance.pending_until_millis {
                boundary
            } else {
                let boundary = next_activation_boundary(instance, now_millis, transport);
                instance.pending_until_millis = Some(boundary);
                // TL-659: the scheduled start instant of a boundary start.
                change_lead.mark(super::change_lead::ledger_micros(boundary));
                boundary
            };
            (now_millis >= boundary).then(|| effective_now.saturating_sub(boundary))
        }
        _ => {
            let elapsed = effective_now
                .saturating_sub(instance.started_at_millis)
                .saturating_sub(instance.paused_elapsed_millis)
                .saturating_sub(instance.speed_paused_elapsed_millis);
            if instance.activation_policy == crate::ActivationPolicy::JoinSyncNow {
                if instance.paused_at_millis.is_some() {
                    instance
                        .synchronized_hold_elapsed_millis
                        .get_or_insert(elapsed);
                } else {
                    instance.last_synchronized_elapsed_millis = Some(elapsed);
                }
            }
            Some(elapsed)
        }
    }
}

fn next_activation_boundary(
    instance: &DynamicInstance,
    now_millis: u64,
    transport: DynamicSpeedTransport,
) -> u64 {
    let beat_millis = (60_000.0 / transport.effective_bpm.max(f64::EPSILON))
        .round()
        .max(1.0) as u64;
    match instance.definition.activation_boundary {
        crate::ActivationBoundary::Beat => now_millis.saturating_add(
            ((1.0 - transport.beat_phase.rem_euclid(1.0)) * beat_millis as f64).round() as u64,
        ),
        crate::ActivationBoundary::Bar => {
            let elapsed = transport
                .phase_reference_millis
                .saturating_sub(transport.phase_origin_millis);
            let completed_beats = elapsed / beat_millis;
            let next_bar_beat = completed_beats
                .checked_div(4)
                .unwrap_or_default()
                .saturating_add(1)
                .saturating_mul(4);
            transport
                .phase_origin_millis
                .saturating_add(next_bar_beat.saturating_mul(beat_millis))
                .max(now_millis.saturating_add(1))
        }
    }
}

fn lifecycle_elapsed(instance: &DynamicInstance, effective_now: u64) -> u64 {
    match instance.activation_policy {
        crate::ActivationPolicy::NextBoundary => instance
            .pending_until_millis
            .map_or(0, |boundary| effective_now.saturating_sub(boundary)),
        crate::ActivationPolicy::StartNow | crate::ActivationPolicy::JoinSyncNow => effective_now
            .saturating_sub(instance.started_at_millis)
            .saturating_sub(instance.paused_elapsed_millis)
            .saturating_sub(instance.speed_paused_elapsed_millis),
    }
}

fn synchronized_resume_mix(instance: &DynamicInstance, now_millis: u64) -> Option<f32> {
    instance.synchronized_resume_transition.map(|transition| {
        if transition.duration_millis == 0 {
            1.0
        } else {
            (now_millis.saturating_sub(transition.started_at_millis) as f32
                / transition.duration_millis as f32)
                .clamp(0.0, 1.0)
        }
    })
}

fn random_phase_by_lane_target(
    definition: &DynamicDefinition,
    targets: &[FixtureId],
    elapsed: u64,
    cycle_duration_millis: u64,
) -> HashMap<Uuid, HashMap<FixtureId, f32>> {
    definition
        .lanes
        .iter()
        .filter_map(|lane| {
            let phase = definition.phase_for_lane(lane);
            matches!(phase.ordering, crate::PhaseOrdering::RandomEachLoop { .. }).then(|| {
                let loop_index = match definition.phase_spread_mode {
                    crate::DynamicPhaseSpreadMode::Uniform => {
                        elapsed / cycle_duration_millis.max(1)
                    }
                    crate::DynamicPhaseSpreadMode::PerLane => {
                        ((elapsed as f64 * lane.speed_multiplier.factor())
                            / cycle_duration_millis.max(1) as f64)
                            .floor() as u64
                    }
                };
                (
                    lane.id,
                    project_phase(phase, targets, &HashMap::new(), loop_index)
                        .into_iter()
                        .map(|phase| (phase.target, phase.degrees))
                        .collect(),
                )
            })
        })
        .collect()
}

fn angle_current_address(lane: &crate::DynamicLane) -> Option<&crate::DynamicValueAddress> {
    let crate::DynamicLaneBody::Programming(body) = &lane.body else {
        return None;
    };
    lane.is_angle_current_passthrough().then_some(&body.address)
}

/// Folding both axes is safe only when the complete old/live Angle membership and
/// source roles agree. A hot edit or a nested interrupted branch must remain zipped.
fn same_angle_sources(
    instance: &DynamicInstance,
    definition: &DynamicDefinition,
    controller: Uuid,
    target: FixtureId,
    held: &[(Uuid, &DynamicSampleExpression)],
) -> bool {
    for (lane_id, expression) in held {
        let Some(lane) = definition.lanes.iter().find(|lane| {
            lane.id == *lane_id && instance.lane_is_active(controller, target, lane.id)
        }) else {
            return false;
        };
        let crate::DynamicLaneBody::Programming(body) = &lane.body else {
            return false;
        };
        let same = if let Some(address) = expression.angle_current_address() {
            angle_current_address(lane) == Some(address)
        } else if let Some((address, _)) = expression.programming_leaf() {
            angle_current_address(lane).is_none() && &body.address == address
        } else {
            false
        };
        if !same {
            return false;
        }
    }
    held.len()
        == definition
            .lanes
            .iter()
            .filter(|lane| {
                lane.is_programming_angles() && instance.lane_is_active(controller, target, lane.id)
            })
            .count()
}

fn expression_address(
    expression: &DynamicSampleExpression,
    lane: &crate::DynamicLane,
    cached: Option<FrameAddress>,
) -> Option<FrameAddress> {
    if expression
        .legacy_leaf()
        .is_some_and(|(attribute, _)| attribute == lane.output_owner_ref())
        || expression
            .programming_leaf()
            .is_some_and(|(address, _)| address.owner().key_ref() == lane.output_owner_ref())
    {
        cached
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_sample(
    instance: &mut DynamicInstance,
    controller: &DynamicController,
    instance_id: Uuid,
    target: FixtureId,
    lane_id: Uuid,
    expression: DynamicSampleExpression,
    activation_mix: f32,
    address: Option<FrameAddress>,
    samples: &mut Vec<DynamicRuntimeSample>,
) -> Result<(), DynamicRuntimeError> {
    instance
        .last_sample_values
        .insert((controller.id, target, lane_id), expression.clone());
    append_sample(
        controller,
        instance_id,
        target,
        lane_id,
        expression,
        activation_mix,
        address,
        samples,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_sample(
    controller: &DynamicController,
    instance_id: Uuid,
    target: FixtureId,
    lane_id: Uuid,
    expression: DynamicSampleExpression,
    activation_mix: f32,
    address: Option<FrameAddress>,
    samples: &mut Vec<DynamicRuntimeSample>,
) -> Result<(), DynamicRuntimeError> {
    // Keep passthrough Current symbolic, including while paused. Preparation validates its
    // value and proof against the final captured frame, after scalar Point output is known.
    // An early availability read would cache adoption against obsolete geometry or remove a
    // required partner before the complete Position cohort can report its requirement.
    let expression = if let Some(address) = expression.angle_current_address() {
        if address.representation != crate::DynamicFamilyRepresentation::Angles {
            return Err(DynamicRuntimeError::InvalidSample(
                "Angle Current requires an Angle address".into(),
            ));
        }
        address
            .validate()
            .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
        expression
    } else if expression.legacy_leaf().is_some() || expression.programming_leaf().is_some() {
        expression
            .into_shallow()
            .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?
    } else {
        expression
    };
    samples.push(DynamicRuntimeSample {
        instance_id,
        controller_id: controller.id,
        target,
        lane_id,
        expression,
        priority: controller.priority,
        activated_at_millis: controller.activated_at_millis,
        activation_mix,
        address,
    });
    Ok(())
}

fn held_or_live_value(
    instance: &DynamicInstance,
    sample_key: (Uuid, FixtureId, Uuid),
    value: DynamicSampleExpression,
    synchronized_resume_mix: Option<f32>,
    preserve_angle_branches: bool,
) -> Option<DynamicSampleExpression> {
    if instance.paused_at_millis.is_some()
        && instance.activation_policy == crate::ActivationPolicy::JoinSyncNow
    {
        Some(
            instance
                .synchronized_hold_values
                .get(&sample_key)
                .cloned()
                .unwrap_or(value),
        )
    } else if let Some(resume_mix) = synchronized_resume_mix {
        if resume_mix >= 1.0 {
            return Some(value);
        }
        let held = instance.synchronized_hold_values.get(&sample_key);
        let reason = crate::DynamicTransitionReason::Resume {
            occurrence_id: instance
                .synchronized_resume_transition
                .expect("resume mix has a transition")
                .occurrence_id,
        };
        if preserve_angle_branches {
            // A sibling axis may have changed its address or become a new Current
            // partner. Keep this common branch boundary until both pairs are built.
            return Some(DynamicSampleExpression::Transition {
                from: held.cloned().map(Arc::new),
                to: Some(Arc::new(value)),
                progress: resume_mix,
                reason,
            });
        }
        let Some(held) = held else { return Some(value) };
        if let Some(address) = value.angle_current_address()
            && held.angle_current_address() == Some(address)
        {
            return Some(value);
        }
        if resume_mix <= 0.0 {
            return Some(held.clone());
        }
        if let Some(blended) = instance
            .programming_lanes
            .get(&sample_key.2)
            .and_then(|lane| {
                if held.has_source_occurrences() || value.has_source_occurrences() {
                    None
                } else {
                    lane.blend_components(held, &value, resume_mix)
                }
            })
        {
            return Some(blended);
        }
        if let (Some((a, from)), Some((b, to))) = (held.legacy_leaf(), value.legacy_leaf())
            && a == b
            && !held.has_source_occurrences()
            && !value.has_source_occurrences()
        {
            return Some(DynamicSampleExpression::LegacyScalar {
                attribute: a.clone(),
                value: from + (to - from) * resume_mix,
                occurrence: None,
                dependency_occurrence: None,
            });
        }
        Some(DynamicSampleExpression::Transition {
            from: Some(Arc::new(held.clone())),
            to: Some(Arc::new(value)),
            progress: resume_mix,
            reason,
        })
    } else {
        Some(value)
    }
}

fn finish_synchronized_resume(
    instance: &mut DynamicInstance,
    synchronized_resume_mix: Option<f32>,
) {
    if synchronized_resume_mix.is_some_and(|mix| mix >= 1.0) {
        instance.synchronized_resume_transition = None;
        instance.synchronized_hold_elapsed_millis = None;
        instance.synchronized_hold_captured = false;
        instance
            .synchronized_hold_values
            .retain(|key, _| instance.unavailable_samples.contains_key(key));
        instance
            .synchronized_hold_angle_sources
            .retain(|key| instance.unavailable_samples.contains_key(key));
    }
}
