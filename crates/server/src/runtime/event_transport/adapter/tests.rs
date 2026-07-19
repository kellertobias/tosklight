use super::*;
use light_application::{
    ActionContext, ActionSource, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowObjectsChange, EventBus, EventDraft, PatchChange, SelectiveShowImportChange,
    SelectiveShowObjectChange,
};
use light_core::ShowId;
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

    let wire::EventServerMessage::Event { event } =
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

    let wire::EventServerMessage::Event { event } =
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

    let wire::EventServerMessage::Event { event } =
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

fn context(source: ActionSource) -> ActionContext {
    ActionContext::operator(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        source,
    )
}
