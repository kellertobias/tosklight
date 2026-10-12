use super::*;

#[tokio::test]
async fn mounted_cuelist_subscription_tracks_assigned_playback_lifecycle() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_pool_cuelist_test_state(&state);
    let id = state.output.snapshot().cue_lists[0].id;
    assert!(
        state
            .output
            .playback_runtime_status_for_cue_list(id)
            .is_none()
    );
    let mut stream = aggregate_subscription(&state, &token, id);
    for (action, cue_number, enabled, paused) in [
        (
            serde_json::json!({"type":"go","pressed":true}),
            "1",
            true,
            false,
        ),
        (
            serde_json::json!({"type":"go_to","cue_number":"2"}),
            "2",
            true,
            false,
        ),
        (
            serde_json::json!({"type":"back","pressed":true}),
            "1",
            true,
            false,
        ),
        (
            serde_json::json!({"type":"pause","pressed":true}),
            "1",
            true,
            true,
        ),
        (
            serde_json::json!({"type":"pause","pressed":true}),
            "1",
            true,
            false,
        ),
        (
            serde_json::json!({"type":"off","pressed":true}),
            "1",
            false,
            false,
        ),
    ] {
        let response = post_action(
            &app,
            Some(&token),
            session.desk.id,
            action_request("aggregate-lifecycle", 1, action),
        )
        .await;
        let status = response.status();
        let response = json(response).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        let value = next_aggregate(&mut stream, id).await;
        assert_eq!(value["object"]["id"], format!("cuelist:{}", id.0));
        let projection = &value["payload"]["change"]["projection"];
        assert_eq!(
            projection["identity"],
            serde_json::json!({"kind":"cue_list","cue_list_id":id})
        );
        assert_eq!(projection["runtime"]["current"]["number"], cue_number);
        assert_eq!(projection["runtime"]["enabled"], enabled);
        assert_eq!(projection["runtime"]["paused"], paused);
        assert!(projection["runtime"]["activated_at"].is_string());
        assert_eq!(
            projection["runtime"]["transition_ordinal"],
            response["projection"]["runtime"]["transition_ordinal"]
        );
        assert!(
            state
                .output
                .playback_runtime_status_for_direct_cue_list(id)
                .is_none(),
            "assigned source must not invent independent direct ownership"
        );
        for position in 1..=4 {
            let address = light_playback::PlaybackAddress::new(1, position).unwrap();
            assert_eq!(
                state
                    .output
                    .playback_runtime_status_at(address)
                    .unwrap()
                    .playback
                    .enabled,
                enabled
            );
        }
        assert!(
            response["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|related| related["projection"]["identity"]
                    == serde_json::json!({"kind":"cue_list","cue_list_id":id}))
        );
    }
    let mut snapshot = (*state.output.snapshot()).clone();
    let mut standalone = snapshot.cue_lists[0].clone();
    standalone.id = light_core::CueListId::new();
    let direct_id = standalone.id;
    Arc::make_mut(&mut snapshot.cue_lists).push(standalone);
    state.output.replace_snapshot(snapshot).unwrap();
    let direct = post_action(
        &app,
        Some(&token),
        session.desk.id,
        serde_json::json!({
            "address":{"kind":"cue_list","cue_list_id":direct_id},
            "action":{"type":"go","pressed":true},
            "control_source":{"surface_id":"main","control_id":2}
        }),
    )
    .await;
    assert_eq!(direct.status(), StatusCode::OK);
    assert!(
        state
            .output
            .playback_runtime_status_for_direct_cue_list(direct_id)
            .is_some_and(|runtime| runtime.playback.enabled)
    );
    let direct = json(direct).await;
    assert!(
        direct["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|related| related["projection"]["identity"]
                == serde_json::json!({"kind":"direct_cue_list","cue_list_id":direct_id}))
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn solo_release_publishes_peer_and_winner_aggregate_cuelists_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_virtual_exclusion_test_state(&state);
    set_virtual_enabled(&state, 1, 102, true);
    put_virtual_exclusion_zone(&state, &app, &token, &[101, 102]).await;
    let cue_lists = state.output.snapshot().cue_lists.clone();
    let winner = cue_lists[0].id;
    let peer = cue_lists[1].id;
    let cursor = state.events.latest_sequence();
    let response = post_action(
        &app,
        Some(&token),
        session.desk.id,
        virtual_action_request(
            "solo-aggregate",
            1,
            101,
            serde_json::json!({"type":"on","pressed":true}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    for (id, enabled) in [(winner, true), (peer, false)] {
        let related = response["related"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| {
                entry["projection"]["identity"]
                    == serde_json::json!({"kind":"cue_list","cue_list_id":id})
            })
            .collect::<Vec<_>>();
        assert_eq!(
            related.len(),
            1,
            "each changed aggregate publishes exactly once"
        );
        assert_eq!(related[0]["projection"]["runtime"]["enabled"], enabled);
        assert!(related[0]["event_sequence"].as_u64().unwrap() > cursor);
        assert!(
            related[0]["event_sequence"].as_u64().unwrap()
                < response["event_sequence"].as_u64().unwrap()
        );
        assert!(
            state
                .output
                .playback_runtime_status_for_direct_cue_list(id)
                .is_none()
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

pub(super) fn exact_events_with_aggregates(
    state: &AppState,
    after: u64,
    aggregate_count: usize,
) -> Vec<Arc<light_application::EventEnvelope>> {
    use light_application::{ApplicationEvent, PlaybackEvent, PlaybackRuntimeIdentity};
    let events = playback_runtime_events(state, after);
    let mut exact = Vec::new();
    let mut aggregates = Vec::new();
    for event in events {
        let ApplicationEvent::Playback(PlaybackEvent::RuntimeChanged(change)) = &event.payload
        else {
            panic!("expected runtime feedback");
        };
        match change.projection.identity {
            PlaybackRuntimeIdentity::Playback(_) => exact.push(event),
            PlaybackRuntimeIdentity::CueList(_) => aggregates.push(event),
            _ => panic!("unexpected source identity in exact/aggregate change set"),
        }
    }
    assert_eq!(
        aggregates.len(),
        aggregate_count,
        "exact aggregate event count"
    );
    let targets = |events: &[Arc<light_application::EventEnvelope>]| {
        events
            .iter()
            .map(|event| {
                let ApplicationEvent::Playback(PlaybackEvent::RuntimeChanged(change)) =
                    &event.payload
                else {
                    unreachable!()
                };
                let light_application::PlaybackTargetProjection::CueList { cue_list_id, .. } =
                    &change.projection.target
                else {
                    panic!("expected Cuelist target");
                };
                *cue_list_id
            })
            .collect::<std::collections::HashSet<_>>()
    };
    assert_eq!(
        targets(&aggregates),
        targets(&exact),
        "aggregate fanout must match changed exact targets only"
    );
    exact
}

pub(super) fn exact_identities_with_aggregates(
    state: &AppState,
    after: u64,
    count: usize,
) -> Vec<light_application::PlaybackRuntimeIdentity> {
    exact_events_with_aggregates(state, after, count)
        .into_iter()
        .map(|event| {
            let light_application::ApplicationEvent::Playback(
                light_application::PlaybackEvent::RuntimeChanged(change),
            ) = &event.payload
            else {
                unreachable!()
            };
            change.projection.identity.clone()
        })
        .collect()
}

pub(super) fn assert_only_aggregate_related(response: &serde_json::Value) {
    let related = response["related"].as_array().unwrap();
    assert_eq!(related.len(), 1);
    assert_eq!(
        related[0]["projection"]["identity"],
        serde_json::json!({
            "kind":"cue_list", "cue_list_id":response["projection"]["cue_list_id"]
        })
    );
    assert!(
        related[0]["event_sequence"].as_u64().unwrap()
            < response["event_sequence"].as_u64().unwrap()
    );
}

fn aggregate_subscription(
    state: &AppState,
    token: &str,
    id: light_core::CueListId,
) -> event_transport::EventStream {
    let mut protocols = HeaderMap::new();
    protocols.insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        format!("light.events.v2, light.token.{token}")
            .parse()
            .unwrap(),
    );
    let event_session = event_transport::authenticate_protocols(&state, &protocols).unwrap();
    let subscribe = serde_json::from_value(serde_json::json!({
        "type":"subscribe", "filter": {"capabilities":["playback"],
            "classes":["transition","projection"],
            "objects":[{"capability":"playback","id":format!("cuelist:{}", id.0)}]},
        "after_sequence":state.events.latest_sequence(), "capacity":32, "rate_limits":[]
    }))
    .unwrap();
    event_transport::EventStream::subscribe(&state.events, &event_session, Ok(subscribe)).unwrap()
}

async fn next_aggregate(
    stream: &mut event_transport::EventStream,
    id: light_core::CueListId,
) -> serde_json::Value {
    tokio::time::timeout(std::time::Duration::from_millis(250), async {
        loop {
            let event = stream
                .next()
                .await
                .expect("mounted event stream remains open");
            let light_wire::v2::events::EventServerMessage::Event { event } = event else {
                continue;
            };
            let value = serde_json::to_value(event).unwrap();
            // Related object routing can also deliver exact-address alias projections.
            // Those do not populate the aggregate key the mounted Cuelist view reads.
            if value["payload"]["change"]["projection"]["identity"]
                == serde_json::json!({"kind":"cue_list","cue_list_id":id})
            {
                break value;
            }
        }
    })
    .await
    .expect("mounted aggregate Cuelist must receive its own accepted projection")
}

pub(in crate::runtime::tests) fn assert_aggregate_feedback_matches_aliases(
    events: &[Arc<light_application::EventEnvelope>],
    exact_count: usize,
) {
    use light_application::{
        ApplicationEvent, PlaybackEvent, PlaybackRuntimeIdentity, PlaybackTargetProjection,
    };
    let projections = events
        .iter()
        .filter_map(|event| match &event.payload {
            ApplicationEvent::Playback(PlaybackEvent::RuntimeChanged(change)) => {
                Some((event, &change.projection))
            }
            ApplicationEvent::Playback(_) => panic!("unexpected Playback event variant"),
            _ => None,
        })
        .collect::<Vec<_>>();
    let aggregates = projections
        .iter()
        .filter(|(_, p)| matches!(p.identity, PlaybackRuntimeIdentity::CueList(_)))
        .collect::<Vec<_>>();
    let exact = projections
        .iter()
        .filter(|(_, p)| matches!(p.identity, PlaybackRuntimeIdentity::Playback(_)))
        .collect::<Vec<_>>();
    assert_eq!(aggregates.len(), 1, "one changed aggregate Cuelist event");
    assert_eq!(
        exact.len(),
        exact_count,
        "every exact alias event remains present"
    );
    assert_eq!(projections.len(), exact_count + 1);
    let (aggregate_event, aggregate) = aggregates[0];
    let PlaybackTargetProjection::CueList {
        cue_list_id,
        runtime: Some(aggregate_runtime),
    } = &aggregate.target
    else {
        panic!("expected aggregate runtime");
    };
    assert_eq!(
        aggregate.identity,
        PlaybackRuntimeIdentity::CueList(*cue_list_id)
    );
    for (event, projection) in &exact {
        let PlaybackTargetProjection::CueList {
            cue_list_id: exact_id,
            runtime: Some(runtime),
        } = &projection.target
        else {
            panic!("expected exact Cuelist runtime");
        };
        assert_eq!(exact_id, cue_list_id);
        assert_eq!(runtime.current, aggregate_runtime.current);
        assert_eq!(runtime.enabled, aggregate_runtime.enabled);
        assert_eq!(
            runtime.transition_ordinal,
            aggregate_runtime.transition_ordinal
        );
        assert_eq!(event.source, aggregate_event.source);
        assert_eq!(event.correlation_id, aggregate_event.correlation_id);
    }
    assert!(
        aggregate_event.sequence < exact.last().unwrap().0.sequence,
        "aggregate precedes addressed primary event"
    );
}

#[tokio::test]
async fn solo_fanout_129_targets_does_not_consume_the_public_snapshot_limit() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_playback_test_show(&app, &token).await;
    let lists = (1..=129)
        .map(|_| playback_test_cue_list())
        .collect::<Vec<_>>();
    let definitions = lists
        .iter()
        .enumerate()
        .map(|(index, list)| {
            let mut definition = playback_test_definition(
                index as u16 + 1,
                light_playback::PlaybackTarget::CueList {
                    cue_list_id: list.id,
                },
            );
            definition.auto_off = false;
            definition
        })
        .collect();
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: lists.into(),
            playback_pages: vec![test_page(1, definitions)].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    put_virtual_exclusion_zone(&state, &app, &token, &(1..=129).collect::<Vec<_>>()).await;
    set_pool_enabled(&state, 129, true);
    let response = post_action(
        &app,
        Some(&token),
        session.desk.id,
        action_request(
            "large-solo",
            1,
            serde_json::json!({"type":"on","pressed":true}),
        ),
    )
    .await;
    let status = response.status();
    let response = json(response).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "valid internal Solo fanout must not inherit the public snapshot request limit: {response}"
    );
    assert!(virtual_is_enabled(&state, 1, 1));
    assert!(!virtual_is_enabled(&state, 1, 129));
    let aggregates = response["related"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|change| change["projection"]["identity"]["kind"] == "cue_list")
        .count();
    assert_eq!(
        aggregates, 2,
        "only actually changed target aggregates publish"
    );
    let oversized = app.clone().oneshot(Request::post("/api/v2/playback-runtime/snapshot")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::json!({"identities":(1..=257).map(|position|
            serde_json::json!({"kind":"playback","page":1,"position":position})).collect::<Vec<_>>()} ).to_string())).unwrap()).await.unwrap();
    assert_eq!(
        oversized.status(),
        StatusCode::BAD_REQUEST,
        "public snapshot cap remains bounded"
    );
    assert!(json(oversized).await.to_string().contains("at most 256"));
    let _ = std::fs::remove_dir_all(data_dir);
}
