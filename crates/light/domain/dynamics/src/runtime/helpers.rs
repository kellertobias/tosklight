use super::*;

pub(super) fn project_instance_phases(
    definition: &DynamicDefinition,
    targets: &[FixtureId],
    stage_positions: &HashMap<FixtureId, SpatialPosition>,
    inherited_spatial_mapping: Option<&SpatialSelectionMapping>,
) -> Result<HashMap<(Uuid, FixtureId), f32>, DynamicRuntimeError> {
    let ranked_selection = definition
        .lanes
        .iter()
        .any(|lane| {
            matches!(
                definition.phase_for_lane(lane).ordering,
                PhaseOrdering::Selection
            )
        })
        .then(|| {
            let spatial_targets = targets
                .iter()
                .copied()
                .map(|fixture_id| SpatialTarget {
                    fixture_id,
                    position: stage_positions.get(&fixture_id).map(|position| Position3d {
                        x: f64::from(position.x),
                        y: f64::from(position.y),
                        z: f64::from(position.z),
                    }),
                })
                .collect::<Vec<_>>();
            evaluate_dynamic_spatial_mapping(
                inherited_spatial_mapping,
                &definition.spatial_mapping,
                &spatial_targets,
                None,
            )
            .map_err(|error| DynamicRuntimeError::InvalidSpatialMapping(error.to_string()))
        })
        .transpose()?;

    Ok(definition
        .lanes
        .iter()
        .flat_map(|lane| {
            let phase = definition.phase_for_lane(lane);
            let projected = if matches!(phase.ordering, PhaseOrdering::Selection) {
                project_ranked_phase(
                    phase,
                    ranked_selection
                        .as_ref()
                        .expect("Selection lanes resolve one shared spatial ranking"),
                )
            } else {
                project_phase(phase, targets, stage_positions, 0)
            };
            projected
                .into_iter()
                .map(move |phase| ((lane.id, phase.target), phase.degrees))
        })
        .collect())
}

pub(super) fn sample_values_snapshot(
    values: &HashMap<(Uuid, FixtureId, Uuid), f32>,
) -> Vec<DynamicHeldSampleSnapshot> {
    let mut values = values
        .iter()
        .map(
            |((controller_id, target, lane_id), value)| DynamicHeldSampleSnapshot {
                controller_id: *controller_id,
                target: *target,
                lane_id: *lane_id,
                value: *value,
            },
        )
        .collect::<Vec<_>>();
    values.sort_by_key(|sample| (sample.controller_id, sample.target.0, sample.lane_id));
    values
}

pub(super) fn sample_values_from_snapshot(
    values: Vec<DynamicHeldSampleSnapshot>,
) -> HashMap<(Uuid, FixtureId, Uuid), f32> {
    values
        .into_iter()
        .map(|sample| {
            (
                (sample.controller_id, sample.target, sample.lane_id),
                sample.value,
            )
        })
        .collect()
}

pub(super) fn random_group_speed_factor(definition: &DynamicDefinition, group_id: Uuid) -> f64 {
    definition
        .lanes
        .iter()
        .find(|lane| lane.random_group_id == Some(group_id))
        .map_or(1.0, |lane| lane.speed_multiplier.factor())
}

