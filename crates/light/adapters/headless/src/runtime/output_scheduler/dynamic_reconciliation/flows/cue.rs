//! Reconciles the Dynamic controllers that active Cue rows author.

use super::programmer_sources::planned_cue_source_rows;
use super::*;

fn cue_controller_plan(
    cue_values: &[light_playback::ActiveCueDynamicValue],
) -> (
    Vec<DesiredCueController<'_>>,
    HashMap<Uuid, light_dynamics::DynamicValueTiming>,
) {
    let mut desired = Vec::<DesiredCueController>::new();
    let mut release_timings = HashMap::<Uuid, light_dynamics::DynamicValueTiming>::new();
    for (captured_index, stored) in cue_values.iter().enumerate() {
        if let light_dynamics::DynamicSemanticValue::DynamicOff {
            instance_link,
            timing,
        } = &stored.value
        {
            release_timings.insert(stored.source_key.controller_id(*instance_link), *timing);
            continue;
        }
        let light_dynamics::DynamicSemanticValue::DynamicOn {
            instance_link,
            dynamic,
            overrides,
            timing,
            lane_id,
        } = &stored.value
        else {
            continue;
        };
        let controller_id = stored.source_key.controller_id(*instance_link);
        if let Some(controller) = desired
            .iter_mut()
            .find(|candidate| candidate.controller_id == controller_id)
        {
            controller.lane_rows.push((stored.fixture_id, *lane_id));
            controller.source_rows.push((captured_index, stored));
            if controller.target_ids.insert(stored.fixture_id) {
                controller.targets.push(stored.fixture_id);
            }
            continue;
        }
        desired.push(DesiredCueController {
            controller_id,
            instance_link: *instance_link,
            cue_list_id: stored.cue_list_id,
            priority: stored.priority,
            activated_at_millis: stored.changed_at_millis,
            reference: dynamic,
            overrides,
            timing: *timing,
            targets: vec![stored.fixture_id],
            target_ids: HashSet::from([stored.fixture_id]),
            lane_rows: vec![(stored.fixture_id, *lane_id)],
            source_rows: vec![(captured_index, stored)],
        });
    }
    (desired, release_timings)
}

pub(in super::super) fn reconcile_cue_dynamics_observed(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    cue_values: &[light_playback::ActiveCueDynamicValue],
    mut emit: SourceEmitter<'_>,
    observer: &mut ReconciliationObserver<'_>,
) {
    let (desired, release_timings) = cue_controller_plan(cue_values);
    let desired_ids = desired
        .iter()
        .map(|controller| controller.controller_id)
        .collect::<HashSet<_>>();
    release_inactive_cue_controllers(
        dynamics,
        &desired_ids,
        &release_timings,
        now_millis,
        observer,
    );
    if desired.is_empty() {
        return;
    }

    let lookup = DestinationLookup::new(snapshot);
    for desired in desired {
        reconcile_cue_controller(dynamics, now_millis, &lookup, &desired, &mut emit, observer);
    }
}

fn reconcile_cue_controller(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    lookup: &DestinationLookup<'_>,
    desired: &DesiredCueController<'_>,
    emit: &mut SourceEmitter<'_>,
    observer: &mut ReconciliationObserver<'_>,
) {
    let controller_id = desired.controller_id;
    let context = Context::cue(controller_id, desired.cue_list_id.0, desired.instance_link);
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
        .map(|_| planned_cue_source_rows(desired, &definition, &lane_selection, &targets));
    if let Some((instance_id, _)) = existing {
        observer.observe(
            &context,
            Operation::CancelRelease,
            controls::cancel_release(dynamics, controller_id, now_millis),
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
        light_dynamics::DynamicStartRequest {
            definition_id: definition.id,
            controller: light_dynamics::DynamicController {
                id: controller_id,
                source: light_dynamics::DynamicControllerSource::Cue {
                    cue_list_id: desired.cue_list_id.0,
                    instance_link: desired.instance_link,
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
            reuse_matching_targetless: false,
        },
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
        if selection_applied {
            emit_programmer_sources(emit, source_rows, instance_id, controller_id);
        }
    }
}
