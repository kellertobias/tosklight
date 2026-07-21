//! Focused v2 event transport contract tests.

use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
use light_application::{
    ActionContext, ActionSource, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowObjectsChange, EventBus, EventDraft, EventSource, PlaybackCueReference as AppCue,
    PlaybackCueTransition as AppTransition, PlaybackRuntimeChange, PlaybackRuntimeIdentity,
    PlaybackRuntimeProjection, PlaybackShowScope, PlaybackTargetProjection,
    PlaybackTransitionCause, ProgrammingCaptureModeChange, ProgrammingCaptureModeProjection,
    ProgrammingLifecycleChange, ProgrammingLifecycleProgrammer, ProgrammingLifecycleSession,
    ProgrammingPreloadPlaybackAction, ProgrammingPreloadPlaybackQueueChange,
    ProgrammingPreloadPlaybackQueueItem, ProgrammingPreloadPlaybackQueueProjection,
    ProgrammingPreloadPlaybackSurface, ProgrammingPreloadValuesChange,
    ProgrammingPreloadValuesProjection, ProgrammingPriorityChange, ProgrammingPriorityProjection,
    ProgrammingValuesChange, ProgrammingValuesProjection, publish_automatic_playback_events,
};
use light_core::{CueListId, ManualClock, ShowId, UserId};
use light_engine::EnginePlaybackCommand;
use light_playback::{Cue, CueList, CueListMode, IntensityPriorityMode, RestartMode, WrapMode};
use light_wire::v2::events as wire;

use super::super::playback_service;
use super::super::{Engine, EngineSnapshot, ProgrammerRegistry, RenderOptions};
use super::*;

#[tokio::test]
async fn running_chaser_wakes_only_its_narrow_subscriber() {
    let started = Utc::now();
    let clock = Arc::new(ManualClock::new(started));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let cue_list = chaser();
    let cue_list_id = cue_list.id;
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list],
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine
        .execute_playback(EnginePlaybackCommand::CueList {
            id: cue_list_id,
            action: light_engine::CueListPlaybackAction::GoAt(started),
        })
        .unwrap();
    let bus = EventBus::new(8);
    let object = wire::EventObject {
        capability: wire::EventCapability::Playback,
        id: format!("cuelist:{}", cue_list_id.0),
    };
    let session = event_session(Uuid::from_u128(1), Uuid::from_u128(11));
    let mut stream =
        EventStream::subscribe(&bus, &session, subscription(Some(object.clone()), Some(0)))
            .unwrap();
    let waiting = tokio::spawn(async move { stream.next().await });
    tokio::task::yield_now().await;

    let mut other_desk = transition_draft(None, cue_list_id.0);
    other_desk.desk_id = Some(Uuid::from_u128(2));
    bus.publish(other_desk);
    bus.publish(transition_draft(Some(99), Uuid::from_u128(99)));
    clock.set(started + ChronoDuration::milliseconds(100));
    let rendered = engine.render(RenderOptions::default()).unwrap();
    let changes = playback_service::automatic_projection_changes(
        &engine,
        test_playback_scope(),
        rendered.automatic_playback_transitions,
    );
    publish_automatic_playback_events(&bus, changes);

    let message = tokio::time::timeout(std::time::Duration::from_secs(1), waiting)
        .await
        .expect("automatic event should wake the subscriber")
        .expect("subscriber task should finish")
        .expect("event bus should remain available");
    let wire::EventServerMessage::Event { event } = message else {
        panic!("expected an event delivery");
    };
    assert_eq!(event.sequence, 3);
    assert_eq!(event.object, Some(object));
    let wire::EventPayload::PlaybackRuntimeChanged { change } = event.payload else {
        panic!("expected a Playback runtime change");
    };
    assert_eq!(
        change.transition.map(|transition| transition.cause),
        Some(light_wire::v2::playback::PlaybackTransitionCause::Chaser)
    );
}

