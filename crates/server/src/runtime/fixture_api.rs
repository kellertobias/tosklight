//! Typed, replay-safe v2 fixture-library transport.

use super::fixture_api_replay::ReplayKey;
use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::fixture_library as wire;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/fixture-library", post(fixture_library_action))
        .route(
            "/api/v2/fixture-library/definitions",
            get(fixture_definitions_snapshot),
        )
        .route(
            "/api/v2/fixture-library/profiles",
            get(fixture_profiles_snapshot),
        )
        .route(
            "/api/v2/fixture-library/warnings",
            get(fixture_library_warnings_snapshot),
        )
        .route(
            "/api/v2/fixture-library/profiles/{id}/revisions",
            get(fixture_profile_revisions),
        )
        .route(
            "/api/v2/fixture-library/profiles/{id}/revisions/{revision}/package",
            get(export_fixture_package),
        )
}

async fn fixture_definitions_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::FixtureDefinitionsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(wire::FixtureDefinitionsSnapshot {
        definitions: json_values(
            state
                .fixture_library
                .lock()
                .definitions()
                .map_err(ApiError::fixture)?,
        )?,
    }))
}

async fn fixture_profiles_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::FixtureProfilesSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(wire::FixtureProfilesSnapshot {
        profiles: json_values(
            state
                .fixture_library
                .lock()
                .profiles()
                .map_err(ApiError::fixture)?,
        )?,
    }))
}

async fn fixture_library_warnings_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::FixtureLibraryWarningsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(wire::FixtureLibraryWarningsSnapshot {
        warnings: state
            .fixture_library
            .lock()
            .migration_warnings()
            .map_err(ApiError::fixture)?,
    }))
}

async fn fixture_profile_revisions(
    State(state): State<AppState>,
    Path(id): Path<light_core::FixtureId>,
    headers: HeaderMap,
) -> Result<Json<wire::FixtureProfileRevisionsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let library = state.fixture_library.lock();
    let profiles = library
        .profile_revisions(id)
        .map_err(ApiError::fixture)?
        .into_iter()
        .map(|revision| {
            library
                .profile(id, revision)
                .map_err(ApiError::fixture)?
                .ok_or_else(|| ApiError::not_found("fixture profile revision"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(wire::FixtureProfileRevisionsSnapshot {
        profiles: json_values(profiles)?,
    }))
}

async fn fixture_library_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::FixtureLibraryActionRequest>,
) -> Result<Json<wire::FixtureLibraryActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let signature = action_signature(&request.action)?;
    let mut replay = state.fixture_library_replay.lock().await;
    if let Some(outcome) = replay.get(&key, &signature)? {
        return Ok(Json(outcome));
    }
    let result = execute_action(&state, request.action)?;
    let outcome = wire::FixtureLibraryActionOutcome {
        request_id: request.request_id,
        replayed: false,
        result,
    };
    replay.insert(key, signature, outcome.clone());
    Ok(Json(outcome))
}

