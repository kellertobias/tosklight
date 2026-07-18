use super::*;

pub(super) async fn open_show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<OpenShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let entry = state
        .desk
        .lock()
        .library()
        .map_err(ApiError::store)?
        .into_iter()
        .find(|entry| entry.id.0 == id)
        .ok_or_else(|| ApiError::not_found("show"))?;
    if !FsPath::new(&entry.path).exists() {
        return Err(ApiError::bad_request("show file is unavailable"));
    }
    validate_show_file(&entry.path).map_err(ApiError::store)?;
    let compiled = load_engine_snapshot(&entry).map_err(ApiError::internal)?;
    compiled
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let _activation = state.activation_lock.lock().await;
    let previous = state.active_show.read().clone();
    if let Some(previous) = &previous {
        state
            .desk
            .lock()
            .set_setting("previous_active_show_id", &previous.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    activate_snapshot(&state, compiled, &transition, input.transition_millis).await?;
    state
        .desk
        .lock()
        .set_active_show(Some(entry.id))
        .map_err(ApiError::store)?;
    *state.active_show.write() = Some(entry.clone());
    *state.active_show_error.write() = None;
    emit(
        &state,
        "show_opened",
        serde_json::json!({"show":entry,"transition":transition,"previous_show":previous}),
    );
    Ok(Json(entry))
}
pub(super) async fn open_clean_default_show(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<OpenShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let name = available_show_name(&state, "Default Stage Show Clean Copy")?;
    let path = state.data_dir.join("shows").join(format!("{name}.show"));
    default_show::initialise(&path).map_err(ApiError::store)?;
    let entry = match state
        .desk
        .lock()
        .upsert_show(&name, &path.display().to_string(), false)
    {
        Ok(entry) => entry,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(ApiError::store(error));
        }
    };
    if let Err(error) =
        ShowStore::open(&path).and_then(|store| store.set_identity(entry.id, &entry.name, None))
    {
        let _ = state.desk.lock().remove_show(entry.id);
        let _ = std::fs::remove_file(&path);
        return Err(ApiError::store(error));
    }
    let compiled = match load_engine_snapshot(&entry) {
        Ok(compiled) => compiled,
        Err(error) => {
            let _ = state.desk.lock().remove_show(entry.id);
            let _ = std::fs::remove_file(&path);
            return Err(ApiError::internal(error));
        }
    };
    compiled
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let _activation = state.activation_lock.lock().await;
    let previous = state.active_show.read().clone();
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    activate_snapshot(&state, compiled, &transition, input.transition_millis).await?;
    state
        .desk
        .lock()
        .set_active_show(Some(entry.id))
        .map_err(ApiError::store)?;
    if let Some(previous) = &previous {
        state
            .desk
            .lock()
            .set_setting("previous_active_show_id", &previous.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    *state.active_show.write() = Some(entry.clone());
    *state.active_show_error.write() = None;
    emit(
        &state,
        "show_opened",
        serde_json::json!({"show":entry,"transition":transition,"previous_show":previous,"source":"built_in_default"}),
    );
    Ok(Json(entry))
}
pub(super) async fn rollback_show(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<OpenShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let previous_id = state
        .desk
        .lock()
        .setting("previous_active_show_id")
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("previous active show"))?;
    let previous_id = light_core::ShowId(
        Uuid::parse_str(&previous_id)
            .map_err(|_| ApiError::bad_request("stored rollback show ID is invalid"))?,
    );
    let entry = state
        .desk
        .lock()
        .show(previous_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("rollback show"))?;
    let compiled = load_engine_snapshot(&entry).map_err(ApiError::internal)?;
    compiled
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let _activation = state.activation_lock.lock().await;
    let current = state.active_show.read().clone();
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    activate_snapshot(&state, compiled, &transition, input.transition_millis).await?;
    state
        .desk
        .lock()
        .set_active_show(Some(entry.id))
        .map_err(ApiError::store)?;
    if let Some(current) = current {
        state
            .desk
            .lock()
            .set_setting("previous_active_show_id", &current.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    *state.active_show.write() = Some(entry.clone());
    *state.active_show_error.write() = None;
    emit(
        &state,
        "show_rolled_back",
        serde_json::json!({"show":entry,"transition":transition}),
    );
    Ok(Json(entry))
}
pub(super) async fn download_show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let entry = state
        .desk
        .lock()
        .library()
        .map_err(ApiError::store)?
        .into_iter()
        .find(|entry| entry.id.0 == id)
        .ok_or_else(|| ApiError::not_found("show"))?;
    let export = state
        .data_dir
        .join(format!(".export-{}.show", Uuid::new_v4()));
    ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .backup_to(&export)
        .map_err(ApiError::store)?;
    let data = std::fs::read(&export).map_err(ApiError::io)?;
    let _ = std::fs::remove_file(export);
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.light.show"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}.show\"", entry.name),
            ),
        ],
        data,
    )
        .into_response())
}