#[tokio::test]
async fn reconnect_gap_repairs_from_an_authoritative_snapshot_cursor() {
    let bus = EventBus::new(2);
    let desk_id = Uuid::from_u128(1);
    let session = event_session(desk_id, Uuid::from_u128(11));
    for sequence in 1..=3_u16 {
        bus.publish(transition_draft(Some(sequence), Uuid::from_u128(20)));
    }
    let mut stream = EventStream::subscribe(&bus, &session, subscription(None, Some(0))).unwrap();

    let Some(wire::EventServerMessage::Gap { gap }) = stream.next().await else {
        panic!("stale reconnect cursor should report a gap");
    };
    assert_eq!(gap.oldest_available, 2);
    assert_eq!(gap.latest_sequence, 3);
    let cursor = wire::EventSnapshotCursor {
        sequence: bus.latest_sequence(),
    };
    assert_eq!(
        stream.repair(cursor),
        wire::EventServerMessage::Repaired { cursor }
    );

    let expected = bus.publish(transition_draft(Some(4), Uuid::from_u128(20)));
    let Some(wire::EventServerMessage::Event { event }) = stream.next().await else {
        panic!("delivery should resume after repair");
    };
    assert_eq!(event.sequence, expected.sequence);
}

#[tokio::test]
async fn exact_show_object_subscription_keeps_the_aggregate_event_identity() {
    let bus = EventBus::new(8);
    let desk_id = Uuid::from_u128(1);
    let show_id = ShowId(Uuid::from_u128(2));
    let group_route = wire::EventObject {
        capability: wire::EventCapability::Show,
        id: format!("objects:{}:kind:group:object:1", show_id.0),
    };
    let session = event_session(desk_id, Uuid::from_u128(11));
    let mut stream = EventStream::subscribe(
        &bus,
        &session,
        show_subscription(group_route.clone(), Some(0)),
    )
    .unwrap();

    bus.publish(show_objects_draft(
        show_id,
        ActiveShowObjectKind::Preset,
        "2.1",
    ));
    bus.publish(show_objects_draft(
        show_id,
        ActiveShowObjectKind::Group,
        "2",
    ));
    assert!(stream.subscription.try_next().is_none());
    let expected = bus.publish(show_objects_draft(
        show_id,
        ActiveShowObjectKind::Group,
        "1",
    ));

    let Some(wire::EventServerMessage::Event { event }) = stream.next().await else {
        panic!("the relevant Group batch should be delivered");
    };
    assert_eq!(event.sequence, expected.sequence);
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Show,
            id: format!("objects:{}", show_id.0),
        })
    );
    assert!(
        event
            .related_objects
            .as_ref()
            .is_some_and(|objects| objects.contains(&group_route))
    );
}

#[test]
fn websocket_protocol_token_is_parsed_without_becoming_the_selected_protocol() {
    let mut protocols = HeaderMap::new();
    protocols.insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        "light.events.v2, light.token.session-token"
            .parse()
            .unwrap(),
    );
    assert_eq!(websocket_token(&protocols), Some("session-token"));
    assert_eq!(websocket_token(&HeaderMap::new()), None);
}

#[test]
fn wire_rate_limits_map_only_replaceable_topics() {
    let object = wire::EventObject {
        capability: wire::EventCapability::Playback,
        id: "playback:2".into(),
    };
    let options = subscription_options(
        Some(8),
        Some(3),
        vec![wire::EventRateLimit {
            capability: wire::EventCapability::Playback,
            class: wire::EventClass::Projection,
            object: Some(object.clone()),
            min_interval_millis: 50,
        }],
    )
    .unwrap();
    assert_eq!(options.capacity, 8);
    assert_eq!(options.after_sequence, Some(3));
    assert_eq!(options.rate_limits[0].min_interval.as_millis(), 50);

    let invalid = wire::EventRateLimit {
        capability: wire::EventCapability::Playback,
        class: wire::EventClass::Transition,
        object: Some(object),
        min_interval_millis: 50,
    };
    assert!(subscription_options(None, None, vec![invalid]).is_err());
}

