use super::cold::ColdDynamicReconciliationCause;
use super::*;

mod cue;
mod programmer_sources;
pub(super) use cue::reconcile_cue_dynamics_observed;
pub(in crate::runtime::output_scheduler) use programmer_sources::ReconciledSourceAssignment;
use programmer_sources::planned_source_rows;

type SourceEmitter<'e> = Option<&'e mut dyn FnMut(ReconciledSourceAssignment)>;
type Operation = ColdDynamicReconciliationOperation;
type Context = ColdDynamicReconciliationContext;

struct DesiredProgrammerLaneSource<'a> {
    captured_index: usize,
    row: &'a light_dynamics::DynamicAddressValue,
}

struct DesiredProgrammerController<'a> {
    programmer_id: Uuid,
    instance_link: Uuid,
    authored: &'a light_dynamics::DynamicAddressValue,
    priority: i16,
    activated_at_millis: u64,
    reference: &'a light_dynamics::DynamicReference,
    overrides: &'a light_dynamics::DynamicInstanceOverrides,
    timing: light_dynamics::DynamicValueTiming,
    targets: Vec<FixtureId>,
    target_ids: HashSet<FixtureId>,
    lane_rows: Vec<(FixtureId, Uuid)>,
    source_rows: Vec<DesiredProgrammerLaneSource<'a>>,
}

struct DesiredCueController<'a> {
    controller_id: Uuid,
    instance_link: Uuid,
    cue_list_id: light_core::CueListId,
    priority: i16,
    activated_at_millis: u64,
    reference: &'a light_dynamics::DynamicReference,
    overrides: &'a light_dynamics::DynamicInstanceOverrides,
    timing: light_dynamics::DynamicValueTiming,
    targets: Vec<FixtureId>,
    target_ids: HashSet<FixtureId>,
    lane_rows: Vec<(FixtureId, Uuid)>,
    source_rows: Vec<(usize, &'a light_playback::ActiveCueDynamicValue)>,
}

pub(in crate::runtime::output_scheduler) fn reconcile_dynamic_playbacks(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    active: &[light_playback::ActiveDynamicPlayback],
) -> HashMap<Uuid, DynamicPlaybackControl> {
    reconcile_dynamic_playbacks_observed(
        dynamics,
        now_millis,
        snapshot,
        active,
        None,
        &mut ReconciliationObserver::warm(),
    )
}

pub(in crate::runtime::output_scheduler) fn reconcile_dynamic_playbacks_with_sources(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    active: &[light_playback::ActiveDynamicPlayback],
    mut emit: impl FnMut(ReconciledSourceAssignment),
) -> HashMap<Uuid, DynamicPlaybackControl> {
    reconcile_dynamic_playbacks_observed(
        dynamics,
        now_millis,
        snapshot,
        active,
        Some(&mut emit),
        &mut ReconciliationObserver::warm(),
    )
}

pub(super) fn reconcile_dynamic_playbacks_observed(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    active: &[light_playback::ActiveDynamicPlayback],
    mut emit: SourceEmitter<'_>,
    observer: &mut ReconciliationObserver<'_>,
) -> HashMap<Uuid, DynamicPlaybackControl> {
    let desired_ids = active
        .iter()
        .filter(|playback| playback.enabled)
        .map(dynamic_playback_controller_id)
        .collect::<HashSet<_>>();
    release_stale_playback_controllers(dynamics, snapshot, &desired_ids, now_millis, observer);
    if active.is_empty() {
        return HashMap::new();
    }

    let lookup = DestinationLookup::new(snapshot);
    let mut controls = HashMap::new();
    for (captured_index, active) in active
        .iter()
        .enumerate()
        .filter(|(_, playback)| playback.enabled)
    {
        reconcile_playback_row(
            dynamics,
            now_millis,
            &lookup,
            captured_index,
            active,
            &mut controls,
            &mut emit,
            observer,
        );
    }
    controls
}

