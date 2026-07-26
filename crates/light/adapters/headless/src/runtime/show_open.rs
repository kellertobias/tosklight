use super::*;

pub(super) async fn open_show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<OpenShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let entry = state
        .installation
        .show_library()
        .map_err(ApiError::store)?
        .into_iter()
        .find(|entry| entry.id.0 == id)
        .ok_or_else(|| ApiError::not_found("show"))?;
    if !FsPath::new(&entry.path).exists() {
        return Err(ApiError::bad_request("show file is unavailable"));
    }
    let _activation = state.active_show.acquire().await;
    validate_show_file(&entry.path).map_err(ApiError::store)?;
    let output_runtime = load_output_runtime_for_show(&state, entry.id)?;
    let previous = state.active_show.current().clone();
    if let Some(previous) = &previous {
        state
            .installation
            .set_setting("previous_active_show_id", &previous.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    let prepared = prepare_show_for_runtime(&state, &entry)?;
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    activate_prepared_snapshot(
        &state,
        prepared,
        &context,
        &transition,
        input.transition_millis,
    )
    .await?;
    state
        .installation
        .set_active_show(Some(entry.id))
        .map_err(ApiError::store)?;
    invalidate_active_show_document(&state);
    state.active_show.replace_current(Some(entry.clone()));
    state.active_show.set_error(None);
    restore_output_runtime_for_show(&state, entry.id, output_runtime);
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
    let session = authenticate(&state, &headers)?;
    let name = available_show_name(&state, "Default Stage Show Clean Copy")?;
    let path = state
        .installation
        .data_dir()
        .join("shows")
        .join(format!("{name}.show"));
    default_show::initialise(&path).map_err(ApiError::store)?;
    let entry = match state
        .installation
        .upsert_show(&name, &path.display().to_string(), false)
    {
        Ok(entry) => entry,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(ApiError::store(error));
        }
    };
    if let Err(error) = ActiveShowRepository::open(&path)
        .and_then(|store| store.set_identity(entry.id, &entry.name, None))
    {
        let _ = state.installation.remove_show(entry.id);
        let _ = std::fs::remove_file(&path);
        return Err(ApiError::store(error));
    }
    let _activation = state.active_show.acquire().await;
    let output_runtime = load_output_runtime_for_show(&state, entry.id)?;
    let prepared = match prepare_show_for_runtime(&state, &entry) {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = state.installation.remove_show(entry.id);
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
    };
    let previous = state.active_show.current().clone();
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    activate_prepared_snapshot(
        &state,
        prepared,
        &context,
        &transition,
        input.transition_millis,
    )
    .await?;
    state
        .installation
        .set_active_show(Some(entry.id))
        .map_err(ApiError::store)?;
    if let Some(previous) = &previous {
        state
            .installation
            .set_setting("previous_active_show_id", &previous.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    invalidate_active_show_document(&state);
    state.active_show.replace_current(Some(entry.clone()));
    state.active_show.set_error(None);
    restore_output_runtime_for_show(&state, entry.id, output_runtime);
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
    let session = authenticate(&state, &headers)?;
    let previous_id = state
        .installation
        .setting("previous_active_show_id")
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("previous active show"))?;
    let previous_id = light_core::ShowId(
        Uuid::parse_str(&previous_id)
            .map_err(|_| ApiError::bad_request("stored rollback show ID is invalid"))?,
    );
    let entry = state
        .installation
        .show(previous_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("rollback show"))?;
    let _activation = state.active_show.acquire().await;
    let output_runtime = load_output_runtime_for_show(&state, entry.id)?;
    let prepared = prepare_show_for_runtime(&state, &entry)?;
    let current = state.active_show.current().clone();
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    activate_prepared_snapshot(
        &state,
        prepared,
        &context,
        &transition,
        input.transition_millis,
    )
    .await?;
    state
        .installation
        .set_active_show(Some(entry.id))
        .map_err(ApiError::store)?;
    if let Some(current) = current {
        state
            .installation
            .set_setting("previous_active_show_id", &current.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    invalidate_active_show_document(&state);
    state.active_show.replace_current(Some(entry.clone()));
    state.active_show.set_error(None);
    restore_output_runtime_for_show(&state, entry.id, output_runtime);
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
        .installation
        .show_library()
        .map_err(ApiError::store)?
        .into_iter()
        .find(|entry| entry.id.0 == id)
        .ok_or_else(|| ApiError::not_found("show"))?;
    let export = state
        .installation
        .data_dir()
        .join(format!(".export-{}.show", Uuid::new_v4()));
    ActiveShowRepository::open(&entry.path)
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
