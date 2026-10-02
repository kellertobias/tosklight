//! Actual action-path evidence for fresh equal-value operator reassertions.
//!
//! The direct capability tests in `capability_resources/output/transition_tests.rs` call
//! `apply_runtime_control` themselves. These tests instead drive the real HTTP routes and the
//! WebSocket live-action dispatcher, where the application service answers `NoChange` without
//! applying, and prove that a subsequent destination merge still preserves the reasserted fields.
use super::*;

const SAVED_REVISION: u64 = 50;

fn saved() -> PersistedOutputRuntime {
    PersistedOutputRuntime {
        revision: SAVED_REVISION,
        grand_master: 0.8,
        blackout: true,
        ..Default::default()
    }
}

struct Bench {
    state: AppState,
    app: Router,
    token: String,
    session: Session,
    data_dir: std::path::PathBuf,
}

impl Bench {
    /// Opens a show and installs one changed base write (grand master 0.4, blackout off).
    async fn new(name: &str) -> Self {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let session = authenticate_token(&state, &token).unwrap();
        let show = create_show(&app, &token, name).await;
        open_show_for_output_test(&app, &token, &show).await;
        let bench = Self {
            state,
            app,
            token,
            session,
            data_dir,
        };
        let initial = bench.snapshot().await;
        let changed = bench
            .post(output_request(
                "base-write",
                &initial,
                Some(0.4),
                Some(false),
            ))
            .await;
        assert_eq!(changed["status"], "changed");
        bench
    }

    async fn snapshot(&self) -> serde_json::Value {
        output_snapshot(&self.app, &self.token, self.session.desk.id).await
    }

