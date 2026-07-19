//! Translation between transport-independent application events and v2 wire DTOs.

mod selective_import;

use light_application as application;
use light_wire::v2::events as wire;
use uuid::Uuid;

pub(super) fn application_rate_limits(
    limits: Vec<wire::EventRateLimit>,
) -> Vec<application::ReplaceableEventRateLimit> {
    limits
        .into_iter()
        .map(|limit| application::ReplaceableEventRateLimit {
            capability: app_capability(limit.capability),
            class: app_class(limit.class),
            object: limit.object.map(app_object),
            min_interval: std::time::Duration::from_millis(limit.min_interval_millis),
        })
        .collect()
}

pub(super) fn application_filter(
    desk_id: Uuid,
    filter: wire::EventSubscriptionFilter,
) -> application::EventFilter {
    application::EventFilter {
        desk_id: Some(desk_id),
        capabilities: filter
            .capabilities
            .into_iter()
            .map(app_capability)
            .collect(),
        classes: filter.classes.into_iter().map(app_class).collect(),
        objects: filter.objects.into_iter().map(app_object).collect(),
    }
}

fn app_object(object: wire::EventObject) -> application::EventObject {
    application::EventObject::new(app_capability(object.capability), object.id)
}

fn app_capability(capability: wire::EventCapability) -> application::EventCapability {
    use application::EventCapability as App;
    match capability {
        wire::EventCapability::Programmer => App::Programmer,
        wire::EventCapability::Playback => App::Playback,
        wire::EventCapability::Show => App::Show,
        wire::EventCapability::Desk => App::Desk,
        wire::EventCapability::Output => App::Output,
        wire::EventCapability::System => App::System,
    }
}

fn app_class(class: wire::EventClass) -> application::EventClass {
    use application::EventClass as App;
    match class {
        wire::EventClass::Transition => App::Transition,
        wire::EventClass::Projection => App::Projection,
        wire::EventClass::CommandOutcome => App::CommandOutcome,
        wire::EventClass::Error => App::Error,
        wire::EventClass::Safety => App::Safety,
        wire::EventClass::Telemetry => App::Telemetry,
    }
}

pub(super) fn wire_delivery(
    delivery: application::SubscriptionDelivery,
) -> Option<wire::EventServerMessage> {
    match delivery {
        application::SubscriptionDelivery::Event(event) => {
            wire_event(&event).map(|event| wire::EventServerMessage::Event {
                event: Box::new(event),
            })
        }
        application::SubscriptionDelivery::Gap(gap) => {
            Some(wire::EventServerMessage::Gap { gap: wire_gap(gap) })
        }
    }
}

pub(super) fn wire_gap(gap: application::SequenceGap) -> wire::SequenceGap {
    wire::SequenceGap {
        after_sequence: gap.after_sequence,
        oldest_available: gap.oldest_available,
        latest_sequence: gap.latest_sequence,
    }
}

fn wire_event(event: &application::EventEnvelope) -> Option<wire::EventEnvelope> {
    Some(wire::EventEnvelope {
        sequence: event.sequence,
        occurred_at: event.occurred_at.to_rfc3339(),
        desk_id: event.desk_id,
        class: wire_class(event.class),
        object: event.object.as_ref().map(wire_object),
        related_objects: (!event.related_objects.is_empty())
            .then(|| event.related_objects.iter().map(wire_object).collect()),
        source: wire_source(event.source),
        correlation_id: event.correlation_id,
        delivery: wire_delivery_policy(event.delivery),
        payload: wire_payload(&event.payload, event.sequence)?,
    })
}

fn wire_object(object: &application::EventObject) -> wire::EventObject {
    wire::EventObject {
        capability: wire_capability(object.capability),
        id: object.id.clone(),
    }
}

fn wire_capability(capability: application::EventCapability) -> wire::EventCapability {
    use application::EventCapability as App;
    match capability {
        App::Programmer => wire::EventCapability::Programmer,
        App::Playback => wire::EventCapability::Playback,
        App::Show => wire::EventCapability::Show,
        App::Desk => wire::EventCapability::Desk,
        App::Output => wire::EventCapability::Output,
        App::System => wire::EventCapability::System,
    }
}

fn wire_class(class: application::EventClass) -> wire::EventClass {
    use application::EventClass as App;
    match class {
        App::Transition => wire::EventClass::Transition,
        App::Projection => wire::EventClass::Projection,
        App::CommandOutcome => wire::EventClass::CommandOutcome,
        App::Error => wire::EventClass::Error,
        App::Safety => wire::EventClass::Safety,
        App::Telemetry => wire::EventClass::Telemetry,
    }
}

fn wire_delivery_policy(policy: application::DeliveryPolicy) -> wire::EventDeliveryPolicy {
    match policy {
        application::DeliveryPolicy::Lossless => wire::EventDeliveryPolicy::Lossless,
        application::DeliveryPolicy::Replaceable => wire::EventDeliveryPolicy::Replaceable,
    }
}

fn wire_source(source: application::EventSource) -> wire::EventSource {
    match source {
        application::EventSource::Runtime => wire::EventSource::Runtime,
        application::EventSource::Action(source) => wire::EventSource::Action {
            source: wire_action_source(source),
        },
    }
}