fn execute_action(
    state: &AppState,
    action: wire::FixtureLibraryAction,
) -> Result<wire::FixtureLibraryActionResult, ApiError> {
    use wire::{FixtureLibraryAction as Action, FixtureLibraryActionResult as Result};
    match action {
        Action::SaveProfile {
            profile,
            expected_revision,
        } => {
            let profile = serde_json::from_value(profile)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            let expected = u32::try_from(expected_revision)
                .map_err(|_| ApiError::bad_request("fixture profile revision exceeds u32"))?;
            let stored = state
                .fixture_library
                .lock()
                .save_profile(profile, expected)
                .map_err(ApiError::fixture)?;
            emit(
                state,
                "fixture_profile_changed",
                serde_json::json!({"id":stored.id,"revision":stored.revision}),
            );
            Ok(Result::Profile {
                profile_id: stored.id.0,
                revision: stored.revision,
            })
        }
        Action::DeleteProfileRevision {
            profile_id,
            revision,
        } => {
            let id = light_core::FixtureId(profile_id);
            if !state
                .fixture_library
                .lock()
                .delete_profile(id, revision)
                .map_err(ApiError::fixture)?
            {
                return Err(ApiError::not_found("fixture profile revision"));
            }
            emit(
                state,
                "fixture_profile_changed",
                serde_json::json!({"id":id,"revision":revision,"deleted":true}),
            );
            Ok(Result::Deleted {
                resource: wire::FixtureLibraryResource::Profile,
                id: profile_id,
                revision,
            })
        }
        Action::ImportPackage { package_base64 } => {
            let package = decode_archive(&package_base64, "fixture package")?;
            let stored = state
                .fixture_library
                .lock()
                .import_fixture_package(&package)
                .map_err(ApiError::fixture)?;
            emit(
                state,
                "fixture_profile_changed",
                serde_json::json!({"id":stored.id,"revision":stored.revision,"imported_package":true}),
            );
            Ok(Result::Profile {
                profile_id: stored.id.0,
                revision: stored.revision,
            })
        }
        Action::AttachGdtf {
            profile_id,
            revision,
            source_base64,
        } => {
            let source = decode_archive(&source_base64, "GDTF source archive")?;
            let id = light_core::FixtureId(profile_id);
            if !state
                .fixture_library
                .lock()
                .set_profile_source_gdtf(id, revision, &source)
                .map_err(ApiError::fixture)?
            {
                return Err(ApiError::not_found("fixture profile revision"));
            }
            emit(
                state,
                "fixture_profile_changed",
                serde_json::json!({"id":id,"revision":revision,"source_gdtf":true}),
            );
            Ok(Result::GdtfAttached {
                profile_id,
                revision,
            })
        }
        Action::SaveDefinition { definition } => {
            let definition: light_fixture::FixtureDefinition =
                serde_json::from_value(definition)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
            let json = serde_json::to_string(&definition).map_err(|error| {
                ApiError::internal(format!("fixture definition encoding failed: {error}"))
            })?;
            let stored = state
                .fixture_library
                .lock()
                .import_json(&json)
                .map_err(ApiError::fixture)?;
            emit(
                state,
                "fixture_library_changed",
                serde_json::json!({"id":stored.id,"revision":stored.revision}),
            );
            Ok(Result::Definition {
                definition_id: stored.id.0,
                revision: stored.revision,
            })
        }
        Action::DeleteDefinitionRevision {
            definition_id,
            revision,
        } => {
            let id = light_core::FixtureId(definition_id);
            if !state
                .fixture_library
                .lock()
                .delete(id, revision)
                .map_err(ApiError::fixture)?
            {
                return Err(ApiError::not_found("fixture definition"));
            }
            emit(
                state,
                "fixture_library_changed",
                serde_json::json!({"id":id,"revision":revision,"deleted":true}),
            );
            Ok(Result::Deleted {
                resource: wire::FixtureLibraryResource::Definition,
                id: definition_id,
                revision,
            })
        }
    }
}

async fn export_fixture_package(
    State(state): State<AppState>,
    Path((id, revision)): Path<(light_core::FixtureId, u32)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let library = state.fixture_library.lock();
    let profile = library
        .profile(id, revision)
        .map_err(ApiError::fixture)?
        .ok_or_else(|| ApiError::not_found("fixture profile revision"))?;
    let bytes = library
        .export_fixture_package(id, revision)
        .map_err(ApiError::fixture)?
        .ok_or_else(|| ApiError::not_found("fixture profile revision"))?;
    let filename = format!(
        "{}-{}.toskfixture",
        profile
            .manufacturer
            .chars()
            .chain(std::iter::once('-'))
            .chain(profile.name.chars())
            .map(|character| if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            })
            .collect::<String>()
            .trim_matches('-'),
        revision
    );
    Ok((
        [
            (
                header::CONTENT_TYPE,
                light_fixture::FIXTURE_PACKAGE_MIME_TYPE,
            ),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

fn decode_archive(encoded: &str, label: &str) -> Result<Vec<u8>, ApiError> {
    if encoded.is_empty() {
        return Err(ApiError::bad_request(format!("{label} is empty")));
    }
    STANDARD
        .decode(encoded)
        .map_err(|error| ApiError::bad_request(format!("invalid {label} base64: {error}")))
}

fn json_values<T: Serialize>(values: Vec<T>) -> Result<Vec<serde_json::Value>, ApiError> {
    values
        .into_iter()
        .map(|value| encode_value(value, "fixture library projection"))
        .collect()
}

fn action_signature(action: &wire::FixtureLibraryAction) -> Result<[u8; 32], ApiError> {
    let bytes = serde_json::to_vec(action).map_err(|error| {
        ApiError::internal(format!("fixture-library action encoding failed: {error}"))
    })?;
    Ok(Sha256::digest(bytes).into())
}

fn encode_value<T: Serialize>(value: T, label: &str) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value)
        .map_err(|error| ApiError::internal(format!("{label} encoding failed: {error}")))
}

fn validate_request_id(request_id: &str) -> Result<(), ApiError> {
    if request_id.trim().is_empty()
        || request_id.len() > 128
        || request_id.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "request_id must contain 1-128 printable bytes",
        ));
    }
    Ok(())
}
