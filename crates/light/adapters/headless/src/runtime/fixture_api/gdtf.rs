//! The canonical importer owns channel precision and mapping on both preview and confirmation.
use super::*;
use sha2::{Digest, Sha256};

pub(super) async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::FixtureGdtfPreviewRequest>,
) -> Result<Json<wire::FixtureGdtfPreview>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let source = decode_archive(&request.source_base64, "GDTF source archive")?;
    let imported = light_fixture::gdtf::read::preview_profile(&source)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let unknown_attributes = unknown_canonical_attributes(&state, &imported.profile)
        .into_iter()
        .map(|(attribute, value_type)| wire::FixtureImportRequirement {
            attribute,
            value_type: fixture_import_value_type(value_type),
        })
        .collect();
    // Preview omits source-association work. Confirmation validates again and atomically
    // associates the archive with the final profile after explicit attribute mappings.
    Ok(Json(wire::FixtureGdtfPreview {
        profile: serde_json::to_value(imported.profile)
            .map_err(|error| ApiError::internal(error.to_string()))?,
        diagnostics: imported
            .diagnostics
            .into_iter()
            .map(|item| wire::FixtureGdtfDiagnostic {
                node: item.node,
                message: item.message,
            })
            .collect(),
        unknown_attributes,
    }))
}

pub(super) async fn import(
    State(state): State<AppState>,
    Path(profile_id): Path<Uuid>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::FixtureGdtfImportRequest>,
) -> Result<Json<wire::FixtureLibraryActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let signature: [u8; 32] = Sha256::digest(
        serde_json::to_vec(&(
            "gdtf-import-v1",
            profile_id,
            request.expected_revision,
            &request.source_base64,
            &request.attribute_mappings,
        ))
        .map_err(|error| ApiError::internal(error.to_string()))?,
    )
    .into();
    let outcome = state
        .replay
        .execute_fixture_library_edit(key, signature, || {
            let source = decode_archive(&request.source_base64, "GDTF source archive")?;
            let mut profile = light_fixture::gdtf::read::preview_profile(&source)
                .map_err(|error| ApiError::bad_request(error.to_string()))?
                .profile;
            if profile.id.0 != profile_id {
                return Err(ApiError::bad_request(
                    "GDTF FixtureTypeID must match the import target",
                ));
            }
            let unknown = unknown_canonical_attributes(&state, &profile);
            apply_fixture_attribute_mappings(
                &state,
                &mut profile,
                &unknown,
                request.attribute_mappings,
            )?;
            let stored = state
                .installation
                .save_fixture_profile_with_gdtf(profile, request.expected_revision, &source)
                .map_err(ApiError::fixture)?;
            emit(
                &state,
                "fixture_profile_changed",
                serde_json::json!({"id":stored.id,"revision":stored.revision,"source_gdtf":true}),
            );
            Ok(wire::FixtureLibraryActionResult::Profile {
                profile_id: stored.id.0,
                revision: stored.revision,
            })
        })
        .await?;
    Ok(Json(outcome))
}
