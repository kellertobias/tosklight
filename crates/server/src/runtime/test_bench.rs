use super::*;

pub(super) fn fixed_test_time() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
        .expect("fixed test timestamp is valid")
        .with_timezone(&chrono::Utc)
}

pub(super) async fn reset_test_clock(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let clock = state
        .manual_clock
        .as_ref()
        .ok_or_else(|| ApiError::not_found("test clock"))?;
    clock.set(fixed_test_time());
    state.programmers.reset_all();
    state.engine.clear_programmer_transitions();
    state.output_sequences.lock().await.clear();
    state.osc_subscribers.lock().clear();
    {
        let configuration = state.configuration.read().clone();
        *state.speed_groups.lock() = std::array::from_fn(|index| {
            SpeedGroupController::new(
                configuration.speed_groups_bpm[index],
                configuration.speed_group_sound_to_light[index].clone(),
            )
            .expect("validated Speed Group configuration")
        });
        *state.sound_capture_owners.lock() = [None; 5];
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
    let clock = state
        .manual_clock
        .as_ref()
        .ok_or_else(|| ApiError::not_found("test clock"))?;
    let now = clock.advance_millis(input.millis);
    refresh_speed_group_engine(&state);
    let rendered = {
        let _activation = state.activation_lock.clone().lock_owned().await;
        let _ordered = state.playback_service.operation_lock();
        let mut rendered = state
            .engine
            .render(state.output_control.lock().render_options())
            .map_err(|error| ApiError::internal(error.to_string()))?;
        let transitions = std::mem::take(&mut rendered.automatic_playback_transitions);
        if let Some(show_id) = state.active_show.read().as_ref().map(|show| show.id.0) {
            let changes = playback_service::automatic_projection_changes(
                &state.engine,
                PlaybackShowScope {
                    show_id,
                    show_revision: rendered.revision,
                },
                transitions,
            );
            publish_automatic_playback_events(state.playback_service.events(), changes);
        }
        rendered
    };
    let frames = {
        let mut control = state.output_control.lock();
        if control.hold {
            control.last_frames.clone()
        } else {
            let mut frames = rendered.universes;
            for (&(universe, address), &value) in &control.raw_overrides {
                if let Some(frame) = frames.get_mut(&universe) {
                    frame[usize::from(address - 1)] = value;
                }
            }
            control.last_frames = frames.clone();
            frames
        }
    };
    let snapshot = state.engine.snapshot();
    let output = state
        .network_output
        .as_ref()
        .ok_or_else(|| ApiError::unavailable("network output is unavailable"))?;
    let packets = output
        .send_routes(
            &snapshot.routes,
            &frames,
            &rendered.patched_slots,
            &mut *state.output_sequences.lock().await,
        )
        .await
        .map_err(ApiError::io)?;
    {
        let mut health = state
            .output_health
            .lock()
            .expect("output health mutex poisoned");
        health.frames_sent += 1;
        health.packets_sent += packets;
        health.send_errors += output.take_send_errors();
    }
    send_osc_feedback(&state, true);
    Ok(Json(serde_json::json!({
        "now": now,
        "revision": rendered.revision,
        "packets_sent": packets,
        "universes": frames.into_iter().map(|(universe, slots)| serde_json::json!({"universe":universe,"slots":slots.to_vec()})).collect::<Vec<_>>(),
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
        .network_output
        .as_ref()
        .ok_or_else(|| ApiError::unavailable("network output is unavailable"))?
        .inject_failure(input.destination, input.enabled);
    Ok(StatusCode::NO_CONTENT)
}
