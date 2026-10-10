use super::*;
use light_application::{
    ApplicationEvent, DynamicRuntimeChange, DynamicRuntimeEventKind, OutputEvent,
};

fn owners() -> DynamicRuntimeSnapshot {
    let controller = |id, source, priority| DynamicController {
        id: Uuid::from_u128(id),
        source,
        priority,
        activated_at_millis: 100,
        size: 1.0,
        speed_multiplier: 1.0,
        phase_offset_degrees: 0.0,
        paused: false,
    };
    let instance: DynamicInstanceSnapshot = serde_json::from_value(serde_json::json!({
        "id": Uuid::from_u128(10), "definition": definition(), "targets": [],
        "controllers": [
            controller(1, DynamicControllerSource::Programmer {
                programmer_id: Uuid::from_u128(20), instance_link: None,
            }, 100),
            controller(2, DynamicControllerSource::Playback {
                playback_number: 28, virtual_page: None,
            }, 0)
        ],
        "started_at_millis": 100, "paused_at_millis": null, "paused_elapsed_millis": 0,
        "activation_policy": "start_now", "pending_until_millis": null,
        "speed_paused_at_millis": null, "speed_paused_elapsed_millis": 0, "random_streams": []
    }))
    .unwrap();
    DynamicRuntimeSnapshot {
        global_paused: false,
        instances: vec![instance],
    }
}

fn changes(
    before: &DynamicRuntimeSnapshot,
    after: &DynamicRuntimeSnapshot,
) -> Vec<DynamicRuntimeChange> {
    dynamic_transition_events(before, after, 200)
        .into_iter()
        .map(|event| {
            let ApplicationEvent::Output(OutputEvent::DynamicRuntimeChanged(change)) =
                event.payload
            else {
                panic!("unexpected event")
            };
            change
        })
        .collect()
}

#[test]
fn dynamic_removal_notifies_when_programmer_winner_survives_immediate_playback_off() {
    let before = owners();
    let mut after = before.clone();
    after.instances[0]
        .controllers
        .retain(|owner| owner.id != Uuid::from_u128(2));
    let events = changes(&before, &after);
    assert_eq!(
        events.len(),
        1,
        "nonwinning removal still invalidates Running"
    );
    assert_eq!(events[0].kind, DynamicRuntimeEventKind::InstanceOff);
    assert_eq!(events[0].controller_id, Some(Uuid::from_u128(2)));
    assert_eq!(events[0].runtime_instance_id, Some(before.instances[0].id));
    assert_eq!(
        events[0].dynamic_id,
        Some(before.instances[0].definition.id)
    );
    assert_eq!(events[0].winning_controller_id, Some(Uuid::from_u128(1)));
    assert_eq!(events[0].occurred_at_millis, 200);
    assert!(
        changes(&after, &after).is_empty(),
        "no repeated invalidation"
    );
}

#[test]
fn dynamic_removal_preserves_release_completion_and_notifies_last_immediate_owner() {
    let mut before = owners();
    before.instances[0]
        .controller_transitions
        .push(DynamicControllerTransitionSnapshot {
            controller_id: Uuid::from_u128(2),
            release_started_at_millis: Some(150),
            ..Default::default()
        });
    let mut after = before.clone();
    after.instances[0]
        .controllers
        .retain(|owner| owner.id != Uuid::from_u128(2));
    after.instances[0].controller_transitions.clear();
    let events = changes(&before, &after);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, DynamicRuntimeEventKind::TransitionCompleted);
    assert_eq!(events[0].controller_id, Some(Uuid::from_u128(2)));
    let events = changes(&after, &DynamicRuntimeSnapshot::default());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, DynamicRuntimeEventKind::InstanceOff);
    assert_eq!(events[0].controller_id, Some(Uuid::from_u128(1)));
    assert_eq!(events[0].winning_controller_id, None);
}
