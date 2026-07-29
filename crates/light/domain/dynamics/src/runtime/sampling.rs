use super::*;
use crate::{DynamicEvaluationContext, DynamicEvaluator, project_phase};

struct SamplingFrame {
    definition: Arc<DynamicDefinition>,
    controllers: Vec<DynamicController>,
    targets: Vec<FixtureId>,
    elapsed: u64,
    synchronized_resume_mix: Option<f32>,
}

struct SampleEnvironment<'a> {
    instance_id: Uuid,
    now_millis: u64,
    cycle_duration_millis: u64,
    output_interval_millis: u64,
    sources: &'a dyn ScalarSourceResolver,
    evaluator: &'a DynamicEvaluator<'a>,
    random_phases: &'a HashMap<Uuid, HashMap<FixtureId, f32>>,
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
        let (samples, completed) = self.sample_with_transport(
            instance_id,
            now_millis,
            cycle_duration_millis,
            output_interval_millis,
            None,
            sources,
        )?;
        if completed {
            self.complete_one_shot(instance_id);
        }
        Ok(samples)
    }

    fn sample_with_transport(
        &mut self,
        instance_id: Uuid,
        now_millis: u64,
        cycle_duration_millis: u64,
        output_interval_millis: u64,
        transport: Option<DynamicSpeedTransport>,
        sources: &dyn ScalarSourceResolver,
    ) -> Result<(Vec<DynamicRuntimeSample>, bool), DynamicRuntimeError> {
        let instance = self
            .instances
            .get_mut(&instance_id)
            .ok_or(DynamicRuntimeError::MissingInstance)?;
        let frame = match prepare_sampling(instance, now_millis, cycle_duration_millis, transport)?
        {
            SamplingPreparation::Idle => return Ok((Vec::new(), false)),
            SamplingPreparation::Complete => return Ok((Vec::new(), true)),
            SamplingPreparation::Ready(frame) => frame,
        };
        let samples = collect_samples(
            instance,
            instance_id,
            now_millis,
            cycle_duration_millis,
            output_interval_millis,
            sources,
            &frame,
        );
        finish_synchronized_resume(instance, frame.synchronized_resume_mix);
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
        self.remove_completed_releases(now_millis);
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
            ) {
                samples.append(&mut instance_samples);
                if completed {
                    self.complete_one_shot(instance_id);
                }
            }
        }
        samples
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
    }
}

fn prepare_sampling(
    instance: &mut DynamicInstance,
    now_millis: u64,
    cycle_duration_millis: u64,
    transport: Option<DynamicSpeedTransport>,
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
    let Some(elapsed) = activation_elapsed(instance, now_millis, effective_now, transport) else {
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
        targets: instance.targets.clone(),
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
                boundary
            };
            (now_millis >= boundary).then(|| effective_now.saturating_sub(boundary))
        }
        _ => Some(
            effective_now
                .saturating_sub(instance.started_at_millis)
                .saturating_sub(instance.paused_elapsed_millis)
                .saturating_sub(instance.speed_paused_elapsed_millis),
        ),
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