fn wire_action_source(source: application::ActionSource) -> wire::EventActionSource {
    use application::ActionSource as App;
    match source {
        App::UserInterface => wire::EventActionSource::UserInterface,
        App::Keyboard => wire::EventActionSource::Keyboard,
        App::Osc => wire::EventActionSource::Osc,
        App::Http => wire::EventActionSource::Http,
        App::Midi => wire::EventActionSource::Midi,
        App::Matter => wire::EventActionSource::Matter,
        App::Cue => wire::EventActionSource::Cue,
        App::Timecode => wire::EventActionSource::Timecode,
        App::Scheduler => wire::EventActionSource::Scheduler,
        App::Macro => wire::EventActionSource::Macro,
        App::System => wire::EventActionSource::System,
    }
}

fn wire_payload(
    payload: &application::ApplicationEvent,
    sequence: u64,
) -> Option<wire::EventPayload> {
    Some(match payload {
        // Programming transport DTOs arrive in the next vertical slice. The application event is
        // already authoritative and advances the shared cursor, but is intentionally not exposed
        // through the v2 socket until that wire contract exists.
        application::ApplicationEvent::Programming(_) => return None,
        application::ApplicationEvent::Playback(application::PlaybackEvent::RuntimeChanged(
            change,
        )) => wire::EventPayload::PlaybackRuntimeChanged {
            change: super::super::playback_v2::runtime_change(change),
        },
        application::ApplicationEvent::Desk(application::DeskEvent::PlaybackViewChanged(
            projection,
        )) => wire::EventPayload::PlaybackViewChanged {
            projection: super::super::playback_v2::desk_projection(*projection),
        },
        application::ApplicationEvent::Output(application::OutputEvent::RuntimeChanged(change)) => {
            wire::EventPayload::OutputRuntimeChanged {
                change: super::super::output_runtime_v2::wire_change(*change),
            }
        }
        application::ApplicationEvent::Show(application::ShowEvent::PatchChanged(change)) => {
            wire::EventPayload::ShowPatchChanged {
                delta: super::super::show_patch_wire::wire_delta(change, Some(sequence)),
            }
        }
        application::ApplicationEvent::Show(application::ShowEvent::OutputRouteChanged(change)) => {
            wire::EventPayload::OutputRouteChanged {
                change: wire_output_route_change(change),
            }
        }
        application::ApplicationEvent::Show(application::ShowEvent::ObjectsChanged(change)) => {
            wire::EventPayload::ShowObjectsChanged {
                change: wire_show_objects_change(change),
            }
        }
        application::ApplicationEvent::Show(application::ShowEvent::SelectiveImportApplied(
            change,
        )) => wire::EventPayload::SelectiveImportApplied {
            change: Box::new(selective_import::wire_change(change)),
        },
    })
}

fn wire_show_objects_change(
    change: &application::ActiveShowObjectsChange,
) -> wire::ShowObjectsChange {
    wire::ShowObjectsChange {
        show_id: change.show_id.0,
        show_revision: change.show_revision.value(),
        changes: change.changes.iter().map(wire_show_object_change).collect(),
    }
}

fn wire_show_object_change(change: &application::ActiveShowObjectChange) -> wire::ShowObjectChange {
    wire::ShowObjectChange {
        kind: match change.kind {
            application::ActiveShowObjectKind::CueList => wire::ShowObjectKind::CueList,
            application::ActiveShowObjectKind::Group => wire::ShowObjectKind::Group,
            application::ActiveShowObjectKind::Playback => wire::ShowObjectKind::Playback,
            application::ActiveShowObjectKind::PlaybackPage => wire::ShowObjectKind::PlaybackPage,
            application::ActiveShowObjectKind::Preset => wire::ShowObjectKind::Preset,
        },
        object_id: change.object_id.clone(),
        object_revision: change.object_revision,
        body: change.body.clone(),
        deleted: change.deleted,
    }
}

fn wire_output_route_change(change: &application::OutputRouteChange) -> wire::OutputRouteChange {
    wire::OutputRouteChange {
        show_id: change.show_id.0,
        show_revision: change.show_revision.value(),
        route_id: change.route_id.clone(),
        object_revision: change.object_revision,
        route: change.route.as_ref().map(wire_output_route),
        deleted: change.deleted,
    }
}

fn wire_output_route(route: &light_output::OutputRoute) -> wire::OutputRoute {
    wire::OutputRoute {
        protocol: match route.protocol {
            light_output::Protocol::ArtNet => wire::OutputProtocol::ArtNet,
            light_output::Protocol::Sacn => wire::OutputProtocol::Sacn,
        },
        logical_universe: route.logical_universe,
        destination_universe: route.destination_universe,
        delivery_mode: match route.resolved_delivery_mode() {
            light_output::DeliveryMode::Broadcast => wire::OutputDeliveryMode::Broadcast,
            light_output::DeliveryMode::Multicast => wire::OutputDeliveryMode::Multicast,
            light_output::DeliveryMode::Unicast => wire::OutputDeliveryMode::Unicast,
        },
        destination: route.destination.map(|destination| destination.to_string()),
        enabled: route.enabled,
        minimum_slots: route.minimum_slots,
    }
}

#[cfg(test)]
mod tests;
