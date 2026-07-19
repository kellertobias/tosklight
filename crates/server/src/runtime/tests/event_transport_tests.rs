//! Focused v2 event transport contract tests.

use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
use light_application::{
    CueReference as AppCue, EventBus, EventDraft, EventReplay, EventSource,
    PlaybackCueTransition as AppTransition, PlaybackTransitionCause,
    publish_automatic_playback_events,
};
use light_core::{CueListId, ManualClock};
use light_playback::{Cue, CueList, CueListMode, IntensityPriorityMode, RestartMode, WrapMode};
use light_wire::v2::events as wire;

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
        .playback()
        .write()
        .go_at(cue_list_id, started)
        .unwrap();
    let bus = EventBus::new(8);
    let object = wire::EventObject {
        capability: wire::EventCapability::Playback,
        id: format!("cuelist:{}", cue_list_id.0),
    };
    let mut stream = EventStream::subscribe(
        &bus,
        Uuid::from_u128(1),
        subscription(Some(object.clone()), Some(0)),
    )
    .unwrap();
    let waiting = tokio::spawn(async move { stream.next().await });
    tokio::task::yield_now().await;

    let mut other_desk = transition_draft(None, cue_list_id.0);
    other_desk.desk_id = Some(Uuid::from_u128(2));
    bus.publish(other_desk);
    bus.publish(transition_draft(Some(99), Uuid::from_u128(99)));
    clock.set(started + ChronoDuration::milliseconds(100));
    let rendered = engine.render(RenderOptions::default()).unwrap();
    publish_automatic_playback_events(&bus, rendered.automatic_playback_transitions);

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
    let wire::EventPayload::PlaybackCueTransition { transition } = event.payload else {
        panic!("expected a Playback Cue transition");
    };
    assert_eq!(transition.cause, wire::PlaybackTransitionCause::Chaser);
}

#[tokio::test]
async fn reconnect_gap_repairs_from_an_authoritative_snapshot_cursor() {
    let bus = EventBus::new(2);
    let desk_id = Uuid::from_u128(1);
    for sequence in 1..=3_u16 {
        bus.publish(transition_draft(Some(sequence), Uuid::from_u128(20)));
    }
    let mut stream = EventStream::subscribe(&bus, desk_id, subscription(None, Some(0))).unwrap();

    let Some(wire::EventServerMessage::Gap { gap }) = stream.next().await else {
        panic!("stale reconnect cursor should report a gap");
    };
    assert_eq!(gap.oldest_available, 2);
    assert_eq!(gap.latest_sequence, 3);
    let snapshot = playback_snapshot_from(&bus, desk_id, Vec::new);
    assert_eq!(
        stream.repair(snapshot.cursor),
        wire::EventServerMessage::Repaired {
            cursor: snapshot.cursor
        }
    );

    let expected = bus.publish(transition_draft(Some(4), Uuid::from_u128(20)));
    let Some(wire::EventServerMessage::Event { event }) = stream.next().await else {
        panic!("delivery should resume after repair");
    };
    assert_eq!(event.sequence, expected.sequence);
}

#[tokio::test]
async fn snapshot_cursor_precedes_projection_so_concurrent_events_replay() {
    let bus = EventBus::new(4);
    let desk_id = Uuid::from_u128(1);
    let snapshot = playback_snapshot_from(&bus, desk_id, || {
        bus.publish(transition_draft(Some(2), Uuid::from_u128(20)));
        Vec::new()
    });

    assert_eq!(snapshot.cursor.sequence, 0);
    let EventReplay::Events(events) = bus.replay(
        snapshot.cursor.sequence,
        &light_application::EventFilter::for_desk(desk_id),
    ) else {
        panic!("cursor captured before projection should remain replayable");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].sequence, 1);
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

fn transition_draft(playback_number: Option<u16>, cue_list_id: Uuid) -> EventDraft {
    EventDraft::playback_transition(
        None,
        AppTransition {
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
        },
        EventSource::Runtime,
        None,
    )
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
