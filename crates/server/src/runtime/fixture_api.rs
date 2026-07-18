use super::*;

pub(super) async fn list_fixture_library(
    State(state): State<AppState>,
) -> Result<Json<Vec<light_fixture::FixtureDefinition>>, ApiError> {
    Ok(Json(
        state
            .fixture_library
            .lock()
            .definitions()
            .map_err(ApiError::fixture)?,
    ))
}

pub(super) async fn put_fixture_library(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(definition): Json<light_fixture::FixtureDefinition>,
) -> Result<Json<light_fixture::FixtureDefinition>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let json = serde_json::to_string(&definition)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let stored = state
        .fixture_library
        .lock()
        .import_json(&json)
        .map_err(ApiError::fixture)?;
    emit(
        &state,
        "fixture_library_changed",
        serde_json::json!({"id":stored.id,"revision":stored.revision}),
    );
    Ok(Json(stored))
}

pub(super) async fn delete_fixture_library(
    State(state): State<AppState>,
    Path((id, revision)): Path<(light_core::FixtureId, u32)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if state
        .fixture_library
        .lock()
        .delete(id, revision)
        .map_err(ApiError::fixture)?
    {
        emit(
            &state,
            "fixture_library_changed",
            serde_json::json!({"id":id,"revision":revision,"deleted":true}),
        );
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("fixture definition"))
    }
}

pub(super) async fn list_fixture_profiles(
    State(state): State<AppState>,
) -> Result<Json<Vec<light_fixture::FixtureProfile>>, ApiError> {
    Ok(Json(
        state
            .fixture_library
            .lock()
            .profiles()
            .map_err(ApiError::fixture)?,
    ))
}

pub(super) async fn list_fixture_profile_warnings(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(
        state
            .fixture_library
            .lock()
            .migration_warnings()
            .map_err(ApiError::fixture)?,
    ))
}

pub(super) async fn list_fixture_profile_revisions(
    State(state): State<AppState>,
    Path(id): Path<light_core::FixtureId>,
) -> Result<Json<Vec<light_fixture::FixtureProfile>>, ApiError> {
    let library = state.fixture_library.lock();
    let revisions = library.profile_revisions(id).map_err(ApiError::fixture)?;
    let profiles = revisions
        .into_iter()
        .map(|revision| {
            library
                .profile(id, revision)
                .map_err(ApiError::fixture)?
                .ok_or_else(|| ApiError::not_found("fixture profile revision"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(profiles))
}

pub(super) async fn put_fixture_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(profile): Json<light_fixture::FixtureProfile>,
) -> Result<Json<light_fixture::FixtureProfile>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let expected = parse_if_match(&headers)?;
    let expected = u32::try_from(expected)
        .map_err(|_| ApiError::bad_request("fixture profile revision exceeds u32"))?;
    let stored = state
        .fixture_library
        .lock()
        .save_profile(profile, expected)
        .map_err(ApiError::fixture)?;
    emit(
        &state,
        "fixture_profile_changed",
        serde_json::json!({"id":stored.id,"revision":stored.revision}),
    );
    Ok(Json(stored))
}

pub(super) async fn import_fixture_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<light_fixture::FixtureProfile>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if body.is_empty() {
        return Err(ApiError::bad_request("fixture package is empty"));
    }
    let stored = state
        .fixture_library
        .lock()
        .import_fixture_package(&body)
        .map_err(ApiError::fixture)?;
    emit(
        &state,
        "fixture_profile_changed",
        serde_json::json!({"id":stored.id,"revision":stored.revision,"imported_package":true}),
    );
    Ok(Json(stored))
}

pub(super) async fn export_fixture_package(
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

pub(super) async fn delete_fixture_profile(
    State(state): State<AppState>,
    Path((id, revision)): Path<(light_core::FixtureId, u32)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if state
        .fixture_library
        .lock()
        .delete_profile(id, revision)
        .map_err(ApiError::fixture)?
    {
        emit(
            &state,
            "fixture_profile_changed",
            serde_json::json!({"id":id,"revision":revision,"deleted":true}),
        );
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("fixture profile revision"))
    }
}

pub(super) async fn put_fixture_profile_source_gdtf(
    State(state): State<AppState>,
    Path((id, revision)): Path<(light_core::FixtureId, u32)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if body.is_empty() {
        return Err(ApiError::bad_request("GDTF source archive is empty"));
    }
    if !state
        .fixture_library
        .lock()
        .set_profile_source_gdtf(id, revision, &body)
        .map_err(ApiError::fixture)?
    {
        return Err(ApiError::not_found("fixture profile revision"));
    }
    emit(
        &state,
        "fixture_profile_changed",
        serde_json::json!({"id":id,"revision":revision,"source_gdtf":true}),
    );
    Ok(StatusCode::NO_CONTENT)
}
