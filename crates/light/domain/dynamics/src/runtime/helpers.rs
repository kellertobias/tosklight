use super::*;
use std::collections::HashSet;

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
    last: &HashMap<(Uuid, FixtureId, Uuid), DynamicSampleExpression>,
    held: &HashMap<(Uuid, FixtureId, Uuid), DynamicSampleExpression>,
) -> (
    Option<Arc<crate::RetainedExpressionTape>>,
    Vec<DynamicHeldSampleSnapshot>,
    Vec<DynamicHeldSampleSnapshot>,
) {
    let sorted = |values: &HashMap<(Uuid, FixtureId, Uuid), DynamicSampleExpression>| {
        let mut values = values
            .iter()
            .map(|(key, value)| (*key, Arc::new(value.clone())))
            .collect::<Vec<_>>();
        values.sort_by_key(|((controller, target, lane), _)| (*controller, target.0, *lane));
        values
    };
    let mut values = sorted(last);
    let last_len = values.len();
    values.extend(sorted(held));
    if values.is_empty() {
        return (None, Vec::new(), Vec::new());
    }
    let roots = values
        .iter()
        .map(|(_, expression)| expression.clone())
        .collect::<Vec<_>>();
    let tape = Arc::new(
        crate::RetainedExpressionTape::from_roots(&roots)
            .expect("validated runtime samples form an acyclic retained graph"),
    );
    let mut rows = values
        .into_iter()
        .zip(&tape.roots)
        .map(
            |(((controller_id, target, lane_id), _), root)| DynamicHeldSampleSnapshot {
                controller_id,
                target,
                lane_id,
                payload: DynamicHeldPayload::TapeRoot { tape_root: *root },
            },
        )
        .collect::<Vec<_>>();
    let held = rows.split_off(last_len);
    (Some(tape), rows, held)
}

/// Promote only at a pause boundary. Ordinary frames wrap immutable historical roots and
/// never append a sample to a growing tape or clone its node vector.
pub(super) fn retain_sample_history(
    values: &mut HashMap<(Uuid, FixtureId, Uuid), DynamicSampleExpression>,
) {
    if values.is_empty() {
        return;
    }
    let keys = values.keys().copied().collect::<Vec<_>>();
    let roots = keys
        .iter()
        .map(|key| Arc::new(values[key].clone()))
        .collect::<Vec<_>>();
    let tape = Arc::new(
        crate::RetainedExpressionTape::from_roots(&roots)
            .expect("validated runtime samples form an acyclic retained graph"),
    );
    for (key, root) in keys.into_iter().zip(&tape.roots) {
        values.insert(
            key,
            DynamicSampleExpression::Retained {
                tape: tape.clone(),
                root: *root,
            },
        );
    }
}

pub(super) fn held_angle_sources(
    values: &HashMap<(Uuid, FixtureId, Uuid), DynamicSampleExpression>,
    prepared: Option<&PreparedSampleTape>,
) -> HashSet<(Uuid, FixtureId, Uuid)> {
    let mut angles = HashSet::new();
    let mut remaining = Vec::new();
    for (key, expression) in values {
        if let (Some(prepared), DynamicSampleExpression::Retained { tape, root }) =
            (prepared, expression)
            && Arc::ptr_eq(tape, &prepared.tape)
        {
            if prepared.contains_angles[root.0 as usize] {
                angles.insert(*key);
            }
        } else {
            remaining.push((*key, Arc::new(expression.clone())));
        }
    }
    if !remaining.is_empty() {
        // At a pause boundary, import all held roots together so shared history is traversed
        // once. Runtime samples have already passed storage and source validation.
        let roots = remaining
            .iter()
            .map(|(_, expression)| Arc::clone(expression))
            .collect::<Vec<_>>();
        let tape = crate::RetainedExpressionTape::from_roots(&roots)
            .expect("validated runtime samples form an acyclic retained graph");
        let flags = angle_flags(&tape);
        for ((key, _), root) in remaining.into_iter().zip(tape.roots) {
            if flags[root.0 as usize] {
                angles.insert(key);
            }
        }
    }
    angles
}

fn angle_flags(tape: &crate::RetainedExpressionTape) -> Vec<bool> {
    let mut flags = Vec::with_capacity(tape.nodes.len());
    for node in &tape.nodes {
        let own = match node {
            crate::RetainedExpressionNode::AngleCurrent { .. }
            | crate::RetainedExpressionNode::AngleNumeric { .. } => true,
            crate::RetainedExpressionNode::Programming { address, .. }
            | crate::RetainedExpressionNode::Scale { address, .. } => {
                address.representation == crate::DynamicFamilyRepresentation::Angles
            }
            _ => false,
        };
        let inherited = node.children().any(|id| flags[id.0 as usize]);
        flags.push(own || inherited);
    }
    flags
}