#[allow(clippy::too_many_arguments)]
fn reconcile_playback_row(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    lookup: &DestinationLookup<'_>,
    captured_index: usize,
    active: &light_playback::ActiveDynamicPlayback,
    controls: &mut HashMap<Uuid, DynamicPlaybackControl>,
    emit: &mut SourceEmitter<'_>,
    observer: &mut ReconciliationObserver<'_>,
) {
    let controller_id = dynamic_playback_controller_id(active);
    let context = Context::playback(controller_id, active);
    let Some(playback) = dynamic_playback_definition(lookup.snapshot, active) else {
        observer.fail(
            &context,
            Operation::PlaybackRow,
            ColdDynamicReconciliationCause::MissingPlaybackDefinition,
        );
        return;
    };
    let light_playback::PlaybackTarget::Dynamic { assignment } = &playback.target else {
        observer.fail(
            &context,
            Operation::PlaybackRow,
            ColdDynamicReconciliationCause::NonDynamicPlaybackTarget,
        );
        return;
    };
    let identity = match active
        .playback_identity
        .map(Ok)
        .unwrap_or_else(|| PlaybackIdentity::physical(active.playback_number))
    {
        Ok(identity) => identity,
        Err(error) if observer.is_cold() => {
            observer.fail(
                &context,
                Operation::PlaybackRow,
                ColdDynamicReconciliationCause::InvalidPlaybackNumber(error),
            );
            return;
        }
        Err(error) => panic!("active physical Dynamic Playback number is validated: {error:?}"),
    };
    controls.insert(
        controller_id,
        DynamicPlaybackControl {
            identity,
            master: active.master,
            crossfade_non_intensity: assignment.crossfade_non_intensity,
            auto_off_full_control: assignment.auto_off_full_control,
            temporary_only: active.flash && active.flash_restore_off,
        },
    );
    let existing = dynamics.controller(controller_id);
    let definition = effective_definition(
        dynamics,
        existing.as_ref().map(|(id, _)| *id),
        &assignment.dynamic,
        &lookup.definitions,
    );
    let speed_multiplier = effective_dynamic_playback_speed(&definition, active);
    let (targets, inherited_spatial_mapping) =
        playback_scope(lookup, observer, &context, &definition, assignment);
    let source_rows = emit.as_ref().map(|_| {
        targets
            .iter()
            .flat_map(|target| {
                definition
                    .lanes
                    .iter()
                    .filter(|lane| !lane.is_angle_current_passthrough())
                    .map(move |lane| (*target, lane.id, captured_index))
            })
            .collect()
    });
    if let Some((instance_id, _)) = existing {
        let targets_applied = refresh_surviving_playback(
            dynamics,
            now_millis,
            lookup,
            observer,
            &context,
            instance_id,
            active,
            assignment,
            (targets, inherited_spatial_mapping),
            speed_multiplier,
        );
        if targets_applied {
            emit_programmer_sources(emit, source_rows, instance_id, controller_id);
        }
        return;
    }
    if !ensure_fallback_definition(
        dynamics,
        lookup,
        observer,
        &context,
        &definition,
        now_millis,
    ) {
        return;
    }
    let activated_at_millis =
        u64::try_from(active.activated_at.timestamp_millis()).unwrap_or_default();
    let scope_was_empty = targets.is_empty();
    let started = controls::start(
        dynamics,
        now_millis,
        light_dynamics::DynamicStartRequest {
            definition_id: definition.id,
            controller: light_dynamics::DynamicController {
                id: controller_id,
                source: context.source.clone(),
                priority: assignment.priority,
                activated_at_millis,
                size: active.size,
                speed_multiplier,
                phase_offset_degrees: 0.0,
                paused: active.paused,
            },
            target_scope: light_dynamics::DynamicTargetScope {
                ordered_targets: targets,
            },
            stage_positions: (*lookup.snapshot.dynamic_stage_positions).clone(),
            inherited_spatial_mapping,
            now_millis: activated_at_millis.min(now_millis),
            activation_delay_millis: 0,
            activation_duration_millis: playback.xfade_millis,
            activation_policy_override: assignment.activation_override,
            reuse_matching_targetless: false,
        },
    );
    match observer.started(&context, scope_was_empty, started) {
        Some(instance_id) => emit_programmer_sources(emit, source_rows, instance_id, controller_id),
        None => {
            controls.remove(&controller_id);
        }
    }
}

