use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Weak},
};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
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
    pub playback_id: Uuid,
    pub cue_list_id: Uuid,
    pub previous: Option<CueReference>,
    pub current: Option<CueReference>,
    pub cause: PlaybackTransitionCause,
    pub advanced_steps: u64,
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
    pub desk_id: Uuid,
    pub class: EventClass,
    pub object: Option<EventObject>,
    pub source: EventSource,
    pub correlation_id: Option<Uuid>,
    pub delivery: DeliveryPolicy,
    pub payload: ApplicationEvent,
}

impl EventDraft {
    pub fn playback_transition(
        desk_id: Uuid,
        transition: PlaybackCueTransition,
        source: EventSource,
        correlation_id: Option<Uuid>,
    ) -> Self {
        let object = EventObject::new(
            EventCapability::Playback,
            transition.playback_id.to_string(),
        );
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
    pub desk_id: Uuid,
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

    fn matches(&self, event: &EventEnvelope) -> bool {
        if self.desk_id.is_some_and(|desk_id| desk_id != event.desk_id) {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubscriptionOptions {
    pub capacity: usize,
    pub after_sequence: Option<u64>,
}

impl Default for SubscriptionOptions {
    fn default() -> Self {
        Self {
            capacity: 256,
            after_sequence: None,
        }
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

#[derive(Clone)]
pub struct EventBus {
    inner: Arc<Mutex<EventBusState>>,
}

struct EventBusState {
    retention: usize,
    next_sequence: u64,
    retained: VecDeque<Arc<EventEnvelope>>,
    next_subscription_id: u64,
    subscriptions: HashMap<u64, SubscriberState>,
}

struct SubscriberState {
    filter: EventFilter,
    capacity: usize,
    queue: VecDeque<Arc<EventEnvelope>>,
    last_delivered: u64,
    gap: Option<(SequenceGap, bool)>,
}

impl EventBus {
    pub fn new(retention: usize) -> Self {
        assert!(retention > 0, "event retention must be greater than zero");
        Self {
            inner: Arc::new(Mutex::new(EventBusState {
                retention,
                next_sequence: 1,
                retained: VecDeque::with_capacity(retention),
                next_subscription_id: 1,
                subscriptions: HashMap::new(),
            })),
        }
    }

    pub fn latest_sequence(&self) -> u64 {
        self.inner.lock().next_sequence - 1
    }

    pub fn publish(&self, draft: EventDraft) -> Arc<EventEnvelope> {
        self.publish_at(draft, Utc::now())
    }

    pub fn publish_at(&self, draft: EventDraft, occurred_at: DateTime<Utc>) -> Arc<EventEnvelope> {
        let mut state = self.inner.lock();
        let event = Arc::new(EventEnvelope {
            sequence: state.next_sequence,
            occurred_at,
            desk_id: draft.desk_id,
            class: draft.class,
            object: draft.object,
            source: draft.source,
            correlation_id: draft.correlation_id,
            delivery: draft.delivery,
            payload: draft.payload,
        });
        state.next_sequence += 1;
        state.retained.push_back(Arc::clone(&event));
        while state.retained.len() > state.retention {
            state.retained.pop_front();
        }
        let oldest_available = state
            .retained
            .front()
            .map_or(event.sequence, |item| item.sequence);
        for subscriber in state.subscriptions.values_mut() {
            subscriber.observe(&event, oldest_available);
        }
        event
    }

    pub fn replay(&self, after_sequence: u64, filter: &EventFilter) -> EventReplay {
        let state = self.inner.lock();
        replay_from(&state, after_sequence, filter)
    }

    pub fn subscribe(
        &self,
        filter: EventFilter,
        options: SubscriptionOptions,
    ) -> EventSubscription {
        assert!(
            options.capacity > 0,
            "subscription capacity must be greater than zero"
        );
        let mut state = self.inner.lock();
        let id = state.next_subscription_id;
        state.next_subscription_id += 1;
        let after_sequence = options.after_sequence.unwrap_or(state.next_sequence - 1);
        let mut subscriber = SubscriberState {
            filter,
            capacity: options.capacity,
            queue: VecDeque::with_capacity(options.capacity),
            last_delivered: after_sequence,
            gap: None,
        };
        match replay_from(&state, after_sequence, &subscriber.filter) {
            EventReplay::Events(events) => {
                for event in events {
                    let oldest_available = state
                        .retained
                        .front()
                        .map_or(event.sequence, |item| item.sequence);
                    subscriber.observe(&event, oldest_available);
                }
            }
            EventReplay::Gap(gap) => subscriber.gap = Some((gap, false)),
        }
        state.subscriptions.insert(id, subscriber);
        EventSubscription {
            id,
            inner: Arc::downgrade(&self.inner),
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(2_048)
    }
}

impl SubscriberState {
    fn observe(&mut self, event: &Arc<EventEnvelope>, oldest_available: u64) {
        if let Some((gap, _)) = &mut self.gap {
            gap.latest_sequence = event.sequence;
            gap.oldest_available = oldest_available;
            return;
        }
        if !self.filter.matches(event) {
            return;
        }
        if event.delivery == DeliveryPolicy::Replaceable
            && let Some(existing) = self.queue.iter_mut().rev().find(|queued| {
                queued.delivery == DeliveryPolicy::Replaceable
                    && queued.class == event.class
                    && queued.object == event.object
            })
        {
            *existing = Arc::clone(event);
            return;
        }
        if self.queue.len() < self.capacity {
            self.queue.push_back(Arc::clone(event));
            return;
        }
        if event.delivery == DeliveryPolicy::Replaceable
            && let Some(index) = self
                .queue
                .iter()
                .position(|queued| queued.delivery == DeliveryPolicy::Replaceable)
        {
            self.queue.remove(index);
            self.queue.push_back(Arc::clone(event));
            return;
        }
        self.queue.clear();
        self.gap = Some((
            SequenceGap {
                after_sequence: self.last_delivered,
                oldest_available,
                latest_sequence: event.sequence,
            },
            false,
        ));
    }
}

pub struct EventSubscription {
    id: u64,
    inner: Weak<Mutex<EventBusState>>,
}

impl EventSubscription {
    pub fn try_next(&self) -> Option<SubscriptionDelivery> {
        let inner = self.inner.upgrade()?;
        let mut state = inner.lock();
        let subscriber = state.subscriptions.get_mut(&self.id)?;
        if let Some((gap, delivered)) = &mut subscriber.gap {
            if !*delivered {
                *delivered = true;
                return Some(SubscriptionDelivery::Gap(*gap));
            }
            return None;
        }
        let event = subscriber.queue.pop_front()?;
        subscriber.last_delivered = event.sequence;
        Some(SubscriptionDelivery::Event(event))
    }

    /// Installs an authoritative snapshot cursor and resumes incremental delivery after it.
    /// Returns a new gap if the retained event horizon no longer reaches the supplied cursor.
    pub fn repair_from_snapshot(&self, snapshot_sequence: u64) -> Result<(), SequenceGap> {
        let Some(inner) = self.inner.upgrade() else {
            return Ok(());
        };
        let mut state = inner.lock();
        let Some(mut subscriber) = state.subscriptions.remove(&self.id) else {
            return Ok(());
        };
        subscriber.queue.clear();
        subscriber.gap = None;
        subscriber.last_delivered = snapshot_sequence;
        match replay_from(&state, snapshot_sequence, &subscriber.filter) {
            EventReplay::Events(events) => {
                for event in events {
                    let oldest_available = state
                        .retained
                        .front()
                        .map_or(event.sequence, |item| item.sequence);
                    subscriber.observe(&event, oldest_available);
                }
                let gap = subscriber.gap.map(|(gap, _)| gap);
                state.subscriptions.insert(self.id, subscriber);
                gap.map_or(Ok(()), Err)
            }
            EventReplay::Gap(gap) => {
                subscriber.gap = Some((gap, false));
                state.subscriptions.insert(self.id, subscriber);
                Err(gap)
            }
        }
    }
}

impl Drop for EventSubscription {
    fn drop(&mut self) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };
        inner.lock().subscriptions.remove(&self.id);
    }
}

fn replay_from(state: &EventBusState, after_sequence: u64, filter: &EventFilter) -> EventReplay {
    let latest_sequence = state.next_sequence - 1;
    let oldest_available = state
        .retained
        .front()
        .map_or(state.next_sequence, |event| event.sequence);
    if after_sequence < oldest_available.saturating_sub(1) {
        return EventReplay::Gap(SequenceGap {
            after_sequence,
            oldest_available,
            latest_sequence,
        });
    }
    EventReplay::Events(
        state
            .retained
            .iter()
            .filter(|event| event.sequence > after_sequence && filter.matches(event))
            .cloned()
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    fn transition_draft(desk_id: Uuid, playback_id: Uuid, delivery: DeliveryPolicy) -> EventDraft {
        let transition = PlaybackCueTransition {
            playback_id,
            cue_list_id: Uuid::from_u128(20),
            previous: None,
            current: Some(CueReference {
                id: Uuid::from_u128(30),
                number: 2.0,
            }),
            cause: PlaybackTransitionCause::Chaser,
            advanced_steps: 1,
        };
        let mut draft =
            EventDraft::playback_transition(desk_id, transition, EventSource::Runtime, None);
        draft.delivery = delivery;
        draft
    }

    fn next_event(subscription: &EventSubscription) -> Arc<EventEnvelope> {
        let Some(SubscriptionDelivery::Event(event)) = subscription.try_next() else {
            panic!("expected an event");
        };
        event
    }

    #[test]
    fn filters_by_desk_capability_class_and_object() {
        let bus = EventBus::new(16);
        let desk = Uuid::from_u128(1);
        let playback = Uuid::from_u128(2);
        let object = EventObject::new(EventCapability::Playback, playback.to_string());
        let subscription = bus.subscribe(
            EventFilter::for_desk(desk)
                .with_capability(EventCapability::Playback)
                .with_class(EventClass::Transition)
                .with_object(object),
            SubscriptionOptions::default(),
        );

        bus.publish(transition_draft(
            Uuid::from_u128(9),
            playback,
            DeliveryPolicy::Lossless,
        ));
        bus.publish(transition_draft(
            desk,
            Uuid::from_u128(8),
            DeliveryPolicy::Lossless,
        ));
        let expected = bus.publish(transition_draft(desk, playback, DeliveryPolicy::Lossless));

        assert_eq!(next_event(&subscription), expected);
        assert!(subscription.try_next().is_none());
    }

    #[test]
    fn concurrent_publication_is_strictly_monotonic() {
        let bus = EventBus::new(512);
        let desk = Uuid::from_u128(1);
        let mut threads = Vec::new();
        for worker in 0..4_u128 {
            let bus = bus.clone();
            threads.push(thread::spawn(move || {
                for index in 0..100_u128 {
                    bus.publish(transition_draft(
                        desk,
                        Uuid::from_u128(1000 + worker * 100 + index),
                        DeliveryPolicy::Lossless,
                    ));
                }
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }

        let EventReplay::Events(events) = bus.replay(0, &EventFilter::default()) else {
            panic!("complete retained history should replay");
        };
        assert_eq!(events.len(), 400);
        assert!(
            events
                .windows(2)
                .all(|pair| pair[1].sequence == pair[0].sequence + 1)
        );
    }

    #[test]
    fn stale_cursor_reports_gap_and_snapshot_repair_resumes() {
        let bus = EventBus::new(3);
        let desk = Uuid::from_u128(1);
        for index in 0..5_u128 {
            bus.publish(transition_draft(
                desk,
                Uuid::from_u128(index + 10),
                DeliveryPolicy::Lossless,
            ));
        }
        let subscription = bus.subscribe(
            EventFilter::for_desk(desk),
            SubscriptionOptions {
                capacity: 3,
                after_sequence: Some(0),
            },
        );
        assert_eq!(
            subscription.try_next(),
            Some(SubscriptionDelivery::Gap(SequenceGap {
                after_sequence: 0,
                oldest_available: 3,
                latest_sequence: 5,
            }))
        );
        assert!(subscription.try_next().is_none());

        subscription.repair_from_snapshot(5).unwrap();
        let expected = bus.publish(transition_draft(
            desk,
            Uuid::from_u128(99),
            DeliveryPolicy::Lossless,
        ));
        assert_eq!(next_event(&subscription), expected);
    }

    #[test]
    fn replaceable_updates_coalesce_to_the_newest_value() {
        let bus = EventBus::new(8);
        let desk = Uuid::from_u128(1);
        let playback = Uuid::from_u128(2);
        let subscription = bus.subscribe(
            EventFilter::for_desk(desk),
            SubscriptionOptions {
                capacity: 1,
                after_sequence: None,
            },
        );
        bus.publish(transition_draft(
            desk,
            playback,
            DeliveryPolicy::Replaceable,
        ));
        let latest = bus.publish(transition_draft(
            desk,
            playback,
            DeliveryPolicy::Replaceable,
        ));

        assert_eq!(next_event(&subscription), latest);
        assert!(subscription.try_next().is_none());
    }

    #[test]
    fn lossless_queue_overflow_becomes_an_explicit_gap() {
        let bus = EventBus::new(8);
        let desk = Uuid::from_u128(1);
        let subscription = bus.subscribe(
            EventFilter::for_desk(desk),
            SubscriptionOptions {
                capacity: 1,
                after_sequence: None,
            },
        );
        bus.publish(transition_draft(
            desk,
            Uuid::from_u128(2),
            DeliveryPolicy::Lossless,
        ));
        bus.publish(transition_draft(
            desk,
            Uuid::from_u128(3),
            DeliveryPolicy::Lossless,
        ));

        assert_eq!(
            subscription.try_next(),
            Some(SubscriptionDelivery::Gap(SequenceGap {
                after_sequence: 0,
                oldest_available: 1,
                latest_sequence: 2,
            }))
        );
    }
}