#[test]
fn programmer_values_objects_are_limited_to_the_authenticated_user() {
    let bus = EventBus::new(8);
    let session = event_session(Uuid::from_u128(1), Uuid::from_u128(11));
    let foreign = wire::EventObject {
        capability: wire::EventCapability::Programmer,
        id: format!("programming-values:{}", Uuid::from_u128(12)),
    };
    let filter_request = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter {
            objects: vec![foreign.clone()],
            ..Default::default()
        },
        after_sequence: None,
        capacity: None,
        rate_limits: Vec::new(),
    });
    assert!(EventStream::subscribe(&bus, &session, filter_request).is_err());

    let rate_request = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter::default(),
        after_sequence: None,
        capacity: None,
        rate_limits: vec![wire::EventRateLimit {
            capability: wire::EventCapability::Programmer,
            class: wire::EventClass::Projection,
            object: Some(foreign),
            min_interval_millis: 16,
        }],
    });
    assert!(EventStream::subscribe(&bus, &session, rate_request).is_err());

    let malformed_request = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter {
            objects: vec![wire::EventObject {
                capability: wire::EventCapability::Programmer,
                id: "programming-values:not-a-uuid".into(),
            }],
            ..Default::default()
        },
        after_sequence: None,
        capacity: None,
        rate_limits: Vec::new(),
    });
    assert!(EventStream::subscribe(&bus, &session, malformed_request).is_err());
}

#[test]
fn programmer_priority_objects_are_limited_to_the_exact_authenticated_user() {
    let bus = EventBus::new(8);
    let user_id = Uuid::from_u128(11);
    let session = event_session(Uuid::from_u128(1), user_id);
    let object = |user_id| wire::EventObject {
        capability: wire::EventCapability::Programmer,
        id: format!("programming-priority:{user_id}"),
    };
    assert!(
        EventStream::subscribe(
            &bus,
            &session,
            programmer_subscription(object(Uuid::from_u128(12)), None),
        )
        .is_err()
    );
    assert!(
        EventStream::subscribe(
            &bus,
            &session,
            programmer_subscription(object(user_id), None),
        )
        .is_ok()
    );
    assert!(
        EventStream::subscribe(
            &bus,
            &session,
            programmer_subscription(
                wire::EventObject {
                    capability: wire::EventCapability::Programmer,
                    id: "programming-priority:not-a-uuid".into(),
                },
                None,
            ),
        )
        .is_err()
    );
}

#[tokio::test]
async fn broad_subscription_delivers_only_authenticated_user_priority() {
    let bus = EventBus::new(8);
    let user_id = Uuid::from_u128(11);
    let session = event_session(Uuid::from_u128(1), user_id);
    let request = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter::default(),
        after_sequence: Some(0),
        capacity: None,
        rate_limits: Vec::new(),
    });
    let mut stream = EventStream::subscribe(&bus, &session, request).unwrap();

    bus.publish(programmer_priority_draft(Uuid::from_u128(12), 1));
    assert!(stream.subscription.try_next().is_none());
    let expected = bus.publish(programmer_priority_draft(user_id, 2));

    let Some(wire::EventServerMessage::Event { event }) = stream.next().await else {
        panic!("expected the authenticated user's Programmer priority")
    };
    assert_eq!(event.sequence, expected.sequence);
    let wire::EventPayload::ProgrammerPriorityChanged {
        change: light_wire::v2::programmer_priority::ProgrammerPriorityChange::Upsert { projection },
    } = event.payload
    else {
        panic!("expected a Programmer priority payload")
    };
    assert_eq!(projection.user_id, user_id);
    assert_eq!(projection.revision, 2);
}

#[test]
fn programmer_lifecycle_is_the_only_aggregate_programmer_object() {
    let bus = EventBus::new(8);
    let session = event_session(Uuid::from_u128(1), Uuid::from_u128(11));
    let lifecycle = wire::EventObject {
        capability: wire::EventCapability::Programmer,
        id: "programming-lifecycle".into(),
    };
    assert!(
        EventStream::subscribe(&bus, &session, programmer_subscription(lifecycle, None)).is_ok()
    );

    for id in ["programming-lifecycle:foreign", "programming-unknown"] {
        let request = programmer_subscription(
            wire::EventObject {
                capability: wire::EventCapability::Programmer,
                id: id.into(),
            },
            None,
        );
        assert!(EventStream::subscribe(&bus, &session, request).is_err());
    }
}

