//! One filtered subscriber's bounded queue, coalescing, gaps, and replaceable rate limits.

use std::{collections::VecDeque, sync::Arc};

use tokio::time::Instant;

use super::{EventBusState, replay_from};
use crate::event::model::{
    DeliveryPolicy, EventEnvelope, EventFilter, EventReplay, ReplaceableEventRateLimit,
    SequenceGap, SubscriptionDelivery, SubscriptionOptions,
};

pub(super) struct SubscriberState {
    filter: EventFilter,
    capacity: usize,
    queue: VecDeque<Arc<EventEnvelope>>,
    last_delivered_sequence: u64,
    gap: Option<(SequenceGap, bool)>,
    rate_limits: Vec<ReplaceableEventRateLimit>,
    last_rate_delivery: Vec<Option<Instant>>,
    deferred: Vec<Option<Arc<EventEnvelope>>>,
}

impl SubscriberState {
    pub(super) fn from_replay(
        state: &EventBusState,
        filter: EventFilter,
        options: SubscriptionOptions,
        after_sequence: u64,
    ) -> Self {
        let rate_count = options.rate_limits.len();
        let mut subscriber = Self {
            filter,
            capacity: options.capacity,
            queue: VecDeque::with_capacity(options.capacity),
            last_delivered_sequence: after_sequence,
            gap: None,
            rate_limits: options.rate_limits,
            last_rate_delivery: vec![None; rate_count],
            deferred: vec![None; rate_count],
        };
        subscriber.install_replay(state, after_sequence);
        subscriber
    }

    pub(super) fn install_replay(&mut self, state: &EventBusState, after_sequence: u64) {
        match replay_from(state, after_sequence, &self.filter) {
            EventReplay::Events(events) => events.iter().for_each(|event| {
                self.observe(event, state.oldest_available(event.sequence));
            }),
            EventReplay::Gap(gap) => self.gap = Some((gap, false)),
        }
    }

    pub(super) fn observe(&mut self, event: &Arc<EventEnvelope>, oldest_available: u64) {
        if self.update_gap(event.sequence, oldest_available) || !self.filter.matches(event) {
            return;
        }
        if self.coalesce(event) || self.defer(event, Instant::now()) || self.enqueue(event) {
            return;
        }
        if !self.replace_telemetry(event) {
            self.mark_gap(event.sequence, oldest_available);
        }
    }

    pub(super) fn next_delivery(&mut self, now: Instant) -> Option<SubscriptionDelivery> {
        if let Some(gap) = self.next_gap() {
            return Some(SubscriptionDelivery::Gap(gap));
        }
        self.promote_due(now);
        let event = self.queue.pop_front()?;
        self.record_delivery(&event, now);
        Some(SubscriptionDelivery::Event(event))
    }

    pub(super) fn next_deadline(&self) -> Option<Instant> {
        self.deferred
            .iter()
            .enumerate()
            .filter_map(|(index, event)| event.as_ref().and(self.rate_deadline(index)))
            .min()
    }

    pub(super) fn reset(&mut self, sequence: u64) {
        self.queue.clear();
        self.gap = None;
        self.last_delivered_sequence = sequence;
        self.last_rate_delivery.fill(None);
        self.deferred.fill(None);
    }

    pub(super) fn gap(&self) -> Option<SequenceGap> {
        self.gap.map(|(gap, _)| gap)
    }

    fn update_gap(&mut self, latest_sequence: u64, oldest_available: u64) -> bool {
        let Some((gap, _)) = &mut self.gap else {
            return false;
        };
        gap.latest_sequence = latest_sequence;
        gap.oldest_available = oldest_available;
        true
    }

    fn next_gap(&mut self) -> Option<SequenceGap> {
        let (gap, delivered) = self.gap.as_mut()?;
        if *delivered {
            return None;
        }
        *delivered = true;
        Some(*gap)
    }

    fn coalesce(&mut self, event: &Arc<EventEnvelope>) -> bool {
        if event.delivery != DeliveryPolicy::Replaceable {
            return false;
        }
        let Some(index) = self.queue.iter().rposition(|queued| {
            queued.delivery == DeliveryPolicy::Replaceable
                && queued.class == event.class
                && queued.object == event.object
        }) else {
            return false;
        };
        self.queue.remove(index);
        self.queue.push_back(Arc::clone(event));
        true
    }

    fn defer(&mut self, event: &Arc<EventEnvelope>, now: Instant) -> bool {
        let Some(index) = self.rate_limit_index(event) else {
            return false;
        };
        if self
            .rate_deadline(index)
            .is_none_or(|deadline| deadline <= now)
        {
            return false;
        }
        self.deferred[index] = Some(Arc::clone(event));
        true
    }

    fn rate_limit_index(&self, event: &EventEnvelope) -> Option<usize> {
        self.rate_limits
            .iter()
            .enumerate()
            .filter(|(_, limit)| limit.matches(event))
            .max_by_key(|(_, limit)| limit.object.is_some())
            .map(|(index, _)| index)
    }

    fn rate_deadline(&self, index: usize) -> Option<Instant> {
        self.last_rate_delivery[index].map(|last| last + self.rate_limits[index].min_interval)
    }

    fn promote_due(&mut self, now: Instant) {
        for index in 0..self.deferred.len() {
            if self
                .rate_deadline(index)
                .is_some_and(|deadline| deadline > now)
            {
                continue;
            }
            let Some(event) = self.deferred[index].take() else {
                continue;
            };
            if !(self.coalesce(&event) || self.enqueue(&event) || self.replace_telemetry(&event)) {
                self.deferred[index] = Some(event);
            }
        }
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

    fn record_delivery(&mut self, event: &EventEnvelope, now: Instant) {
        self.last_delivered_sequence = event.sequence;
        if let Some(index) = self.rate_limit_index(event) {
            self.last_rate_delivery[index] = Some(now);
        }
        for deferred in &mut self.deferred {
            if deferred
                .as_ref()
                .is_some_and(|pending| superseded_by(pending, event))
            {
                *deferred = None;
            }
        }
    }

    fn mark_gap(&mut self, latest_sequence: u64, oldest_available: u64) {
        self.queue.clear();
        self.deferred.fill(None);
        self.gap = Some((
            SequenceGap {
                after_sequence: self.last_delivered_sequence,
                oldest_available,
                latest_sequence,
            },
            false,
        ));
    }
}

fn superseded_by(pending: &EventEnvelope, delivered: &EventEnvelope) -> bool {
    pending.sequence <= delivered.sequence
        && pending.class == delivered.class
        && pending.object == delivered.object
}
