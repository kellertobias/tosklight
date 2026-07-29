use super::*;
use crate::tolerant_json::TolerantJson;

pub(super) fn fixed_test_time() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
        .expect("fixed test timestamp is valid")
        .with_timezone(&chrono::Utc)
}

pub(super) async fn reset_test_clock(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let clock = state.output.acquire_test_clock().await?;
    clock.set(fixed_test_time());
    state.programming.reset_all();
    state.output.clear_programmer_transitions();
    state.output.clear_sequences().await;
    state.integrations.clear_osc_subscribers();
    {
        let configuration = state.installation.configuration();
        state.output.reset_speed_groups(
            configuration.speed_groups_bpm,
            configuration.speed_group_sound_to_light,
        );
    }
    refresh_speed_group_engine(&state);
    emit(
        &state,
        "hardware_connection_changed",
        serde_json::json!({"connected":false}),
    );
    Ok(Json(serde_json::json!({"now":clock.now()})))
}

#[derive(Deserialize)]
pub(super) struct AdvanceTestClock {
    #[serde(default)]
    pub(super) millis: i64,
}

pub(super) async fn advance_test_clock(
    State(state): State<AppState>,
    Json(input): Json<AdvanceTestClock>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !(0..=604_800_000).contains(&input.millis) {
        return Err(ApiError::bad_request("millis must be within 0-604800000"));
    }
    let clock = state.output.acquire_test_clock().await?;
    let now = clock.advance_millis(input.millis);
    refresh_speed_group_engine(&state);
    let action_timing = state.action_timing.begin_output_render();
    let (rendered, visualization_scope) = {
        let _activation = state.active_show.acquire().await;
        let visualization_scope = light_wire::v2::visualization::VisualizationScope {
            show_id: state.active_show.current().map(|show| show.id.0),
        };
        let playback = state.playback.render_capability();
        let rendered = state
            .output
            .render_with_playback_events(
                &state.active_show.output_projection(),
                &playback,
                state.output.render_options(),
            )
            .map_err(|error| ApiError::internal(error.to_string()))?;
        (rendered, visualization_scope)
    };
    let frames = state
        .output
        .render_frames_and_publish(&rendered, visualization_scope);
    let snapshot = state.output.snapshot();
    let packets = state
        .output
        .send_network_routes(&snapshot.routes, &frames, &rendered.patched_slots)
        .await
        .map_err(ApiError::io)?;
    let send_errors = state.output.take_send_errors();
    state.output.record_output_health(packets, send_errors);
    state.action_timing.complete_output_render(action_timing);
    send_osc_feedback(&state, true);
    Ok(Json(serde_json::json!({
        "now": now,
        "revision": rendered.revision,
        "packets_sent": packets,
        "universes": frames.into_iter().map(|(universe, slots)| serde_json::json!({"universe":universe,"slots":slots.to_vec()})).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub(super) struct FreeRunTestClock {
    pub(super) millis: u64,
}

/// Runs the production deadline scheduler against the manual application clock for a bounded
/// wall-time interval. The request resolves only after the exact final application-time frame has
/// rendered and the clock is frozen again.
pub(super) async fn free_run_test_clock(
    State(state): State<AppState>,
    Json(input): Json<FreeRunTestClock>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !(1..=60_000).contains(&input.millis) {
        return Err(ApiError::bad_request("millis must be within 1-60000"));
    }
    let clock_session = state.output.acquire_test_clock().await?;
    let clock = clock_session.driver();
    let started = std::time::Instant::now();
    let cancellation = tokio_util::sync::CancellationToken::new();
    let cancellation_after_tick = cancellation.clone();
    let mut advanced = 0_u64;
    let output = state.output.clone();
    let scheduler_state = state.clone();
    let scheduler_clock = clock.clone();
    output
        .run_output_scheduler(cancellation, move || {
            let elapsed = u64::try_from(started.elapsed().as_millis())
                .unwrap_or(u64::MAX)
                .min(input.millis);
            let delta = elapsed.saturating_sub(advanced);
            advanced = elapsed;
            if delta > 0 {
                scheduler_clock.advance_millis(i64::try_from(delta).unwrap_or(i64::MAX));
            }
            refresh_speed_group_engine(&scheduler_state);
            if elapsed == input.millis {
                cancellation_after_tick.cancel();
            }
            let tick_state = scheduler_state.clone();
            async move {
                let result = output_scheduler::render_test_tick(tick_state.clone()).await;
                send_osc_feedback(&tick_state, true);
                result
            }
        })
        .await;
    Ok(Json(serde_json::json!({
        "now": clock.now(),
        "wall_millis": started.elapsed().as_millis(),
    })))
}

#[derive(Deserialize)]
pub(super) struct TestOutputFailure {
    pub(super) destination: SocketAddr,
    pub(super) enabled: bool,
}

pub(super) async fn set_test_output_failure(
    State(state): State<AppState>,
    Json(input): Json<TestOutputFailure>,
) -> Result<StatusCode, ApiError> {
    state
        .output
        .inject_network_failure(input.destination, input.enabled)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(super) struct TestShowObjectSeedRequest {
    pub(super) expected_revision: u64,
    pub(super) action: TestShowObjectSeedAction,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum TestShowObjectSeedAction {
    Put { body: serde_json::Value },
    Delete,
}

/// Test-bench-only adapter for seeding portable object fixtures.
///
/// The route is registered only when `--test-bench` supplies a manual clock on a loopback
/// server. It calls a private seeding boundary so the public v1 mutation routes can remain absent.
pub(super) async fn seed_test_show_object(
    State(state): State<AppState>,
    Path((show_id, kind, object_id)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<TestShowObjectSeedRequest>,
) -> Result<Response, ApiError> {
    let session = authenticate(&state, &headers)?;
    let show_id = light_core::ShowId(show_id);
    match request.action {
        TestShowObjectSeedAction::Put { body } => {
            seed_object_for_test_put(
                &state,
                &session,
                show_id,
                kind,
                object_id,
                request.expected_revision,
                body,
            )
            .await
        }
        TestShowObjectSeedAction::Delete => Ok(seed_object_for_test_delete(
            &state,
            &session,
            show_id,
            kind,
            object_id,
            request.expected_revision,
        )
        .await?
        .into_response()),
    }
}