#[tokio::test]
async fn lifecycle_aggregate_delivers_foreign_safe_rows_through_the_wire_adapter() {
    let bus = EventBus::new(8);
    let session = event_session(Uuid::from_u128(1), Uuid::from_u128(11));
    let object = wire::EventObject {
        capability: wire::EventCapability::Programmer,
        id: "programming-lifecycle".into(),
    };
    let mut stream = EventStream::subscribe(
        &bus,
        &session,
        programmer_subscription(object.clone(), Some(0)),
    )
    .unwrap();
    let foreign_user = UserId(Uuid::from_u128(12));
    let change = ProgrammingLifecycleChange::upsert(
        1,
        ProgrammingLifecycleProgrammer {
            programmer_id: light_core::ProgrammerId(Uuid::from_u128(20)),
            user_id: foreign_user,
            connected: true,
            selected_fixture_count: 3,
            normal_value_count: 2,
            sessions: vec![ProgrammingLifecycleSession {
                session_id: light_core::SessionId(Uuid::from_u128(30)),
            }],
        },
    );
    bus.publish(EventDraft::programming_lifecycle_changed(
        change,
        EventSource::Runtime,
        None,
    ));

    let Some(wire::EventServerMessage::Event { event }) = stream.next().await else {
        panic!("expected the installation lifecycle event")
    };
    assert_eq!(event.object, Some(object));
    let wire::EventPayload::ProgrammingLifecycleChanged { change } = event.payload else {
        panic!("expected a Programmer lifecycle payload")
    };
    assert_eq!(change.revision, 1);
    let light_wire::v2::programmer_lifecycle::ProgrammingLifecycleDelta::Upsert { programmer } =
        change.delta
    else {
        panic!("expected the foreign safe row")
    };
    assert_eq!(programmer.user_id, foreign_user.0);
    assert_eq!(programmer.normal_value_count, 2);
    assert_eq!(programmer.selected_fixture_count, 3);
}

#[test]
fn programmer_capture_mode_objects_are_limited_to_the_authenticated_user() {
    let bus = EventBus::new(8);
    let user_id = Uuid::from_u128(11);
    let session = event_session(Uuid::from_u128(1), user_id);
    let own = wire::EventObject {
        capability: wire::EventCapability::Programmer,
        id: format!("programming-capture-mode:{user_id}"),
    };
    let own_request = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter {
            objects: vec![own],
            ..Default::default()
        },
        after_sequence: None,
        capacity: None,
        rate_limits: Vec::new(),
    });
    assert!(EventStream::subscribe(&bus, &session, own_request).is_ok());

    for id in [
        format!("programming-capture-mode:{}", Uuid::from_u128(12)),
        "programming-capture-mode:not-a-uuid".into(),
    ] {
        let request = Ok(wire::EventClientMessage::Subscribe {
            filter: wire::EventSubscriptionFilter {
                objects: vec![wire::EventObject {
                    capability: wire::EventCapability::Programmer,
                    id,
                }],
                ..Default::default()
            },
            after_sequence: None,
            capacity: None,
            rate_limits: Vec::new(),
        });
        assert!(EventStream::subscribe(&bus, &session, request).is_err());
    }

    let foreign_rate = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter::default(),
        after_sequence: None,
        capacity: None,
        rate_limits: vec![wire::EventRateLimit {
            capability: wire::EventCapability::Programmer,
            class: wire::EventClass::Projection,
            object: Some(wire::EventObject {
                capability: wire::EventCapability::Programmer,
                id: format!("programming-capture-mode:{}", Uuid::from_u128(12)),
            }),
            min_interval_millis: 16,
        }],
    });
    assert!(EventStream::subscribe(&bus, &session, foreign_rate).is_err());
}

#[test]
fn programmer_preload_values_objects_are_limited_to_the_authenticated_user() {
    let bus = EventBus::new(8);
    let user_id = Uuid::from_u128(11);
    let session = event_session(Uuid::from_u128(1), user_id);
    for id in [
        format!("programming-preload-values:{}", Uuid::from_u128(12)),
        "programming-preload-values:not-a-uuid".into(),
    ] {
        let request = Ok(wire::EventClientMessage::Subscribe {
            filter: wire::EventSubscriptionFilter {
                objects: vec![wire::EventObject {
                    capability: wire::EventCapability::Programmer,
                    id,
                }],
                ..Default::default()
            },
            after_sequence: None,
            capacity: None,
            rate_limits: Vec::new(),
        });
        assert!(EventStream::subscribe(&bus, &session, request).is_err());
    }

    let own = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter {
            objects: vec![wire::EventObject {
                capability: wire::EventCapability::Programmer,
                id: format!("programming-preload-values:{user_id}"),
            }],
            ..Default::default()
        },
        after_sequence: None,
        capacity: None,
        rate_limits: Vec::new(),
    });
    assert!(EventStream::subscribe(&bus, &session, own).is_ok());
}

