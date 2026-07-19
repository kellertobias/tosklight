use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Weak},
};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use tokio::sync::watch;
use tokio::time::Instant;

mod subscriber;
use subscriber::SubscriberState;

use super::{
    model::{EventDraft, EventEnvelope},
    subscription::{
        EventFilter, EventReplay, SequenceGap, SubscriptionDelivery, SubscriptionOptions,
    },
};

#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

struct EventBusInner {
    state: Mutex<EventBusState>,
    changed: watch::Sender<u64>,
}

struct EventBusState {
    retention: usize,
    next_sequence: u64,
    retained: VecDeque<Arc<EventEnvelope>>,
    next_subscription_id: u64,
    subscriptions: HashMap<u64, SubscriberState>,
}

impl EventBus {
    pub fn new(retention: usize) -> Self {
        assert!(retention > 0, "event retention must be greater than zero");
        let (changed, _) = watch::channel(0);
        Self {
            inner: Arc::new(EventBusInner {
                state: Mutex::new(EventBusState {
                    retention,
                    next_sequence: 1,
                    retained: VecDeque::with_capacity(retention),
                    next_subscription_id: 1,
                    subscriptions: HashMap::new(),
                }),
                changed,
            }),
        }
    }

    pub fn latest_sequence(&self) -> u64 {
        self.inner.state.lock().next_sequence - 1
    }

    pub fn publish(&self, draft: EventDraft) -> Arc<EventEnvelope> {
        self.publish_at(draft, Utc::now())
    }

    pub fn publish_at(&self, draft: EventDraft, occurred_at: DateTime<Utc>) -> Arc<EventEnvelope> {
        let event = self.store(draft, occurred_at);
        self.inner.changed.send_replace(event.sequence);
        event
    }

    pub fn replay(&self, after_sequence: u64, filter: &EventFilter) -> EventReplay {
        replay_from(&self.inner.state.lock(), after_sequence, filter)
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
        let mut state = self.inner.state.lock();
        let id = state.next_subscription_id;
        state.next_subscription_id += 1;
        let after_sequence = options.after_sequence.unwrap_or(state.next_sequence - 1);
        let subscriber = SubscriberState::from_replay(&state, filter, options, after_sequence);
        state.subscriptions.insert(id, subscriber);
        EventSubscription {
            id,
            inner: Arc::downgrade(&self.inner),
            changed: self.inner.changed.subscribe(),
        }
    }

    fn store(&self, draft: EventDraft, occurred_at: DateTime<Utc>) -> Arc<EventEnvelope> {
        let mut state = self.inner.state.lock();
        let event = state.envelope(draft, occurred_at);
        state.retain(Arc::clone(&event));
        let oldest_available = state.oldest_available(event.sequence);
        for subscriber in state.subscriptions.values_mut() {
            subscriber.observe(&event, oldest_available);
        }
        event
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
            related_objects: draft.related_objects,
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

pub struct EventSubscription {
    id: u64,
    inner: Weak<EventBusInner>,
    changed: watch::Receiver<u64>,
}

impl EventSubscription {
    pub fn try_next(&self) -> Option<SubscriptionDelivery> {
        let inner = self.inner.upgrade()?;
        let mut state = inner.state.lock();
        let subscriber = state.subscriptions.get_mut(&self.id)?;
        subscriber.next_delivery(Instant::now())
    }

    /// Waits without polling until a matching event or sequence gap is available.
    pub async fn next(&mut self) -> Option<SubscriptionDelivery> {
        loop {
            if let Some(delivery) = self.try_next() {
                return Some(delivery);
            }
            self.wait_for_change().await?;
        }
    }

    async fn wait_for_change(&mut self) -> Option<()> {
        let Some(deadline) = self.next_deadline() else {
            return self.changed.changed().await.ok();
        };
        tokio::select! {
            changed = self.changed.changed() => changed.ok(),
            () = tokio::time::sleep_until(deadline) => Some(()),
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        let inner = self.inner.upgrade()?;
        inner
            .state
            .lock()
            .subscriptions
            .get(&self.id)
            .and_then(SubscriberState::next_deadline)
    }

    /// Installs an authoritative snapshot cursor and resumes incremental delivery after it.
    /// Returns a new gap if the retained event horizon no longer reaches the supplied cursor.
    pub fn repair_from_snapshot(&self, snapshot_sequence: u64) -> Result<(), SequenceGap> {
        let Some(inner) = self.inner.upgrade() else {
            return Ok(());
        };
        let mut state = inner.state.lock();
        let Some(mut subscriber) = state.subscriptions.remove(&self.id) else {
            return Ok(());
        };
        subscriber.reset(snapshot_sequence);
        subscriber.install_replay(&state, snapshot_sequence);
        let gap = subscriber.gap();
        state.subscriptions.insert(self.id, subscriber);
        gap.map_or(Ok(()), Err)
    }
}

impl Drop for EventSubscription {
    fn drop(&mut self) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };
        inner.state.lock().subscriptions.remove(&self.id);
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