/// Validate the shared checkpoint graph once. Topological contract summaries let keyed rows
/// retain their original root without reimporting the complete tape for every fixture.
pub(super) struct PreparedSampleTape {
    pub(super) tape: Arc<crate::RetainedExpressionTape>,
    roots: HashSet<crate::RetainedNodeId>,
    required_contract: Vec<u16>,
    contains_angles: Vec<bool>,
}

pub(super) fn prepare_sample_tape(
    tape: Option<&Arc<crate::RetainedExpressionTape>>,
) -> Result<Option<PreparedSampleTape>, DynamicRuntimeError> {
    let Some(tape) = tape else { return Ok(None) };
    tape.validate()
        .map_err(|error| DynamicRuntimeError::InvalidSnapshot(error.to_string()))?;
    let mut required_contract = Vec::<u16>::with_capacity(tape.nodes.len());
    for node in &tape.nodes {
        let own = match node {
            crate::RetainedExpressionNode::Programming { .. }
            | crate::RetainedExpressionNode::AngleCurrent { .. }
            | crate::RetainedExpressionNode::AngleNumeric { .. }
            | crate::RetainedExpressionNode::Scale { .. } => {
                light_core::programming::PROGRAMMING_CONTRACT_VERSION
            }
            _ => 0,
        };
        let children = node
            .children()
            .map(|id| required_contract[id.0 as usize])
            .max()
            .unwrap_or(0);
        required_contract.push(own.max(children));
    }
    Ok(Some(PreparedSampleTape {
        tape: Arc::clone(tape),
        roots: tape.roots.iter().copied().collect(),
        required_contract,
        contains_angles: angle_flags(tape),
    }))
}

/// Restore-time object-table check, before any restored state is installed: every witness
/// belongs to this instance, and every operation reachable from a held row belongs to that
/// row's controller, target and lane. Malformed references reject the complete restore.
pub(super) fn validate_retained_operations(
    prepared: Option<&PreparedSampleTape>,
    instance_id: Uuid,
    rows: [&[DynamicHeldSampleSnapshot]; 2],
) -> Result<(), DynamicRuntimeError> {
    let Some(prepared) = prepared else {
        return Ok(());
    };
    if prepared.tape.operation_emissions().is_empty() {
        return Ok(());
    }
    let invalid = |message: &str| DynamicRuntimeError::InvalidSnapshot(message.into());
    let owners = prepared
        .tape
        .operation_owners()
        .map_err(|error| DynamicRuntimeError::InvalidSnapshot(error.to_string()))?;
    for row in rows.into_iter().flatten() {
        let DynamicHeldPayload::TapeRoot { tape_root } = row.payload else {
            continue;
        };
        match owners.get(tape_root.0 as usize) {
            None | Some(crate::RetainedOperationOwner::Unattributed) => {}
            Some(crate::RetainedOperationOwner::Exact {
                instance_id: owner_instance,
                controller_id,
                target,
                lane_id,
            }) if *owner_instance == instance_id
                && *controller_id == row.controller_id
                && *target == row.target
                && *lane_id == row.lane_id => {}
            Some(_) => {
                return Err(invalid(
                    "held Dynamic operation provenance belongs to another emission row",
                ));
            }
        }
    }
    if prepared
        .tape
        .operation_emissions()
        .iter()
        .any(|emission| emission.instance_id() != instance_id)
    {
        return Err(invalid(
            "retained Dynamic emission belongs to another instance",
        ));
    }
    Ok(())
}

