use std::{sync::Arc, thread};

use light_core::ShowId;
use light_show::PortableShowObjectKey;
use uuid::Uuid;

use super::*;
use crate::{
    ActionContext, ActionSource, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowObjectsChange, PlaybackCueReference, PlaybackCueTransition, PlaybackRuntimeChange,
    PlaybackRuntimeIdentity, PlaybackRuntimeProjection, PlaybackShowScope,
    PlaybackTargetProjection, PlaybackTransitionCause, SelectiveShowImportChange,
    SelectiveShowObjectChange,
};

fn transition_draft(desk_id: Uuid, playback_number: u16, delivery: DeliveryPolicy) -> EventDraft {
    let transition = PlaybackCueTransition {
        playback_number: Some(playback_number),
        cue_list_id: Uuid::from_u128(20),
        previous: None,
        current: Some(PlaybackCueReference {
            id: Uuid::from_u128(30),
            number: 2.0,
        }),
        cause: PlaybackTransitionCause::Chaser,
        advanced_steps: 1,
    };
    let mut draft = EventDraft::playback_runtime_changed(
        Some(desk_id),
        PlaybackRuntimeChange {
            projection: PlaybackRuntimeProjection {
                scope: PlaybackShowScope {
                    show_id: Uuid::nil(),
                    show_revision: 0,
                },
                requested: PlaybackRuntimeIdentity::Playback(playback_number),
                playback_number: Some(playback_number),
                target: PlaybackTargetProjection::CueList {
                    cue_list_id: light_core::CueListId(transition.cue_list_id),
                    runtime: None,
                },
            },
            transition: Some(transition),
        },
        EventSource::Runtime,
        None,
    );
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
fn active_show_batches_route_by_aggregate_kind_or_exact_object() {
    let bus = EventBus::new(8);
    let show_id = ShowId(Uuid::from_u128(40));
    let group_kind = EventObject::show_object_kind(show_id, ActiveShowObjectKind::Group);
    let exact_group = EventObject::show_object(show_id, ActiveShowObjectKind::Group, "1");
    let subscriptions = [
        EventObject::show_objects(show_id),
        group_kind.clone(),
        exact_group.clone(),
    ]
    .map(|object| {
        bus.subscribe(
            EventFilter::default().with_object(object),
            SubscriptionOptions::default(),
        )
    });
    let unrelated = bus.subscribe(
        EventFilter::default().with_object(EventObject::show_object_kind(
            show_id,
            ActiveShowObjectKind::Preset,
        )),
        SubscriptionOptions::default(),
    );

    let expected = bus.publish(show_objects_draft(
        show_id,
        ActiveShowObjectKind::Group,
        "1",
    ));

    for subscription in subscriptions {
        assert_eq!(next_event(&subscription), expected);
    }
    assert!(unrelated.try_next().is_none());
    assert_eq!(expected.related_objects, vec![group_kind, exact_group]);
}

#[test]
fn selective_import_keeps_aggregate_identity_and_routes_changed_objects() {
    let bus = EventBus::new(4);
    let show_id = ShowId(Uuid::from_u128(50));
    let exact_macro = EventObject::show_storage_object(show_id, "macro", "future");
    let subscription = bus.subscribe(
        EventFilter::default().with_object(exact_macro.clone()),
        SubscriptionOptions::default(),
    );
    let context = ActionContext::system(Uuid::from_u128(1), ActionSource::System);

    let expected = bus.publish(EventDraft::selective_import_applied(
        &context,
        SelectiveShowImportChange {
            show_id,
            show_revision: Default::default(),
            outcomes: Vec::new(),
            objects: vec![SelectiveShowObjectChange {
                key: PortableShowObjectKey::new("macro", "future"),
                object_revision: 2,
                body: serde_json::json!({"source": "future language"}),
            }],
            profiles: Vec::new(),
            managed_assets: Vec::new(),
        },
    ));

    assert_eq!(next_event(&subscription), expected);
    assert_eq!(expected.object, Some(EventObject::show_objects(show_id)));
    assert!(expected.related_objects.contains(&exact_macro));
    assert!(matches!(
        expected.payload,
        ApplicationEvent::Show(ShowEvent::SelectiveImportApplied(_))
    ));
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
            rate_limits: Vec::new(),
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
            rate_limits: Vec::new(),
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
            rate_limits: Vec::new(),
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

#[tokio::test]
async fn async_delivery_wakes_without_polling() {
    let bus = EventBus::new(4);
    let desk = Uuid::from_u128(1);
    let mut subscription =
        bus.subscribe(EventFilter::for_desk(desk), SubscriptionOptions::default());
    let waiting = tokio::spawn(async move { subscription.next().await });
    tokio::task::yield_now().await;

    let expected = bus.publish(transition_draft(desk, 2, DeliveryPolicy::Lossless));
    let delivery = tokio::time::timeout(std::time::Duration::from_secs(1), waiting)
        .await
        .expect("subscriber should wake")
        .expect("subscriber task should finish");

    assert_eq!(delivery, Some(SubscriptionDelivery::Event(expected)));
}

#[tokio::test(start_paused = true)]
async fn replaceable_topic_rate_limit_delivers_the_latest_update() {
    let bus = EventBus::new(8);
    let desk = Uuid::from_u128(1);
    let object = EventObject::new(EventCapability::Playback, "playback:2");
    let mut subscription = bus.subscribe(
        EventFilter::for_desk(desk),
        SubscriptionOptions {
            capacity: 4,
            after_sequence: None,
            rate_limits: vec![ReplaceableEventRateLimit {
                capability: EventCapability::Playback,
                class: EventClass::Projection,
                object: Some(object),
                min_interval: std::time::Duration::from_millis(100),
            }],
        },
    );

    let first = bus.publish(projection_draft(desk, 2));
    assert_eq!(
        subscription.next().await,
        Some(SubscriptionDelivery::Event(first))
    );
    bus.publish(projection_draft(desk, 2));
    let latest = bus.publish(projection_draft(desk, 2));
    assert!(subscription.try_next().is_none());

    tokio::time::advance(std::time::Duration::from_millis(100)).await;
    assert_eq!(
        subscription.next().await,
        Some(SubscriptionDelivery::Event(latest))
    );
}

#[tokio::test(start_paused = true)]
async fn object_rate_limit_matches_a_related_event_route() {
    let bus = EventBus::new(8);
    let desk = Uuid::from_u128(1);
    let cue_list = EventObject::cue_list(Uuid::from_u128(20));
    let mut subscription = bus.subscribe(
        EventFilter::for_desk(desk).with_object(cue_list.clone()),
        SubscriptionOptions {
            capacity: 4,
            after_sequence: None,
            rate_limits: vec![ReplaceableEventRateLimit {
                capability: EventCapability::Playback,
                class: EventClass::Projection,
                object: Some(cue_list),
                min_interval: std::time::Duration::from_millis(100),
            }],
        },
    );

    let first = bus.publish(projection_draft(desk, 2));
    assert_eq!(
        subscription.next().await,
        Some(SubscriptionDelivery::Event(first))
    );
    let latest = bus.publish(projection_draft(desk, 2));
    assert!(subscription.try_next().is_none());

    tokio::time::advance(std::time::Duration::from_millis(100)).await;
    assert_eq!(
        subscription.next().await,
        Some(SubscriptionDelivery::Event(latest))
    );
}

#[tokio::test(start_paused = true)]
async fn unrelated_lossless_delivery_does_not_discard_a_deferred_projection() {
    let bus = EventBus::new(8);
    let desk = Uuid::from_u128(1);
    let object = EventObject::new(EventCapability::Playback, "playback:2");
    let mut subscription = bus.subscribe(
        EventFilter::for_desk(desk),
        SubscriptionOptions {
            capacity: 4,
            after_sequence: None,
            rate_limits: vec![ReplaceableEventRateLimit {
                capability: EventCapability::Playback,
                class: EventClass::Projection,
                object: Some(object),
                min_interval: std::time::Duration::from_millis(100),
            }],
        },
    );

    let first = bus.publish(projection_draft(desk, 2));
    assert_eq!(
        subscription.next().await,
        Some(SubscriptionDelivery::Event(first))
    );
    let deferred = bus.publish(projection_draft(desk, 2));
    let transition = bus.publish(transition_draft(desk, 3, DeliveryPolicy::Lossless));
    assert_eq!(
        subscription.next().await,
        Some(SubscriptionDelivery::Event(transition))
    );

    tokio::time::advance(std::time::Duration::from_millis(100)).await;
    assert_eq!(
        subscription.next().await,
        Some(SubscriptionDelivery::Event(deferred))
    );
}

#[tokio::test(start_paused = true)]
async fn lossless_safety_error_and_transitions_bypass_rate_limits() {
    let bus = EventBus::new(8);
    let desk = Uuid::from_u128(1);
    let mut subscription = bus.subscribe(
        EventFilter::for_desk(desk),
        SubscriptionOptions {
            capacity: 4,
            after_sequence: None,
            rate_limits: [
                EventClass::Transition,
                EventClass::Error,
                EventClass::Safety,
            ]
            .map(|class| ReplaceableEventRateLimit {
                capability: EventCapability::Playback,
                class,
                object: None,
                min_interval: std::time::Duration::from_secs(60),
            })
            .into(),
        },
    );
    let expected = [
        EventClass::Transition,
        EventClass::Error,
        EventClass::Safety,
    ]
    .into_iter()
    .map(|class| bus.publish(lossless_draft(desk, class)))
    .collect::<Vec<_>>();

    for event in expected {
        assert_eq!(
            subscription.next().await,
            Some(SubscriptionDelivery::Event(event))
        );
    }
}

fn projection_draft(desk_id: Uuid, playback_number: u16) -> EventDraft {
    let mut draft = transition_draft(desk_id, playback_number, DeliveryPolicy::Replaceable);
    draft.class = EventClass::Projection;
    draft
}

fn lossless_draft(desk_id: Uuid, class: EventClass) -> EventDraft {
    let mut draft = transition_draft(desk_id, 2, DeliveryPolicy::Lossless);
    draft.class = class;
    draft
}

fn show_objects_draft(show_id: ShowId, kind: ActiveShowObjectKind, object_id: &str) -> EventDraft {
    EventDraft::active_show_objects_changed(
        &ActionContext::system(Uuid::from_u128(1), ActionSource::System),
        ActiveShowObjectsChange {
            show_id,
            show_revision: Default::default(),
            changes: vec![ActiveShowObjectChange {
                kind,
                object_id: object_id.into(),
                object_revision: 1,
                body: Some(serde_json::json!({})),
                deleted: false,
            }],
        },
    )
}
