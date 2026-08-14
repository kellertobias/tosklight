use super::*;
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, header},
    response::IntoResponse,
    routing::{get, post},
};
use light_wire::v2::live_action::{
    CommandTargetHttpActionOutcome, CommandTargetHttpActionRequest,
    FixtureControlHttpActionRequest, FixtureControlLiveActionRequest, FixtureControlOutcome,
    FixtureFreezeActionOutcome, FixtureFreezeLiveActionRequest, GenerateFixturePresetsOutcome,
    GenerateFixturePresetsRequest, ProgrammerCaptureModeHttpActionRequest,
    ProgrammerCaptureModeLiveActionRequest, ProgrammerCaptureModeOutcome,
    ProgrammerUndoHttpActionOutcome, ProgrammingAlignHttpActionRequest,
    ProgrammingAlignLiveActionRequest, ProgrammingAlignOutcome,
};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/programming-align/actions", post(programming_align))
        .route(
            "/api/v2/programmer-capture-mode/actions",
            post(programmer_capture_mode),
        )
        .route("/api/v2/command-target/actions", post(command_target))
        .route("/api/v2/programmer-undo/actions", get(programmer_undo))
        .route(
            "/api/v2/fixture-freeze/actions",
            get(fixture_freeze_full).post(fixture_freeze_partial),
        )
        .route("/api/v2/fixture-controls/actions", post(fixture_control))
        .route(
            "/api/v2/preset-profile-generation/update",
            post(generate_fixture_presets),
        )
}

async fn fixture_freeze_full(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    fixture_freeze_action(
        state,
        show,
        desk,
        headers,
        FixtureFreezeLiveActionRequest::default(),
    )
    .await
}

async fn fixture_freeze_partial(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<FixtureFreezeLiveActionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if request.families.is_empty() {
        return Err(ApiError::bad_request(
            "partial Freeze requires at least one attribute family",
        ));
    }
    fixture_freeze_action(state, show, desk, headers, request).await
}

async fn fixture_freeze_action(
    state: AppState,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    request: FixtureFreezeLiveActionRequest,
) -> Result<impl IntoResponse, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let request_id = Uuid::new_v4().to_string();
    let context = programming_context(
        &session,
        light_application::ActionSource::Http,
        Some(&request_id),
    );
    let worker_state = state.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        super::fixture_freeze::toggle_selected(&worker_state, &session, &request, &context)
    })
    .await
    .map_err(|error| ApiError::internal(format!("Fixture Freeze action failed: {error}")))??;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json::<FixtureFreezeActionOutcome>(outcome),
    ))
}

async fn command_target(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<CommandTargetHttpActionRequest>,
) -> Result<Json<CommandTargetHttpActionOutcome>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let request_id = Uuid::new_v4().to_string();
    let outcome_request_id = request_id.clone();
    run_http_programming_action(state, session, request_id, move |state, session, _| {
        let value = match request.value {
            light_wire::v2::command_line::CommandTarget::Fixture => "FIXTURE",
            light_wire::v2::command_line::CommandTarget::Group => "GROUP",
        };
        if !state
            .programming
            .set_command_target(session.id, value.into())
        {
            return Err("command target must be FIXTURE or GROUP".into());
        }
        Ok(CommandTargetHttpActionOutcome {
            request_id: outcome_request_id,
            target: request.value,
        })
    })
    .await
    .map(Json)
}

async fn programmer_undo(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let request_id = Uuid::new_v4().to_string();
    let outcome_request_id = request_id.clone();
    let outcome =
        run_http_programming_action(state, session, request_id, move |state, session, _| {
            let context = programming_context(
                session,
                light_application::ActionSource::Http,
                Some(&outcome_request_id),
            );
            let changed = super::fixture_freeze::undo_latest(state, session, &context)
                .map_err(|error| error.message)?
                .unwrap_or_else(|| state.programming.undo(session.id));
            persist_programmer(state, session).map_err(|error| error.message)?;
            Ok(ProgrammerUndoHttpActionOutcome {
                request_id: outcome_request_id,
                changed,
            })
        })
        .await?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(outcome)))
}

