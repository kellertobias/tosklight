use std::{collections::HashSet, sync::Arc, time::Duration};

use uuid::Uuid;

use super::model::{
    DeliveryPolicy, EventCapability, EventClass, EventEnvelope, EventObject, EventTopic,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EventFilter {
    pub desk_id: Option<Uuid>,
    pub capabilities: HashSet<EventCapability>,
    pub classes: HashSet<EventClass>,
    pub objects: HashSet<EventObject>,
    /// Opt-in topics this subscription receives in addition to the default stream.
    pub topics: HashSet<EventTopic>,
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

    pub fn with_topic(mut self, topic: EventTopic) -> Self {
        self.topics.insert(topic);
        self
    }

    pub(super) fn matches(&self, event: &EventEnvelope) -> bool {
        if event
            .payload
            .opt_in_topic()
            .is_some_and(|topic| !self.topics.contains(&topic))
        {
            return false;
        }
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
        let route_matches = |object: &EventObject| {
            (self.capabilities.is_empty() || self.capabilities.contains(&object.capability))
                && (self.objects.is_empty() || self.objects.contains(object))
        };
        if (!self.capabilities.is_empty() || !self.objects.is_empty())
            && !event
                .object
                .iter()
                .chain(&event.related_objects)
                .any(route_matches)
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
            && event
                .object
                .iter()
                .chain(&event.related_objects)
                .any(|object| {
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
