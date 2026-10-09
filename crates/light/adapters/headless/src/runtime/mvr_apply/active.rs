use super::super::*;

pub(super) fn active_show_is(state: &AppState, show_id: light_core::ShowId) -> bool {
    state
        .active_show
        .current()
        .as_ref()
        .is_some_and(|show| show.id == show_id)
}

pub(super) struct ActiveMvrImport {
    pub entry: ShowEntry,
    pub destination_revision: Option<(
        light_show::PortableShowRevision,
        light_show::PortablePatchRevision,
    )>,
    pub document: light_mvr::MvrDocument,
    pub definitions: HashMap<Uuid, light_fixture::FixtureDefinition>,
    pub new_profiles: Vec<light_fixture::FixtureProfile>,
    pub warnings: Vec<String>,
    pub resolutions: HashMap<Uuid, MvrResolution>,
}

pub(super) async fn apply_active_mvr_import(
    state: &AppState,
    session: &Session,
    import: ActiveMvrImport,
) -> Result<Json<ApplyMvrResult>, ApiError> {
    let ActiveMvrImport {
        entry,
        destination_revision,
        document,
        definitions,
        new_profiles,
        mut warnings,
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
    let active_show = state.active_show.clone();
    let result = tokio::task::spawn_blocking(move || {
        let ports = ServerShowPatchPorts::new(worker_state);
        active_show.apply_mvr_import_at_preview_revision(action, destination_revision, &ports)
    })
    .await
    .map_err(|error| ApiError::internal(format!("MVR import task failed: {error}")))?
    .map_err(application_api_error)?;
    warnings.extend(result.warnings);
    publish_mvr_profiles(state, new_profiles, &mut warnings);
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

pub(super) fn publish_mvr_profiles(
    state: &AppState,
    profiles: Vec<light_fixture::FixtureProfile>,
    warnings: &mut Vec<String>,
) {
    let mut inserted = 0;
    for profile in profiles {
        match state
            .installation
            .publish_fixture_profile_revision(&profile)
        {
            Ok(true) => inserted += 1,
            Ok(false) => {}
            Err(error) => warnings.push(format!(
                "Imported {} {}, but could not publish exact fixture profile revision {}: {error}",
                profile.manufacturer, profile.name, profile.revision
            )),
        }
    }
    if inserted > 0 {
        emit(
            state,
            "fixture_library_changed",
            serde_json::json!({"reason":"mvr_import", "profiles":inserted}),
        );
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
