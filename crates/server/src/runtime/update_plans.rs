use super::*;

pub(super) async fn preview_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateApiRequest>,
) -> Result<Json<UpdatePreviewResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(preview_update_request(&state, &session, &request)?))
}

pub(super) fn plan_update_request(
    state: &AppState,
    session: &Session,
    store: &ShowStore,
    request: &UpdateApiRequest,
) -> Result<update::AtomicUpdatePlan, ApiError> {
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let content = programmer.update_content();
    let programmer_revision = update_content_revision(&content)?;
    if request
        .expected_programmer_revision
        .as_ref()
        .is_some_and(|expected| expected != &programmer_revision)
    {
        return Err(ApiError::conflict(
            "programmer content changed after the Update preview; preview again",
        ));
    }
    match request.target.family {
        UpdateApiTargetFamily::Cue => {
            let update::UpdateMode::Cue(mode) = request.mode else {
                return Err(ApiError::bad_request(
                    "Cue targets require one of the four Cue Update modes",
                ));
            };
            let target =
                resolve_update_cue_target(&request.target, &active_update_cue_contexts(state))?;
            let id = target.cue_list_id.0.to_string();
            let object = stored_update_object(store, "cue_list", &id)?;
            let cue_list = serde_json::from_value::<light_playback::CueList>(object.body)
                .map_err(|error| ApiError::bad_request(format!("invalid Cuelist: {error}")))?;
            update::plan_cue_update(
                &cue_list,
                object.revision,
                request.expected_revision.unwrap_or(object.revision),
                &target,
                mode,
                &content,
            )
            .map_err(update_api_error)
        }
        UpdateApiTargetFamily::Preset => {
            let update::UpdateMode::ExistingContent(mode) = request.mode else {
                return Err(ApiError::bad_request(
                    "Preset targets require Update Existing or Add New",
                ));
            };
            let id = request
                .target
                .object_id
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("Preset Update requires object_id"))?;
            let object = stored_update_object(store, "preset", id)?;
            let preset = serde_json::from_value::<light_programmer::Preset>(object.body)
                .map_err(|error| ApiError::bad_request(format!("invalid Preset: {error}")))?;
            update::plan_preset_update(
                id,
                &preset,
                object.revision,
                request.expected_revision.unwrap_or(object.revision),
                mode,
                &content,
            )
            .map_err(update_api_error)
        }
        UpdateApiTargetFamily::Group => {
            let update::UpdateMode::ExistingContent(mode) = request.mode else {
                return Err(ApiError::bad_request(
                    "Group targets require Update Existing or Add New",
                ));
            };
            let id = request
                .target
                .object_id
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("Group Update requires object_id"))?;
            let object = stored_update_object(store, "group", id)?;
            let mut group =
                serde_json::from_value::<light_programmer::GroupDefinition>(object.body)
                    .map_err(|error| ApiError::bad_request(format!("invalid Group: {error}")))?;
            group.id = id.to_owned();
            let groups = state
                .engine
                .snapshot()
                .groups
                .iter()
                .cloned()
                .map(|candidate| (candidate.id.clone(), candidate))
                .collect::<HashMap<_, _>>();
            let membership =
                light_programmer::resolve_group(id, &groups).map_err(ApiError::bad_request)?;
            update::plan_group_update(
                &group,
                &membership,
                object.revision,
                request.expected_revision.unwrap_or(object.revision),
                mode,
                &content,
            )
            .map_err(update_api_error)
        }
    }
}

pub(super) fn perform_update(
    state: &AppState,
    session: &Session,
    request: &UpdateApiRequest,
) -> Result<update::UpdateResult, ApiError> {
    let context = operator_action_context(session, light_application::ActionSource::Http);
    perform_update_from(state, session, request, &context)
}

pub(super) fn perform_update_from(
    state: &AppState,
    session: &Session,
    request: &UpdateApiRequest,
    context: &light_application::ActionContext,
) -> Result<update::UpdateResult, ApiError> {
    let _activation = state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| ApiError::conflict("the active show is changing; retry Update"))?;
    let (entry, store) = active_show_store(state).map_err(ApiError::bad_request)?;
    let plan = plan_update_request(state, session, &store, request)?;
    let kind = plan.object_kind().to_owned();
    let id = plan.object_id().to_owned();
    let body = plan
        .body()
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let object_kind = active_show_update_kind(&plan);
    let action = active_show_object_action(
        context.clone(),
        entry.id,
        vec![put_active_show_object(
            object_kind,
            id.clone(),
            plan.expected_revision,
            body,
        )],
    );
    let revision = run_active_show_object_action(state, action)?.changes[0].object_revision;
    let result = plan.complete(revision);
    emit(
        state,
        "show_object_changed",
        serde_json::json!({
            "show_id":entry.id,
            "kind":kind,
            "id":id,
            "revision":revision,
            "source":"update",
            "result":result,
            "session_id":session.id,
        }),
    );
    Ok(result)
}

