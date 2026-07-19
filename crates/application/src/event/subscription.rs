use std::{collections::HashSet, sync::Arc, time::Duration};

use uuid::Uuid;

use super::model::{DeliveryPolicy, EventCapability, EventClass, EventEnvelope, EventObject};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EventFilter {
    pub desk_id: Option<Uuid>,
    /// Authenticated owner allowed to observe user-scoped Programmer objects. This does not
    /// constrain desk-local Programmer interaction routes or future non-user Programmer topics.
    pub programmer_user_id: Option<Uuid>,
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
        if self.programmer_user_id.is_some_and(|allowed| {
            event
                .object
                .iter()
                .chain(&event.related_objects)
                .filter_map(EventObject::programming_user_id)
                .any(|actual| actual != allowed)
        }) {
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
