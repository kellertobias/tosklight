use super::super::*;

pub(super) fn active_show_is(state: &AppState, show_id: light_core::ShowId) -> bool {
    state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|show| show.id == show_id)
}

pub(super) struct ActiveMvrImport {
    pub entry: ShowEntry,
    pub document: light_mvr::MvrDocument,
    pub definitions: Vec<light_fixture::FixtureDefinition>,
    pub new_definitions: Vec<(light_fixture::FixtureDefinition, Vec<u8>)>,
    pub resolutions: HashMap<Uuid, MvrResolution>,
}

pub(super) async fn apply_active_mvr_import(
    state: &AppState,
    session: &Session,
    import: ActiveMvrImport,
) -> Result<Json<ApplyMvrResult>, ApiError> {
    let ActiveMvrImport {
        entry,
        document,
        definitions,
        new_definitions,
        resolutions,
    } = import;
    let context = operator_action_context(session, light_application::ActionSource::Http);
    let action = light_application::ActionEnvelope {
        context,
        command: light_application::ApplyActiveMvrImportCommand {
            show_id: entry.id,
            document,
            definitions,
            resolutions: application_mvr_resolutions(resolutions),
        },
    };
    let worker_state = state.clone();
    let service = light_application::MvrImportService::new(state.active_show_service.clone());
    let result = tokio::task::spawn_blocking(move || {
        let ports = ServerShowPatchPorts::new(worker_state);
        service.apply(action, &ports)
    })
    .await
    .map_err(|error| ApiError::internal(format!("MVR import task failed: {error}")))?
    .map_err(application_api_error)?;
    let mut warnings = result.warnings;
    publish_mvr_definitions(state, new_definitions, &mut warnings);
    emit(
        state,
        "mvr_imported",
        serde_json::json!({
            "show": entry,
            "fixtures": result.imported_fixtures,
            "unresolved": result.unresolved_fixtures,
            "scenery": 0,
        }),
    );
    Ok(Json(ApplyMvrResult {
        show: entry,
        imported_fixtures: result.imported_fixtures,
        unresolved_fixtures: result.unresolved_fixtures,
        imported_scenery: 0,
        opened: true,
        warnings,
    }))
}

fn application_mvr_resolutions(
    resolutions: HashMap<Uuid, MvrResolution>,
) -> HashMap<Uuid, light_application::MvrImportResolution> {
    resolutions
        .into_iter()
        .map(|(id, resolution)| {
            let resolution = match resolution {
                MvrResolution::Import => light_application::MvrImportResolution::Import,
                MvrResolution::Skip => light_application::MvrImportResolution::Skip,
                MvrResolution::ImportUnpatched => {
                    light_application::MvrImportResolution::ImportUnpatched
                }
                MvrResolution::Replace => light_application::MvrImportResolution::Replace,
                MvrResolution::Address { universe, address } => {
                    light_application::MvrImportResolution::Address { universe, address }
                }
            };
            (id, resolution)
        })
        .collect()
}

pub(super) fn publish_mvr_definitions(
    state: &AppState,
    definitions: Vec<(light_fixture::FixtureDefinition, Vec<u8>)>,
    warnings: &mut Vec<String>,
) {
    for (definition, source) in definitions {
        let label = format!(
            "{} {} mode {}",
            definition.manufacturer, definition.model, definition.mode
        );
        let json = match serde_json::to_string(&definition) {
            Ok(json) => json,
            Err(error) => {
                warnings.push(format!(
                    "Imported {label}, but could not publish its fixture profile: {error}"
                ));
                continue;
            }
        };
        if let Err(error) = state
            .fixture_library
            .lock()
            .import_json_with_source(&json, Some(&source))
        {
            warnings.push(format!(
                "Imported {label}, but could not publish its fixture profile: {error}"
            ));
        }
    }
}

fn application_api_error(error: light_application::ActionError) -> ApiError {
    let status = match error.kind {
        light_application::ActionErrorKind::Invalid => StatusCode::BAD_REQUEST,
        light_application::ActionErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
        light_application::ActionErrorKind::Forbidden => StatusCode::FORBIDDEN,
        light_application::ActionErrorKind::NotFound => StatusCode::NOT_FOUND,
        light_application::ActionErrorKind::Conflict | light_application::ActionErrorKind::Busy => {
            StatusCode::CONFLICT
        }
        light_application::ActionErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        light_application::ActionErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiError {
        status,
        message: error.message,
    }
}
