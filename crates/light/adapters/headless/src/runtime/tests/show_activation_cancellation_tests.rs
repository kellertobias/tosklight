use super::*;

type ActivationProbe = crate::runtime::active_show_adapter::ActiveShowLifecyclePause;

/// Release blocking worker probes even if an assertion unwinds the test.
pub(super) struct ArmedProbe(Arc<ActivationProbe>);

impl ArmedProbe {
    pub(super) fn new(probe: Arc<ActivationProbe>) -> Self {
        probe.arm();
        Self(probe)
    }

    pub(super) async fn reached(&self) {
        let probe = Arc::clone(&self.0);
        tokio::task::spawn_blocking(move || probe.wait_until_started())
            .await
            .unwrap();
    }

    pub(super) fn release(&self) {
        self.0.release();
    }
}

impl Drop for ArmedProbe {
    fn drop(&mut self) {
        self.0.release();
    }
}

pub(super) async fn activation_route_fixture()
-> (AppState, Router, String, String, String, std::path::PathBuf) {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Activation cancellation").await;
    let prior = create_show(&app, &token, "Before cancellation").await;
    let destination = create_show(&app, &token, "After admission").await;
    let prior = prior["id"].as_str().unwrap().to_owned();
    let destination = destination["id"].as_str().unwrap().to_owned();
    let response = app
        .clone()
        .oneshot(open_show_request(&token, &prior))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(state.active_show.current().unwrap().id.0.to_string(), prior);
    assert_eq!(
        state
            .installation
            .active_show()
            .unwrap()
            .unwrap()
            .id
            .0
            .to_string(),
        prior
    );
    (state, app, token, prior, destination, data_dir)
}

