//! Translation between transport-independent application events and v2 wire DTOs.

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
) -> wire::EventServerMessage {
    match delivery {
        application::SubscriptionDelivery::Event(event) => wire::EventServerMessage::Event {
            event: wire_event(&event),
        },
        application::SubscriptionDelivery::Gap(gap) => {
            wire::EventServerMessage::Gap { gap: wire_gap(gap) }
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

fn wire_event(event: &application::EventEnvelope) -> wire::EventEnvelope {
    wire::EventEnvelope {
        sequence: event.sequence,
        occurred_at: event.occurred_at.to_rfc3339(),
        desk_id: event.desk_id,
        class: wire_class(event.class),
        object: event.object.as_ref().map(wire_object),
        source: wire_source(event.source),
        correlation_id: event.correlation_id,
        delivery: wire_delivery_policy(event.delivery),
        payload: wire_payload(&event.payload, event.sequence),
    }
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

fn wire_payload(payload: &application::ApplicationEvent, sequence: u64) -> wire::EventPayload {
    match payload {
        application::ApplicationEvent::Playback(application::PlaybackEvent::CueTransition(
            transition,
        )) => wire::EventPayload::PlaybackCueTransition {
            transition: wire_transition(transition),
        },
        application::ApplicationEvent::Show(application::ShowEvent::PatchChanged(change)) => {
            wire::EventPayload::ShowPatchChanged {
                delta: super::super::show_patch_wire::wire_delta(change, Some(sequence)),
            }
        }
    }
}

fn wire_transition(transition: &application::PlaybackCueTransition) -> wire::PlaybackCueTransition {
    wire::PlaybackCueTransition {
        playback_number: transition.playback_number,
        cue_list_id: transition.cue_list_id,
        previous: transition.previous.as_ref().map(wire_cue),
        current: transition.current.as_ref().map(wire_cue),
        cause: wire_cause(transition.cause),
        advanced_steps: transition.advanced_steps,
    }
}

fn wire_cue(cue: &application::CueReference) -> wire::CueReference {
    wire::CueReference {
        id: cue.id,
        number: cue.number,
    }
}

fn wire_cause(cause: application::PlaybackTransitionCause) -> wire::PlaybackTransitionCause {
    use application::PlaybackTransitionCause as App;
    match cause {
        App::Go => wire::PlaybackTransitionCause::Go,
        App::Back => wire::PlaybackTransitionCause::Back,
        App::Jump => wire::PlaybackTransitionCause::Jump,
        App::Chaser => wire::PlaybackTransitionCause::Chaser,
        App::Follow => wire::PlaybackTransitionCause::Follow,
        App::Wait => wire::PlaybackTransitionCause::Wait,
        App::Timecode => wire::PlaybackTransitionCause::Timecode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_application::{ActionContext, ActionSource, EventBus, EventDraft, PatchChange};
    use light_core::ShowId;

    #[test]
    fn patch_event_delta_uses_the_authoritative_envelope_sequence() {
        let bus = EventBus::new(4);
        let context = ActionContext::operator(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            ActionSource::Http,
        );
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
}