/// Refresh a surviving Playback controller in place: release cancellation, current owner and
/// priority, targets, size/speed and pause. The instance is never restarted, so its identity,
/// clocks, phase, Random streams and held samples survive a moved or reassigned Playback.
#[allow(clippy::too_many_arguments)]
fn refresh_surviving_playback(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    lookup: &DestinationLookup<'_>,
    observer: &mut ReconciliationObserver<'_>,
    context: &Context,
    instance_id: Uuid,
    active: &light_playback::ActiveDynamicPlayback,
    assignment: &light_playback::DynamicPlaybackAssignment,
    (targets, inherited_spatial_mapping): (
        Vec<FixtureId>,
        Option<light_dynamics::SpatialSelectionMapping>,
    ),
    speed_multiplier: f32,
) -> bool {
    observer.observe(
        context,
        Operation::CancelRelease,
        controls::cancel_release(dynamics, context.controller_id, now_millis),
    );
    // A moved or reassigned Playback keeps its logical controller; only its current
    // operational owner and priority follow the assignment (recorded, never restarted).
    observer.observe(
        context,
        Operation::Owner,
        controls::owner(
            dynamics,
            context.controller_id,
            context.source.clone(),
            assignment.priority,
            now_millis,
        ),
    );
    let targets_applied = observer
        .observe(
            context,
            Operation::Targets,
            dynamics.reconcile_instance_targets_recorded(
                instance_id,
                light_dynamics::DynamicTargetScope {
                    ordered_targets: targets,
                },
                lookup.snapshot.dynamic_stage_positions.as_ref(),
                inherited_spatial_mapping.as_ref(),
                now_millis,
            ),
        )
        .is_some();
    observer.observe(
        context,
        Operation::Controller,
        controls::update(
            dynamics,
            context.controller_id,
            Some(active.size),
            Some(speed_multiplier),
            None,
            now_millis,
        ),
    );
    let resume_policy = activation_policy(assignment.resume_policy);
    observer.observe(
        context,
        Operation::Pause,
        controls::pause(
            dynamics,
            instance_id,
            context.controller_id,
            active.paused,
            now_millis,
            resume_policy,
        ),
    );
    targets_applied
}

/// Destination definitions and Groups, indexed once per flow.
struct DestinationLookup<'s> {
    snapshot: &'s light_engine::EngineSnapshot,
    definitions: HashMap<Uuid, &'s light_dynamics::DynamicDefinition>,
    groups: HashMap<String, light_programmer::GroupDefinition>,
}

impl<'s> DestinationLookup<'s> {
    fn new(snapshot: &'s light_engine::EngineSnapshot) -> Self {
        Self {
            snapshot,
            definitions: snapshot
                .dynamics
                .iter()
                .map(|definition| (definition.id, definition))
                .collect(),
            groups: snapshot
                .groups
                .iter()
                .map(|group| (group.id.clone(), group.clone()))
                .collect(),
        }
    }

    fn group(
        &self,
        observer: &mut ReconciliationObserver<'_>,
        context: &Context,
        group_id: &str,
    ) -> (
        Vec<FixtureId>,
        Option<light_dynamics::SpatialSelectionMapping>,
    ) {
        observer.group_scope(
            context,
            group_id,
            resolve_dynamic_group(group_id, &self.groups, self.snapshot),
        )
    }
}