#[test]
fn programmer_preload_playback_queue_objects_are_limited_to_the_authenticated_user() {
    let bus = EventBus::new(8);
    let user_id = Uuid::from_u128(11);
    let session = event_session(Uuid::from_u128(1), user_id);
    for id in [
        format!("programming-preload-playback-queue:{}", Uuid::from_u128(12)),
        "programming-preload-playback-queue:not-a-uuid".into(),
    ] {
        let request = Ok(wire::EventClientMessage::Subscribe {
            filter: wire::EventSubscriptionFilter {
                objects: vec![wire::EventObject {
                    capability: wire::EventCapability::Programmer,
                    id,
                }],
                ..Default::default()
            },
            after_sequence: None,
            capacity: None,
            rate_limits: Vec::new(),
        });
        assert!(EventStream::subscribe(&bus, &session, request).is_err());
    }

    let own = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter {
            objects: vec![wire::EventObject {
                capability: wire::EventCapability::Programmer,
                id: format!("programming-preload-playback-queue:{user_id}"),
            }],
            ..Default::default()
        },
        after_sequence: None,
        capacity: None,
        rate_limits: Vec::new(),
    });
    assert!(EventStream::subscribe(&bus, &session, own).is_ok());
}

#[tokio::test]
async fn broad_subscription_delivers_only_authenticated_user_programmer_values() {
    let bus = EventBus::new(8);
    let user_id = Uuid::from_u128(11);
    let session = event_session(Uuid::from_u128(1), user_id);
    let request = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter::default(),
        after_sequence: Some(0),
        capacity: None,
        rate_limits: Vec::new(),
    });
    let mut stream = EventStream::subscribe(&bus, &session, request).unwrap();

    bus.publish(programmer_values_draft(Uuid::from_u128(12), 1));
    assert!(stream.subscription.try_next().is_none());
    let expected = bus.publish(programmer_values_draft(user_id, 2));

    let Some(wire::EventServerMessage::Event { event }) = stream.next().await else {
        panic!("expected the authenticated user's Programmer values")
    };
    assert_eq!(event.sequence, expected.sequence);
    let wire::EventPayload::ProgrammingValuesChanged { change } = event.payload else {
        panic!("expected a Programmer values payload")
    };
    assert_eq!(change.projection.user_id, user_id);
    assert_eq!(change.projection.revision, 2);
}

#[tokio::test]
async fn broad_subscription_delivers_only_authenticated_user_capture_mode() {
    let bus = EventBus::new(8);
    let user_id = Uuid::from_u128(11);
    let session = event_session(Uuid::from_u128(1), user_id);
    let request = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter::default(),
        after_sequence: Some(0),
        capacity: None,
        rate_limits: Vec::new(),
    });
    let mut stream = EventStream::subscribe(&bus, &session, request).unwrap();

    bus.publish(programmer_capture_mode_draft(Uuid::from_u128(12), 1));
    assert!(stream.subscription.try_next().is_none());
    let expected = bus.publish(programmer_capture_mode_draft(user_id, 2));

    let Some(wire::EventServerMessage::Event { event }) = stream.next().await else {
        panic!("expected the authenticated user's Programmer capture mode")
    };
    assert_eq!(event.sequence, expected.sequence);
    let wire::EventPayload::ProgrammingCaptureModeChanged { change } = event.payload else {
        panic!("expected a Programmer capture-mode payload")
    };
    assert_eq!(change.projection.user_id, user_id);
    assert_eq!(change.projection.revision, 2);
}

