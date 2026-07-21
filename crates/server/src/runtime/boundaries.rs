use super::*;

pub(super) async fn desk_boundary(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let Some(required) = &state.desk_token else {
        return next.run(request).await;
    };
    let ticketed_file_stream = request.method() == Method::GET
        && request.uri().path().starts_with("/api/v1/files/")
        && request.uri().path().ends_with("/content")
        && request.uri().query().is_some_and(|query| {
            query
                .split('&')
                .any(|part| part.starts_with("ticket=") && part.len() > "ticket=".len())
        });
    if request.uri().path() == "/"
        || request.uri().path().starts_with("/assets/")
        || request.uri().path().starts_with("/api/v1/help/assets/")
        // Native audio elements cannot attach the desk-boundary header. The
        // content handler still validates the path-bound, expiring stream
        // capability and its active authenticated session.
        || ticketed_file_stream
    {
        return next.run(request).await;
    }
    let supplied_header = request
        .headers()
        .get("x-light-desk-token")
        .and_then(|value| value.to_str().ok());
    let supplied_ws = request
        .headers()
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .split(',')
                .map(str::trim)
                .find_map(|value| value.strip_prefix("light.desk.b64."))
        })
        .and_then(|encoded| URL_SAFE_NO_PAD.decode(encoded).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok());
    if supplied_header == Some(required.as_ref())
        || supplied_ws.as_deref() == Some(required.as_ref())
    {
        next.run(request).await
    } else {
        ApiError::unauthorized("desk boundary token is required").into_response()
    }
}

pub(super) fn desk_lock_key(id: Uuid) -> String {
    format!("desk_lock:{id}")
}

pub(super) fn read_desk_lock(state: &AppState, id: Uuid) -> DeskLockConfiguration {
    state
        .desk
        .lock()
        .setting(&desk_lock_key(id))
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

pub(super) fn write_desk_lock(
    state: &AppState,
    id: Uuid,
    configuration: &DeskLockConfiguration,
) -> Result<(), ApiError> {
    let value = serde_json::to_string(configuration)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .desk
        .lock()
        .set_setting(&desk_lock_key(id), &value)
        .map_err(ApiError::store)
}

pub(super) fn desk_lock_response(configuration: DeskLockConfiguration) -> DeskLockResponse {
    DeskLockResponse {
        locked: configuration.locked,
        message: configuration.message,
        wallpaper: configuration.wallpaper,
        unlock_mode: configuration.unlock_mode,
    }
}

pub(super) fn pin_hash(salt: &str, pin: &str) -> String {
    let digest = Sha256::digest(format!("{salt}:{pin}").as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) async fn desk_lock_boundary(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if request.method() == Method::GET
        || request.method() == Method::OPTIONS
        || is_programming_update_route(request.method(), path)
        || is_output_runtime_action_route(request.method(), path)
        || path == "/api/v1/sessions"
        || path.starts_with("/api/v1/desk-lock")
    {
        return next.run(request).await;
    }
    let session = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|token| authenticate_token(&state, token).ok());
    if session
        .as_ref()
        .is_some_and(|session| read_desk_lock(&state, session.desk.id).locked)
    {
        return ApiError::conflict("desk is locked").into_response();
    }
    next.run(request).await
}

fn is_output_runtime_action_route(method: &Method, path: &str) -> bool {
    let parts = path.trim_matches('/').split('/').collect::<Vec<_>>();
    matches!(
        (method, parts.as_slice()),
        (
            &Method::POST,
            ["api", "v2", "desks", _, "output-runtime", _]
        )
    )
}

fn is_programming_update_route(method: &Method, path: &str) -> bool {
    let parts = path.trim_matches('/').split('/').collect::<Vec<_>>();
    matches!(
        (method, parts.as_slice()),
        (
            &Method::POST,
            [
                "api",
                "v2",
                "shows",
                _,
                "programming-update",
                "preview" | "targets" | "actions"
            ]
        ) | (
            &Method::PUT,
            ["api", "v2", "desks", _, "programming-update", "settings"]
        )
    )
}

pub(super) async fn desk_lock(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeskLockResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(desk_lock_response(read_desk_lock(
        &state,
        session.desk.id,
    ))))
}