/// Programmer and Cue scope: Targetless definitions follow the authored row targets.
fn authored_scope(
    lookup: &DestinationLookup<'_>,
    observer: &mut ReconciliationObserver<'_>,
    context: &Context,
    definition: &light_dynamics::DynamicDefinition,
    authored_targets: &[FixtureId],
) -> (
    Vec<FixtureId>,
    Option<light_dynamics::SpatialSelectionMapping>,
) {
    match &definition.target_binding {
        light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
            lookup.group(observer, context, group_id)
        }
        light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => {
            (observer.explicit_scope(context, targets.clone()), None)
        }
        light_dynamics::DynamicTargetBinding::Targetless => (
            observer.explicit_scope(context, authored_targets.to_vec()),
            None,
        ),
    }
}

/// Playback scope: Targetless definitions follow the Playback assignment's own scope, whose
/// Live Group supplies membership only.
fn playback_scope(
    lookup: &DestinationLookup<'_>,
    observer: &mut ReconciliationObserver<'_>,
    context: &Context,
    definition: &light_dynamics::DynamicDefinition,
    assignment: &light_playback::DynamicPlaybackAssignment,
) -> (
    Vec<FixtureId>,
    Option<light_dynamics::SpatialSelectionMapping>,
) {
    match &definition.target_binding {
        light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
            lookup.group(observer, context, group_id)
        }
        light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => {
            (observer.explicit_scope(context, targets.clone()), None)
        }
        light_dynamics::DynamicTargetBinding::Targetless => match &assignment.target_scope {
            Some(light_playback::DynamicPlaybackTargetScope::LiveGroup { group_id }) => {
                (lookup.group(observer, context, group_id).0, None)
            }
            Some(light_playback::DynamicPlaybackTargetScope::FrozenTargets { targets }) => {
                (observer.explicit_scope(context, targets.clone()), None)
            }
            None => (observer.explicit_scope(context, Vec::new()), None),
        },
    }
}

/// Embedded fallbacks of definitions absent from the destination registry are installed before
/// start. An invalid fallback is a cold failure.
fn ensure_fallback_definition(
    dynamics: &mut light_dynamics::DynamicRuntime,
    lookup: &DestinationLookup<'_>,
    observer: &mut ReconciliationObserver<'_>,
    context: &Context,
    definition: &light_dynamics::DynamicDefinition,
    now_millis: u64,
) -> bool {
    lookup.definitions.contains_key(&definition.id)
        || observer
            .observe(
                context,
                Operation::FallbackDefinition,
                controls::fallback_definition(dynamics, definition.clone(), now_millis),
            )
            .is_some()
}

fn activation_policy(
    policy: light_playback::DynamicPlaybackResumePolicy,
) -> Option<light_dynamics::ActivationPolicy> {
    match policy {
        light_playback::DynamicPlaybackResumePolicy::FollowDynamic => None,
        light_playback::DynamicPlaybackResumePolicy::ResumeFrozenPhase => {
            Some(light_dynamics::ActivationPolicy::StartNow)
        }
        light_playback::DynamicPlaybackResumePolicy::RejoinSynchronizedPosition => {
            Some(light_dynamics::ActivationPolicy::JoinSyncNow)
        }
        light_playback::DynamicPlaybackResumePolicy::ResumeOnNextBoundary => {
            Some(light_dynamics::ActivationPolicy::NextBoundary)
        }
    }
}

/// Reconciliation must select the same lanes and target binding that sampling will execute.
/// In blind Preload the current show definition may differ from a running pinned instance.
fn effective_definition(
    dynamics: &light_dynamics::DynamicRuntime,
    instance_id: Option<Uuid>,
    reference: &light_dynamics::DynamicReference,
    definitions: &HashMap<Uuid, &light_dynamics::DynamicDefinition>,
) -> Arc<light_dynamics::DynamicDefinition> {
    instance_id
        .and_then(|id| dynamics.instance_definition(id))
        .cloned()
        .unwrap_or_else(|| {
            reference
                .dynamic_id
                .and_then(|id| definitions.get(&id))
                .map(|definition| Arc::new((**definition).clone()))
                .unwrap_or_else(|| Arc::clone(&reference.embedded_fallback.definition))
        })
}