fn active_show_update_kind(
    plan: &update::AtomicUpdatePlan,
) -> light_application::ActiveShowObjectKind {
    match &plan.object {
        update::PlannedUpdateObject::CueList(_) => light_application::ActiveShowObjectKind::CueList,
        update::PlannedUpdateObject::Preset(_) => light_application::ActiveShowObjectKind::Preset,
        update::PlannedUpdateObject::Group(_) => light_application::ActiveShowObjectKind::Group,
    }
}

pub(super) async fn apply_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateApiRequest>,
) -> Result<Json<update::UpdateResult>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(perform_update(&state, &session, &request)?))
}

pub(super) fn referenced_update_targets(
    state: &AppState,
    session: &Session,
) -> Result<Vec<UpdateApiTarget>, ApiError> {
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let mut targets = active_update_cue_contexts(state)
        .into_iter()
        .map(|context| UpdateApiTarget {
            family: UpdateApiTargetFamily::Cue,
            object_id: Some(context.cue_list_id.0.to_string()),
            playback_number: Some(context.playback_number),
            cue_id: Some(context.cue_id),
            cue_number: Some(context.cue_number),
            validate_active_context: true,
        })
        .collect::<Vec<_>>();
    if let Some(id) = programmer
        .active_context
        .as_deref()
        .and_then(|context| context.strip_prefix("preset:"))
    {
        targets.push(UpdateApiTarget {
            family: UpdateApiTargetFamily::Preset,
            object_id: Some(id.to_owned()),
            playback_number: None,
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        });
    }
    let mut group_ids = programmer.group_values.keys().cloned().collect::<Vec<_>>();
    match programmer.selection_expression {
        Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. }) => {
            group_ids.push(group_id)
        }
        Some(light_programmer::SelectionExpression::Sources { items }) => {
            group_ids.extend(items.into_iter().filter_map(|item| match item {
                light_programmer::SelectionReference::LiveGroup { group_id }
                | light_programmer::SelectionReference::RemoveLiveGroup { group_id } => {
                    Some(group_id)
                }
                _ => None,
            }));
        }
        _ => {}
    }
    group_ids.sort();
    group_ids.dedup();
    targets.extend(group_ids.into_iter().map(|id| UpdateApiTarget {
        family: UpdateApiTargetFamily::Group,
        object_id: Some(id),
        playback_number: None,
        cue_id: None,
        cue_number: None,
        validate_active_context: false,
    }));
    Ok(targets)
}

pub(super) async fn update_targets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<UpdateTargetsQuery>,
) -> Result<Json<Vec<UpdateMenuResponseEntry>>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let mut entries = Vec::new();
    for target in referenced_update_targets(&state, &session)? {
        let modes = match target.family {
            UpdateApiTargetFamily::Cue => (
                update::UpdateMode::Cue(update::CueUpdateMode::ExistingOnly),
                update::UpdateMode::Cue(update::CueUpdateMode::AddNew),
            ),
            UpdateApiTargetFamily::Preset | UpdateApiTargetFamily::Group => (
                update::UpdateMode::ExistingContent(update::ExistingContentMode::UpdateExisting),
                update::UpdateMode::ExistingContent(update::ExistingContentMode::AddNew),
            ),
        };
        let existing = preview_update_request(
            &state,
            &session,
            &UpdateApiRequest {
                target: target.clone(),
                mode: modes.0,
                expected_revision: None,
                expected_programmer_revision: None,
            },
        );
        let add_new = preview_update_request(
            &state,
            &session,
            &UpdateApiRequest {
                target: target.clone(),
                mode: modes.1,
                expected_revision: None,
                expected_programmer_revision: None,
            },
        );
        let (Ok(existing), Ok(add_new)) = (existing, add_new) else {
            continue;
        };
        if query.filter == update::UpdateTargetFilter::EligibleForUpdateExisting
            && !existing.preview.has_real_change()
        {
            continue;
        }
        entries.push(UpdateMenuResponseEntry {
            target,
            revision: existing.revision,
            active_or_referenced: true,
            existing_preview: existing,
            add_new_preview: add_new,
        });
    }
    Ok(Json(entries))
}
