use super::*;

pub(super) async fn rename_show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RenameShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let name = input.name.trim();
    validate_show_name(name)?;
    let id = light_core::ShowId(id);
    let current = state
        .active_show
        .read()
        .clone()
        .filter(|show| show.id == id)
        .ok_or_else(|| ApiError::conflict("only the active show can be renamed"))?;
    if current.name == name {
        return Ok(Json(current));
    }
    if state
        .desk
        .lock()
        .library()
        .map_err(ApiError::store)?
        .into_iter()
        .any(|show| show.id != id && show.name.eq_ignore_ascii_case(name))
    {
        return Err(ApiError::conflict("a show with that name already exists"));
    }

    let destination = state.data_dir.join("shows").join(format!("{name}.show"));
    if destination.exists() {
        return Err(ApiError::conflict(
            "a show file with that name already exists",
        ));
    }
    let staged = state
        .data_dir
        .join("shows")
        .join(format!(".rename-{}.tmp", Uuid::new_v4()));
    let stage_result = ShowStore::open(&current.path)
        .and_then(|store| store.backup_to(&staged))
        .and_then(|_| ShowStore::open(&staged))
        .and_then(|store| store.set_identity(current.id, name, current.revision_copy.as_ref()))
        .and_then(|_| validate_show_file(&staged).map(|_| ()));
    if let Err(error) = stage_result {
        let _ = std::fs::remove_file(&staged);
        return Err(ApiError::store(error));
    }
    if let Err(error) = std::fs::rename(&staged, &destination) {
        let _ = std::fs::remove_file(&staged);
        return Err(ApiError::io(error));
    }
    let renamed =
        match state
            .desk
            .lock()
            .rename_show(current.id, name, &destination.display().to_string())
        {
            Ok(entry) => entry,
            Err(error) => {
                let _ = std::fs::remove_file(&destination);
                return Err(ApiError::store(error));
            }
        };
    *state.active_show.write() = Some(renamed.clone());
    if let Err(error) = std::fs::remove_file(&current.path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(path=%current.path, %error, "renamed show retained its superseded file");
    }
    emit(
        &state,
        "show_renamed",
        serde_json::json!({"previous_name":current.name,"show":renamed}),
    );
    Ok(Json(renamed))
}
pub(super) async fn overwrite_show(
    State(state): State<AppState>,
    Path((source_id, destination_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if source_id == destination_id {
        return Err(ApiError::bad_request(
            "source and overwrite destination must be different shows",
        ));
    }
    if state
        .active_show
        .read()
        .as_ref()
        .is_none_or(|show| show.id.0 != source_id)
    {
        return Err(ApiError::conflict(
            "only the active show can be saved over another show",
        ));
    }
    let (source, destination) = {
        let desk = state.desk.lock();
        let source = desk
            .show(light_core::ShowId(source_id))
            .map_err(ApiError::store)?
            .ok_or_else(|| ApiError::not_found("source show"))?;
        let destination = desk
            .show(light_core::ShowId(destination_id))
            .map_err(ApiError::store)?
            .ok_or_else(|| ApiError::not_found("overwrite destination"))?;
        (source, destination)
    };
    let staged = state
        .data_dir
        .join("shows")
        .join(format!(".overwrite-{}.tmp", Uuid::new_v4()));
    let stage_result = ShowStore::open(&source.path)
        .and_then(|store| store.backup_to(&staged))
        .and_then(|_| ShowStore::open(&staged))
        .and_then(|store| {
            store.set_identity(
                destination.id,
                &destination.name,
                destination.revision_copy.as_ref(),
            )
        })
        .and_then(|_| validate_show_file(&staged).map(|_| ()));
    if let Err(error) = stage_result {
        let _ = std::fs::remove_file(&staged);
        return Err(ApiError::store(error));
    }
    if let Err(error) = backup_show(&state, &destination) {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&staged, &destination.path) {
        let _ = std::fs::remove_file(&staged);
        return Err(ApiError::io(error));
    }
    let destination = state
        .desk
        .lock()
        .mark_show_updated(destination.id)
        .map_err(ApiError::store)?;
    emit(
        &state,
        "show_overwritten",
        serde_json::json!({"source_show":source,"destination_show":destination}),
    );
    Ok(Json(destination))
}
pub(super) async fn delete_show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let id = light_core::ShowId(id);
    if state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|show| show.id == id)
    {
        return Err(ApiError::conflict("the active show cannot be deleted"));
    }
    let entry = state
        .desk
        .lock()
        .show(id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let revisions = state
        .desk
        .lock()
        .show_revisions(id)
        .map_err(ApiError::store)?;
    state.desk.lock().remove_show(id).map_err(ApiError::store)?;
    if let Err(error) = std::fs::remove_file(&entry.path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(ApiError::io(error));
    }
    for revision in revisions {
        if let Err(error) = std::fs::remove_file(revision.path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(ApiError::io(error));
        }
    }
    emit(&state, "show_deleted", serde_json::json!({"show_id":id}));
    Ok(StatusCode::NO_CONTENT)
}