pub(in crate::runtime::output_scheduler) fn reconcile_programmer_dynamics(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    extra_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
) {
    reconcile_programmer_dynamics_observed(
        dynamics,
        now_millis,
        snapshot,
        programmer_values,
        extra_values,
        None,
        &mut ReconciliationObserver::warm(),
    );
}

/// Emit only fresh active assignments selected by this same reconciliation plan. Historical
/// expression occurrences are never visited or rebound. Indices name `programmer_values`
/// followed by `extra_values`, allowing the caller to read their exact captured source rows.
pub(in crate::runtime::output_scheduler) fn reconcile_programmer_dynamics_with_sources(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    extra_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    mut emit: impl FnMut(ReconciledSourceAssignment),
) {
    reconcile_programmer_dynamics_observed(
        dynamics,
        now_millis,
        snapshot,
        programmer_values,
        extra_values,
        Some(&mut emit),
        &mut ReconciliationObserver::warm(),
    );
}

fn programmer_controller_plan<'a>(
    programmer_values: &'a [(Uuid, i16, light_dynamics::DynamicAddressValue)],
    extra_values: &'a [(Uuid, i16, light_dynamics::DynamicAddressValue)],
) -> (
    HashMap<Uuid, DesiredProgrammerController<'a>>,
    HashMap<Uuid, light_dynamics::DynamicValueTiming>,
) {
    // A Preload edit targets the same logical controller as its Live authored link. Resolve
    // those edit tracks before grouping, preserving independent lanes and newer Off/On order.
    // The Programmer scope keeps equal imported links on different desks independent.
    let mut by_programmer =
        HashMap::<Uuid, (i16, Vec<&light_dynamics::DynamicAddressValue>)>::new();
    // Ephemeral index lookup over these borrowed vectors only. Pointer identities never enter
    // a cache, runtime expression, source record or callback; returned indices are authoritative.
    let mut captured_indices = HashMap::new();
    for (index, (programmer, priority, value)) in
        programmer_values.iter().chain(extra_values).enumerate()
    {
        captured_indices.insert(value as *const light_dynamics::DynamicAddressValue, index);
        by_programmer
            .entry(*programmer)
            .or_insert_with(|| (*priority, Vec::new()))
            .1
            .push(value);
    }
    let mut desired = HashMap::<Uuid, DesiredProgrammerController>::new();
    let mut off = HashMap::<Uuid, light_dynamics::DynamicValueTiming>::new();
    for (programmer_id, (priority, rows)) in by_programmer {
        let mut effective = light_dynamics::merge_dynamic_address_values(rows.iter().copied());
        let covered_links = effective
            .iter()
            .filter_map(|row| match row.value {
                light_dynamics::DynamicSemanticValue::DynamicOff { instance_link, .. } => {
                    Some(instance_link)
                }
                _ => None,
            })
            .collect::<HashSet<_>>();
        // An Off in another retained source lane masks the surviving underlying On. Keep
        // sampling that logical controller so removing the overlay reveals its continuing
        // phase/Random/history. A replaced On is absent from these captured authored rows and
        // therefore still uses destructive Off. Never manufacture a tombstone from runtime.
        if !covered_links.is_empty() {
            effective.extend(
                light_dynamics::merge_dynamic_address_values(rows.iter().copied().filter(|row| {
                    !matches!(
                        row.value,
                        light_dynamics::DynamicSemanticValue::DynamicOff { .. }
                    )
                }))
                .into_iter()
                .filter(|row| {
                    matches!(row.value,
                        light_dynamics::DynamicSemanticValue::DynamicOn { instance_link, .. }
                        if covered_links.contains(&instance_link))
                }),
            );
        }
        for stored in effective {
            match &stored.value {
                light_dynamics::DynamicSemanticValue::DynamicOn {
                    instance_link,
                    dynamic,
                    overrides,
                    timing,
                    lane_id,
                } => {
                    let controller_id = light_dynamics::programmer_dynamic_controller_id(
                        light_core::ProgrammerId(programmer_id),
                        *instance_link,
                    );
                    let controller = desired.entry(controller_id).or_insert_with(|| {
                        DesiredProgrammerController {
                            programmer_id,
                            instance_link: *instance_link,
                            authored: stored,
                            priority,
                            activated_at_millis: stored.changed_at_millis,
                            reference: dynamic,
                            overrides,
                            timing: *timing,
                            targets: Vec::new(),
                            target_ids: HashSet::new(),
                            lane_rows: Vec::new(),
                            source_rows: Vec::new(),
                        }
                    });
                    if light_dynamics::dynamic_address_edit_is_later(stored, controller.authored) {
                        controller.authored = stored;
                        controller.activated_at_millis = stored.changed_at_millis;
                        controller.reference = dynamic;
                        controller.overrides = overrides;
                        controller.timing = *timing;
                    }
                    controller.lane_rows.push((stored.fixture_id, *lane_id));
                    controller.source_rows.push(DesiredProgrammerLaneSource {
                        captured_index: captured_indices
                            [&(stored as *const light_dynamics::DynamicAddressValue)],
                        row: stored,
                    });
                    if controller.target_ids.insert(stored.fixture_id) {
                        controller.targets.push(stored.fixture_id);
                    }
                }
                light_dynamics::DynamicSemanticValue::DynamicOff {
                    instance_link,
                    timing,
                } => {
                    let controller_id = light_dynamics::programmer_dynamic_controller_id(
                        light_core::ProgrammerId(programmer_id),
                        *instance_link,
                    );
                    off.entry(controller_id).or_insert(*timing);
                }
                light_dynamics::DynamicSemanticValue::Static { .. }
                | light_dynamics::DynamicSemanticValue::FixAt { .. }
                | light_dynamics::DynamicSemanticValue::ProgrammingFixAt { .. }
                | light_dynamics::DynamicSemanticValue::ProgrammingRelease { .. }
                | light_dynamics::DynamicSemanticValue::Release => {}
            }
        }
    }
    (desired, off)
}

