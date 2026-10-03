//! Runtime events derived from the difference between two Dynamic runtime snapshots.

use super::*;

pub(in crate::runtime) fn dynamic_transition_events(
    before: &light_dynamics::DynamicRuntimeSnapshot,
    after: &light_dynamics::DynamicRuntimeSnapshot,
    now_millis: u64,
) -> Vec<light_application::EventDraft> {
    let mut events = Vec::new();
    if before.global_paused != after.global_paused {
        events.push(light_application::EventDraft::dynamic_runtime_changed(
            None,
            light_application::DynamicRuntimeChange {
                kind: if after.global_paused {
                    light_application::DynamicRuntimeEventKind::Paused
                } else {
                    light_application::DynamicRuntimeEventKind::Resumed
                },
                dynamic_id: None,
                runtime_instance_id: None,
                controller_id: None,
                winning_controller_id: None,
                occurred_at_millis: now_millis,
                message: Some("global Dynamic transport".into()),
            },
        ));
    }
    let before_instances = before
        .instances
        .iter()
        .map(|instance| (instance.id, instance))
        .collect::<HashMap<_, _>>();
    let after_instances = after
        .instances
        .iter()
        .map(|instance| (instance.id, instance))
        .collect::<HashMap<_, _>>();
    append_current_instance_events(&mut events, after, &before_instances, now_millis);
    for instance in &before.instances {
        let after_instance = after_instances.get(&instance.id).copied();
        let after_controllers = after_instance
            .map(|instance| {
                instance
                    .controllers
                    .iter()
                    .map(|controller| controller.id)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        for transition in &instance.controller_transitions {
            if transition.release_started_at_millis.is_some()
                && !after_controllers.contains(&transition.controller_id)
            {
                events.push(light_application::EventDraft::dynamic_runtime_changed(
                    None,
                    light_application::DynamicRuntimeChange {
                        kind: light_application::DynamicRuntimeEventKind::TransitionCompleted,
                        dynamic_id: Some(instance.definition.id),
                        runtime_instance_id: Some(instance.id),
                        controller_id: Some(transition.controller_id),
                        winning_controller_id: winning_controller(after_instance),
                        occurred_at_millis: now_millis,
                        message: None,
                    },
                ));
            }
        }
        let before_winner = winning_controller(Some(instance));
        let after_winner = winning_controller(after_instance);
        if before_winner != after_winner && after_winner.is_some() {
            events.push(light_application::EventDraft::dynamic_runtime_changed(
                None,
                light_application::DynamicRuntimeChange {
                    kind: light_application::DynamicRuntimeEventKind::ControllerWinnerChanged,
                    dynamic_id: Some(instance.definition.id),
                    runtime_instance_id: Some(instance.id),
                    controller_id: after_winner,
                    winning_controller_id: after_winner,
                    occurred_at_millis: now_millis,
                    message: None,
                },
            ));
        }
    }
    events
}

fn append_current_instance_events(
    events: &mut Vec<light_application::EventDraft>,
    after: &light_dynamics::DynamicRuntimeSnapshot,
    before_instances: &HashMap<Uuid, &light_dynamics::DynamicInstanceSnapshot>,
    now_millis: u64,
) {
    for instance in &after.instances {
        let previous = before_instances.get(&instance.id).copied();
        if instance.completed && !previous.is_some_and(|previous| previous.completed) {
            events.push(light_application::EventDraft::dynamic_runtime_changed(
                None,
                light_application::DynamicRuntimeChange {
                    kind: light_application::DynamicRuntimeEventKind::InstanceOff,
                    dynamic_id: Some(instance.definition.id),
                    runtime_instance_id: Some(instance.id),
                    controller_id: winning_controller(previous),
                    winning_controller_id: None,
                    occurred_at_millis: now_millis,
                    message: Some("one-shot completed".into()),
                },
            ));
        }
        let previous_controllers = previous
            .map(|instance| {
                instance
                    .controllers
                    .iter()
                    .map(|controller| controller.id)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        if previous.is_some_and(|previous| previous.completed) && !instance.completed {
            let controller_id = winning_controller(Some(instance));
            for kind in [
                light_application::DynamicRuntimeEventKind::InstanceStarted,
                if instance
                    .pending_until_millis
                    .is_some_and(|boundary| now_millis < boundary)
                {
                    light_application::DynamicRuntimeEventKind::InstancePending
                } else {
                    light_application::DynamicRuntimeEventKind::InstanceActive
                },
            ] {
                events.push(light_application::EventDraft::dynamic_runtime_changed(
                    None,
                    light_application::DynamicRuntimeChange {
                        kind,
                        dynamic_id: Some(instance.definition.id),
                        runtime_instance_id: Some(instance.id),
                        controller_id,
                        winning_controller_id: controller_id,
                        occurred_at_millis: now_millis,
                        message: Some("one-shot retriggered".into()),
                    },
                ));
            }
        }
        for controller in &instance.controllers {
            if instance.completed || previous.is_some_and(|previous| previous.completed) {
                continue;
            }
            if previous_controllers.contains(&controller.id) {
                let old_gate = previous.and_then(|previous| {
                    previous
                        .controller_transitions
                        .iter()
                        .find(|transition| transition.controller_id == controller.id)
                        .and_then(|transition| transition.output_gate)
                });
                let new_gate = instance
                    .controller_transitions
                    .iter()
                    .find(|transition| transition.controller_id == controller.id)
                    .and_then(|transition| transition.output_gate);
                if old_gate != new_gate {
                    events.push(light_application::EventDraft::dynamic_runtime_changed(
                        None,
                        light_application::DynamicRuntimeChange {
                            kind: light_application::DynamicRuntimeEventKind::ControllerUpdated,
                            dynamic_id: Some(instance.definition.id),
                            runtime_instance_id: Some(instance.id),
                            controller_id: Some(controller.id),
                            winning_controller_id: winning_controller(Some(instance)),
                            occurred_at_millis: now_millis,
                            message: None,
                        },
                    ));
                }
            }
            if !previous_controllers.contains(&controller.id) {
                for kind in [
                    light_application::DynamicRuntimeEventKind::InstanceStarted,
                    if instance
                        .pending_until_millis
                        .is_some_and(|boundary| now_millis < boundary)
                    {
                        light_application::DynamicRuntimeEventKind::InstancePending
                    } else {
                        light_application::DynamicRuntimeEventKind::InstanceActive
                    },
                ] {
                    events.push(light_application::EventDraft::dynamic_runtime_changed(
                        None,
                        light_application::DynamicRuntimeChange {
                            kind,
                            dynamic_id: Some(instance.definition.id),
                            runtime_instance_id: Some(instance.id),
                            controller_id: Some(controller.id),
                            winning_controller_id: winning_controller(Some(instance)),
                            occurred_at_millis: now_millis,
                            message: None,
                        },
                    ));
                }
            }
        }
        if !instance.completed
            && previous.is_some_and(|previous| previous.pending_until_millis.is_some())
            && instance
                .pending_until_millis
                .is_none_or(|boundary| now_millis >= boundary)
        {
            events.push(light_application::EventDraft::dynamic_runtime_changed(
                None,
                light_application::DynamicRuntimeChange {
                    kind: light_application::DynamicRuntimeEventKind::InstanceActive,
                    dynamic_id: Some(instance.definition.id),
                    runtime_instance_id: Some(instance.id),
                    controller_id: winning_controller(Some(instance)),
                    winning_controller_id: winning_controller(Some(instance)),
                    occurred_at_millis: now_millis,
                    message: None,
                },
            ));
        }
    }
}

fn winning_controller(instance: Option<&light_dynamics::DynamicInstanceSnapshot>) -> Option<Uuid> {
    instance
        .filter(|instance| !instance.completed)
        .and_then(|instance| {
            instance
                .controllers
                .iter()
                .max_by_key(|controller| {
                    (
                        controller.priority,
                        controller.activated_at_millis,
                        controller.id,
                    )
                })
                .map(|controller| controller.id)
        })
}
