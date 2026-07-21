use super::*;

pub(super) async fn list_shows(
    State(state): State<AppState>,
) -> Result<Json<Vec<ShowEntry>>, ApiError> {
    Ok(Json(state.desk.lock().library().map_err(ApiError::store)?))
}
pub(super) async fn list_show_revisions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ShowRevision>>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let id = light_core::ShowId(id);
    if state
        .desk
        .lock()
        .show(id)
        .map_err(ApiError::store)?
        .is_none()
    {
        return Err(ApiError::not_found("show"));
    }
    Ok(Json(
        state
            .desk
            .lock()
            .show_revisions(id)
            .map_err(ApiError::store)?,
    ))
}
pub(super) async fn save_show_revision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<SaveShowRevision>,
) -> Result<(StatusCode, Json<ShowRevision>), ApiError> {
    let _session = authenticate(&state, &headers)?;
    let id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    if input.name.trim().is_empty() || input.name.trim().len() > 120 {
        return Err(ApiError::bad_request(
            "revision name must contain 1-120 characters",
        ));
    }
    let directory = state
        .data_dir
        .join("revisions")
        .join(entry.id.0.to_string());
    std::fs::create_dir_all(&directory).map_err(ApiError::io)?;
    let destination = directory.join(format!("{}.show", Uuid::new_v4()));
    ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .backup_to(&destination)
        .map_err(ApiError::store)?;
    let revision = match state.desk.lock().add_show_revision(
        entry.id,
        input.name.trim(),
        &destination.display().to_string(),
    ) {
        Ok(revision) => revision,
        Err(error) => {
            let _ = std::fs::remove_file(destination);
            return Err(ApiError::store(error));
        }
    };
    emit(
        &state,
        "show_revision_saved",
        serde_json::json!({"show_id":entry.id,"revision":revision}),
    );
    Ok((StatusCode::CREATED, Json(revision)))
}
pub(super) async fn open_show_revision(
    State(state): State<AppState>,
    Path((id, revision)): Path<(Uuid, u64)>,
    headers: HeaderMap,
    Json(input): Json<OpenShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let saved_revision = state
        .desk
        .lock()
        .show_revision(id, revision)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show revision"))?;
    if !FsPath::new(&saved_revision.path).exists() {
        return Err(ApiError::bad_request("saved show revision is unavailable"));
    }
    validate_show_file(&saved_revision.path).map_err(ApiError::store)?;
    let copied_at = chrono::Utc::now();
    let revision_copy = RevisionCopySource {
        show_id: entry.id,
        show_name: entry.name.clone(),
        revision: saved_revision.revision,
        revision_name: saved_revision.name.clone(),
        copied_at: copied_at.to_rfc3339(),
    };
    let copy_name = revision_copy_name(&state, &entry.name, revision, copied_at.date_naive())?;
    let copy_path = state
        .data_dir
        .join("shows")
        .join(format!("{copy_name}.show"));
    std::fs::copy(&saved_revision.path, &copy_path).map_err(ApiError::io)?;
    let copy = match state.desk.lock().upsert_show_with_revision_copy(
        &copy_name,
        &copy_path.display().to_string(),
        false,
        Some(&revision_copy),
    ) {
        Ok(copy) => copy,
        Err(error) => {
            let _ = std::fs::remove_file(&copy_path);
            return Err(ApiError::store(error));
        }
    };
    if let Err(error) = ShowStore::open(&copy.path)
        .and_then(|store| store.set_identity(copy.id, &copy.name, copy.revision_copy.as_ref()))
    {
        let _ = state.desk.lock().remove_show(copy.id);
        let _ = std::fs::remove_file(&copy_path);
        return Err(ApiError::store(error));
    }
    let _activation = state.activation_lock.lock().await;
    let output_runtime = load_output_runtime_for_show(&state, copy.id)?;
    let prepared = match prepare_show_for_runtime(&state, &copy) {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = state.desk.lock().remove_show(copy.id);
            let _ = std::fs::remove_file(&copy_path);
            return Err(error);
        }
    };
    let previous = state.active_show.read().clone();
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
        .desk
        .lock()
        .set_active_show(Some(copy.id))
        .map_err(ApiError::store)?;
    if let Some(previous) = &previous
        && previous.id != copy.id
    {
        state
            .desk
            .lock()
            .set_setting("previous_active_show_id", &previous.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    *state.active_show.write() = Some(copy.clone());
    *state.active_show_error.write() = None;
    restore_output_runtime_for_show(&state, copy.id, output_runtime);
    emit(
        &state,
        "show_opened",
        serde_json::json!({"show":copy,"revision_copy":revision_copy,"transition":transition}),
    );
    Ok(Json(copy))
}
pub(super) async fn upload_show(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UploadShow>,
) -> Result<(StatusCode, Json<ShowEntry>), ApiError> {
    let _session = authenticate(&state, &headers)?;
    validate_show_name(&input.name)?;
    let path = state
        .data_dir
        .join("shows")
        .join(format!("{}.show", input.name));
    let mut uploaded_revision_copy = None;
    if let Some(data) = input.data_base64 {
        let bytes = STANDARD
            .decode(data)
            .map_err(|_| ApiError::bad_request("data_base64 is invalid"))?;
        if bytes.len() < 100 || !bytes.starts_with(b"SQLite format 3\0") {
            return Err(ApiError::bad_request(
                "uploaded show is not a SQLite database",
            ));
        }
        let staged = state
            .data_dir
            .join("shows")
            .join(format!(".upload-{}.tmp", Uuid::new_v4()));
        std::fs::write(&staged, bytes).map_err(ApiError::io)?;
        if let Err(error) = validate_show_file(&staged) {
            let _ = std::fs::remove_file(&staged);
            return Err(ApiError::store(error));
        }
        uploaded_revision_copy = ShowStore::open(&staged)
            .map_err(ApiError::store)?
            .revision_copy_source()
            .map_err(ApiError::store)?;
        if path.exists() && !input.overwrite {
            let _ = std::fs::remove_file(&staged);
            return Err(ApiError::conflict("a show with that name already exists"));
        }
        if path.exists()
            && let Some(existing) = state
                .desk
                .lock()
                .library()
                .map_err(ApiError::store)?
                .into_iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(&input.name))
        {
            backup_show(&state, &existing)?;
        }
        std::fs::rename(&staged, &path).map_err(ApiError::io)?;
    } else if !path.exists() {
        if input.name == default_show::name() {
            default_show::initialise(&path).map_err(ApiError::store)?;
        } else {
            initialise_show(&path, &input.name).map_err(ApiError::store)?;
        }
    }
    let entry = state
        .desk
        .lock()
        .upsert_show_with_revision_copy(
            &input.name,
            &path.display().to_string(),
            input.overwrite,
            uploaded_revision_copy.as_ref(),
        )
        .map_err(ApiError::store)?;
    ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .set_identity(entry.id, &entry.name, entry.revision_copy.as_ref())
        .map_err(ApiError::store)?;
    emit(&state, "show_uploaded", serde_json::json!({"show":entry}));
    Ok((StatusCode::CREATED, Json(entry)))
}