    async fn post(&self, request: serde_json::Value) -> serde_json::Value {
        let response = post_output(&self.app, &self.token, self.session.desk.id, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        json(response).await
    }

    fn ws(&self, request_id: &str, request: serde_json::Value) -> WsResponse {
        dispatch_live_action(
            &self.state,
            &self.session,
            live_action_frame(
                &self.session,
                request_id,
                light_wire::v2::live_action::LiveAction::OutputRuntime(
                    serde_json::from_value(request).unwrap(),
                ),
            ),
        )
    }

    fn marks(&self) -> Marks {
        Marks {
            cursor: self.state.events.latest_sequence(),
            attempts: output_persistence_attempts(&self.state),
            revision: self.state.output.control_projection().revision,
        }
    }

    /// NoChange left values, output revision, change events and persistence attempts intact.
    fn assert_untouched_since(&self, marks: &Marks) {
        let base = self.state.output.control_projection();
        assert_eq!(base.revision, marks.revision);
        assert_eq!(base.grand_master, 0.4);
        assert!(!base.blackout);
        assert_eq!(output_events(&self.state, marks.cursor).len(), 0);
        assert_eq!(self.state.events.latest_sequence(), marks.cursor);
        assert_eq!(output_persistence_attempts(&self.state), marks.attempts);
    }

    /// Merges the saved destination and reports which fields kept operator intent.
    fn merge(&self, lease: OutputTransitionLease) -> (bool, bool, u64) {
        lease.restore_destination_control(&saved());
        drop(lease);
        let base = self.state.output.control_projection();
        let kept_master = base.grand_master == 0.4;
        let kept_blackout = !base.blackout;
        assert_eq!(base.grand_master, if kept_master { 0.4 } else { 0.8 });
        (kept_master, kept_blackout, base.revision)
    }

    fn finish(self) {
        let _ = std::fs::remove_dir_all(self.data_dir);
    }
}

struct Marks {
    cursor: u64,
    attempts: u64,
    revision: u64,
}

fn assert_no_change(payload: &serde_json::Value, revision: u64) {
    assert_eq!(payload["status"], "no_change", "{payload}");
    assert_eq!(payload["replayed"], false);
    assert!(payload.get("event_sequence").is_none());
    assert_eq!(payload["projection"]["revision"], revision);
}

#[tokio::test]
async fn fresh_equal_value_http_action_stamps_only_supplied_fields() {
    for (grand_master, blackout) in [
        (Some(0.4), None),
        (None, Some(false)),
        (Some(0.4), Some(false)),
    ] {
        let bench = Bench::new("HTTP equal-value reassert").await;
        let lease = bench.state.output.begin_transition_fade();
        let marks = bench.marks();
        let current = bench.snapshot().await;
        let payload = bench
            .post(output_request(
                "http-reassert",
                &current,
                grand_master,
                blackout,
            ))
            .await;
        assert_no_change(&payload, marks.revision);
        bench.assert_untouched_since(&marks);

        let (kept_master, kept_blackout, revision) = bench.merge(lease);
        assert_eq!(kept_master, grand_master.is_some());
        assert_eq!(kept_blackout, blackout.is_some());
        // The destination revision is installed only when both fields were restored.
        assert_eq!(revision, marks.revision);
        bench.finish();
    }
}

#[tokio::test]
async fn fresh_equal_value_websocket_action_stamps_only_supplied_fields() {
    for (grand_master, blackout) in [
        (Some(0.4), None),
        (None, Some(false)),
        (Some(0.4), Some(false)),
    ] {
        let bench = Bench::new("WS equal-value reassert").await;
        let lease = bench.state.output.begin_transition_fade();
        let marks = bench.marks();
        let current = bench.snapshot().await;
        let result = bench.ws(
            "ws-reassert",
            output_request("ws-reassert", &current, grand_master, blackout),
        );
        assert!(result.ok, "{:?}", result.error);
        assert_no_change(&result.payload.unwrap(), marks.revision);
        bench.assert_untouched_since(&marks);

        let (kept_master, kept_blackout, revision) = bench.merge(lease);
        assert_eq!(kept_master, grand_master.is_some());
        assert_eq!(kept_blackout, blackout.is_some());
        assert_eq!(revision, marks.revision);
        bench.finish();
    }
}

#[tokio::test]
async fn empty_legacy_http_command_stamps_nothing() {
    let bench = Bench::new("Empty reassert").await;
    let lease = bench.state.output.begin_transition_fade();
    let marks = bench.marks();
    let response = put_master(&bench.app, &bench.token, serde_json::json!({})).await;
    assert_eq!(response.status(), StatusCode::OK);
    bench.assert_untouched_since(&marks);

    // Neither field was reasserted, so the destination installs both and its revision.
    assert_eq!(bench.merge(lease), (false, false, SAVED_REVISION));
    bench.finish();
}

#[tokio::test]
async fn equal_value_legacy_http_command_stamps_only_supplied_fields() {
    let bench = Bench::new("Legacy reassert").await;
    let lease = bench.state.output.begin_transition_fade();
    let marks = bench.marks();
    let response = put_master(
        &bench.app,
        &bench.token,
        serde_json::json!({"blackout":false}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    bench.assert_untouched_since(&marks);
    assert_eq!(bench.merge(lease), (false, true, marks.revision));
    bench.finish();
}

#[tokio::test]
async fn cached_http_and_websocket_replays_do_not_stamp() {
    let bench = Bench::new("Replay reassert").await;
    let current = bench.snapshot().await;
    let http_request = output_request("http-replayed", &current, Some(0.4), Some(false));
    let ws_request = output_request("ws-replayed", &current, Some(0.4), Some(false));
    // Fresh NoChange outcomes are cached before the transition captures the write identities.
    assert_no_change(&bench.post(http_request.clone()).await, 1);
    let first = bench.ws("ws-replayed", ws_request.clone());
    assert!(first.ok, "{:?}", first.error);

    let lease = bench.state.output.begin_transition_fade();
    let marks = bench.marks();
    let replay = bench.post(http_request).await;
    assert_eq!(replay["status"], "no_change");
    assert_eq!(replay["replayed"], true);
    let replay = bench.ws("ws-replayed", ws_request);
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(replay.payload.unwrap()["replayed"], true);
    bench.assert_untouched_since(&marks);

    assert_eq!(bench.merge(lease), (false, false, SAVED_REVISION));
    bench.finish();
}

#[tokio::test]
async fn rejected_equal_value_actions_do_not_stamp() {
    let bench = Bench::new("Rejected reassert").await;
    let lease = bench.state.output.begin_transition_fade();
    let marks = bench.marks();
    let current = bench.snapshot().await;

    // Failed exact expectation over HTTP and WS.
    let mut stale = output_request("http-stale", &current, Some(0.4), Some(false));
    stale["expected_revision"] = serde_json::json!(0);
    let response = post_output(&bench.app, &bench.token, bench.session.desk.id, stale).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let mut stale = output_request("ws-stale", &current, Some(0.4), Some(false));
    stale["expected_show_id"] = serde_json::json!(Uuid::new_v4());
    assert!(!bench.ws("ws-stale", stale).ok);

    // Unauthenticated route and adapter authorization rejection.
    let response = post_output(
        &bench.app,
        "invalid-token",
        bench.session.desk.id,
        output_request("http-unauthorized", &current, Some(0.4), Some(false)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let foreign = light_application::ActionContext::operator(
        bench.session.desk.id,
        Uuid::new_v4(),
        light_application::ActionSource::Http,
    );
    let error = output_runtime_service::execute_action(
        &bench.state,
        Some(&bench.session),
        foreign,
        light_application::OutputRuntimeCommand::new(
            light_application::OutputLevel::new(0.4),
            Some(false),
        ),
    )
    .unwrap_err();
    assert_eq!(error.kind, light_application::ActionErrorKind::Unauthorized);

    // Desk-lock rejection precedes the service.
    write_desk_lock(
        &bench.state,
        &DeskLockConfiguration {
            locked: true,
            ..DeskLockConfiguration::default()
        },
    )
    .unwrap();
    let locked = bench.ws(
        "ws-locked",
        output_request("ws-locked", &current, Some(0.4), Some(false)),
    );
    assert_eq!(locked.error.as_deref(), Some("desk is locked"));
    bench.assert_untouched_since(&marks);

    assert_eq!(bench.merge(lease), (false, false, SAVED_REVISION));
    bench.finish();
}

#[tokio::test]
async fn changed_value_actions_keep_revisioned_apply_semantics_under_a_transition() {
    let bench = Bench::new("Changed under transition").await;
    let lease = bench.state.output.begin_transition_fade();
    let marks = bench.marks();
    let current = bench.snapshot().await;
    let changed = bench
        .post(output_request("http-changed", &current, Some(0.6), None))
        .await;
    assert_eq!(changed["status"], "changed");
    assert_eq!(changed["projection"]["revision"], marks.revision + 1);
    assert_eq!(output_events(&bench.state, marks.cursor).len(), 1);
    assert_eq!(
        output_persistence_attempts(&bench.state),
        marks.attempts + 1
    );

    lease.restore_destination_control(&saved());
    drop(lease);
    let base = bench.state.output.control_projection();
    assert_eq!(base.grand_master, 0.6);
    assert!(base.blackout);
    assert_eq!(base.revision, marks.revision + 1);
    bench.finish();
}