async fn programmer_capture_mode(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<ProgrammerCaptureModeHttpActionRequest>,
) -> Result<Json<ProgrammerCaptureModeOutcome>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let request_id = Uuid::new_v4().to_string();
    let action_request = ProgrammerCaptureModeLiveActionRequest {
        request_id: request_id.clone(),
        blind: request.blind,
        preview: request.preview,
        active_context: request.active_context,
    };
    run_http_programming_action(state, session, request_id, move |state, session, _| {
        ws_programmer_capture_mode(state, session, action_request)
    })
    .await
    .map(Json)
}

async fn programming_align(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<ProgrammingAlignHttpActionRequest>,
) -> Result<Json<ProgrammingAlignOutcome>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let request_id = Uuid::new_v4().to_string();
    let outcome_request_id = request_id.clone();
    let action_request = ProgrammingAlignLiveActionRequest {
        request_id: request_id.clone(),
        mode: request.mode,
    };
    let activation = state.active_show.acquire().await;
    let worker_state = state.clone();
    let worker_session = session.clone();
    let completed = tokio::task::spawn_blocking(move || {
        let context = programming_context(
            &worker_session,
            light_application::ActionSource::Http,
            Some(&outcome_request_id),
        );
        let ports =
            command_http::ServerProgrammingPorts::new(&worker_state, &worker_session, "http", true);
        ws_programmer_align(&worker_state, action_request, &context, &ports)
    })
    .await
    .map_err(|error| ApiError::internal(format!("Programming Align task failed: {error}")))?;
    drop(activation);
    completed.map(Json).map_err(action_api_error)
}

async fn fixture_control(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<FixtureControlHttpActionRequest>,
) -> Result<Json<FixtureControlOutcome>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let request_id = Uuid::new_v4().to_string();
    let outcome_request_id = request_id.clone();
    let action_request = FixtureControlLiveActionRequest {
        request_id: request_id.clone(),
        fixture_id: request.fixture_id,
        action_id: request.action_id,
        active: request.active,
    };
    run_http_programming_action(state, session, request_id, move |state, session, action| {
        #[derive(Deserialize)]
        struct FixtureControlPayload {
            action_id: Uuid,
            active: bool,
            kind: light_wire::v2::live_action::FixtureControlKind,
            pulse_duration_millis: Option<u64>,
        }
        let result = ws_programmer_control_action(
            state,
            session,
            &typed_action_request(action, action_request),
        )?;
        let payload: FixtureControlPayload =
            serde_json::from_value(result.payload).map_err(|error| error.to_string())?;
        Ok(FixtureControlOutcome {
            request_id: outcome_request_id,
            action_id: payload.action_id,
            active: payload.active,
            kind: payload.kind,
            pulse_duration_millis: payload.pulse_duration_millis,
        })
    })
    .await
    .map(Json)
}

async fn generate_fixture_presets(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<GenerateFixturePresetsRequest>,
) -> Result<Json<GenerateFixturePresetsOutcome>, ApiError> {
    output_runtime_v2::validate_request_id(&request.request_id).map_err(ApiError::bad_request)?;
    let session = session_for_desk(&state, &headers, &desk)?;
    let show_id = show.resolve(&state)?;
    let replay_key = PresetGenerationReplayKey {
        session_id: session.id.0,
        show_id,
        request_id: request.request_id.clone(),
    };
    if let Some(outcome) = state
        .replay
        .lookup_preset_generation(&replay_key, &request)
        .await?
    {
        return Ok(Json(outcome));
    }
    let replay_request = request.clone();
    let context = programming_context(
        &session,
        light_application::ActionSource::Http,
        Some(&request.request_id),
    )
    .with_expected_revision(request.expected_show_revision);
    let fixture_ids = request
        .fixture_ids
        .into_iter()
        .map(light_core::FixtureId)
        .collect();
    let worker_state = state.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        generate_profile_presets_action(&worker_state, fixture_ids, context, show_id)
    })
    .await
    .map_err(|error| ApiError::internal(format!("Preset generation task failed: {error}")))?
    .map_err(action_api_error)?;
    state
        .replay
        .insert_preset_generation(replay_key, replay_request, outcome.clone())
        .await;
    Ok(Json(outcome))
}