#[tokio::test]
async fn broad_subscription_delivers_only_authenticated_user_preload_values() {
    let bus = EventBus::new(8);
    let user_id = Uuid::from_u128(11);
    let session = event_session(Uuid::from_u128(1), user_id);
    let request = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter::default(),
        after_sequence: Some(0),
        capacity: None,
        rate_limits: Vec::new(),
    });
    let mut stream = EventStream::subscribe(&bus, &session, request).unwrap();

    bus.publish(programmer_preload_values_draft(Uuid::from_u128(12), 1));
    assert!(stream.subscription.try_next().is_none());
    let expected = bus.publish(programmer_preload_values_draft(user_id, 2));

    let Some(wire::EventServerMessage::Event { event }) = stream.next().await else {
        panic!("expected the authenticated user's Preload values")
    };
    assert_eq!(event.sequence, expected.sequence);
    let wire::EventPayload::ProgrammingPreloadValuesChanged { change } = event.payload else {
        panic!("expected a Preload values payload")
    };
    assert_eq!(change.projection.user_id, user_id);
    assert_eq!(change.projection.revision, 2);
}

#[tokio::test]
async fn broad_subscription_delivers_only_authenticated_user_preload_playback_queue() {
    let bus = EventBus::new(8);
    let user_id = Uuid::from_u128(11);
    let session = event_session(Uuid::from_u128(1), user_id);
    let request = Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter::default(),
        after_sequence: Some(0),
        capacity: None,
        rate_limits: Vec::new(),
    });
    let mut stream = EventStream::subscribe(&bus, &session, request).unwrap();

    bus.publish(programmer_preload_playback_queue_draft(
        Uuid::from_u128(12),
        1,
    ));
    assert!(stream.subscription.try_next().is_none());
    let expected = bus.publish(programmer_preload_playback_queue_draft(user_id, 2));

    let Some(wire::EventServerMessage::Event { event }) = stream.next().await else {
        panic!("expected the authenticated user's Preload playback queue")
    };
    assert_eq!(event.sequence, expected.sequence);
    let wire::EventPayload::ProgrammingPreloadPlaybackQueueChanged { change } = event.payload
    else {
        panic!("expected a Preload playback queue payload")
    };
    assert_eq!(change.projection.user_id, user_id);
    assert_eq!(change.projection.revision, 2);
    assert_eq!(change.projection.actions[0].page, Some(3));
}

fn event_session(desk_id: Uuid, user_id: Uuid) -> Session {
    Session {
        id: light_core::SessionId(Uuid::new_v4()),
        user: light_show::DeskUser {
            id: light_core::UserId(user_id),
            name: "Event operator".into(),
            enabled: true,
        },
        token: "event-token".into(),
        connected: true,
        desk: light_show::ControlDesk {
            id: desk_id,
            name: "Event desk".into(),
            osc_alias: "events".into(),
            columns: 1,
            rows: 1,
            buttons: 1,
            playback_layout: None,
        },
    }
}

fn subscription(
    object: Option<wire::EventObject>,
    after_sequence: Option<u64>,
) -> Result<wire::EventClientMessage, String> {
    Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter {
            capabilities: vec![wire::EventCapability::Playback],
            classes: vec![wire::EventClass::Transition],
            objects: object.into_iter().collect(),
        },
        after_sequence,
        capacity: Some(4),
        rate_limits: Vec::new(),
    })
}

fn programmer_subscription(
    object: wire::EventObject,
    after_sequence: Option<u64>,
) -> Result<wire::EventClientMessage, String> {
    Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter {
            capabilities: vec![wire::EventCapability::Programmer],
            classes: vec![wire::EventClass::Projection],
            objects: vec![object],
        },
        after_sequence,
        capacity: Some(4),
        rate_limits: Vec::new(),
    })
}