pub(super) fn reconcile_programmer_dynamics_observed(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    extra_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    mut emit: SourceEmitter<'_>,
    observer: &mut ReconciliationObserver<'_>,
) {
    let (desired, off) = programmer_controller_plan(programmer_values, extra_values);
    let desired_ids = desired.keys().copied().collect::<HashSet<_>>();
    release_programmer_off(dynamics, now_millis, &off, &desired_ids, observer);
    for (instance_id, controller) in dynamics.controllers() {
        if matches!(
            controller.source,
            light_dynamics::DynamicControllerSource::Programmer { .. }
        ) && !desired_ids.contains(&controller.id)
            && !off.contains_key(&controller.id)
        {
            let released = controls::off(dynamics, instance_id, controller.id, now_millis, 0, 0);
            observer.observe(
                &Context::released(ColdDynamicReconciliationFlow::Programmer, &controller),
                Operation::Release,
                released,
            );
        }
    }
    if desired.is_empty() {
        return;
    }

    let lookup = DestinationLookup::new(snapshot);
    for (controller_id, desired) in desired {
        reconcile_programmer_controller(
            dynamics,
            now_millis,
            &lookup,
            controller_id,
            &desired,
            off.get(&controller_id),
            &mut emit,
            observer,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn reconcile_programmer_controller(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    lookup: &DestinationLookup<'_>,
    controller_id: Uuid,
    desired: &DesiredProgrammerController<'_>,
    off: Option<&light_dynamics::DynamicValueTiming>,
    emit: &mut SourceEmitter<'_>,
    observer: &mut ReconciliationObserver<'_>,
) {
    let context = Context::programmer(controller_id, desired.programmer_id, desired.instance_link);
    let existing = dynamics.controller(controller_id);
    let definition = effective_definition(
        dynamics,
        existing.as_ref().map(|(id, _)| *id),
        desired.reference,
        &lookup.definitions,
    );
    let lane_selection = light_dynamics::DynamicLaneSelection::for_recorded_values(
        desired.reference,
        &definition,
        &desired.lane_rows,
    );
    let (targets, inherited_spatial_mapping) =
        authored_scope(lookup, observer, &context, &definition, &desired.targets);
    let source_rows = emit
        .as_ref()
        .map(|_| planned_source_rows(desired, &definition, &lane_selection, &targets));
    if let Some((instance_id, _)) = existing {
        observer.observe(
            &context,
            Operation::CancelRelease,
            controls::cancel_release(dynamics, controller_id, now_millis),
        );
        observer.observe(
            &context,
            Operation::Rank,
            controls::rank(
                dynamics,
                controller_id,
                desired.priority,
                desired.activated_at_millis,
                now_millis,
            ),
        );
        let selection_applied = observer
            .observe(
                &context,
                Operation::LaneSelection,
                controls::lanes(
                    dynamics,
                    instance_id,
                    controller_id,
                    lane_selection,
                    now_millis,
                ),
            )
            .is_some();
        let targets_applied = observer
            .observe(
                &context,
                Operation::Targets,
                dynamics.reconcile_instance_targets_recorded(
                    instance_id,
                    light_dynamics::DynamicTargetScope {
                        ordered_targets: targets,
                    },
                    lookup.snapshot.dynamic_stage_positions.as_ref(),
                    inherited_spatial_mapping.as_ref(),
                    now_millis,
                ),
            )
            .is_some();
        observer.observe(
            &context,
            Operation::Controller,
            controls::update(
                dynamics,
                controller_id,
                Some(desired.overrides.size),
                Some(desired.overrides.speed_multiplier.factor() as f32),
                Some(desired.overrides.phase_offset_degrees),
                now_millis,
            ),
        );
        apply_programmer_output_gate(dynamics, observer, &context, now_millis, off);
        if selection_applied && targets_applied {
            emit_programmer_sources(emit, source_rows, instance_id, controller_id);
        }
        return;
    }
    if !ensure_fallback_definition(
        dynamics,
        lookup,
        observer,
        &context,
        &definition,
        now_millis,
    ) {
        return;
    }
    let scope_was_empty = targets.is_empty();
    let started = controls::start(
        dynamics,
        now_millis,
        programmer_start_request(
            controller_id,
            desired,
            &definition,
            targets,
            inherited_spatial_mapping,
            lookup,
            now_millis,
        ),
    );
    if let Some(instance_id) = observer.started(&context, scope_was_empty, started) {
        let selection_applied = observer
            .observe(
                &context,
                Operation::LaneSelection,
                controls::lanes(
                    dynamics,
                    instance_id,
                    controller_id,
                    lane_selection,
                    now_millis,
                ),
            )
            .is_some();
        apply_programmer_output_gate(dynamics, observer, &context, now_millis, off);
        if selection_applied {
            emit_programmer_sources(emit, source_rows, instance_id, controller_id);
        }
    }
}

/// The start request for a Programmer controller whose Dynamic is not yet running.
#[allow(clippy::too_many_arguments)]
fn programmer_start_request(
    controller_id: Uuid,
    desired: &DesiredProgrammerController<'_>,
    definition: &light_dynamics::DynamicDefinition,
    targets: Vec<FixtureId>,
    inherited_spatial_mapping: Option<light_dynamics::SpatialSelectionMapping>,
    lookup: &DestinationLookup<'_>,
    now_millis: u64,
) -> light_dynamics::DynamicStartRequest {
    light_dynamics::DynamicStartRequest {
        definition_id: definition.id,
        controller: light_dynamics::DynamicController {
            id: controller_id,
            source: light_dynamics::DynamicControllerSource::Programmer {
                programmer_id: desired.programmer_id,
                instance_link: Some(desired.instance_link),
            },
            priority: desired.priority,
            activated_at_millis: desired.activated_at_millis,
            size: desired.overrides.size,
            speed_multiplier: desired.overrides.speed_multiplier.factor() as f32,
            phase_offset_degrees: desired.overrides.phase_offset_degrees,
            paused: false,
        },
        target_scope: light_dynamics::DynamicTargetScope {
            ordered_targets: targets,
        },
        stage_positions: (*lookup.snapshot.dynamic_stage_positions).clone(),
        inherited_spatial_mapping,
        now_millis: desired.activated_at_millis.min(now_millis),
        activation_delay_millis: desired.timing.delay_millis.unwrap_or_default(),
        activation_duration_millis: desired.timing.fade_millis.unwrap_or_default(),
        activation_policy_override: None,
        reuse_matching_targetless: true,
    }
}

fn emit_programmer_sources(
    emit: &mut SourceEmitter<'_>,
    rows: Option<Vec<(FixtureId, Uuid, usize)>>,
    instance_id: Uuid,
    controller_id: Uuid,
) {
    if let (Some(emit), Some(rows)) = (emit.as_deref_mut(), rows) {
        for (target, lane_id, captured_index) in rows {
            emit(ReconciledSourceAssignment {
                instance_id,
                controller_id,
                target,
                lane_id,
                captured_index,
            });
        }
    }
}

fn apply_programmer_output_gate(
    dynamics: &mut light_dynamics::DynamicRuntime,
    observer: &mut ReconciliationObserver<'_>,
    context: &Context,
    now_millis: u64,
    off: Option<&light_dynamics::DynamicValueTiming>,
) {
    let gated = if let Some(timing) = off {
        controls::output_gate(
            dynamics,
            context.controller_id,
            false,
            now_millis,
            timing.delay_millis.unwrap_or_default(),
            timing.fade_millis.unwrap_or_default(),
        )
    } else {
        controls::clear_output_gate(dynamics, context.controller_id, now_millis)
    };
    observer.observe(context, Operation::OutputGate, gated);
}

fn release_programmer_off(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    off: &HashMap<Uuid, light_dynamics::DynamicValueTiming>,
    retained: &HashSet<Uuid>,
    observer: &mut ReconciliationObserver<'_>,
) {
    for (controller_id, timing) in off {
        if retained.contains(controller_id) {
            continue;
        }
        if let Some((instance_id, controller)) = dynamics.controller(*controller_id) {
            let released = controls::off(
                dynamics,
                instance_id,
                *controller_id,
                now_millis,
                timing.delay_millis.unwrap_or_default(),
                timing.fade_millis.unwrap_or_default(),
            );
            observer.observe(
                &Context::released(ColdDynamicReconciliationFlow::Programmer, &controller),
                Operation::Release,
                released,
            );
        }
    }
}

pub(in crate::runtime::output_scheduler) fn reconcile_cue_dynamics(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    cue_values: &[light_playback::ActiveCueDynamicValue],
) {
    reconcile_cue_dynamics_observed(
        dynamics,
        now_millis,
        snapshot,
        cue_values,
        None,
        &mut ReconciliationObserver::warm(),
    );
}

pub(in crate::runtime::output_scheduler) fn reconcile_cue_dynamics_with_sources(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    cue_values: &[light_playback::ActiveCueDynamicValue],
    mut emit: impl FnMut(ReconciledSourceAssignment),
) {
    reconcile_cue_dynamics_observed(
        dynamics,
        now_millis,
        snapshot,
        cue_values,
        Some(&mut emit),
        &mut ReconciliationObserver::warm(),
    );
}
