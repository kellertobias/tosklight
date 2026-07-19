use super::*;
use light_application::{
    ActionContext, ActionSource, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowObjectsChange, EventBus, EventDraft, OutputRuntimeChange, OutputRuntimeIdentity,
    OutputRuntimeProjection, OutputRuntimeScope, PatchChange, ProgrammingInteractionChange,
    ProgrammingValuesChange, ProgrammingValuesProjection, SelectiveShowImportChange,
    SelectiveShowObjectChange,
};
use light_core::{AttributeKey, AttributeValue, ShowId, UserId};
use light_show::PortableShowObjectKey;

#[test]
fn patch_event_delta_uses_the_authoritative_envelope_sequence() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Http);
    let show_id = ShowId(Uuid::from_u128(4));
    let event = bus.publish(EventDraft::patch_changed(
        &context,
        PatchChange {
            show_id,
            show_revision: Default::default(),
            patch_revision: Default::default(),
            fixtures: Vec::new(),
            removed_fixture_ids: Vec::new(),
            profile_revisions: Vec::new(),
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected an event delivery");
    };
    let wire::EventPayload::ShowPatchChanged { delta } = event.payload else {
        panic!("expected a show Patch event");
    };
    assert_eq!(event.sequence, 1);
    assert_eq!(delta.show_id, show_id.0);
    assert_eq!(delta.event_sequence, Some(event.sequence));
}

#[test]
fn show_object_batch_keeps_one_event_and_targeted_raw_deltas() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Osc);
    let show_id = ShowId(Uuid::from_u128(4));
    let object_id = Uuid::from_u128(7);
    let event = bus.publish(EventDraft::active_show_objects_changed(
        &context,
        ActiveShowObjectsChange {
            show_id,
            show_revision: Default::default(),
            changes: vec![ActiveShowObjectChange {
                kind: ActiveShowObjectKind::CueList,
                object_id: object_id.to_string(),
                object_revision: 3,
                body: Some(serde_json::json!({
                    "id": object_id,
                    "cues": [{"id": Uuid::from_u128(8), "future": true}]
                })),
                deleted: false,
            }],
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected an event delivery");
    };
    let wire::EventPayload::ShowObjectsChanged { change } = event.payload else {
        panic!("expected a show-object event");
    };
    assert_eq!(event.sequence, 1);
    assert_eq!(
        event.source,
        wire::EventSource::Action {
            source: wire::EventActionSource::Osc
        }
    );
    assert_eq!(event.correlation_id, Some(context.correlation_id));
    assert_eq!(
        event.related_objects,
        Some(vec![
            wire::EventObject {
                capability: wire::EventCapability::Show,
                id: format!("objects:{}:kind:cue_list", show_id.0),
            },
            wire::EventObject {
                capability: wire::EventCapability::Show,
                id: format!("objects:{}:kind:cue_list:object:{object_id}", show_id.0),
            },
        ])
    );
    assert_eq!(change.show_id, show_id.0);
    assert_eq!(change.show_revision, 0);
    assert_eq!(change.changes[0].kind, wire::ShowObjectKind::CueList);
    assert_eq!(change.changes[0].object_id, object_id.to_string());
    assert_eq!(
        change.changes[0].body.as_ref().unwrap()["cues"][0]["future"],
        true
    );
}

#[test]
fn selective_import_maps_exact_raw_changes_and_related_routes() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Http);
    let show_id = ShowId(Uuid::from_u128(4));
    let event = bus.publish(EventDraft::selective_import_applied(
        &context,
        SelectiveShowImportChange {
            show_id,
            show_revision: Default::default(),
            outcomes: Vec::new(),
            objects: vec![SelectiveShowObjectChange {
                key: PortableShowObjectKey::new("group", "1"),
                object_revision: 7,
                body: serde_json::json!({"fixtures":["fixture-1"]}),
            }],
            profiles: Vec::new(),
            managed_assets: Vec::new(),
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected an event delivery");
    };
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Show,
            id: format!("objects:{}", show_id.0),
        })
    );
    assert!(event.related_objects.as_ref().is_some_and(|objects| {
        objects
            .iter()
            .any(|object| object.id == format!("objects:{}:kind:group:object:1", show_id.0))
    }));
    let wire::EventPayload::SelectiveImportApplied { change } = event.payload else {
        panic!("expected a selective-import event");
    };
    assert_eq!(change.show_id, show_id.0);
    assert_eq!(change.objects[0].kind, "group");
    assert_eq!(change.objects[0].object_revision, 7);
    assert_eq!(change.objects[0].body["fixtures"][0], "fixture-1");
}