fn collect_samples(
    instance: &mut DynamicInstance,
    instance_id: Uuid,
    now_millis: u64,
    cycle_duration_millis: u64,
    output_interval_millis: u64,
    sources: &dyn ScalarSourceResolver,
    frame: &SamplingFrame,
) -> Vec<DynamicRuntimeSample> {
    let evaluator = DynamicEvaluator::new(&frame.definition);
    let random_phases = random_phase_by_lane_target(
        &frame.definition,
        &frame.targets,
        frame.elapsed,
        cycle_duration_millis,
    );
    let environment = SampleEnvironment {
        instance_id,
        now_millis,
        cycle_duration_millis,
        output_interval_millis,
        sources,
        evaluator: &evaluator,
        random_phases: &random_phases,
    };
    let mut samples = Vec::new();
    for controller in &frame.controllers {
        if controller.size == 0.0 {
            continue;
        }
        collect_controller_samples(instance, frame, controller, &environment, &mut samples);
    }
    samples
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

fn collect_controller_samples(
    instance: &mut DynamicInstance,
    frame: &SamplingFrame,
    controller: &DynamicController,
    environment: &SampleEnvironment<'_>,
    samples: &mut Vec<DynamicRuntimeSample>,
) {
    let transition = instance
        .controller_transitions
        .get(&controller.id)
        .copied()
        .unwrap_or(DynamicControllerTransitionSnapshot {
            controller_id: controller.id,
            activation_started_at_millis: controller.activated_at_millis,
            ..Default::default()
        });
    let activation_mix = transition_mix(transition, environment.now_millis);
    for target in &frame.targets {
        for lane in &frame.definition.lanes {
            let phase = environment
                .random_phases
                .get(&lane.id)
                .and_then(|phases| phases.get(target))
                .or_else(|| instance.phase_by_lane_target.get(&(lane.id, *target)))
                .copied()
                .unwrap_or(0.0)
                + controller.phase_offset_degrees;
            let random_envelope = lane.random_group_id.and_then(|group_id| {
                frame
                    .definition
                    .random_groups
                    .iter()
                    .find(|group| group.id == group_id)
                    .map(|group| {
                        let stream = instance
                            .random_streams
                            .entry((group_id, *target))
                            .or_default();
                        random_envelope(
                            stream,
                            group,
                            environment.instance_id,
                            *target,
                            frame.elapsed,
                            random_group_speed_factor(&frame.definition, group_id),
                            environment.output_interval_millis,
                        )
                    })
            });
            let Some(value) = environment.evaluator.sample_lane(
                lane,
                DynamicEvaluationContext {
                    instance_id: environment.instance_id,
                    target: *target,
                    elapsed_millis: frame.elapsed,
                    cycle_duration_millis: environment.cycle_duration_millis,
                    phase_degrees: phase,
                    output_interval_millis: environment.output_interval_millis,
                    random_envelope,
                    sources: environment.sources,
                },
            ) else {
                continue;
            };
            let value = environment
                .sources
                .current(*target, &lane.attribute)
                .map_or(value, |base| base + (value - base) * controller.size);
            let sample_key = (controller.id, *target, lane.id);
            let value =
                held_or_live_value(instance, sample_key, value, frame.synchronized_resume_mix);
            instance.last_sample_values.insert(sample_key, value);
            samples.push(DynamicRuntimeSample {
                instance_id: environment.instance_id,
                controller_id: controller.id,
                target: *target,
                lane_id: lane.id,
                attribute: lane.attribute.clone(),
                value,
                priority: controller.priority,
                activated_at_millis: controller.activated_at_millis,
                activation_mix,
            });
        }
    }
}

fn held_or_live_value(
    instance: &mut DynamicInstance,
    sample_key: (Uuid, FixtureId, Uuid),
    value: f32,
    synchronized_resume_mix: Option<f32>,
) -> f32 {
    if instance.paused_at_millis.is_some()
        && instance.activation_policy == crate::ActivationPolicy::JoinSyncNow
    {
        *instance
            .synchronized_hold_values
            .entry(sample_key)
            .or_insert(value)
    } else if let Some(resume_mix) = synchronized_resume_mix {
        instance
            .synchronized_hold_values
            .get(&sample_key)
            .map_or(value, |held| held + (value - held) * resume_mix)
    } else {
        value
    }
}

fn finish_synchronized_resume(
    instance: &mut DynamicInstance,
    synchronized_resume_mix: Option<f32>,
) {
    if synchronized_resume_mix.is_some_and(|mix| mix >= 1.0) {
        instance.synchronized_resume_transition = None;
        instance.synchronized_hold_elapsed_millis = None;
        instance.synchronized_hold_values.clear();
    }
}