pub(super) fn opened_events(state: &AppState, destination: &str) -> usize {
    state
        .events
        .audit_events()
        .iter()
        .filter(|event| event.kind == "show_opened" && event.payload["show"]["id"] == destination)
        .count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_open_before_admission_cannot_publish_or_change_durable_identity() {
    let (state, app, token, prior, destination, data_dir) = activation_route_fixture().await;
    let before = ArmedProbe::new(state.active_show.activation_before_admission_probe());
    let completed = ArmedProbe::new(state.active_show.activation_completed_probe());
    let snapshot = state.output.snapshot();
    let previous = state
        .installation
        .setting("previous_active_show_id")
        .unwrap();
    let revision = state
        .output
        .apply_runtime_control(Some(0.42), Some(true))
        .unwrap();
    let request = open_show_with_transition_request(&token, &destination, "timed_fade", Some(100));
    let opening = tokio::spawn(async move { app.oneshot(request).await.unwrap() });
    before.reached().await;

    // Worker is paused immediately before its Pending -> Committing CAS. Request Drop wins.
    opening.abort();
    assert!(opening.await.unwrap_err().is_cancelled());
    assert!(Arc::ptr_eq(&snapshot, &state.output.snapshot()));
    assert_eq!(state.active_show.current().unwrap().id.0.to_string(), prior);
    assert_eq!(
        state
            .installation
            .active_show()
            .unwrap()
            .unwrap()
            .id
            .0
            .to_string(),
        prior
    );
    assert_eq!(
        state
            .installation
            .setting("previous_active_show_id")
            .unwrap(),
        previous
    );
    assert_eq!(opened_events(&state, &destination), 0);
    // Cancellation marks admission; the owned workflow still owns transition cleanup.
    before.release();
    completed.reached().await;
    completed.release();
    // Reacquiring show-change proves the owned workflow joined its worker and cleaned up.
    let show_change = tokio::time::timeout(
        Duration::from_secs(5),
        state.active_show.acquire_show_change(),
    )
    .await
    .expect("cancelled activation worker did not finish");
    assert_eq!(state.output.render_options().grand_master, 0.42);
    assert!(state.output.render_options().blackout);
    assert_eq!(state.output.control_projection().revision, revision);
    assert!(Arc::ptr_eq(&snapshot, &state.output.snapshot()));
    assert_eq!(
        state
            .installation
            .active_show()
            .unwrap()
            .unwrap()
            .id
            .0
            .to_string(),
        prior
    );
    assert_eq!(opened_events(&state, &destination), 0);
    drop(show_change);
    drop(completed);
    drop(before);
    drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_open_after_admission_completes_durable_identity_and_event_tail_once() {
    let (state, app, token, prior, destination, data_dir) = activation_route_fixture().await;
    let admitted = ArmedProbe::new(state.active_show.activation_after_admission_probe());
    let completed = ArmedProbe::new(state.active_show.activation_completed_probe());
    let before = state.output.snapshot();
    let request = open_show_request(&token, &destination);
    let opening = tokio::spawn(async move { app.oneshot(request).await.unwrap() });
    admitted.reached().await;

    // Worker has won Pending -> Committing and taken ownership, but has not installed.
    assert!(Arc::ptr_eq(&before, &state.output.snapshot()));
    assert_eq!(state.active_show.current().unwrap().id.0.to_string(), prior);
    opening.abort();
    assert!(opening.await.unwrap_err().is_cancelled());
    assert_eq!(opened_events(&state, &destination), 0);
    admitted.release();

    // This barrier is after the owned durable receipt, coherent identity and audit-event tail.
    completed.reached().await;
    assert!(!Arc::ptr_eq(&before, &state.output.snapshot()));
    assert_eq!(
        state.active_show.current().unwrap().id.0.to_string(),
        destination
    );
    let durable = state.installation.active_show().unwrap().unwrap();
    assert_eq!(durable.id.0.to_string(), destination);
    assert!(durable.last_loaded_at.is_some());
    assert_eq!(
        state
            .installation
            .setting("previous_active_show_id")
            .unwrap(),
        Some(prior.clone())
    );
    assert_eq!(opened_events(&state, &destination), 1);
    let event = state
        .events
        .audit_events()
        .into_iter()
        .find(|event| event.kind == "show_opened" && event.payload["show"]["id"] == destination)
        .unwrap();
    assert_eq!(event.payload["previous_show"]["id"], prior);
    assert_eq!(event.payload["transition"], "hold_current");

    completed.release();
    // Workflow owns this guard through the post-commit transition and lease cleanup.
    let show_change = tokio::time::timeout(
        Duration::from_secs(5),
        state.active_show.acquire_show_change(),
    )
    .await
    .expect("admitted activation worker did not finish");
    assert_eq!(
        state.active_show.current().unwrap().id.0.to_string(),
        destination
    );
    assert_eq!(
        state
            .installation
            .active_show()
            .unwrap()
            .unwrap()
            .id
            .0
            .to_string(),
        destination
    );
    assert_eq!(opened_events(&state, &destination), 1);
    drop(show_change);
    drop(completed);
    drop(admitted);
    drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timed_fade_rejects_changed_retained_programmer_without_metadata_or_event() {
    let (state, app, token, prior, destination, data_dir) = activation_route_fixture().await;
    let (editor_token, session_id) = login(&app, "Programmer authority edit").await;
    let session_id = SessionId(Uuid::parse_str(&session_id).unwrap());
    let before = ArmedProbe::new(state.active_show.activation_before_admission_probe());
    let snapshot = state.output.snapshot();
    let checkpoint = state.output.dynamic_source_checkpoint().unwrap();
    let previous = state
        .installation
        .setting("previous_active_show_id")
        .unwrap();
    let destination_id = light_core::ShowId(Uuid::parse_str(&destination).unwrap());
    let destination_before = state.installation.show(destination_id).unwrap().unwrap();
    let revision = state
        .output
        .apply_runtime_control(Some(0.41), Some(false))
        .unwrap();
    let request =
        open_show_with_transition_request(&editor_token, &destination, "timed_fade", Some(100));
    let opening = tokio::spawn(async move { app.oneshot(request).await.unwrap() });
    before.reached().await;

    // Preparation captured retained authority before transition waiting. No activation or
    // Programmer gate may be held at this probe; a genuine desk edit must remain possible.
    let mut programmer = state.programming.get(session_id).unwrap();
    programmer.priority += 1;
    let new_priority = programmer.priority;
    state.programming.restore(programmer);
    before.release();
    let response = tokio::time::timeout(Duration::from_secs(5), opening)
        .await
        .unwrap()
        .unwrap();
    assert!(!response.status().is_success());
    let error = json(response).await;
    assert!(error.to_string().contains("authority is stale"), "{error}");
    assert!(Arc::ptr_eq(&snapshot, &state.output.snapshot()));
    assert_eq!(
        state.output.dynamic_source_checkpoint().unwrap(),
        checkpoint
    );
    assert_eq!(
        state.programming.get(session_id).unwrap().priority,
        new_priority
    );
    assert_eq!(state.active_show.current().unwrap().id.0.to_string(), prior);
    assert_eq!(
        state
            .installation
            .active_show()
            .unwrap()
            .unwrap()
            .id
            .0
            .to_string(),
        prior
    );
    assert_eq!(
        state
            .installation
            .setting("previous_active_show_id")
            .unwrap(),
        previous
    );
    assert_eq!(
        state
            .installation
            .show(destination_id)
            .unwrap()
            .unwrap()
            .last_loaded_at,
        destination_before.last_loaded_at
    );
    assert_eq!(opened_events(&state, &destination), 0);
    assert_eq!(state.output.render_options().grand_master, 0.41);
    assert!(!state.output.render_options().blackout);
    assert_eq!(state.output.control_projection().revision, revision);
    drop(before);
    drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
    drop(token);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_metadata_failure_rolls_back_durable_identity_and_preserves_live_runtime() {
    let (state, app, token, prior, destination, data_dir) = activation_route_fixture().await;
    let destination_id = light_core::ShowId(Uuid::parse_str(&destination).unwrap());
    let destination_before = state.installation.show(destination_id).unwrap().unwrap();
    let snapshot = state.output.snapshot();
    let checkpoint = state.output.dynamic_source_checkpoint().unwrap();
    let previous = state
        .installation
        .setting("previous_active_show_id")
        .unwrap();
    let revision = state
        .output
        .apply_runtime_control(Some(0.63), Some(false))
        .unwrap();
    let database = rusqlite::Connection::open(data_dir.join("desk.sqlite")).unwrap();
    database
        .execute_batch(
            "CREATE TRIGGER reject_activation_previous BEFORE INSERT ON settings
         WHEN NEW.key='previous_active_show_id'
         BEGIN SELECT RAISE(ABORT, 'injected activation metadata failure'); END;",
        )
        .unwrap();

    let response = app
        .clone()
        .oneshot(open_show_with_transition_request(
            &token,
            &destination,
            "safe_blackout",
            None,
        ))
        .await
        .unwrap();
    assert!(!response.status().is_success());
    let error = json(response).await;
    assert!(
        error
            .to_string()
            .contains("injected activation metadata failure"),
        "{error}"
    );
    assert!(Arc::ptr_eq(&snapshot, &state.output.snapshot()));
    assert_eq!(
        state.output.dynamic_source_checkpoint().unwrap(),
        checkpoint
    );
    assert_eq!(state.active_show.current().unwrap().id.0.to_string(), prior);
    assert_eq!(
        state
            .installation
            .active_show()
            .unwrap()
            .unwrap()
            .id
            .0
            .to_string(),
        prior
    );
    assert_eq!(
        state
            .installation
            .setting("previous_active_show_id")
            .unwrap(),
        previous
    );
    assert_eq!(
        state
            .installation
            .show(destination_id)
            .unwrap()
            .unwrap()
            .last_loaded_at,
        destination_before.last_loaded_at
    );
    assert_eq!(opened_events(&state, &destination), 0);
    assert_eq!(state.output.render_options().grand_master, 0.63);
    assert!(!state.output.render_options().blackout);
    assert_eq!(state.output.control_projection().revision, revision);
    // Demonstrate failure also released ownership, so a following legitimate open succeeds.
    database
        .execute_batch("DROP TRIGGER reject_activation_previous")
        .unwrap();
    drop(database);
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        app.oneshot(open_show_request(&token, &destination)),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        state.active_show.current().unwrap().id.0.to_string(),
        destination
    );
    assert_eq!(opened_events(&state, &destination), 1);
    drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
}

pub(super) async fn activation_output_snapshot(
    app: &Router,
    token: &str,
    desk: Uuid,
) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/output-runtime/global-master")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-desk", desk.to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn activation_output_action(
    app: &Router,
    token: &str,
    desk: Uuid,
    snapshot: &serde_json::Value,
    master: Option<f32>,
    blackout: Option<bool>,
) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/output-runtime/global-master")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-desk", desk.to_string())
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": Uuid::new_v4().to_string(),
                        "expected_show_id": snapshot["projection"]["scope"]["show_id"],
                        "expected_revision": snapshot["projection"]["revision"],
                        "grand_master": master,
                        "blackout": blackout,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timed_fade_keeps_rendering_and_real_operator_writes_while_restoring_private_group_masters()
{
    let (state, app, token, prior, destination, data_dir) = activation_route_fixture().await;
    let session = authenticate_token(&state, &token).unwrap();
    let destination_id = light_core::ShowId(Uuid::parse_str(&destination).unwrap());
    let entry = state.installation.show(destination_id).unwrap().unwrap();
    let store = light_show::ShowStore::open(&entry.path).unwrap();
    for id in ["front", "unassigned"] {
        store
            .put_object(
                "group",
                id,
                &serde_json::to_value(light_programmer::GroupDefinition {
                    id: id.into(),
                    ..Default::default()
                })
                .unwrap(),
                0,
            )
            .unwrap();
    }
    let target = light_playback::PlaybackTarget::Group {
        group_id: "front".into(),
        initial_master: Some(0.25),
    };
    let playback = light_playback::PlaybackDefinition {
        number: 1,
        name: "Front master".into(),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        button_count: 3,
        fader: light_playback::PlaybackDefinition::default_fader(&target),
        has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
        target,
    };
    store
        .put_object("playback", "1", &serde_json::to_value(playback).unwrap(), 0)
        .unwrap();
    drop(store);
    state
        .installation
        .set_setting(
            &output_runtime_setting(destination_id),
            &serde_json::to_string(&PersistedOutputRuntime {
                revision: 50,
                grand_master: 0.8,
                blackout: false,
                group_masters: HashMap::from([("front".into(), 0.65), ("unassigned".into(), 0.1)]),
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
    let initial = activation_output_snapshot(&app, &token, session.desk.id).await;
    let base = activation_output_action(
        &app,
        &token,
        session.desk.id,
        &initial,
        Some(0.4),
        Some(false),
    )
    .await;
    assert_eq!(base["status"], "changed");
    let base_revision = base["projection"]["revision"].as_u64().unwrap();
    let old_snapshot = state.output.snapshot();
    // This hook is after fade-out and before exclusive activation acquisition. The fade
    // lease and show-change workflow still exist, but render and operator actions can run.
    let fading = ArmedProbe::new(state.active_show.activation_before_commit_probe());
    let request = open_show_with_transition_request(&token, &destination, "timed_fade", Some(100));
    let opening_app = app.clone();
    let opening = tokio::spawn(async move { opening_app.oneshot(request).await.unwrap() });
    fading.reached().await;
    assert!(!opening.is_finished());
    assert!(Arc::ptr_eq(&old_snapshot, &state.output.snapshot()));
    assert_eq!(state.active_show.current().unwrap().id.0.to_string(), prior);
    assert_eq!(
        state.output.group_master("front"),
        None,
        "destination group master must stay private before commit"
    );

    let current = tokio::time::timeout(
        Duration::from_secs(2),
        activation_output_snapshot(&app, &token, session.desk.id),
    )
    .await
    .expect("output snapshot blocked during pending fade");
    let reassert = tokio::time::timeout(
        Duration::from_secs(2),
        activation_output_action(&app, &token, session.desk.id, &current, Some(0.4), None),
    )
    .await
    .expect("equal-value output action blocked during pending fade");
    assert_eq!(reassert["status"], "no_change");
    assert_eq!(reassert["replayed"], false);
    assert_eq!(reassert["projection"]["revision"], base_revision);
    let changed = tokio::time::timeout(
        Duration::from_secs(2),
        activation_output_action(&app, &token, session.desk.id, &reassert, None, Some(true)),
    )
    .await
    .expect("changed output action blocked during pending fade");
    assert_eq!(changed["status"], "changed");
    assert_eq!(changed["projection"]["revision"], base_revision + 1);

    let render_state = state.clone();
    let rendering = tokio::spawn(async move {
        let _permit = render_state.active_show.acquire_shared().await;
        let scope = light_wire::v2::visualization::VisualizationScope {
            show_id: Some(render_state.active_show.current().unwrap().id.0),
        };
        let mut sequence = None;
        for _ in 0..2 {
            let frame = render_state
                .output
                .render_with_playback_events(
                    &render_state.active_show.output_projection(),
                    &render_state.playback.render_capability(),
                    render_state.output.render_options(),
                )
                .unwrap();
            render_state.output.render_frames_and_publish(&frame, scope);
            let published = render_state.output.latest_visualization_frame().unwrap();
            if let Some(previous) = sequence {
                assert!(published.sequence > previous);
            }
            sequence = Some(published.sequence);
        }
    });
    tokio::time::timeout(Duration::from_secs(2), rendering)
        .await
        .expect("activation excluded render while fade was pending")
        .unwrap();
    assert!(!opening.is_finished());
    fading.release();
    let response = tokio::time::timeout(Duration::from_secs(5), opening)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(state.active_show.current().unwrap().id, destination_id);
    let final_controls = state.output.control_projection();
    assert_eq!(
        final_controls.grand_master, 0.4,
        "fresh equal-value HTTP reassert must override saved destination master 0.8"
    );
    assert!(
        final_controls.blackout,
        "intervening changed blackout must override saved destination blackout false"
    );
    assert_eq!(final_controls.revision, base_revision + 1);
    assert_eq!(state.output.render_options().grand_master, 0.4);
    assert!(state.output.render_options().blackout);
    assert_eq!(state.output.group_master("front"), Some(0.65));
    assert_eq!(state.output.group_master("unassigned"), None);
    assert_eq!(opened_events(&state, &destination), 1);
    drop(fading);
    drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_admitted_v2_rollback_replays_receipt_without_rolling_forward_again() {
    let (state, app, token, first, second, data_dir) = activation_route_fixture().await;
    let response = app
        .clone()
        .oneshot(open_show_request(&token, &second))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        state
            .installation
            .setting("previous_active_show_id")
            .unwrap(),
        Some(first.clone())
    );
    let admitted = ArmedProbe::new(state.active_show.activation_after_admission_probe());
    let completed = ArmedProbe::new(state.active_show.activation_completed_probe());
    let library_before = state.installation.show_library().unwrap().len();
    let rollback_events_before = state
        .events
        .audit_events()
        .into_iter()
        .filter(|event| event.kind == "show_rolled_back")
        .count();
    let request_id = "cancelled-admitted-rollback-retry";
    let body = serde_json::json!({
        "request_id": request_id,
        "action": {
            "type": "rollback",
            "transition": "hold_current",
            "transition_millis": null
        }
    });
    let request = || {
        Request::post("/api/v2/shows")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let first_request = request();
    let first_app = app.clone();
    let original = tokio::spawn(async move { first_app.oneshot(first_request).await.unwrap() });
    tokio::time::timeout(Duration::from_secs(5), admitted.reached())
        .await
        .expect("rollback never reached commit admission");
    original.abort();
    assert!(original.await.unwrap_err().is_cancelled());
    admitted.release();
    tokio::time::timeout(Duration::from_secs(5), completed.reached())
        .await
        .expect("admitted cancelled rollback did not finish its worker");
    assert_eq!(state.active_show.current().unwrap().id.0.to_string(), first);
    assert_eq!(
        state
            .installation
            .setting("previous_active_show_id")
            .unwrap(),
        Some(second.clone())
    );
    let committed = state.output.snapshot();
    // Retry while the original owned workflow is still completing. It must wait for
    // the original receipt, not enter another rollback after show-change is released.
    let retry_request = request();
    let retry_app = app.clone();
    let retry = tokio::spawn(async move { retry_app.oneshot(retry_request).await.unwrap() });
    completed.release();
    let response = tokio::time::timeout(Duration::from_secs(5), retry)
        .await
        .expect("same-ID retry did not receive the original activation receipt")
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let replay = json(response).await;
    assert_eq!(replay["request_id"], request_id);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["result"]["show"]["id"], first);
    assert!(
        Arc::ptr_eq(&committed, &state.output.snapshot()),
        "replaying an admitted activation must not reinstall any runtime"
    );
    assert_eq!(state.active_show.current().unwrap().id.0.to_string(), first);
    assert_eq!(
        state
            .installation
            .active_show()
            .unwrap()
            .unwrap()
            .id
            .0
            .to_string(),
        first
    );
    assert_eq!(
        state
            .installation
            .setting("previous_active_show_id")
            .unwrap(),
        Some(second)
    );
    assert_eq!(
        state.installation.show_library().unwrap().len(),
        library_before
    );
    assert_eq!(
        state
            .events
            .audit_events()
            .into_iter()
            .filter(|event| event.kind == "show_rolled_back")
            .count(),
        rollback_events_before + 1
    );
    drop(completed);
    drop(admitted);
    drop(app);
    drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
}

// ---- TL-589: after-swap cancellation, real frames, stale authority and replay intent ----

use super::show_activation_caller_tests::{join_owned_workflow, show_change_is_held};

/// Renders and publishes one real frame under a shared activation permit. Timing out proves
/// activation exclusion blocked rendering, which transitions must never do outside the swap.
async fn render_published_frame(
    state: &AppState,
) -> Arc<crate::runtime::visualization_frame::PublishedVisualizationFrame> {
    let render_state = state.clone();
    let rendering = tokio::spawn(async move {
        let _permit = render_state.active_show.acquire_shared().await;
        let scope = light_wire::v2::visualization::VisualizationScope {
            show_id: Some(render_state.active_show.current().unwrap().id.0),
        };
        let frame = render_state
            .output
            .render_with_playback_events(
                &render_state.active_show.output_projection(),
                &render_state.playback.render_capability(),
                render_state.output.render_options(),
            )
            .unwrap();
        render_state.output.render_frames_and_publish(&frame, scope);
        render_state.output.latest_visualization_frame().unwrap()
    });
    tokio::time::timeout(Duration::from_secs(2), rendering)
        .await
        .expect("rendering was excluded during the activation transition")
        .unwrap()
}

fn rolled_back_events(state: &AppState) -> usize {
    state
        .events
        .audit_events()
        .iter()
        .filter(|event| event.kind == "show_rolled_back")
        .count()
}

fn rollback_request_with_id(token: &str, request_id: &str) -> Request<Body> {
    Request::post("/api/v2/shows")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(
            serde_json::json!({
                "request_id": request_id,
                "action": {"type": "rollback", "transition": "timed_fade", "transition_millis": 100}
            })
            .to_string(),
        ))
        .unwrap()
}

async fn output_action_with_id(
    app: &Router,
    token: &str,
    desk: Uuid,
    snapshot: &serde_json::Value,
    request_id: &str,
    master: Option<f32>,
    blackout: Option<bool>,
) -> serde_json::Value {
    let body = serde_json::json!({
        "request_id": request_id,
        "expected_show_id": snapshot["projection"]["scope"]["show_id"],
        "expected_revision": snapshot["projection"]["revision"],
        "grand_master": master,
        "blackout": blackout,
    });
    let request = Request::post("/api/v2/output-runtime/global-master")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("x-tosk-desk", desk.to_string())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(2), app.clone().oneshot(request))
        .await
        .expect("output action blocked during the paused fade")
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_after_coherent_swap_keeps_fade_cleanup_owned_and_renders_through_it() {
    let (state, app, token, prior, destination, data_dir) = activation_route_fixture().await;
    let destination_id = Uuid::parse_str(&destination).unwrap();
    state
        .output
        .apply_runtime_control(Some(0.7), Some(false))
        .unwrap();
    let outgoing = render_published_frame(&state).await;
    assert_eq!(outgoing.options.grand_master, 0.7);
    let completed = ArmedProbe::new(state.active_show.activation_completed_probe());
    let request = open_show_with_transition_request(&token, &destination, "timed_fade", Some(100));
    let opening_app = app.clone();
    let opening = tokio::spawn(async move { opening_app.oneshot(request).await.unwrap() });
    completed.reached().await;

    // Coherent swap, durable metadata and event are complete; the request is dropped now.
    opening.abort();
    assert!(opening.await.unwrap_err().is_cancelled());
    assert_eq!(state.active_show.current().unwrap().id.0, destination_id);
    let durable = state.installation.active_show().unwrap().unwrap();
    assert_eq!(durable.id.0, destination_id);
    let previous = state.installation.setting("previous_active_show_id");
    assert_eq!(previous.unwrap(), Some(prior.clone()));
    assert_eq!(opened_events(&state, &destination), 1);
    // Fade-in has not begun: the owned overlay still darkens output and holds show-change.
    assert_eq!(state.output.render_options().grand_master, 0.0);
    assert!(show_change_is_held(&state));
    let during = render_published_frame(&state).await;
    assert!(during.sequence > outgoing.sequence);
    assert_eq!(during.scope.show_id, Some(destination_id));
    assert_eq!(during.options.grand_master, 0.0, "fade overlay must apply");

    completed.release();
    let show_change = join_owned_workflow(&state).await;
    let controls = state.output.control_projection();
    assert!(controls.grand_master > 0.0);
    assert_eq!(
        state.output.render_options().grand_master,
        controls.grand_master
    );
    let after = render_published_frame(&state).await;
    assert!(after.sequence > during.sequence);
    assert_eq!(after.scope.show_id, Some(destination_id));
    assert_eq!(after.options.grand_master, controls.grand_master);
    assert_eq!(after.options.blackout, controls.blackout);
    assert_eq!(opened_events(&state, &destination), 1);
    drop(show_change);
    drop(completed);
    drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Opens the second show so that Rollback returns to the first (fixture `prior`).
async fn rollback_fixture() -> (AppState, Router, String, String, String, PathBuf, Uuid) {
    let (state, app, token, prior, second, data_dir) = activation_route_fixture().await;
    let response = app
        .clone()
        .oneshot(open_show_request(&token, &second))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let desk = authenticate_token(&state, &token).unwrap().desk.id;
    (state, app, token, prior, second, data_dir, desk)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_http_priority_write_during_paused_rollback_fade_keeps_live_and_operator_controls() {
    let (state, app, token, prior, second, data_dir, desk) = rollback_fixture().await;
    let second_id = Uuid::parse_str(&second).unwrap();
    let snapshot = state.output.snapshot();
    let checkpoint = state.output.dynamic_source_checkpoint().unwrap();
    let base = activation_output_snapshot(&app, &token, desk).await;
    let rollbacks = rolled_back_events(&state);
    let fading = ArmedProbe::new(state.active_show.activation_before_commit_probe());
    let opening_app = app.clone();
    let request = rollback_request_with_id(&token, "tl589-stale-rollback");
    let opening = tokio::spawn(async move { opening_app.oneshot(request).await.unwrap() });
    fading.reached().await;

    // Real frame while the fade is paused at full fade-out, before activation exclusion.
    let frame = render_published_frame(&state).await;
    assert_eq!(frame.scope.show_id, Some(second_id));
    assert_eq!(frame.options.grand_master, 0.0);
    // Real operator writes: Programmer priority (retained authority) and a changed blackout.
    let priority = app
        .clone()
        .oneshot(
            Request::post("/api/v2/programmer/priority/actions")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id": "tl589-priority", "expected_revision": 0, "priority": 71})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(priority.status(), StatusCode::OK);
    let blackout = output_action_with_id(
        &app,
        &token,
        desk,
        &base,
        "tl589-blackout",
        None,
        Some(true),
    )
    .await;
    assert_eq!(blackout["status"], "changed");
    fading.release();
    let response = tokio::time::timeout(Duration::from_secs(5), opening)
        .await
        .unwrap()
        .unwrap();
    assert!(!response.status().is_success());
    let error = json(response).await;
    assert!(error.to_string().contains("authority is stale"), "{error}");
    let show_change = join_owned_workflow(&state).await;
    assert!(Arc::ptr_eq(&snapshot, &state.output.snapshot()));
    assert_eq!(
        state.output.dynamic_source_checkpoint().unwrap(),
        checkpoint
    );
    assert_eq!(state.active_show.current().unwrap().id.0, second_id);
    assert_eq!(
        state.installation.active_show().unwrap().unwrap().id.0,
        second_id
    );
    let previous = state.installation.setting("previous_active_show_id");
    assert_eq!(previous.unwrap(), Some(prior.clone()));
    assert_eq!(rolled_back_events(&state), rollbacks);
    let controls = state.output.control_projection();
    assert!(
        controls.blackout,
        "intervening operator blackout must survive"
    );
    assert_eq!(
        controls.revision,
        blackout["projection"]["revision"].as_u64().unwrap()
    );
    let options = state.output.render_options();
    assert_eq!(
        options.grand_master, controls.grand_master,
        "fade overlay leaked"
    );
    assert!(options.blackout);
    drop(show_change);

    // The rejected attempt was not cached; the same request ID now commits exactly once.
    let retry = app
        .clone()
        .oneshot(rollback_request_with_id(&token, "tl589-stale-rollback"))
        .await
        .unwrap();
    assert_eq!(retry.status(), StatusCode::OK);
    assert_eq!(json(retry).await["replayed"], false);
    assert_eq!(state.active_show.current().unwrap().id.0.to_string(), prior);
    let previous = state.installation.setting("previous_active_show_id");
    assert_eq!(previous.unwrap(), Some(second));
    assert_eq!(rolled_back_events(&state), rollbacks + 1);
    drop(fading);
    drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rollback_fade_restores_controls_only_where_no_fresh_intent_including_cached_replay() {
    let (state, app, token, prior, _second, data_dir, desk) = rollback_fixture().await;
    let prior_id = light_core::ShowId(Uuid::parse_str(&prior).unwrap());
    state
        .installation
        .set_setting(
            &output_runtime_setting(prior_id),
            &serde_json::to_string(&PersistedOutputRuntime {
                revision: 50,
                grand_master: 0.8,
                blackout: true,
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
    let initial = activation_output_snapshot(&app, &token, desk).await;
    let cached = output_action_with_id(
        &app,
        &token,
        desk,
        &initial,
        "tl589-cached",
        Some(0.4),
        None,
    )
    .await;
    assert_eq!(cached["status"], "changed");
    let cached_revision = cached["projection"]["revision"].as_u64().unwrap();
    let rollbacks = rolled_back_events(&state);
    let fading = ArmedProbe::new(state.active_show.activation_before_commit_probe());
    let opening_app = app.clone();
    let request = rollback_request_with_id(&token, "tl589-restoring-rollback");
    let opening = tokio::spawn(async move { opening_app.oneshot(request).await.unwrap() });
    fading.reached().await;

    let frame = render_published_frame(&state).await;
    assert_eq!(
        frame.options.grand_master, 0.0,
        "real frame renders the fade"
    );
    // Cached replay of the pre-fade master write is not a new operator intent.
    let replay = output_action_with_id(
        &app,
        &token,
        desk,
        &initial,
        "tl589-cached",
        Some(0.4),
        None,
    )
    .await;
    assert_eq!(replay["replayed"], true);
    // A fresh equal-value blackout reassert is a new intent even though nothing changes.
    let current = activation_output_snapshot(&app, &token, desk).await;
    let fresh = output_action_with_id(
        &app,
        &token,
        desk,
        &current,
        "tl589-fresh",
        None,
        Some(false),
    )
    .await;
    assert_eq!(fresh["status"], "no_change");
    assert_eq!(fresh["replayed"], false);
    assert_eq!(fresh["projection"]["revision"], cached_revision);
    fading.release();
    let response = tokio::time::timeout(Duration::from_secs(5), opening)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let show_change = join_owned_workflow(&state).await;
    assert_eq!(state.active_show.current().unwrap().id, prior_id);
    assert_eq!(rolled_back_events(&state), rollbacks + 1);
    let controls = state.output.control_projection();
    assert_eq!(
        controls.grand_master, 0.8,
        "a cached replay must not block the destination master"
    );
    assert!(
        !controls.blackout,
        "a fresh equal-value blackout must override the destination blackout"
    );
    assert_eq!(
        controls.revision, cached_revision,
        "partial restore keeps revision"
    );
    let options = state.output.render_options();
    assert_eq!((options.grand_master, options.blackout), (0.8, false));
    drop(show_change);
    drop(fading);
    drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
}