#[test]
fn global_output_change_keeps_identity_source_and_correlation() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Midi);
    let event = bus.publish(EventDraft::output_runtime_changed(
        &context,
        OutputRuntimeChange {
            projection: OutputRuntimeProjection {
                scope: OutputRuntimeScope {
                    show_id: Uuid::from_u128(40),
                    show_revision: 7,
                },
                identity: OutputRuntimeIdentity::GlobalMaster,
                grand_master: 0.6,
                blackout: true,
            },
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected an event delivery");
    };
    assert_eq!(event.desk_id, None);
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Output,
            id: "runtime:global-master".into(),
        })
    );
    assert_eq!(
        event.source,
        wire::EventSource::Action {
            source: wire::EventActionSource::Midi
        }
    );
    assert_eq!(event.correlation_id, Some(context.correlation_id));
    let wire::EventPayload::OutputRuntimeChanged { change } = event.payload else {
        panic!("expected an output-runtime event");
    };
    assert_eq!(
        change.projection.identity,
        wire::OutputRuntimeIdentity::GlobalMaster
    );
    assert_eq!(change.projection.scope.show_revision, 7);
    assert_eq!(change.projection.grand_master, 0.6);
    assert!(change.projection.blackout);
}

#[test]
fn programming_interaction_keeps_exact_desk_scope_and_sparse_payload() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::UserInterface);
    let event = bus.publish(EventDraft::programming_interaction_changed(
        &context,
        ProgrammingInteractionChange::from_components(
            context.desk_id,
            Some(Default::default()),
            None,
        )
        .unwrap(),
    ));
    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a Programming interaction delivery")
    };
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Desk,
            id: format!("programming-command-line:{}", context.desk_id),
        })
    );
    let wire::EventPayload::ProgrammingInteractionChanged { change } = event.payload else {
        panic!("expected a Programming interaction payload")
    };
    let light_wire::v2::command_line::ProgrammingInteractionChange::CommandLine {
        desk_id,
        command_line,
    } = change
    else {
        panic!("expected a command-line-only change")
    };
    assert_eq!(desk_id, context.desk_id);
    assert_eq!(command_line.text, "FIXTURE");
    assert_eq!(event.delivery, wire::EventDeliveryPolicy::Lossless);
}

#[test]
fn combined_programming_change_routes_once_through_both_exact_objects() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Osc);
    let event = bus.publish(EventDraft::programming_interaction_changed(
        &context,
        ProgrammingInteractionChange::from_components(
            context.desk_id,
            Some(Default::default()),
            Some(Default::default()),
        )
        .unwrap(),
    ));
    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a Programming interaction delivery")
    };
    assert_eq!(
        event.object.as_ref().unwrap().id,
        format!("programming-command-line:{}", context.desk_id)
    );
    assert_eq!(
        event.related_objects.as_ref().unwrap(),
        &[wire::EventObject {
            capability: wire::EventCapability::Desk,
            id: format!("programming-selection:{}", context.desk_id),
        }]
    );
    let wire::EventPayload::ProgrammingInteractionChanged { change } = event.payload else {
        panic!("expected a Programming interaction payload")
    };
    assert!(matches!(
        change,
        light_wire::v2::command_line::ProgrammingInteractionChange::Both { .. }
    ));
}

#[test]
fn programming_values_keep_user_scope_full_projection_and_action_identity() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Osc);
    let user_id = UserId(Uuid::from_u128(3));
    let event = bus.publish(EventDraft::programming_values_changed(
        &context,
        ProgrammingValuesChange {
            projection: ProgrammingValuesProjection {
                user_id,
                revision: 7,
                fixture_values: Vec::new(),
                group_values: vec![light_programmer::ProgrammerGroupUpdate {
                    group_id: "2.1".into(),
                    attribute: AttributeKey::intensity(),
                    value: AttributeValue::Normalized(0.75),
                    programmer_order: 9,
                    fade: true,
                    fade_millis: Some(1_000),
                    delay_millis: Some(250),
                }],
            },
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a Programmer values delivery")
    };
    assert_eq!(event.desk_id, None);
    assert_eq!(event.class, wire::EventClass::Projection);
    assert_eq!(event.delivery, wire::EventDeliveryPolicy::Replaceable);
    assert_eq!(event.correlation_id, Some(context.correlation_id));
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Programmer,
            id: format!("programming-values:{}", user_id.0),
        })
    );
    let wire::EventPayload::ProgrammingValuesChanged { change } = event.payload else {
        panic!("expected a Programmer values payload")
    };
    assert_eq!(change.projection.user_id, user_id.0);
    assert_eq!(change.projection.revision, 7);
    let value = &change.projection.group_values[0];
    assert_eq!(value.group_id, "2.1");
    assert_eq!(value.programmer_order, 9);
    assert!(value.fade);
    assert_eq!(value.fade_millis, Some(1_000));
    assert_eq!(value.delay_millis, Some(250));
}

#[test]
fn sequence_gaps_always_forward() {
    let Some(wire::EventServerMessage::Gap { gap }) = wire_delivery(
        application::SubscriptionDelivery::Gap(application::SequenceGap {
            after_sequence: 1,
            oldest_available: 3,
            latest_sequence: 4,
        }),
    ) else {
        panic!("sequence gaps must remain visible")
    };
    assert_eq!(gap.after_sequence, 1);
    assert_eq!(gap.oldest_available, 3);
    assert_eq!(gap.latest_sequence, 4);
}

fn context(source: ActionSource) -> ActionContext {
    ActionContext::operator(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        source,
    )
}
