use super::*;

#[derive(Deserialize)]
pub(super) struct HighlightActionInput {
    pub(super) action: HighlightAction,
}

pub(super) async fn highlight_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<HighlightState>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let transition = current_highlight_transition(&state, &session)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    if apply_highlight_selection_write(&state, &session, transition.working_selection.as_ref())? {
        emit(
            &state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"source":"highlight_status_reconcile"}),
        );
    }
    sync_highlight_output(&state);
    Ok(Json(transition.state))
}

pub(super) async fn highlight_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<HighlightActionInput>,
) -> Result<Json<HighlightState>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let selection = state
        .programmers
        .selection(session.id)
        .ok_or_else(|| ApiError::not_found("programmer selection"))?;
    let snapshot = state.engine.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    let transition = state
        .highlight
        .action_guarded(
            session.desk.id,
            session.user.id,
            Some(&session.user.name),
            input.action,
            &selection,
            &fixtures,
            &groups,
            programmer.blind || programmer.preview,
        )
        .map_err(|error| match error {
            HighlightError::OwnedByAnotherUser(_) => ApiError::conflict(error.to_string()),
        })?;
    if apply_highlight_selection_write(&state, &session, transition.working_selection.as_ref())? {
        emit(
            &state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"source":"highlight","action":input.action}),
        );
    }
    sync_highlight_output(&state);
    emit(
        &state,
        "highlight_changed",
        serde_json::json!({
            "desk_id": session.desk.id,
            "user_id": session.user.id,
            "action": input.action,
            "state": &transition.state,
        }),
    );
    send_osc_feedback(&state, false);
    Ok(Json(transition.state))
}

pub(super) fn highlight_fixture_summaries(
    fixtures: &[light_fixture::PatchedFixture],
) -> Vec<HighlightFixture> {
    let mut summaries = Vec::new();
    let mut seen = HashSet::new();
    for fixture in fixtures {
        let base_name = if fixture.name.trim().is_empty() {
            fixture.definition.display_name()
        } else {
            &fixture.name
        };
        if seen.insert(fixture.fixture_id) {
            summaries.push(HighlightFixture {
                fixture_id: fixture.fixture_id,
                name: Some(base_name.to_owned()),
                number: fixture.fixture_number,
            });
        }
        for patched_head in &fixture.logical_heads {
            if !seen.insert(patched_head.fixture_id) {
                continue;
            }
            let head_name = fixture
                .definition
                .heads
                .iter()
                .find(|head| head.index == patched_head.head_index)
                .map(|head| head.name.as_str())
                .unwrap_or("Head");
            summaries.push(HighlightFixture {
                fixture_id: patched_head.fixture_id,
                name: Some(format!("{base_name} / {head_name}")),
                number: fixture.fixture_number,
            });
        }
    }
    summaries
}

pub(super) fn highlight_groups(
    snapshot: &EngineSnapshot,
) -> HashMap<String, light_programmer::GroupDefinition> {
    snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect()
}

pub(super) fn apply_highlight_selection_write(
    state: &AppState,
    session: &Session,
    write: Option<&HighlightSelectionWrite>,
) -> Result<bool, ApiError> {
    let Some(write) = write else {
        return Ok(false);
    };
    match write.expression.clone() {
        Some(expression) => {
            state
                .programmers
                .select_expression(session.id, write.selected.clone(), expression);
        }
        None => {
            state.programmers.select(session.id, write.selected.clone());
        }
    }
    let selection = state
        .programmers
        .selection(session.id)
        .ok_or_else(|| ApiError::not_found("programmer selection"))?;
    state
        .highlight
        .acknowledge_internal_selection(session.desk.id, session.user.id, &selection);
    persist_programmer(state, session)?;
    Ok(true)
}

pub(super) fn current_highlight_transition(
    state: &AppState,
    session: &Session,
) -> Option<HighlightTransition> {
    let programmer = state.programmers.get(session.id)?;
    let selection = state.programmers.selection(session.id)?;
    let snapshot = state.engine.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    Some(state.highlight.status(
        session.desk.id,
        session.user.id,
        Some(&session.user.name),
        &selection,
        &fixtures,
        &groups,
        programmer.blind || programmer.preview,
    ))
}

pub(super) fn reconcile_highlight_selection(
    state: &AppState,
    session: &Session,
    source: &str,
) -> Option<HighlightState> {
    let transition = current_highlight_transition(state, session)?;
    let selection_changed = match apply_highlight_selection_write(
        state,
        session,
        transition.working_selection.as_ref(),
    ) {
        Ok(changed) => changed,
        Err(error) => {
            emit(
                state,
                "highlight_rejected",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "user_id":session.user.id,
                    "source":source,
                    "error":error.message,
                }),
            );
            return None;
        }
    };
    if selection_changed {
        emit(
            state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"source":source,"action":"highlight_selection_reconcile"}),
        );
    }
    sync_highlight_output(state);
    emit(
        state,
        "highlight_changed",
        serde_json::json!({
            "desk_id":session.desk.id,
            "user_id":session.user.id,
            "source":source,
            "state":&transition.state,
        }),
    );
    send_osc_feedback(state, false);
    Some(transition.state)
}

pub(super) fn sync_highlight_output(state: &AppState) {
    let mut fixtures = state
        .highlight
        .output_fixtures()
        .into_iter()
        .collect::<HashSet<_>>();
    for preview in state.patch_preview_highlights.lock().values() {
        fixtures.extend(preview.iter().copied());
    }
    state.engine.set_highlighted_fixtures(fixtures);
}

#[derive(Deserialize)]
pub(super) struct PatchPreviewHighlightInput {
    pub(super) active: bool,
    #[serde(default)]
    pub(super) fixture_ids: Vec<light_core::FixtureId>,
}

pub(super) async fn patch_preview_highlight(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PatchPreviewHighlightInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let allowed = state.configuration.read().patch_preview_highlight_dmx;
    let mut active = false;
    if allowed && input.active && !input.fixture_ids.is_empty() {
        let known = state
            .engine
            .snapshot()
            .fixtures
            .iter()
            .flat_map(selectable_fixture_ids)
            .collect::<HashSet<_>>();
        let fixtures = input
            .fixture_ids
            .into_iter()
            .filter(|fixture| known.contains(fixture))
            .collect::<HashSet<_>>();
        active = !fixtures.is_empty();
        if active {
            state
                .patch_preview_highlights
                .lock()
                .insert(session.id, fixtures);
        } else {
            state.patch_preview_highlights.lock().remove(&session.id);
        }
    } else {
        state.patch_preview_highlights.lock().remove(&session.id);
    }
    sync_highlight_output(&state);
    emit(
        &state,
        "patch_preview_highlight_changed",
        serde_json::json!({"session_id":session.id,"active":active}),
    );
    Ok(Json(serde_json::json!({"active":active,"allowed":allowed})))
}

pub(super) fn reconcile_highlight_capture_mode(
    state: &AppState,
    session: &Session,
    source: &str,
) -> Option<HighlightState> {
    reconcile_highlight_selection(state, session, source)
}
