use std::{collections::HashSet, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::ActionSource;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EventSource {
    Action(ActionSource),
    Runtime,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EventClass {
    Transition,
    Projection,
    CommandOutcome,
    Error,
    Safety,
    Telemetry,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EventCapability {
    Programmer,
    Playback,
    Show,
    Desk,
    Output,
    System,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EventObject {
    pub capability: EventCapability,
    pub id: String,
}

impl EventObject {
    pub fn new(capability: EventCapability, id: impl Into<String>) -> Self {
        Self {
            capability,
            id: id.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeliveryPolicy {
    /// A queue overflow becomes an explicit sequence gap requiring snapshot repair.
    Lossless,
    /// An older queued event for the same object and class may be replaced by the newest value.
    Replaceable,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlaybackTransitionCause {
    Go,
    Back,
    Jump,
    Chaser,
    Follow,
    Wait,
    Timecode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CueReference {
    pub id: Uuid,
    pub number: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackCueTransition {
    pub playback_number: Option<u16>,
    pub cue_list_id: Uuid,
    pub previous: Option<CueReference>,
    pub current: Option<CueReference>,
    pub cause: PlaybackTransitionCause,
    pub advanced_steps: u64,
}

impl PlaybackCueTransition {
    fn object_id(&self) -> String {
        self.playback_number.map_or_else(
            || format!("cuelist:{}", self.cue_list_id),
            |number| format!("playback:{number}"),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlaybackEvent {
    CueTransition(PlaybackCueTransition),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ApplicationEvent {
    Playback(PlaybackEvent),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventDraft {
    /// `None` denotes an installation-global transition observed by every desk.
    pub desk_id: Option<Uuid>,
    pub class: EventClass,
    pub object: Option<EventObject>,
    pub source: EventSource,
    pub correlation_id: Option<Uuid>,
    pub delivery: DeliveryPolicy,
    pub payload: ApplicationEvent,
}

impl EventDraft {
    pub fn playback_transition(
        desk_id: Option<Uuid>,
        transition: PlaybackCueTransition,
        source: EventSource,
        correlation_id: Option<Uuid>,
    ) -> Self {
        let object = EventObject::new(EventCapability::Playback, transition.object_id());
        Self {
            desk_id,
            class: EventClass::Transition,
            object: Some(object),
            source,
            correlation_id,
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Playback(PlaybackEvent::CueTransition(transition)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventEnvelope {
    pub sequence: u64,
    pub occurred_at: DateTime<Utc>,
    pub desk_id: Option<Uuid>,
    pub class: EventClass,
    pub object: Option<EventObject>,
    pub source: EventSource,
    pub correlation_id: Option<Uuid>,
    pub delivery: DeliveryPolicy,
    pub payload: ApplicationEvent,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EventFilter {
    pub desk_id: Option<Uuid>,
    pub capabilities: HashSet<EventCapability>,
    pub classes: HashSet<EventClass>,
    pub objects: HashSet<EventObject>,
}

impl EventFilter {
    pub fn for_desk(desk_id: Uuid) -> Self {
        Self {
            desk_id: Some(desk_id),
            ..Self::default()
        }
    }

    pub fn with_capability(mut self, capability: EventCapability) -> Self {
        self.capabilities.insert(capability);
        self
    }

    pub fn with_class(mut self, class: EventClass) -> Self {
        self.classes.insert(class);
        self
    }

    pub fn with_object(mut self, object: EventObject) -> Self {
        self.objects.insert(object);
        self
    }

    pub(super) fn matches(&self, event: &EventEnvelope) -> bool {
        if self
            .desk_id
            .zip(event.desk_id)
            .is_some_and(|(requested, actual)| requested != actual)
        {
            return false;
        }
        if !self.classes.is_empty() && !self.classes.contains(&event.class) {
            return false;
        }
        if !self.capabilities.is_empty()
            && !event
                .object
                .as_ref()
                .is_some_and(|object| self.capabilities.contains(&object.capability))
        {
            return false;
        }
        if !self.objects.is_empty()
            && !event
                .object
                .as_ref()
                .is_some_and(|object| self.objects.contains(object))
        {
            return false;
        }
        true
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionOptions {
    pub capacity: usize,
    pub after_sequence: Option<u64>,
    pub rate_limits: Vec<ReplaceableEventRateLimit>,
}

impl Default for SubscriptionOptions {
    fn default() -> Self {
        Self {
            capacity: 256,
            after_sequence: None,
            rate_limits: Vec::new(),
        }
    }
}

/// A delivery bucket for high-rate replaceable projections or telemetry.
///
/// `object: None` limits the complete capability/class pair. An object-specific rule takes
/// precedence over a broader rule. Lossless and discrete event classes always bypass limits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplaceableEventRateLimit {
    pub capability: EventCapability,
    pub class: EventClass,
    pub object: Option<EventObject>,
    pub min_interval: Duration,
}

impl ReplaceableEventRateLimit {
    pub(super) fn matches(&self, event: &EventEnvelope) -> bool {
        event.delivery == DeliveryPolicy::Replaceable
            && matches!(event.class, EventClass::Projection | EventClass::Telemetry)
            && event.class == self.class
            && event.object.as_ref().is_some_and(|object| {
                object.capability == self.capability
                    && self
                        .object
                        .as_ref()
                        .is_none_or(|expected| expected == object)
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceGap {
    pub after_sequence: u64,
    pub oldest_available: u64,
    pub latest_sequence: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SubscriptionDelivery {
    Event(Arc<EventEnvelope>),
    Gap(SequenceGap),
}

#[derive(Clone, Debug, PartialEq)]
pub enum EventReplay {
    Events(Vec<Arc<EventEnvelope>>),
    Gap(SequenceGap),
}
