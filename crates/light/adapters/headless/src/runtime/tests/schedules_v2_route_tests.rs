use super::*;

async fn post_schedule(
    app: &Router,
    token: &str,
    show_id: &str,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| serde_json::json!({"raw": String::from_utf8_lossy(&bytes)}));
    (status, body)
}

async fn schedule_snapshot(
    app: &Router,
    token: &str,
    show_id: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/schedules")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (status, body)
}

async fn seeded_schedule_show(state: &AppState, app: &Router, token: &str) -> String {
    let show = create_show(app, token, "Schedule routes").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let entry = state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(&show_id).unwrap()))
        .unwrap()
        .unwrap();
    let store = ShowStore::open(&entry.path).unwrap();
    store
        .put_object(
            "group",
            "worklights",
            &serde_json::json!({"id":"worklights","name":"Worklights","fixtures":[]}),
            0,
        )
        .unwrap();
    store
        .put_object(
            "playback",
            "7",
            &serde_json::json!({
                "number":7,
                "name":"Worklights",
                "target":{"type":"group","group_id":"worklights"}
            }),
            0,
        )
        .unwrap();
    store
        .put_object(
            "playback_page",
            "1",
            &serde_json::json!({
                "number":1,
                "name":"Main",
                "slots":{"3":7},
                "virtual_playbacks":{}
            }),
            0,
        )
        .unwrap();
    let opened = app
        .clone()
        .oneshot(open_show_request(token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    show_id
}

fn interval_create(request_id: &str) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "definition":{
            "name":"Every five minutes",
            "enabled":true,
            "trigger":{
                "type":"interval",
                "every_seconds":300,
                "enabled_at":"2000-01-01T00:00:00Z"
            },
            "target":{
                "type":"playback",
                "page":1,
                "slot":3,
                "playback_number":7,
                "action":"on",
                "master_transition":null
            }
        }
    })
}