pub(super) fn sample_values_from_snapshot(
    values: Vec<DynamicHeldSampleSnapshot>,
    definition: &DynamicDefinition,
    supported_contract: u16,
    prepared: Option<&PreparedSampleTape>,
) -> Result<HashMap<(Uuid, FixtureId, Uuid), DynamicSampleExpression>, DynamicRuntimeError> {
    let mut samples = HashMap::new();
    for sample in values {
        let (expression, required) = match sample.payload {
            DynamicHeldPayload::TapeRoot { tape_root } => {
                let prepared = prepared
                    .filter(|prepared| prepared.roots.contains(&tape_root))
                    .ok_or_else(|| {
                        DynamicRuntimeError::InvalidSnapshot(
                            "held sample references an absent retained tape root".into(),
                        )
                    })?;
                (
                    DynamicSampleExpression::Retained {
                        tape: Arc::clone(&prepared.tape),
                        root: tape_root,
                    },
                    prepared.required_contract[tape_root.0 as usize],
                )
            }
            DynamicHeldPayload::Expression { expression } => {
                expression
                    .validate()
                    .map_err(|error| DynamicRuntimeError::InvalidSnapshot(error.to_string()))?;
                let required = expression.required_programming_contract();
                (expression, required)
            }
            DynamicHeldPayload::Legacy { value } => {
                let lane = definition
                    .lanes
                    .iter()
                    .find(|lane| lane.id == sample.lane_id)
                    .and_then(|lane| lane.legacy())
                    .ok_or_else(|| {
                        DynamicRuntimeError::InvalidSnapshot(
                            "legacy held value has no retained scalar lane".into(),
                        )
                    })?;
                let expression = DynamicSampleExpression::LegacyScalar {
                    attribute: lane.attribute.clone(),
                    value,
                    occurrence: None,
                    dependency_occurrence: None,
                };
                expression
                    .validate()
                    .map_err(|error| DynamicRuntimeError::InvalidSnapshot(error.to_string()))?;
                (expression, 0)
            }
        };
        if required > supported_contract {
            return Err(DynamicRuntimeError::InvalidSnapshot(format!(
                "held Dynamic requires programming contract {required}; this runtime supports {supported_contract}"
            )));
        }
        if samples
            .insert(
                (sample.controller_id, sample.target, sample.lane_id),
                expression,
            )
            .is_some()
        {
            return Err(DynamicRuntimeError::InvalidSnapshot(
                "duplicate held Dynamic address".into(),
            ));
        }
    }
    Ok(samples)
}

pub(super) fn random_group_speed_factor(definition: &DynamicDefinition, group_id: Uuid) -> f64 {
    let typed = definition
        .random_groups
        .iter()
        .find(|group| group.id == group_id)
        .is_some_and(|group| matches!(group.range, crate::DynamicRandomRange::Programming { .. }));
    definition
        .lanes
        .iter()
        .find(|lane| {
            lane.random_group_id == Some(group_id)
                && (!typed || lane.mode() == crate::DynamicLaneMode::Random)
        })
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
        instance.synchronized_hold_captured = false;
        instance.synchronized_resume_transition = None;
        instance.synchronized_hold_values.clear();
        instance.synchronized_hold_angle_sources.clear();
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
        instance.synchronized_hold_captured = false;
        instance.synchronized_resume_transition = None;
        instance.synchronized_hold_values.clear();
        instance.synchronized_hold_angle_sources.clear();
        return;
    }
    instance.synchronized_resume_transition = Some(DynamicSynchronizedResumeTransitionSnapshot {
        occurrence_id: synchronized_resume_occurrence(instance, now_millis, held_elapsed_millis),
        started_at_millis: now_millis,
        duration_millis,
        held_elapsed_millis,
    });
}

fn synchronized_resume_occurrence(instance: &DynamicInstance, now: u64, held: u64) -> Uuid {
    // Replaying the same command against a restored checkpoint must produce the
    // same identity. Retained nested occurrences distinguish repeated interruptions
    // even when commands share the same millisecond.
    let mut ids = Vec::new();
    for expression in instance.synchronized_hold_values.values() {
        expression.visit_resume_occurrences(&mut |id| ids.push(id));
    }
    ids.sort_unstable();
    ids.dedup();
    let mut key = b"tosklight:dynamic-resume:v1".to_vec();
    key.extend_from_slice(&now.to_le_bytes());
    key.extend_from_slice(&held.to_le_bytes());
    for id in ids {
        key.extend_from_slice(id.as_bytes());
    }
    Uuid::new_v5(&instance.id, &key)
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
                instance.synchronized_hold_captured =
                    instance.last_synchronized_elapsed_millis.is_some();
                instance
                    .synchronized_hold_values
                    .clone_from(&instance.last_sample_values);
                retain_sample_history(&mut instance.synchronized_hold_values);
                instance.synchronized_hold_angle_sources =
                    held_angle_sources(&instance.synchronized_hold_values, None);
            } else {
                instance.synchronized_hold_elapsed_millis = None;
                instance.synchronized_hold_captured = false;
                instance.synchronized_hold_values.clear();
                instance.synchronized_hold_angle_sources.clear();
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
