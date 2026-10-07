//! The PSN routes as a client meets them.
//!
//! What is being checked here is not arithmetic — that is tested against real packets beside the
//! receiver — but the contract an operator's tab depends on: a show that has never heard of
//! tracking reads as off, an edit carries only what changed, a refusal says what is wrong in the
//! words that were typed, and a resent edit does not bind a tracker twice.

use super::*;
use light_psn_wire::{PsnTrackerData, PsnVector3, encode_data_frame};

async fn open_show_for(app: &Router, token: &str, show_id: &str) {
    let response = app
        .clone()
        .oneshot(open_show_request(token, show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn read_psn(app: &Router, token: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/psn")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn update_psn(app: &Router, token: &str, body: &serde_json::Value) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/v2/psn/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn open_desk(app: &Router, name: &str) -> (String, String) {
    let (token, _) = login(app, name).await;
    let show = create_show(app, &token, "Tracking").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    open_show_for(app, &token, &show_id).await;
    (token, show_id)
}

#[tokio::test]
async fn a_show_that_has_never_heard_of_tracking_reads_as_off() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;

    let snapshot = read_psn(&app, &token).await;

    assert_eq!(snapshot["configuration"]["enabled"], false);
    assert_eq!(snapshot["configuration"]["group"], "236.10.10.10");
    assert_eq!(snapshot["configuration"]["port"], 56565);
    assert!(
        snapshot["configuration"]["bindings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(snapshot["revision"], 0);
    // Nothing is being listened to, so there is no health to report and nothing is wrong.
    assert!(snapshot["status"]["health"].is_null());
    assert!(snapshot["status"]["error"].is_null());
}

#[tokio::test]
async fn binding_a_tracker_to_a_point_is_stored_and_read_back() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;
    let binding = Uuid::new_v4();
    let point = Uuid::new_v4();

    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({
            "request_id": "bind-1",
            "enabled": true,
            "bindings": [{
                "id": binding,
                "tracker_id": 3,
                "point_fixture_id": point,
                "enabled": true,
            }],
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["unchanged"], false);

    let snapshot = read_psn(&app, &token).await;
    assert_eq!(snapshot["configuration"]["enabled"], true);
    assert_eq!(snapshot["configuration"]["bindings"][0]["tracker_id"], 3);
    assert_eq!(
        snapshot["configuration"]["bindings"][0]["point_fixture_id"],
        serde_json::json!(point)
    );
    // The running receiver was told, rather than finding out when the show is next read.
    assert!(state.psn.configuration().enabled);
    assert_eq!(state.psn.configuration().bindings[0].tracker_id, 3);
}

#[tokio::test]
async fn an_edit_carries_only_what_changed() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;
    let binding = Uuid::new_v4();
    update_psn(
        &app,
        &token,
        &serde_json::json!({
            "request_id": "bind-1",
            "enabled": true,
            "bindings": [{
                "id": binding,
                "tracker_id": 3,
                "point_fixture_id": Uuid::new_v4(),
                "enabled": true,
            }],
        }),
    )
    .await;

    // Turning the source off must not forget which tracker was which point.
    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "off-1", "enabled": false}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let snapshot = read_psn(&app, &token).await;
    assert_eq!(snapshot["configuration"]["enabled"], false);
    assert_eq!(snapshot["configuration"]["bindings"][0]["tracker_id"], 3);
}

#[tokio::test]
async fn an_address_that_is_not_a_multicast_group_is_refused_in_the_words_that_were_typed() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;

    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "bad-1", "group": "10.0.0.4"}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error = json(response).await["error"].as_str().unwrap().to_owned();
    assert!(error.contains("10.0.0.4"), "{error}");
    assert!(error.contains("multicast"), "{error}");
    // Nothing was stored, so the desk keeps listening where it was.
    assert_eq!(
        read_psn(&app, &token).await["configuration"]["group"],
        "236.10.10.10"
    );
}

#[tokio::test]
async fn a_resent_edit_is_answered_from_the_replay_window_rather_than_applied_twice() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;
    let edit = serde_json::json!({
        "request_id": "enable-1",
        "enabled": true,
    });

    let first = json(update_psn(&app, &token, &edit).await).await;
    let second = json(update_psn(&app, &token, &edit).await).await;

    assert_eq!(first["replayed"], false);
    assert_eq!(second["replayed"], true);
    assert_eq!(first["revision"], second["revision"]);

    // The same identity carrying a different edit is a client bug, and is refused rather than
    // silently applied.
    let conflicting = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "enable-1", "enabled": false}),
    )
    .await;
    assert_eq!(conflicting.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn asking_for_what_is_already_stored_changes_no_revision() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;
    let first = json(
        update_psn(
            &app,
            &token,
            &serde_json::json!({"request_id": "enable-1", "enabled": true}),
        )
        .await,
    )
    .await;

    let again = json(
        update_psn(
            &app,
            &token,
            &serde_json::json!({"request_id": "enable-2", "enabled": true}),
        )
        .await,
    )
    .await;

    assert_eq!(again["unchanged"], true);
    assert_eq!(again["revision"], first["revision"]);
}

