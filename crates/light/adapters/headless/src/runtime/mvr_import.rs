use super::*;

pub(super) async fn preview_mvr_import(
    State(state): State<AppState>,
    Query(query): Query<MvrPreviewQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<MvrImportPreview>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    tokio::task::spawn_blocking(move || prepare_mvr_preview(&state, query, &body))
        .await
        .map_err(|error| ApiError::internal(format!("MVR preview task failed: {error}")))?
        .map(Json)
}

fn prepare_mvr_preview(
    state: &AppState,
    query: MvrPreviewQuery,
    body: &[u8],
) -> Result<MvrImportPreview, ApiError> {
    let document =
        light_mvr::read(&body).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let definitions = mvr_definitions(&state, &document)?;
    let mut occupied = Vec::new();
    let mut owners = HashMap::new();
    if let Some(id) = query.show_id
        && let Some(show) = state
            .installation
            .show(light_core::ShowId(id))
            .map_err(ApiError::store)?
    {
        let destination = ActiveShowRepository::open(show.path)
            .map_err(ApiError::store)?
            .portable_document()
            .map_err(ApiError::store)?;
        owners = light_application::mvr_import::mvr_destination_fixture_ids(
            &destination,
            &light_application::mvr_export::tosklight_mvr_fixture_metadata(&document),
        );
        occupied = light_application::mvr_import::occupied_patches(&destination)
            .map_err(|error| ApiError::bad_request(error.message))?;
    }
    let missing_profiles = document
        .fixtures
        .iter()
        .filter(|f| !definitions.definitions.contains_key(&f.uuid))
        .map(|f| format!("{} · {}", f.gdtf_spec, f.gdtf_mode))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut address_conflicts = Vec::new();
    for fixture in &document.fixtures {
        if let (Some(u), Some(a), Some(definition)) = (
            fixture.universe,
            fixture.address,
            definitions.definitions.get(&fixture.uuid),
        ) {
            let end = a.saturating_add(
                light_application::mvr_import::primary_footprint(definition).saturating_sub(1),
            );
            let owner = owners.get(&fixture.uuid).map(|id| id.0.to_string());
            if occupied
                .iter()
                .any(|(other_universe, start, footprint, id)| {
                    owner.as_ref() != Some(id)
                        && *other_universe == u
                        && *start <= end
                        && start.saturating_add(footprint.saturating_sub(1)) >= a
                })
            {
                address_conflicts.push(format!(
                    "{} conflicts at universe {} address {}-{}",
                    fixture.name, u, a, end
                ));
            }
        }
    }
    let token = Uuid::new_v4();
    let preview = MvrImportPreview {
        token,
        fixtures: document
            .fixtures
            .iter()
            .map(|f| MvrPreviewFixture {
                uuid: f.uuid,
                name: f.name.clone(),
                gdtf_spec: f.gdtf_spec.clone(),
                gdtf_mode: f.gdtf_mode.clone(),
                universe: f.universe,
                address: f.address,
                matched: definitions.definitions.contains_key(&f.uuid),
            })
            .collect(),
        scenery: document.geometry.len(),
        missing_profiles,
        warnings: definitions
            .warnings
            .iter()
            .cloned()
            .chain(address_conflicts.iter().cloned())
            .collect(),
        address_conflicts,
    };
    state.active_show.stage_mvr_import(
        token,
        StagedMvrImport {
            document,
            definitions,
            created: Instant::now(),
        },
    );
    Ok(preview)
}

/// Preview owns the exact source/mode binding. Both operator surfaces use the same parser.
pub(super) use light_application::mvr_import::MvrDefinitions;
mod profiles;
pub(super) use profiles::prepare_mvr_profiles;

pub(super) fn mvr_definitions(
    state: &AppState,
    document: &light_mvr::MvrDocument,
) -> Result<MvrDefinitions, ApiError> {
    let profiles = state
        .installation
        .fixture_profiles()
        .map_err(ApiError::fixture)?;
    let legacy = state
        .installation
        .fixture_definitions()
        .map_err(ApiError::fixture)?;
    light_application::mvr_import::bind_mvr_sources(
        document,
        &profiles,
        &legacy,
        |id, revision| {
            let value = state
                .installation
                .fixture_profile_revision_document(id, revision)
                .map_err(|e| {
                    light_application::ActionError::new(
                        light_application::ActionErrorKind::Invalid,
                        e.to_string(),
                    )
                })?;
            value.map(serde_json::from_value).transpose().map_err(|e| {
                light_application::ActionError::new(
                    light_application::ActionErrorKind::Invalid,
                    e.to_string(),
                )
            })
        },
        |profile| {
            super::fixture_api::unknown_canonical_attributes(state, profile)
                .into_iter()
                .map(|(name, _)| name)
                .collect()
        },
    )
    .map_err(mvr_api_error)
}
pub(super) fn mvr_api_error(error: light_application::ActionError) -> ApiError {
    ApiError {
        status: if error.kind == light_application::ActionErrorKind::Conflict {
            StatusCode::CONFLICT
        } else {
            StatusCode::BAD_REQUEST
        },
        message: error.message,
    }
}
