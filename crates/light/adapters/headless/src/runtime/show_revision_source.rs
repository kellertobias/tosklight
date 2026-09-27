//! Immutable named-revision sources for downloads and selective imports.
use super::*;

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v2/shows/{id}/revisions/{revision}/download",
        get(download_named_revision),
    )
}

/// The replay-safe library action serializes this creation with its request cache.
/// This deliberately never installs a runtime snapshot or changes the active show.
pub(super) fn prepare_named_revision_source(
    state: &AppState,
    show_id: Uuid,
    revision: u64,
) -> Result<ShowEntry, ApiError> {
    let (entry, saved) = named_revision(state, show_id, revision)?;
    let copied_at = chrono::Utc::now();
    let provenance = RevisionCopySource {
        show_id: entry.id,
        show_name: entry.name.clone(),
        revision: saved.revision,
        revision_name: saved.name,
        copied_at: copied_at.to_rfc3339(),
    };
    let name = revision_copy_name(state, &entry.name, revision, copied_at.date_naive())?;
    let path = state
        .installation
        .data_dir()
        .join("shows")
        .join(format!("{name}.show"));
    std::fs::copy(&saved.path, &path).map_err(ApiError::io)?;
    // Validation may migrate a supported older file: only migrate the independent copy.
    if let Err(error) = validate_show_file(&path) {
        let _ = std::fs::remove_file(&path);
        return Err(ApiError::store(error));
    }
    let copy = match state.installation.upsert_show_with_revision_copy(
        &name,
        &path.display().to_string(),
        false,
        Some(&provenance),
    ) {
        Ok(copy) => copy,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(ApiError::store(error));
        }
    };
    if let Err(error) = ActiveShowRepository::open(&copy.path)
        .and_then(|store| store.set_identity(copy.id, &copy.name, copy.revision_copy.as_ref()))
    {
        let _ = state.installation.remove_show(copy.id);
        let _ = std::fs::remove_file(&path);
        return Err(ApiError::store(error));
    }
    emit(state, "show_uploaded", serde_json::json!({"show": copy}));
    Ok(copy)
}

fn named_revision(
    state: &AppState,
    show_id: Uuid,
    revision: u64,
) -> Result<(ShowEntry, ShowRevision), ApiError> {
    let id = light_core::ShowId(show_id);
    let entry = state
        .installation
        .show(id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let saved = state
        .installation
        .show_revision(id, revision)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show revision"))?;
    if !FsPath::new(&saved.path).is_file() {
        return Err(ApiError::bad_request("saved show revision is unavailable"));
    }
    Ok((entry, saved))
}

async fn download_named_revision(
    State(state): State<AppState>,
    Path((show_id, revision)): Path<(Uuid, u64)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (entry, saved) = named_revision(&state, show_id, revision)?;
    let provenance = RevisionCopySource {
        show_id: entry.id,
        show_name: entry.name.clone(),
        revision: saved.revision,
        revision_name: saved.name.clone(),
        copied_at: chrono::Utc::now().to_rfc3339(),
    };
    let export = state
        .installation
        .data_dir()
        .join(format!(".revision-export-{}.show", Uuid::new_v4()));
    let source = state
        .installation
        .data_dir()
        .join(format!(".revision-source-{}.show", Uuid::new_v4()));
    let result = (|| {
        std::fs::copy(&saved.path, &source).map_err(ApiError::io)?;
        let store = ActiveShowRepository::open(&source).map_err(ApiError::store)?;
        store
            .set_identity(entry.id, &entry.name, Some(&provenance))
            .map_err(ApiError::store)?;
        store.backup_to(&export).map_err(ApiError::store)?;
        std::fs::read(&export).map_err(ApiError::io)
    })();
    let _ = std::fs::remove_file(&export);
    let _ = std::fs::remove_file(&source);
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.light.show"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"revision-{revision}.show\""),
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        result?,
    )
        .into_response())
}
