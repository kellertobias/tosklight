use super::*;

pub(super) fn ensure_playback_page_for_advance(
    state: &AppState,
    show: &ShowEntry,
    requested: u8,
) -> Result<bool, ApiError> {
    let snapshot = state.engine.snapshot();
    if snapshot
        .playback_pages
        .iter()
        .any(|page| page.number == requested)
    {
        return Ok(true);
    }
    let Some(last) = snapshot
        .playback_pages
        .iter()
        .max_by_key(|page| page.number)
    else {
        return Ok(false);
    };
    if last.slots.is_empty() || last.number.checked_add(1) != Some(requested) {
        return Ok(false);
    }
    let page = light_playback::PlaybackPage {
        number: requested,
        name: format!("Page {requested}"),
        slots: HashMap::new(),
    };
    let body =
        serde_json::to_value(&page).map_err(|error| ApiError::internal(error.to_string()))?;
    let store = ShowStore::open(&show.path).map_err(ApiError::store)?;
    backup_show(state, show)?;
    let revision = store
        .put_object("playback_page", &requested.to_string(), &body, 0)
        .map_err(ApiError::store)?;
    state
        .engine
        .replace_snapshot(load_engine_snapshot(show).map_err(ApiError::internal)?)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    emit(
        state,
        "show_object_changed",
        serde_json::json!({"show_id":show.id,"kind":"playback_page","id":requested.to_string(),"revision":revision}),
    );
    Ok(true)
}

pub(super) async fn list_screens(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let show = state.active_show.read().clone();
    let store = state.desk.lock();
    let screens = store.screens().map_err(ApiError::store)?;
    let mut pages = serde_json::Map::new();
    if let Some(show) = show {
        for screen in &screens {
            let page = if screen.page_mode == "follow_main" {
                store.desk_page(session.desk.id, show.id)
            } else {
                store.screen_page(screen.id, show.id)
            }
            .map_err(ApiError::store)?;
            pages.insert(screen.id.to_string(), serde_json::json!(page));
        }
    }
    Ok(Json(
        serde_json::json!({"screens":screens,"active_pages":pages}),
    ))
}
pub(super) async fn put_screen(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(mut input): Json<ScreenConfiguration>,
) -> Result<Json<ScreenConfiguration>, ApiError> {
    let _ = authenticate(&state, &headers)?;
    input.id = id;
    let screen = state
        .desk
        .lock()
        .put_screen(input)
        .map_err(ApiError::store)?;
    emit(
        &state,
        "screen_configuration_changed",
        serde_json::json!({"screen":screen}),
    );
    Ok(Json(screen))
}
pub(super) async fn delete_screen(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let _ = authenticate(&state, &headers)?;
    state
        .desk
        .lock()
        .delete_screen(id)
        .map_err(ApiError::store)?;
    emit(
        &state,
        "screen_configuration_changed",
        serde_json::json!({"screen_id":id,"deleted":true}),
    );
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn update_screen_page(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DeskPageInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = authenticate(&state, &headers)?;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    if !state
        .engine
        .snapshot()
        .playback_pages
        .iter()
        .any(|page| page.number == input.page)
    {
        return Err(ApiError::bad_request("playback page does not exist"));
    }
    let store = state.desk.lock();
    let screen = store
        .screen(id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("screen"))?;
    if screen.page_mode != "independent" {
        return Err(ApiError::bad_request("screen follows the main page"));
    }
    store
        .set_screen_page(id, show.id, input.page)
        .map_err(ApiError::store)?;
    drop(store);
    emit(
        &state,
        "screen_page_changed",
        serde_json::json!({"screen_id":id,"show_id":show.id,"page":input.page}),
    );
    Ok(Json(serde_json::json!({"screen_id":id,"page":input.page})))
}

pub(super) async fn paged_playback_action(
    State(state): State<AppState>,
    Path((id, slot, action)): Path<(Uuid, u8, String)>,
    headers: HeaderMap,
    input: Option<Json<PoolPlaybackInput>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if session.desk.id != id {
        return Err(ApiError::bad_request(
            "session is not attached to this desk",
        ));
    }
    let input = input.map(|Json(value)| value).unwrap_or_default();
    let result = playback_service::http_action(
        &state,
        &session,
        PlaybackAddress::CurrentPage { slot },
        &action,
        &input,
    )?;
    Ok(Json(playback_service::pool_http_payload(
        &state, &session, &action, result,
    )?))
}

pub(super) async fn pool_playback_action(
    State(state): State<AppState>,
    Path((number, action)): Path<(u16, String)>,
    headers: HeaderMap,
    input: Option<Json<PoolPlaybackInput>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let input = input.map(|Json(value)| value).unwrap_or_default();
    let result = playback_service::http_action(
        &state,
        &session,
        PlaybackAddress::Pool(number),
        &action,
        &input,
    )?;
    Ok(Json(playback_service::pool_http_payload(
        &state, &session, &action, result,
    )?))
}
