//! Shared helpers for the Control ↔ Architect sync route and feed tests.
use super::*;

pub(super) async fn open_sync_show(app: &Router, token: &str, name: &str) -> String {
    let show = create_show(app, token, name).await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let response = app
        .clone()
        .oneshot(open_show_request(token, &show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    show_id
}

pub(super) fn sync_body(
    request_id: &str,
    association: Uuid,
    show_id: &str,
    operations: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "association_id": association,
        "show_id": show_id,
        "base_show_revision": 0,
        "origin": {"app": "architect"},
        "operations": operations,
    })
}

pub(super) async fn post_sync(app: &Router, token: &str, body: &serde_json::Value) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/v2/show-sync/transactions")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

pub(super) async fn read_object(
    app: &Router,
    token: &str,
    show_id: &str,
    kind: &str,
    id: &str,
) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(v2_show_object_get(token, show_id, kind, Some(id)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

pub(super) fn active_revision(state: &AppState) -> u64 {
    let entry = state.active_show.current().unwrap();
    ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap()
        .revision()
        .value()
}

/// Opts into the sync feed the way a bound Architect does; the feed publishes only while such a
/// subscription is alive, so keep the returned handle for the test's duration.
pub(super) fn subscribe_sync_feed(state: &AppState) -> light_application::EventSubscription {
    state.events.subscribe(
        light_application::EventFilter::default()
            .with_topic(light_application::EventTopic::ShowSync),
        light_application::SubscriptionOptions::default(),
    )
}

pub(super) fn sync_cursor(state: &AppState) -> u64 {
    state.events.latest_sequence()
}

/// Sync-feed events published after `cursor`.
pub(super) fn sync_events(state: &AppState, cursor: u64) -> Vec<light_application::ShowEvent> {
    let light_application::EventReplay::Events(events) = state.events.replay(
        cursor,
        &light_application::EventFilter::default()
            .with_topic(light_application::EventTopic::ShowSync),
    ) else {
        panic!("sync events remain replayable")
    };
    events
        .iter()
        .filter_map(|event| match &event.payload {
            light_application::ApplicationEvent::Show(
                show @ (light_application::ShowEvent::SyncCommitted(_)
                | light_application::ShowEvent::SyncGap(_)),
            ) => Some(show.clone()),
            _ => None,
        })
        .collect()
}
