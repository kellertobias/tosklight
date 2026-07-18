use std::{sync::Arc, thread};

use uuid::Uuid;

use super::*;

fn transition_draft(desk_id: Uuid, playback_number: u16, delivery: DeliveryPolicy) -> EventDraft {
    let transition = PlaybackCueTransition {
        playback_number: Some(playback_number),
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
        EventDraft::playback_transition(Some(desk_id), transition, EventSource::Runtime, None);
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
    let object = EventObject::new(EventCapability::Playback, "playback:2");
    let subscription = bus.subscribe(
        EventFilter::for_desk(desk)
            .with_capability(EventCapability::Playback)
            .with_class(EventClass::Transition)
            .with_object(object),
        SubscriptionOptions::default(),
    );

    bus.publish(transition_draft(
        Uuid::from_u128(9),
        2,
        DeliveryPolicy::Lossless,
    ));
    bus.publish(transition_draft(desk, 8, DeliveryPolicy::Lossless));
    let expected = bus.publish(transition_draft(desk, 2, DeliveryPolicy::Lossless));

    assert_eq!(next_event(&subscription), expected);
    assert!(subscription.try_next().is_none());
}

#[test]
fn installation_global_events_reach_desk_filters() {
    let bus = EventBus::new(4);
    let subscription = bus.subscribe(
        EventFilter::for_desk(Uuid::from_u128(1)),
        SubscriptionOptions::default(),
    );
    let mut draft = transition_draft(Uuid::from_u128(9), 2, DeliveryPolicy::Lossless);
    draft.desk_id = None;
    let expected = bus.publish(draft);

    assert_eq!(next_event(&subscription), expected);
}

#[test]
fn concurrent_publication_is_strictly_monotonic() {
    let bus = EventBus::new(512);
    let desk = Uuid::from_u128(1);
    let mut threads = Vec::new();
    for worker in 0..4_u16 {
        let bus = bus.clone();
        threads.push(thread::spawn(move || {
            for index in 0..100_u16 {
                bus.publish(transition_draft(
                    desk,
                    1_000 + worker * 100 + index,
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
    for index in 0..5_u16 {
        bus.publish(transition_draft(desk, index + 10, DeliveryPolicy::Lossless));
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
    let expected = bus.publish(transition_draft(desk, 99, DeliveryPolicy::Lossless));
    assert_eq!(next_event(&subscription), expected);
}

#[test]
fn replaceable_updates_coalesce_to_the_newest_value() {
    let bus = EventBus::new(8);
    let desk = Uuid::from_u128(1);
    let subscription = bus.subscribe(
        EventFilter::for_desk(desk),
        SubscriptionOptions {
            capacity: 1,
            after_sequence: None,
        },
    );
    bus.publish(transition_draft(desk, 2, DeliveryPolicy::Replaceable));
    let latest = bus.publish(transition_draft(desk, 2, DeliveryPolicy::Replaceable));

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
    bus.publish(transition_draft(desk, 2, DeliveryPolicy::Lossless));
    bus.publish(transition_draft(desk, 3, DeliveryPolicy::Lossless));

    assert_eq!(
        subscription.try_next(),
        Some(SubscriptionDelivery::Gap(SequenceGap {
            after_sequence: 0,
            oldest_available: 1,
            latest_sequence: 2,
        }))
    );
}
