//! Portable operator descriptions; absent legacy metadata reads as an empty description.
use super::*;

const DESCRIPTION_KEY: &str = "description";

pub(super) fn read_description(path: &str) -> Result<String, ApiError> {
    // An unavailable library file must remain browsable through recovery/load UI.
    if !FsPath::new(path).is_file() {
        return Ok(String::new());
    }
    match ActiveShowRepository::open(path).and_then(|store| store.metadata_value(DESCRIPTION_KEY)) {
        Ok(description) => Ok(description.unwrap_or_default()),
        Err(error) => {
            tracing::warn!(%error,"show description unavailable; keeping recovery entry browsable");
            Ok(String::new())
        }
    }
}

pub(super) fn set_description(
    state: &AppState,
    show_id: Uuid,
    description: &str,
) -> Result<ShowEntry, ApiError> {
    if description.chars().count() > 2000 {
        return Err(ApiError::bad_request(
            "description must contain at most 2000 characters",
        ));
    }
    let entry = state
        .installation
        .show(light_core::ShowId(show_id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let description = description.trim();
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    if store
        .metadata_value(DESCRIPTION_KEY)
        .map_err(ApiError::store)?
        .unwrap_or_default()
        == description
    {
        return Ok(entry);
    }
    store
        .set_metadata_values(&[(DESCRIPTION_KEY, description)])
        .map_err(ApiError::store)?;
    state
        .installation
        .mark_show_updated(entry.id)
        .map_err(ApiError::store)?;
    let updated = state
        .installation
        .show(entry.id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    if state
        .active_show
        .current()
        .is_some_and(|show| show.id == entry.id)
    {
        state.active_show.clear_document_cache();
        state.active_show.replace_current(Some(updated.clone()));
    }
    emit(
        state,
        "show_description_changed",
        serde_json::json!({"show_id":entry.id}),
    );
    Ok(updated)
}