pub(super) fn random_envelope(
    state: &mut RandomStreamState,
    group: &crate::DynamicRandomGroup,
    instance_id: Uuid,
    target: FixtureId,
    elapsed_millis: u64,
    speed_factor: f64,
    output_interval_millis: u64,
) -> f32 {
    if elapsed_millis < state.last_elapsed_millis {
        *state = RandomStreamState::default();
    }
    let speed_factor = speed_factor.max(f64::EPSILON);
    let interval_millis = (group.decision_interval_millis as f64 / speed_factor)
        .round()
        .max(1.0) as u64;
    while state.next_decision_index.saturating_mul(interval_millis) <= elapsed_millis {
        let boundary = state.next_decision_index.saturating_mul(interval_millis);
        if state.active.is_some_and(|pulse| {
            pulse
                .started_at_millis
                .saturating_add(pulse.duration_millis)
                <= boundary
        }) {
            state.active = None;
        }
        if state.active.is_none()
            && crate::evaluate::uniform(group.seed, instance_id, target, state.next_decision_index)
                <= f64::from(group.start_probability)
        {
            let gaussian = crate::evaluate::gaussian(
                group.seed,
                instance_id,
                target,
                state.next_decision_index,
            );
            let duration_millis = ((group.mean_duration_millis as f64
                + gaussian * group.duration_spread_millis as f64)
                / speed_factor)
                .round()
                .max(output_interval_millis.max(1) as f64) as u64;
            state.active = Some(RandomPulse {
                started_at_millis: boundary,
                duration_millis,
            });
        }
        state.next_decision_index = state.next_decision_index.saturating_add(1);
    }
    state.last_elapsed_millis = elapsed_millis;
    let Some(pulse) = state.active else {
        return 0.0;
    };
    let end = pulse
        .started_at_millis
        .saturating_add(pulse.duration_millis);
    if elapsed_millis >= end {
        state.active = None;
        return 0.0;
    }
    let progress = elapsed_millis.saturating_sub(pulse.started_at_millis) as f32
        / pulse.duration_millis.max(1) as f32;
    if group.attack_ratio > 0.0 && progress < group.attack_ratio {
        progress / group.attack_ratio
    } else if group.decay_ratio > 0.0 && progress > 1.0 - group.decay_ratio {
        ((1.0 - progress) / group.decay_ratio).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

pub(super) fn speed_group_transport(
    speed: &crate::DynamicSpeed,
    speed_groups: &[DynamicSpeedTransport; 5],
) -> Option<DynamicSpeedTransport> {
    let crate::DynamicSpeed::SpeedGroup { group, .. } = speed else {
        return None;
    };
    Some(speed_groups[speed_group_index(*group)])
}

pub(super) fn speed_group_index(group: crate::SpeedGroup) -> usize {
    match group {
        crate::SpeedGroup::A => 0,
        crate::SpeedGroup::B => 1,
        crate::SpeedGroup::C => 2,
        crate::SpeedGroup::D => 3,
        crate::SpeedGroup::E => 4,
    }
}

pub(super) fn cycle_duration(
    speed: &crate::DynamicSpeed,
    speed_groups: &[DynamicSpeedTransport; 5],
) -> u64 {
    match speed {
        crate::DynamicSpeed::Fixed { duration_millis } => (*duration_millis).max(1),
        crate::DynamicSpeed::SpeedGroup {
            group,
            beats_per_cycle,
        } => {
            let bpm = speed_groups[speed_group_index(*group)]
                .effective_bpm
                .max(f64::EPSILON);
            (60_000.0 / bpm * beats_per_cycle.factor()).round().max(1.0) as u64
        }
    }
}

pub(super) fn validate_controller(
    controller: &DynamicController,
) -> Result<(), DynamicRuntimeError> {
    if !controller.size.is_finite()
        || controller.size < 0.0
        || !controller.speed_multiplier.is_finite()
        || controller.speed_multiplier <= 0.0
        || !controller.phase_offset_degrees.is_finite()
    {
        return Err(DynamicRuntimeError::InvalidController);
    }
    Ok(())
}

pub(super) fn transition_mix(
    transition: DynamicControllerTransitionSnapshot,
    now_millis: u64,
) -> f32 {
    if let Some(release_started) = transition.release_started_at_millis {
        let release_elapsed = now_millis
            .saturating_sub(release_started)
            .saturating_sub(transition.release_delay_millis);
        if now_millis < release_started.saturating_add(transition.release_delay_millis) {
            return 1.0;
        }
        if transition.release_duration_millis == 0 {
            return 0.0;
        }
        return (1.0 - release_elapsed as f32 / transition.release_duration_millis as f32)
            .clamp(0.0, 1.0);
    }
    if now_millis
        < transition
            .activation_started_at_millis
            .saturating_add(transition.activation_delay_millis)
    {
        return 0.0;
    }
    if transition.activation_duration_millis == 0 {
        return 1.0;
    }
    let elapsed = now_millis
        .saturating_sub(transition.activation_started_at_millis)
        .saturating_sub(transition.activation_delay_millis);
    (elapsed as f32 / transition.activation_duration_millis as f32).clamp(0.0, 1.0)
}

pub(super) fn winning_controller(instance: &DynamicInstance) -> Option<&DynamicController> {
    instance.controllers.values().max_by_key(|controller| {
        (
            controller.priority,
            controller.activated_at_millis,
            controller.id,
        )
    })
}

pub(super) fn schedule_synchronized_resume(instance: &mut DynamicInstance, now_millis: u64) {
    if instance.activation_policy != crate::ActivationPolicy::JoinSyncNow {
        instance.synchronized_hold_elapsed_millis = None;
        instance.synchronized_resume_transition = None;
        instance.synchronized_hold_values.clear();
        return;
    }
    let Some(held_elapsed_millis) = instance.synchronized_hold_elapsed_millis else {
        return;
    };
    let duration_millis = winning_controller(instance)
        .and_then(|controller| instance.controller_transitions.get(&controller.id))
        .map_or(0, |transition| transition.activation_duration_millis);
    if duration_millis == 0 {
        instance.synchronized_hold_elapsed_millis = None;
        instance.synchronized_resume_transition = None;
        instance.synchronized_hold_values.clear();
        return;
    }
    instance.synchronized_resume_transition = Some(DynamicSynchronizedResumeTransitionSnapshot {
        started_at_millis: now_millis,
        duration_millis,
        held_elapsed_millis,
    });
}

pub(super) fn reconcile_pause(
    instance: &mut DynamicInstance,
    global_paused: bool,
    now_millis: u64,
) {
    let paused =
        global_paused || winning_controller(instance).is_some_and(|controller| controller.paused);
    match (paused, instance.paused_at_millis) {
        (true, None) => {
            instance.paused_at_millis = Some(now_millis);
            instance.synchronized_resume_transition = None;
            if instance.activation_policy == crate::ActivationPolicy::JoinSyncNow {
                instance.synchronized_hold_elapsed_millis =
                    instance.last_synchronized_elapsed_millis;
                instance
                    .synchronized_hold_values
                    .clone_from(&instance.last_sample_values);
            } else {
                instance.synchronized_hold_elapsed_millis = None;
                instance.synchronized_hold_values.clear();
            }
        }
        (false, Some(paused_at)) => {
            instance.paused_elapsed_millis = instance
                .paused_elapsed_millis
                .saturating_add(now_millis.saturating_sub(paused_at));
            instance.paused_at_millis = None;
        }
        _ => {}
    }
}