pub(super) async fn update_desk_lock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<DeskLockUpdate>,
) -> Result<Json<DeskLockResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let operation_lock = state.programming.desk_lock(session.desk.id);
    let _operation = operation_lock.lock();
    let mut configuration = read_desk_lock(&state, session.desk.id);
    if configuration.locked {
        return Err(ApiError::conflict(
            "unlock the desk before changing its lock configuration",
        ));
    }
    if !matches!(input.unlock_mode.as_str(), "button" | "pin") {
        return Err(ApiError::bad_request("unlock mode must be button or pin"));
    }
    if input.message.len() > 500 {
        return Err(ApiError::bad_request(
            "lock message must not exceed 500 characters",
        ));
    }
    configuration.message = input.message;
    configuration.wallpaper = input.wallpaper.filter(|value| !value.trim().is_empty());
    configuration.unlock_mode = input.unlock_mode;
    if configuration.unlock_mode == "pin" {
        if let Some(pin) = input.pin {
            if !(4..=12).contains(&pin.len())
                || !pin.chars().all(|character| character.is_ascii_digit())
            {
                return Err(ApiError::bad_request("PIN must contain 4-12 digits"));
            }
            let salt = Uuid::new_v4().to_string();
            configuration.pin_hash = Some(pin_hash(&salt, &pin));
            configuration.pin_salt = Some(salt);
        }
        if configuration.pin_hash.is_none() {
            return Err(ApiError::bad_request(
                "PIN required mode needs a configured PIN",
            ));
        }
    } else {
        configuration.pin_hash = None;
        configuration.pin_salt = None;
    }
    write_desk_lock(&state, session.desk.id, &configuration)?;
    emit(
        &state,
        "desk_lock_changed",
        serde_json::json!({"desk_id":session.desk.id,"locked":false}),
    );
    Ok(Json(desk_lock_response(configuration)))
}

pub(super) async fn lock_desk(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeskLockResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let operation_lock = state.programming.desk_lock(session.desk.id);
    let _operation = operation_lock.lock();
    let mut configuration = read_desk_lock(&state, session.desk.id);
    configuration.locked = true;
    write_desk_lock(&state, session.desk.id, &configuration)?;
    emit(
        &state,
        "desk_lock_changed",
        serde_json::json!({"desk_id":session.desk.id,"locked":true}),
    );
    Ok(Json(desk_lock_response(configuration)))
}

pub(super) async fn unlock_desk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<DeskUnlockInput>,
) -> Result<Json<DeskLockResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let operation_lock = state.programming.desk_lock(session.desk.id);
    let _operation = operation_lock.lock();
    let mut configuration = read_desk_lock(&state, session.desk.id);
    if configuration.unlock_mode == "pin" {
        let Some(pin) = input.pin else {
            return Err(ApiError::unauthorized("PIN is required"));
        };
        let valid = configuration
            .pin_salt
            .as_deref()
            .zip(configuration.pin_hash.as_deref())
            .is_some_and(|(salt, expected)| pin_hash(salt, &pin) == expected);
        if !valid {
            return Err(ApiError::unauthorized("incorrect PIN"));
        }
    }
    configuration.locked = false;
    write_desk_lock(&state, session.desk.id, &configuration)?;
    emit(
        &state,
        "desk_lock_changed",
        serde_json::json!({"desk_id":session.desk.id,"locked":false}),
    );
    Ok(Json(desk_lock_response(configuration)))
}

pub(super) async fn force_unlock_desk(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeskLockResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let operation_lock = state.programming.desk_lock(session.desk.id);
    let _operation = operation_lock.lock();
    let supplied = headers
        .get("x-light-admin-recovery")
        .and_then(|value| value.to_str().ok());
    let expected = env::var("LIGHT_ADMIN_RECOVERY_TOKEN").ok();
    if expected
        .as_deref()
        .is_none_or(|expected| supplied != Some(expected))
    {
        return Err(ApiError::unauthorized(
            "administrative recovery token is required",
        ));
    }
    let mut configuration = read_desk_lock(&state, session.desk.id);
    configuration.locked = false;
    write_desk_lock(&state, session.desk.id, &configuration)?;
    emit(
        &state,
        "desk_lock_changed",
        serde_json::json!({"desk_id":session.desk.id,"locked":false,"forced":true}),
    );
    Ok(Json(desk_lock_response(configuration)))
}