#[tokio::test]
async fn a_zone_with_its_corners_the_wrong_way_round_is_refused() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;

    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({
            "request_id": "zone-1",
            "zones": [{
                "id": Uuid::new_v4(),
                "name": "Downstage",
                "min_metres": [2.0, 0.0, 0.0],
                "max_metres": [-2.0, 3.0, 3.0],
                "tracker_ids": [],
                "dwell_millis": 250,
            }],
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        json(response).await["error"]
            .as_str()
            .unwrap()
            .contains("low corner above its high corner")
    );
}

#[tokio::test]
async fn an_unknown_field_is_accepted_rather_than_refused() {
    // api-rules §5: a client from a later version must not be turned away by the desk.
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;

    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({
            "request_id": "future-1",
            "enabled": true,
            "beam_tracking_mode": "wide",
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        read_psn(&app, &token).await["configuration"]["enabled"],
        true
    );
}

#[tokio::test]
async fn repeated_status_gets_do_not_consume_a_zone_enter() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _show_id) = open_desk(&app, "Operator").await;
    let zone = Uuid::new_v4();
    let enter_macro = Uuid::new_v4();
    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({
            "request_id": "zone-read-only-1",
            "enabled": true,
            "zones": [{
                "id": zone,
                "name": "Downstage",
                "min_metres": [-1.0, -1.0, -1.0],
                "max_metres": [1.0, 1.0, 1.0],
                "tracker_ids": [3],
                "dwell_millis": 0,
                "enter_macro_id": enter_macro,
            }],
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let now = super::psn::listener::now_millis();
    let tracker = PsnTrackerData {
        id: 3,
        position: Some(PsnVector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }),
        ..PsnTrackerData::default()
    };
    let source = "10.0.0.9:56565".parse().unwrap();
    for datagram in encode_data_frame(now * 1_000, 1, &[tracker]) {
        state.psn.observe(source, &datagram, now);
    }

    for _ in 0..3 {
        let status = read_psn(&app, &token).await;
        assert_eq!(status["status"]["trackers"][0]["tracker_id"], 3);
        assert_eq!(status["status"]["occupied_zone_ids"], serde_json::json!([]));
    }
    let committed = state.psn.tick(now);
    assert_eq!(
        committed.zone_transitions,
        vec![(zone, super::psn::zones::ZoneTransition::Entered)]
    );
    assert_eq!(
        read_psn(&app, &token).await["status"]["occupied_zone_ids"],
        serde_json::json!([zone])
    );
}

#[tokio::test]
async fn opening_and_reopening_shows_installs_tracking_without_waiting_for_a_listener_poll() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, first_id) = open_desk(&app, "Operator").await;
    let edit = serde_json::json!({ "request_id": "enable-first", "enabled": true });
    assert_eq!(
        update_psn(&app, &token, &edit).await.status(),
        StatusCode::OK
    );
    let first_generation = state.psn.generation();
    let tracker = PsnTrackerData {
        id: 3,
        position: Some(PsnVector3 {
            x: 2.0,
            y: 1.0,
            z: -4.0,
        }),
        ..Default::default()
    };
    let bytes = encode_data_frame(1_000, 1, &[tracker]);
    let source = "127.0.0.1:56565".parse().unwrap();
    for datagram in &bytes {
        state.psn.observe(source, datagram, 1);
    }
    assert_eq!(state.psn.status(1).trackers.len(), 1);

    let second = create_show(&app, &token, "Second Tracking Show").await;
    let second_id = second["id"].as_str().unwrap();
    open_show_for(&app, &token, second_id).await;
    assert!(!state.psn.configuration().enabled);
    assert!(state.psn.status(2).trackers.is_empty());
    assert!(state.psn.generation() > first_generation);
    assert_eq!(
        update_psn(
            &app,
            &token,
            &serde_json::json!({
                "request_id": "enable-second", "enabled": true
            })
        )
        .await
        .status(),
        StatusCode::OK
    );
    for datagram in &bytes {
        state.psn.observe(source, datagram, 2);
    }
    assert_eq!(state.psn.status(2).trackers.len(), 1);
    let second_generation = state.psn.generation();

    // Both shows now hold exactly the same settings. Their live tracking ownership is distinct.
    open_show_for(&app, &token, &first_id).await;
    assert!(state.psn.configuration().enabled);
    assert!(state.psn.status(3).trackers.is_empty());
    assert!(state.psn.generation() > second_generation);
    for datagram in &bytes {
        state.psn.observe(source, datagram, 3);
    }
    let before_reopen = state.psn.generation();
    open_show_for(&app, &token, &first_id).await;
    assert!(state.psn.status(4).trackers.is_empty());
    assert!(state.psn.generation() > before_reopen);
}

