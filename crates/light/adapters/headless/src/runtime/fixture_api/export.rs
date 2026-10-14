//! Explicit confined save; no browser/WebKit download or fixture-library mutation.
use super::*;
use crate::runtime::desk_management_v2::ReplayKey;

pub(super) async fn save_package(
    State(state): State<AppState>,
    Path((id, revision)): Path<(light_core::FixtureId, u32)>,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<wire::FixturePackageExportRequest>,
) -> Result<Json<wire::FixturePackageExportOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&input.request_id)?;
    let key = ReplayKey::new(session.id, "fixture-package-export", &input.request_id);
    let fingerprint = serde_json::json!({"profile_id": id, "revision": revision, "target": input});
    // Serialize the lookup and exclusive file creation, including concurrent identical intents.
    let _guard = crate::runtime::file_manager::FILE_MUTATION_LOCK
        .lock()
        .await;
    if let Some(value) = state
        .replay
        .lookup_desk_management(&key, &fingerprint)
        .await?
    {
        return serde_json::from_value(value)
            .map(Json)
            .map_err(|error| ApiError::internal(error.to_string()));
    }
    let directory =
        crate::runtime::file_manager::writable_show_folder(&state, &input.root_id, &input.path)?;
    let leaf = crate::runtime::show_save_destination::file_name(&input.name, "toskfixture")?;
    let (_, bytes) = state
        .installation
        .export_fixture_package(id, revision)
        .map_err(ApiError::fixture)?
        .ok_or_else(|| ApiError::not_found("fixture profile revision"))?;
    crate::runtime::show_save_destination::write_new(&directory.join(&leaf), &bytes)?;
    let relative = if input.path.is_empty() {
        leaf
    } else {
        format!("{}/{}", input.path.trim_end_matches('/'), leaf)
    };
    let outcome = wire::FixturePackageExportOutcome {
        request_id: input.request_id,
        replayed: false,
        profile_id: id.0,
        revision,
        root_id: input.root_id,
        path: relative,
    };
    state
        .replay
        .insert_desk_management(
            key,
            fingerprint,
            serde_json::to_value(&outcome)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        )
        .await;
    emit(
        &state,
        "file_operation_completed",
        serde_json::json!({"operation": "create_file", "items": [{
            "source_root_id": outcome.root_id, "source": "", "destination_root_id": outcome.root_id,
            "destination": outcome.path, "status": "completed", "error": null
        }]}),
    );
    Ok(Json(outcome))
}