fn show_subscription(
    object: wire::EventObject,
    after_sequence: Option<u64>,
) -> Result<wire::EventClientMessage, String> {
    Ok(wire::EventClientMessage::Subscribe {
        filter: wire::EventSubscriptionFilter {
            capabilities: vec![wire::EventCapability::Show],
            classes: vec![wire::EventClass::Projection],
            objects: vec![object],
        },
        after_sequence,
        capacity: Some(4),
        rate_limits: Vec::new(),
    })
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

fn programmer_values_draft(user_id: Uuid, revision: u64) -> EventDraft {
    EventDraft::programming_values_changed(
        &ActionContext::operator(
            Uuid::from_u128(1),
            user_id,
            Uuid::new_v4(),
            ActionSource::UserInterface,
        ),
        ProgrammingValuesChange {
            projection: ProgrammingValuesProjection {
                user_id: UserId(user_id),
                revision,
                fixture_values: Vec::new(),
                group_values: Vec::new(),
            }
            .into(),
        },
    )
}

fn programmer_priority_draft(user_id: Uuid, revision: u64) -> EventDraft {
    EventDraft::programming_priority_changed(
        &ActionContext::operator(
            Uuid::from_u128(1),
            user_id,
            Uuid::new_v4(),
            ActionSource::UserInterface,
        ),
        ProgrammingPriorityChange::Upsert {
            projection: ProgrammingPriorityProjection {
                user_id: UserId(user_id),
                revision,
                priority: 90,
                changed_at: Utc::now(),
            },
        },
    )
}

fn programmer_capture_mode_draft(user_id: Uuid, revision: u64) -> EventDraft {
    EventDraft::programming_capture_mode_changed(
        &ActionContext::operator(
            Uuid::from_u128(1),
            user_id,
            Uuid::new_v4(),
            ActionSource::UserInterface,
        ),
        ProgrammingCaptureModeChange {
            projection: ProgrammingCaptureModeProjection {
                user_id: UserId(user_id),
                revision,
                blind: true,
                preview: false,
                preload_capture_programmer: true,
            }
            .into(),
        },
    )
}

fn programmer_preload_values_draft(user_id: Uuid, revision: u64) -> EventDraft {
    EventDraft::programming_preload_values_changed(
        &ActionContext::operator(
            Uuid::from_u128(1),
            user_id,
            Uuid::new_v4(),
            ActionSource::UserInterface,
        ),
        ProgrammingPreloadValuesChange {
            projection: ProgrammingPreloadValuesProjection {
                user_id: UserId(user_id),
                revision,
                fixture_values: Vec::new(),
                group_values: Vec::new(),
            }
            .into(),
        },
    )
}

fn programmer_preload_playback_queue_draft(user_id: Uuid, revision: u64) -> EventDraft {
    EventDraft::programming_preload_playback_queue_changed(
        &ActionContext::operator(
            Uuid::from_u128(1),
            user_id,
            Uuid::new_v4(),
            ActionSource::UserInterface,
        ),
        ProgrammingPreloadPlaybackQueueChange {
            projection: ProgrammingPreloadPlaybackQueueProjection {
                user_id: UserId(user_id),
                revision,
                actions: vec![ProgrammingPreloadPlaybackQueueItem {
                    playback_number: 7,
                    page: Some(3),
                    action: ProgrammingPreloadPlaybackAction::Go,
                    surface: ProgrammingPreloadPlaybackSurface::Virtual,
                }],
            }
            .into(),
        },
    )
}

fn transition_draft(playback_number: Option<u16>, cue_list_id: Uuid) -> EventDraft {
    EventDraft::playback_runtime_changed(
        None,
        PlaybackRuntimeChange {
            projection: PlaybackRuntimeProjection {
                scope: test_playback_scope(),
                requested: playback_number.map_or(
                    PlaybackRuntimeIdentity::CueList(CueListId(cue_list_id)),
                    PlaybackRuntimeIdentity::Playback,
                ),
                playback_number,
                target: PlaybackTargetProjection::CueList {
                    cue_list_id: CueListId(cue_list_id),
                    runtime: None,
                },
            },
            transition: Some(AppTransition {
                playback_number,
                cue_list_id,
                previous: Some(AppCue {
                    id: Uuid::from_u128(1),
                    number: 1.0,
                }),
                current: Some(AppCue {
                    id: Uuid::from_u128(2),
                    number: 2.0,
                }),
                cause: PlaybackTransitionCause::Chaser,
                advanced_steps: 1,
            }),
        },
        EventSource::Runtime,
        None,
    )
}

fn test_playback_scope() -> PlaybackShowScope {
    PlaybackShowScope {
        show_id: Uuid::from_u128(10),
        show_revision: 1,
    }
}

fn chaser() -> CueList {
    CueList {
        id: CueListId::new(),
        name: "Event transport Chaser".into(),
        priority: 0,
        mode: CueListMode::Chaser,
        looped: true,
        chaser_step_millis: 100,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Reset),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![Cue::new(1.0), Cue::new(2.0)],
    }
}
