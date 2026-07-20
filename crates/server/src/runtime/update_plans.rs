use super::*;

pub(super) async fn preview_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateApiRequest>,
) -> Result<Json<UpdatePreviewResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(preview_update_application(
        &state, &session, &request,
    )?))
}

pub(super) fn preview_update_application(
    state: &AppState,
    session: &Session,
    request: &UpdateApiRequest,
) -> Result<UpdatePreviewResponse, ApiError> {
    let show_id = state
        .active_show
        .read()
        .as_ref()
        .map(|show| show.id)
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let context = operator_action_context(session, light_application::ActionSource::Http)
        .with_request_id(format!("legacy-update-preview-{}", Uuid::new_v4()));
    let command = light_application::programming_update::ProgrammingUpdatePreviewRequest {
        show_id,
        target: application_update_target(state, &request.target)?,
        mode: request.mode,
    };
    let ports = ServerProgrammingUpdatePorts::new(state.clone(), session.clone(), false, false);
    let result = state
        .programming
        .preview_update(
            light_application::ActionEnvelope { context, command },
            &state.active_show_service,
            &ports,
        )
        .map_err(programming_action_error)?;
    Ok(UpdatePreviewResponse {
        revision: result.object_revision,
        show_revision: result.show_revision.value(),
        programmer_revision: result.programmer_revision,
        preview: result.preview,
    })
}

pub(super) fn perform_update(
    state: &AppState,
    session: &Session,
    request: &UpdateApiRequest,
) -> Result<update::UpdateResult, ApiError> {
    let context = operator_action_context(session, light_application::ActionSource::Http);
    perform_update_with_boundary(
        state,
        session,
        request,
        &context,
        UpdateProgrammingBoundary::Unowned,
    )
}

pub(super) fn perform_update_from(
    state: &AppState,
    session: &Session,
    request: &UpdateApiRequest,
    context: &light_application::ActionContext,
) -> Result<update::UpdateResult, ApiError> {
    perform_update_with_boundary(
        state,
        session,
        request,
        context,
        UpdateProgrammingBoundary::HeldByCaller,
    )
}

#[derive(Clone, Copy)]
enum UpdateProgrammingBoundary {
    Unowned,
    HeldByCaller,
}

fn perform_update_with_boundary(
    state: &AppState,
    session: &Session,
    request: &UpdateApiRequest,
    context: &light_application::ActionContext,
    programming: UpdateProgrammingBoundary,
) -> Result<update::UpdateResult, ApiError> {
    validate_confirmed_legacy_cue(request)?;
    let show_id = state
        .active_show
        .read()
        .as_ref()
        .map(|show| show.id)
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let command = light_application::programming_update::ProgrammingUpdateCommand {
        show_id,
        target: application_update_target(state, &request.target)?,
        mode: request.mode,
        expected_object_revision: request.expected_revision,
        expected_programmer_revision: request.expected_programmer_revision.clone(),
        expected_show_revision: request
            .expected_show_revision
            .map(light_show::PortableShowRevision::from_value),
    };
    let context = if context.request_id.is_some() {
        context.clone()
    } else {
        context
            .clone()
            .with_request_id(format!("legacy-update-{}", context.correlation_id))
    };
    let within_interaction = matches!(programming, UpdateProgrammingBoundary::HeldByCaller);
    let ports =
        ServerProgrammingUpdatePorts::new(state.clone(), session.clone(), within_interaction, true);
    let action = light_application::ActionEnvelope { context, command };
    let result = match programming {
        UpdateProgrammingBoundary::Unowned => {
            state
                .programming
                .handle_update(action, &state.active_show_service, &ports)
        }
        UpdateProgrammingBoundary::HeldByCaller => {
            state
                .programming
                .update_within_interaction(action, &state.active_show_service, &ports)
        }
    }
    .map_err(programming_action_error)?;
    if !result.replayed {
        publish_legacy_update(state, session, &result);
    }
    Ok(result.outcome.summary)
}

fn validate_confirmed_legacy_cue(request: &UpdateApiRequest) -> Result<(), ApiError> {
    let confirmed = request.expected_revision.is_some()
        || request.expected_programmer_revision.is_some()
        || request.expected_show_revision.is_some();
    if !confirmed || request.target.family != UpdateApiTargetFamily::Cue {
        return Ok(());
    }
    if request.target.object_id.is_none()
        || request.target.cue_id.is_none()
        || request.target.cue_number.is_none()
    {
        return Err(ApiError::conflict(
            "confirmed Cue Update requires the exact previewed Cue; preview again",
        ));
    }
    Ok(())
}