pub(super) async fn run_http_programming_action<T>(
    state: AppState,
    session: Session,
    request_id: String,
    operation: impl FnOnce(&AppState, &Session, &WsActionRequest) -> Result<T, String> + Send + 'static,
) -> Result<T, ApiError>
where
    T: Send + 'static,
{
    let activation = state.active_show.acquire().await;
    let worker_state = state.clone();
    let worker_session = session.clone();
    let context = programming_context(
        &session,
        light_application::ActionSource::Http,
        Some(&request_id),
    );
    let (completed, _activation) = tokio::task::spawn_blocking(move || {
        let action = WsActionRequest {
            request_id,
            payload: serde_json::Value::Null,
        };
        (
            run_programming_interaction(
                &worker_state,
                &worker_session,
                &context,
                "http_live_action",
                ProgrammingLockPolicy::RequireUnlocked,
                || operation(&worker_state, &worker_session, &action),
            ),
            activation,
        )
    })
    .await
    .map_err(|error| ApiError::internal(format!("Programming action task failed: {error}")))?;
    completed?.output.map_err(action_api_error)
}

pub(super) async fn run_fire_and_forget_http_programming_action<T>(
    state: AppState,
    session: Session,
    operation: impl FnOnce(&AppState, &Session) -> Result<T, String> + Send + 'static,
) -> Result<T, ApiError>
where
    T: Send + 'static,
{
    let activation = state.active_show.acquire().await;
    let worker_state = state.clone();
    let worker_session = session.clone();
    let context = programming_context(&session, light_application::ActionSource::Http, None);
    let (completed, _activation) = tokio::task::spawn_blocking(move || {
        (
            run_programming_interaction(
                &worker_state,
                &worker_session,
                &context,
                "http_live_action",
                ProgrammingLockPolicy::RequireUnlocked,
                || operation(&worker_state, &worker_session),
            ),
            activation,
        )
    })
    .await
    .map_err(|error| ApiError::internal(format!("Programming action task failed: {error}")))?;
    completed?.output.map_err(action_api_error)
}

fn action_api_error(message: String) -> ApiError {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("revision")
        || normalized.contains("locked")
        || normalized.contains("no longer active")
        || normalized.contains("request_id was already used")
    {
        ApiError::conflict(message)
    } else if normalized.contains("changing") || normalized.contains("retry") {
        ApiError::unavailable(message)
    } else if normalized.contains("does not exist")
        || normalized.contains("not found")
        || normalized.contains("programmer does not exist")
    {
        ApiError::not_found(message)
    } else {
        ApiError::bad_request(message)
    }
}

const PRESET_GENERATION_REPLAY_LIMIT: usize = 1_024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct PresetGenerationReplayKey {
    session_id: Uuid,
    show_id: light_core::ShowId,
    request_id: String,
}

struct PresetGenerationReplayEntry {
    request: GenerateFixturePresetsRequest,
    outcome: GenerateFixturePresetsOutcome,
}

#[derive(Default)]
pub(super) struct PresetGenerationReplayCache {
    entries: HashMap<PresetGenerationReplayKey, PresetGenerationReplayEntry>,
    order: VecDeque<PresetGenerationReplayKey>,
}

impl PresetGenerationReplayCache {
    pub(super) fn get(
        &self,
        key: &PresetGenerationReplayKey,
        request: &GenerateFixturePresetsRequest,
    ) -> Result<Option<GenerateFixturePresetsOutcome>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.request != request {
            return Err(ApiError::conflict(
                "request_id was already used for different Preset generation intent",
            ));
        }
        let mut outcome = entry.outcome.clone();
        outcome.replayed = true;
        Ok(Some(outcome))
    }

    pub(super) fn insert(
        &mut self,
        key: PresetGenerationReplayKey,
        request: GenerateFixturePresetsRequest,
        outcome: GenerateFixturePresetsOutcome,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries
            .insert(key, PresetGenerationReplayEntry { request, outcome });
        while self.entries.len() > PRESET_GENERATION_REPLAY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

fn typed_action_request<T: serde::Serialize>(
    action: &WsActionRequest,
    request: T,
) -> WsActionRequest {
    WsActionRequest {
        request_id: action.request_id.clone(),
        payload: serde_json::to_value(request).expect("typed HTTP actions always serialize"),
    }
}