#[tokio::test]
async fn schedule_routes_are_authenticated_revisioned_previewed_and_replay_safe() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show_id = seeded_schedule_show(&state, &app, &token).await;

    let unauthenticated = app
        .clone()
        .oneshot(
            Request::get("/api/v2/schedules")
                .header("x-tosk-show", &show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let create = interval_create("schedule-create-1");
    let (status, created) = post_schedule(
        &app,
        &token,
        &show_id,
        "/api/v2/schedules/create",
        create.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["schedule"]["object_revision"], 1);
    assert_eq!(
        created["schedule"]["definition"]["trigger"]["every_seconds"],
        300
    );
    assert_ne!(
        created["schedule"]["definition"]["trigger"]["enabled_at"],
        "2000-01-01T00:00:00Z"
    );
    let schedule_id = created["schedule"]["definition"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, replayed) =
        post_schedule(&app, &token, &show_id, "/api/v2/schedules/create", create).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["schedule"]["definition"]["id"], schedule_id);

    let (status, collision) = post_schedule(
        &app,
        &token,
        &show_id,
        "/api/v2/schedules/create",
        interval_create("schedule-create-1")
            .as_object()
            .map(|body| {
                let mut body = body.clone();
                body["definition"]["name"] = "Different".into();
                serde_json::Value::Object(body)
            })
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{collision}");

    let (status, snapshot) = schedule_snapshot(&app, &token, &show_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snapshot["show_id"], show_id);
    assert!(
        snapshot["timezone"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(snapshot["schedules"].as_array().unwrap().len(), 1);
    assert!(
        snapshot["schedules"][0]["next_occurrence"]
            .as_object()
            .is_some()
    );

    let (status, preview) = post_schedule(
        &app,
        &token,
        &show_id,
        "/api/v2/schedules/preview",
        serde_json::json!({
            "trigger":{
                "type":"calendar",
                "rule":{"type":"expression","expression":"0 14 * * 1"}
            },
            "count":3
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["occurrences"].as_array().unwrap().len(), 3);

    let (status, disabled) = post_schedule(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/schedules/{schedule_id}/update"),
        serde_json::json!({
            "request_id":"schedule-disable-1",
            "expected_revision":1,
            "patch":{"enabled":false}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{disabled}");
    assert_eq!(disabled["schedule"]["definition"]["enabled"], false);
    assert_eq!(disabled["schedule"]["object_revision"], 2);

    let (status, duplicated) = post_schedule(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/schedules/{schedule_id}/duplicate"),
        serde_json::json!({
            "request_id":"schedule-duplicate-1",
            "expected_revision":2
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{duplicated}");
    assert_ne!(
        duplicated["schedule"]["definition"]["id"],
        serde_json::Value::String(schedule_id.clone())
    );
    assert_eq!(duplicated["schedule"]["history"], serde_json::json!([]));

    let (status, deleted) = post_schedule(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/schedules/{schedule_id}/delete"),
        serde_json::json!({
            "request_id":"schedule-delete-1",
            "expected_revision":2
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert!(deleted["schedule"].is_null());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn enabled_schedule_rejects_past_macro_missing_and_moved_targets() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show_id = seeded_schedule_show(&state, &app, &token).await;

    let (status, past) = post_schedule(
        &app,
        &token,
        &show_id,
        "/api/v2/schedules/create",
        serde_json::json!({
            "request_id":"past",
            "definition":{
                "name":"Past",
                "enabled":true,
                "trigger":{"type":"one_time","at":"2000-01-01T00:00:00"},
                "target":{
                    "type":"playback","page":1,"slot":3,"playback_number":7,"action":"on"
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{past}");

    let (status, macro_target) = post_schedule(
        &app,
        &token,
        &show_id,
        "/api/v2/schedules/create",
        serde_json::json!({
            "request_id":"macro",
            "definition":{
                "name":"Macro",
                "enabled":true,
                "trigger":{
                    "type":"interval","every_seconds":300,"enabled_at":"2000-01-01T00:00:00Z"
                },
                "target":{"type":"macro","macro_id":"doors"}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{macro_target}");

    let mut moved = interval_create("moved");
    moved["definition"]["target"]["slot"] = 4.into();
    let (status, moved) =
        post_schedule(&app, &token, &show_id, "/api/v2/schedules/create", moved).await;
    assert_eq!(status, StatusCode::CONFLICT, "{moved}");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn due_one_time_schedule_executes_through_playback_and_disables_itself() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show_id = seeded_schedule_show(&state, &app, &token).await;
    let (_, initial) = schedule_snapshot(&app, &token, &show_id).await;
    let timezone = jiff::tz::TimeZone::get(initial["timezone"].as_str().unwrap()).unwrap();
    let local_due = jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::from_secs(2))
        .unwrap()
        .to_zoned(timezone)
        .strftime("%Y-%m-%dT%H:%M:%S")
        .to_string();
    let (status, created) = post_schedule(
        &app,
        &token,
        &show_id,
        "/api/v2/schedules/create",
        serde_json::json!({
            "request_id":"one-time-execution",
            "definition":{
                "name":"Worklights now",
                "enabled":true,
                "trigger":{"type":"one_time","at":local_due},
                "target":{
                    "type":"playback","page":1,"slot":3,"playback_number":7,"action":"on"
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let schedule_id = created["schedule"]["definition"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let events_before_execution = state.events.latest_sequence();

    let cancellation = CancellationToken::new();
    let task = tokio::spawn(super::super::schedules_v2::run_scheduler(
        state.clone(),
        cancellation.clone(),
    ));
    let completed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, snapshot) = schedule_snapshot(&app, &token, &show_id).await;
            let schedule = snapshot["schedules"]
                .as_array()
                .unwrap()
                .iter()
                .find(|schedule| schedule["definition"]["id"] == schedule_id)
                .unwrap();
            if schedule["history"][0]["status"] == "completed" {
                assert_eq!(schedule["definition"]["enabled"], false);
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    if completed.is_err() {
        let (_, snapshot) = schedule_snapshot(&app, &token, &show_id).await;
        panic!("one-time Schedule must execute promptly: {snapshot}");
    }
    cancellation.cancel();
    task.await.unwrap().unwrap();

    let light_application::EventReplay::Events(events) = state.events.replay(
        events_before_execution,
        &light_application::EventFilter::default(),
    ) else {
        panic!("Schedule runtime event must remain inside the retained event horizon");
    };
    let change = events
        .iter()
        .find_map(|event| match &event.payload {
            light_application::ApplicationEvent::Show(
                light_application::ShowEvent::ScheduleRuntimeChanged(change),
            ) if change.schedule_id.to_string() == schedule_id => Some(change),
            _ => None,
        })
        .expect("completed Schedule must publish a semantic runtime event");
    assert!(change.next_occurrence.is_none());
    assert_eq!(
        change.last_result.as_ref().map(|result| result.status),
        Some(light_application::ScheduleOccurrenceStatus::Completed)
    );

    let _ = std::fs::remove_dir_all(data_dir);
}