pub(super) fn application_update_target(
    state: &AppState,
    target: &UpdateApiTarget,
) -> Result<light_application::programming_update::ProgrammingUpdateTargetRequest, ApiError> {
    use light_application::programming_update::ProgrammingUpdateTargetRequest;
    match target.family {
        UpdateApiTargetFamily::Cue => {
            let resolved = resolve_update_cue_target(target, &active_update_cue_contexts(state))?;
            Ok(ProgrammingUpdateTargetRequest::Cue {
                cue_list_id: resolved.cue_list_id,
                playback_number: resolved.playback_number,
                cue_id: Some(resolved.cue_id),
                cue_number: Some(resolved.cue_number),
                validate_active_context: target.validate_active_context,
            })
        }
        UpdateApiTargetFamily::Preset => Ok(ProgrammingUpdateTargetRequest::Preset {
            object_id: required_update_object_id(target, "Preset")?,
        }),
        UpdateApiTargetFamily::Group => Ok(ProgrammingUpdateTargetRequest::Group {
            object_id: required_update_object_id(target, "Group")?,
        }),
    }
}

fn required_update_object_id(target: &UpdateApiTarget, label: &str) -> Result<String, ApiError> {
    target
        .object_id
        .clone()
        .ok_or_else(|| ApiError::bad_request(format!("{label} Update requires object_id")))
}

fn publish_legacy_update(
    state: &AppState,
    session: &Session,
    result: &light_application::programming_update::ProgrammingUpdateResult,
) {
    let projection = &result.outcome.projection;
    emit(
        state,
        "show_object_changed",
        serde_json::json!({
            "show_id":projection.show_id,
            "kind":projection.kind.as_str(),
            "id":projection.object_id,
            "revision":projection.object_revision,
            "source":"update",
            "result":result.outcome.summary,
            "session_id":session.id,
            "application_event_sequence":result.outcome.event_sequence,
        }),
    );
}

pub(super) async fn apply_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateApiRequest>,
) -> Result<Json<update::UpdateResult>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(perform_update(&state, &session, &request)?))
}

pub(super) async fn update_targets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<UpdateTargetsQuery>,
) -> Result<Json<Vec<UpdateMenuResponseEntry>>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let show_id = state
        .active_show
        .read()
        .as_ref()
        .map(|show| show.id)
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let context = operator_action_context(&session, light_application::ActionSource::Http)
        .with_request_id(format!("legacy-update-targets-{}", Uuid::new_v4()));
    let command = light_application::programming_update::ProgrammingUpdateTargetsRequest {
        show_id,
        filter: query.filter,
    };
    let ports = ServerProgrammingUpdatePorts::new(state.clone(), session, false, false);
    let result = state
        .programming
        .update_targets(
            light_application::ActionEnvelope { context, command },
            &state.active_show_service,
            &ports,
        )
        .map_err(programming_action_error)?;
    let show_revision = result.show_revision.value();
    Ok(Json(
        result
            .entries
            .into_iter()
            .map(|entry| legacy_menu_entry(entry, show_revision))
            .collect(),
    ))
}

fn legacy_menu_entry(
    entry: light_application::programming_update::ProgrammingUpdateMenuEntry,
    show_revision: u64,
) -> UpdateMenuResponseEntry {
    let revision = entry.object_revision;
    let programmer_revision = entry.programmer_revision;
    UpdateMenuResponseEntry {
        target: legacy_update_target(entry.target),
        revision,
        active_or_referenced: entry.active_or_referenced,
        existing_preview: UpdatePreviewResponse {
            revision,
            show_revision,
            programmer_revision: programmer_revision.clone(),
            preview: entry.existing_preview,
        },
        add_new_preview: UpdatePreviewResponse {
            revision,
            show_revision,
            programmer_revision,
            preview: entry.add_new_preview,
        },
    }
}

fn legacy_update_target(
    target: light_application::programming_update::ProgrammingUpdateTargetRequest,
) -> UpdateApiTarget {
    use light_application::programming_update::ProgrammingUpdateTargetRequest;
    match target {
        ProgrammingUpdateTargetRequest::Cue {
            cue_list_id,
            playback_number,
            cue_id,
            cue_number,
            validate_active_context,
        } => UpdateApiTarget {
            family: UpdateApiTargetFamily::Cue,
            object_id: Some(cue_list_id.0.to_string()),
            playback_number,
            cue_id,
            cue_number,
            validate_active_context,
        },
        ProgrammingUpdateTargetRequest::Preset { object_id } => {
            legacy_object_target(UpdateApiTargetFamily::Preset, object_id)
        }
        ProgrammingUpdateTargetRequest::Group { object_id } => {
            legacy_object_target(UpdateApiTargetFamily::Group, object_id)
        }
    }
}

fn legacy_object_target(family: UpdateApiTargetFamily, object_id: String) -> UpdateApiTarget {
    UpdateApiTarget {
        family,
        object_id: Some(object_id),
        playback_number: None,
        cue_id: None,
        cue_number: None,
        validate_active_context: false,
    }
}