#[tokio::test]
async fn disabling_tracking_releases_engine_holds_before_the_update_returns() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = open_desk(&app, "Operator").await;
    assert_eq!(
        update_psn(
            &app,
            &token,
            &serde_json::json!({
                "request_id": "tracking-on", "enabled": true
            })
        )
        .await
        .status(),
        StatusCode::OK
    );
    state
        .output
        .engine()
        .set_tracked_overrides([light_engine::TrackedOverride::new(
            light_core::FixtureId::new(),
            light_core::AttributeKey("point.position.x".into()),
            light_core::AttributeValue::Normalized(0.75),
        )]);
    assert_eq!(state.output.engine().tracked_overrides().len(), 1);
    assert_eq!(
        update_psn(
            &app,
            &token,
            &serde_json::json!({
                "request_id": "tracking-off", "enabled": false
            })
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert!(state.output.engine().tracked_overrides().is_empty());
}

fn stored_binding(id: Uuid, tracker_id: u16, point: Uuid) -> serde_json::Value {
    serde_json::json!({"id": id, "trackerId": tracker_id, "pointFixtureId": point, "enabled": true})
}

async fn open_seeded_psn_show(
    state: &AppState,
    app: &Router,
    token: &str,
    body: &serde_json::Value,
) -> light_show::ShowEntry {
    let show = create_show(app, token, "Older tracking configuration").await;
    let show_id = light_core::ShowId(Uuid::parse_str(show["id"].as_str().unwrap()).unwrap());
    let entry = state.installation.show(show_id).unwrap().unwrap();
    // Seed portable data through the store to represent a file saved before write validation.
    let store = ShowStore::open(&entry.path).unwrap();
    store.put_object("psn", "main", body, 0).unwrap();
    drop(store);
    open_show_for(app, token, &show_id.0.to_string()).await;
    entry
}

#[tokio::test]
async fn a_new_duplicate_binding_id_is_rejected_without_changing_tracking_or_show_data() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = open_desk(&app, "Operator").await;
    let before = read_psn(&app, &token).await;
    let id = Uuid::new_v4();
    let response = update_psn(
        &app,
        &token,
        &serde_json::json!({
            "request_id": "new-duplicate-binding", "enabled": true,
            "bindings": [
                {"id": id, "tracker_id": 3, "point_fixture_id": Uuid::new_v4(), "enabled": true},
                {"id": id, "tracker_id": 4, "point_fixture_id": Uuid::new_v4(), "enabled": false}
            ]
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        json(response).await["error"]
            .as_str()
            .unwrap()
            .contains("binding ID")
    );
    let after = read_psn(&app, &token).await;
    assert_eq!(after["configuration"], before["configuration"]);
    assert_eq!(after["revision"], before["revision"]);
    assert!(!state.psn.configuration().enabled);
    assert!(state.output.engine().tracking_frame().points.is_empty());
}

#[tokio::test]
async fn older_duplicate_bindings_load_and_restart_without_rewriting_or_aliasing_independent_points()
 {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let collision = Uuid::new_v4();
    let independent = Uuid::new_v4();
    let independent_point = Uuid::new_v4();
    let raw = serde_json::json!({
        "enabled": true,
        "bindings": [
            stored_binding(collision, 3, Uuid::new_v4()),
            stored_binding(collision, 4, Uuid::new_v4()),
            stored_binding(independent, 5, independent_point)
        ],
        "future_tracking_field": {"preserve": true}
    });
    let entry = open_seeded_psn_show(&state, &app, &token, &raw).await;
    let store = ShowStore::open(&entry.path).unwrap();
    let revision = store.portable_revision().unwrap();
    assert_eq!(store.objects("psn").unwrap()[0].body, raw);
    let snapshot = read_psn(&app, &token).await;
    assert_eq!(
        snapshot["configuration"]["bindings"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        snapshot["status"]["diagnostics"]["conflicting_binding_rows"],
        2
    );
    assert!(snapshot["status"]["error"].is_null());
    let rows: Vec<_> = [3, 4, 5]
        .into_iter()
        .map(|id| PsnTrackerData {
            id,
            position: Some(PsnVector3 {
                x: f32::from(id),
                y: 0.0,
                z: 0.0,
            }),
            ..Default::default()
        })
        .collect();
    let source = "127.0.0.1:56565".parse().unwrap();
    for datagram in encode_data_frame(100_000, 1, &rows) {
        state.psn.observe(source, &datagram, 100);
    }
    let tick = state.psn.tick(100);
    super::psn::output::publish(&state, tick.tracking);
    let input = state.output.engine().tracking_frame();
    assert_eq!(input.points.len(), 1);
    assert_eq!(input.points[0].binding_id, independent);
    assert_eq!(input.points[0].fixture_id.0, independent_point);
    assert_eq!(input.points[0].position_metres, [5.0, 0.0, 0.0]);
    assert_eq!(store.objects("psn").unwrap()[0].body, raw);
    assert_eq!(store.portable_revision().unwrap(), revision);
    drop(store);
    drop(app);
    drop(state);

    // The real server startup loader must not turn optional malformed tracking into show recovery.
    let startup = startup_state::StartupState::load(startup_options::StartupOptions {
        data_dir: data_dir.clone(),
        show_file: None,
        fixture_package_dir: None,
        extensions_dir: Some(data_dir.join("extensions")),
        bind: "127.0.0.1:0".parse().unwrap(),
        test_bench: true,
        osc_bind_override: None,
        output_bind_override: None,
    })
    .unwrap();
    assert!(
        startup.active_show_error.is_none(),
        "{:?}",
        startup.active_show_error
    );
    assert_eq!(
        startup.persistent.active_show.as_ref().unwrap().id,
        entry.id
    );
    let reopened = ShowStore::open(&entry.path).unwrap();
    assert_eq!(reopened.objects("psn").unwrap()[0].body, raw);
    assert_eq!(reopened.portable_revision().unwrap(), revision);
}

#[tokio::test]
async fn older_binding_collisions_allow_receive_off_and_incremental_repair_but_never_new_collisions()
 {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let first = stored_binding(Uuid::new_v4(), 3, Uuid::new_v4());
    let second = stored_binding(Uuid::new_v4(), 4, Uuid::new_v4());
    let raw = serde_json::json!({
        "enabled": true, "bindings": [first.clone(), first.clone(), first, second.clone(), second],
        "future_tracking_field": {"preserve": true}
    });
    let entry = open_seeded_psn_show(&state, &app, &token, &raw).await;
    let off = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "safe-off", "enabled": false}),
    )
    .await;
    assert_eq!(off.status(), StatusCode::OK);
    assert!(!state.psn.configuration().enabled);
    assert_eq!(
        read_psn(&app, &token).await["status"]["diagnostics"]["conflicting_binding_rows"],
        5
    );
    let initial = read_psn(&app, &token).await;
    let mut bindings = initial["configuration"]["bindings"]
        .as_array()
        .unwrap()
        .clone();
    let removed = bindings.remove(0);
    let partial = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "partial-repair", "bindings": bindings}),
    )
    .await;
    assert_eq!(partial.status(), StatusCode::OK);
    let before = read_psn(&app, &token).await;
    assert_eq!(
        before["status"]["diagnostics"]["conflicting_binding_rows"],
        4
    );

    let mut increased = bindings.clone();
    increased.push(removed);
    let rejected = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "increase-collision", "bindings": increased}),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    let new_id = Uuid::new_v4();
    let new_row = serde_json::json!({"id": new_id, "tracker_id": 8, "point_fixture_id": Uuid::new_v4(), "enabled": true});
    let mut new_collision = bindings.clone();
    new_collision.extend([new_row.clone(), new_row]);
    let rejected = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "introduce-collision", "bindings": new_collision}),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    let after = read_psn(&app, &token).await;
    assert_eq!(after["revision"], before["revision"]);
    assert_eq!(after["configuration"], before["configuration"]);

    bindings.remove(0);
    let first_repaired = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "repair-one-id", "bindings": bindings}),
    )
    .await;
    assert_eq!(first_repaired.status(), StatusCode::OK);
    assert_eq!(
        read_psn(&app, &token).await["status"]["diagnostics"]["conflicting_binding_rows"],
        2
    );
    bindings.pop();
    let repaired = update_psn(
        &app,
        &token,
        &serde_json::json!({"request_id": "repair-all-ids", "bindings": bindings}),
    )
    .await;
    assert_eq!(repaired.status(), StatusCode::OK);
    state.psn.configuration().validate().unwrap();
    let snapshot = read_psn(&app, &token).await;
    assert_eq!(
        snapshot["status"]["diagnostics"]["conflicting_binding_rows"],
        0
    );
    assert!(snapshot["status"]["error"].is_null());
    let stored = ShowStore::open(&entry.path)
        .unwrap()
        .objects("psn")
        .unwrap()
        .remove(0)
        .body;
    assert_eq!(
        stored["future_tracking_field"],
        raw["future_tracking_field"]
    );
}
