use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Weak},
};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;

use super::model::{
    DeliveryPolicy, EventDraft, EventEnvelope, EventFilter, EventReplay, SequenceGap,
    SubscriptionDelivery, SubscriptionOptions,
};

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
        let event = state.envelope(draft, occurred_at);
        state.retain(Arc::clone(&event));
        let oldest_available = state.oldest_available(event.sequence);
        for subscriber in state.subscriptions.values_mut() {
            subscriber.observe(&event, oldest_available);
        }
        event
    }

    pub fn replay(&self, after_sequence: u64, filter: &EventFilter) -> EventReplay {
        replay_from(&self.inner.lock(), after_sequence, filter)
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
        let subscriber =
            SubscriberState::from_replay(&state, filter, options.capacity, after_sequence);
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

impl EventBusState {
    fn envelope(&mut self, draft: EventDraft, occurred_at: DateTime<Utc>) -> Arc<EventEnvelope> {
        let envelope = Arc::new(EventEnvelope {
            sequence: self.next_sequence,
            occurred_at,
            desk_id: draft.desk_id,
            class: draft.class,
            object: draft.object,
            source: draft.source,
            correlation_id: draft.correlation_id,
            delivery: draft.delivery,
            payload: draft.payload,
        });
        self.next_sequence += 1;
        envelope
    }

    fn retain(&mut self, event: Arc<EventEnvelope>) {
        self.retained.push_back(event);
        while self.retained.len() > self.retention {
            self.retained.pop_front();
        }
    }

    fn oldest_available(&self, fallback: u64) -> u64 {
        self.retained
            .front()
            .map_or(fallback, |event| event.sequence)
    }
}

impl SubscriberState {
    fn from_replay(
        state: &EventBusState,
        filter: EventFilter,
        capacity: usize,
        after_sequence: u64,
    ) -> Self {
        let mut subscriber = Self {
            filter,
            capacity,
            queue: VecDeque::with_capacity(capacity),
            last_delivered: after_sequence,
            gap: None,
        };
        subscriber.install_replay(state, after_sequence);
        subscriber
    }

    fn install_replay(&mut self, state: &EventBusState, after_sequence: u64) {
        match replay_from(state, after_sequence, &self.filter) {
            EventReplay::Events(events) => {
                for event in events {
                    self.observe(&event, state.oldest_available(event.sequence));
                }
            }
            EventReplay::Gap(gap) => self.gap = Some((gap, false)),
        }
    }

    fn observe(&mut self, event: &Arc<EventEnvelope>, oldest_available: u64) {
        if self.update_gap(event.sequence, oldest_available) || !self.filter.matches(event) {
            return;
        }
        if self.coalesce(event) || self.enqueue(event) || self.replace_telemetry(event) {
            return;
        }
        self.mark_gap(event.sequence, oldest_available);
    }

    fn update_gap(&mut self, latest_sequence: u64, oldest_available: u64) -> bool {
        let Some((gap, _)) = &mut self.gap else {
            return false;
        };
        gap.latest_sequence = latest_sequence;
        gap.oldest_available = oldest_available;
        true
    }

    fn coalesce(&mut self, event: &Arc<EventEnvelope>) -> bool {
        if event.delivery != DeliveryPolicy::Replaceable {
            return false;
        }
        let Some(existing) = self.queue.iter_mut().rev().find(|queued| {
            queued.delivery == DeliveryPolicy::Replaceable
                && queued.class == event.class
                && queued.object == event.object
        }) else {
            return false;
        };
        *existing = Arc::clone(event);
        true
    }

    fn enqueue(&mut self, event: &Arc<EventEnvelope>) -> bool {
        if self.queue.len() >= self.capacity {
            return false;
        }
        self.queue.push_back(Arc::clone(event));
        true
    }

    fn replace_telemetry(&mut self, event: &Arc<EventEnvelope>) -> bool {
        if event.delivery != DeliveryPolicy::Replaceable {
            return false;
        }
        let Some(index) = self
            .queue
            .iter()
            .position(|queued| queued.delivery == DeliveryPolicy::Replaceable)
        else {
            return false;
        };
        self.queue.remove(index);
        self.queue.push_back(Arc::clone(event));
        true
    }

    fn mark_gap(&mut self, latest_sequence: u64, oldest_available: u64) {
        self.queue.clear();
        self.gap = Some((
            SequenceGap {
                after_sequence: self.last_delivered,
                oldest_available,
                latest_sequence,
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
        if let Some(delivery) = next_gap(subscriber) {
            return delivery;
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
        subscriber.install_replay(&state, snapshot_sequence);
        let gap = subscriber.gap.map(|(gap, _)| gap);
        state.subscriptions.insert(self.id, subscriber);
        gap.map_or(Ok(()), Err)
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

fn next_gap(subscriber: &mut SubscriberState) -> Option<Option<SubscriptionDelivery>> {
    let (gap, delivered) = subscriber.gap.as_mut()?;
    if *delivered {
        return Some(None);
    }
    *delivered = true;
    Some(Some(SubscriptionDelivery::Gap(*gap)))
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
