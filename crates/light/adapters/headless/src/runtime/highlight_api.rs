use super::*;
use light_application::{ActionEnvelope, ActionSource, HighlightCommand};

pub(super) async fn highlight_status(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<Json<HighlightState>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let status = run_highlight_http_interaction(
        &state,
        &session,
        ProgrammingLockPolicy::AllowLockedReconciliation,
        reconcile_highlight_status,
    )
    .await?;
    Ok(Json(status))
}

fn reconcile_highlight_status(
    state: &AppState,
    session: &Session,
) -> Result<HighlightState, ApiError> {
    execute_highlight(
        state,
        session,
        HighlightCommand::status(),
        ActionSource::Http,
        false,
    )
}

pub(super) async fn highlight_action(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<light_wire::v2::output_control::HighlightActionRequest>,
) -> Result<Json<HighlightState>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    output_runtime_v2::validate_request_id(&input.request_id).map_err(ApiError::bad_request)?;
    let action = highlight_action_from_wire(input.action);
    let highlight = run_highlight_http_interaction(
        &state,
        &session,
        ProgrammingLockPolicy::RequireUnlocked,
        move |state, session| apply_highlight_action(state, session, action, ActionSource::Http),
    )
    .await?;
    Ok(Json(highlight))
}

pub(super) fn highlight_action_from_wire(
    action: light_wire::v2::output_control::HighlightAction,
) -> HighlightAction {
    use light_wire::v2::output_control::HighlightAction as Wire;
    match action {
        Wire::On => HighlightAction::On,
        Wire::Off => HighlightAction::Off,
        Wire::Toggle => HighlightAction::Toggle,
        Wire::Next => HighlightAction::Next,
        Wire::Previous => HighlightAction::Previous,
        Wire::All => HighlightAction::All,
    }
}

async fn run_highlight_http_interaction<T: Send + 'static>(
    state: &AppState,
    session: &Session,
    lock_policy: ProgrammingLockPolicy,
    operation: impl FnOnce(&AppState, &Session) -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    let activation = state.active_show.acquire().await;
    let worker_state = state.clone();
    let worker_session = session.clone();
    let (completed, _activation) = tokio::task::spawn_blocking(move || {
        let context =
            programming_context(&worker_session, light_application::ActionSource::Http, None);
        (
            run_programming_interaction(
                &worker_state,
                &worker_session,
                &context,
                "http",
                lock_policy,
                || operation(&worker_state, &worker_session),
            ),
            activation,
        )
    })
    .await
    .map_err(|error| ApiError::internal(format!("Highlight interaction task failed: {error}")))?;
    completed?.output
}

pub(super) fn apply_highlight_action(
    state: &AppState,
    session: &Session,
    action: HighlightAction,
    source: ActionSource,
) -> Result<HighlightState, ApiError> {
    execute_highlight(
        state,
        session,
        HighlightCommand::action(action),
        source,
        false,
    )
}

pub(super) fn execute_highlight(
    state: &AppState,
    session: &Session,
    command: HighlightCommand,
    source: ActionSource,
    preserve_osc_selection_write_failure: bool,
) -> Result<HighlightState, ApiError> {
    let context = programming_context(session, source, None);
    let ports = if preserve_osc_selection_write_failure {
        highlight_service_adapter::HeadlessHighlightPorts::for_osc(state, session)
    } else {
        highlight_service_adapter::HeadlessHighlightPorts::new(state, session)
    };
    state
        .highlight
        .handle(ActionEnvelope { context, command }, &ports)
        .map(|result| result.state)
        .map_err(highlight_service_adapter::api_error)
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
                .programming
                .select_expression(session.id, write.selected.clone(), expression);
        }
        None => {
            state.programming.select(session.id, write.selected.clone());
        }
    }
    let selection = state
        .programming
        .selection(session.id)
        .ok_or_else(|| ApiError::not_found("programmer selection"))?;
    state
        .highlight
        .acknowledge_selection(session.desk.id, &selection);
    persist_programmer(state, session)?;
    Ok(true)
}

#[cfg(test)]
pub(super) fn current_highlight_transition(
    state: &AppState,
    session: &Session,
) -> Option<light_programmer::HighlightTransition> {
    let programmer = state.programming.get(session.id)?;
    let selection = state.programming.selection(session.id)?;
    let snapshot = state.output.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    Some(state.highlight.transition(
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
    match execute_highlight(
        state,
        session,
        HighlightCommand::reconcile(source),
        ActionSource::System,
        false,
    ) {
        Ok(highlight) => Some(highlight),
        Err(error) => {
            emit(
                state,
                "highlight_rejected",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "source":source,
                    "error":error.message,
                }),
            );
            None
        }
    }
}

pub(super) fn sync_highlight_output(state: &AppState) {
    state
        .output
        .set_highlight_layers(state.highlight.output_layers());
}

pub(super) async fn patch_preview_highlight(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<light_wire::v2::output_control::PatchPreviewHighlightRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    output_runtime_v2::validate_request_id(&input.request_id).map_err(ApiError::bad_request)?;
    Ok(Json(apply_patch_preview_highlight(&state, &session, input)))
}

pub(super) fn apply_patch_preview_highlight(
    state: &AppState,
    session: &Session,
    input: light_wire::v2::output_control::PatchPreviewHighlightRequest,
) -> serde_json::Value {
    let allowed = state
        .installation
        .configuration()
        .patch_preview_highlight_dmx;
    let mut active = false;
    if allowed && input.active && !input.fixture_ids.is_empty() {
        let known = state
            .output
            .snapshot()
            .fixtures
            .iter()
            .flat_map(selectable_fixture_ids)
            .collect::<HashSet<_>>();
        let fixtures = input
            .fixture_ids
            .into_iter()
            .map(light_core::FixtureId)
            .filter(|fixture| known.contains(fixture))
            .collect::<HashSet<_>>();
        active = state.highlight.set_patch_preview(session.id, fixtures);
    } else {
        state.highlight.remove_patch_preview(session.id);
    }
    sync_highlight_output(state);
    emit(
        state,
        "patch_preview_highlight_changed",
        serde_json::json!({"session_id":session.id,"active":active}),
    );
    serde_json::json!({"active":active,"allowed":allowed})
}

pub(super) fn reconcile_highlight_capture_mode(
    state: &AppState,
    session: &Session,
    source: &str,
) -> Option<HighlightState> {
    reconcile_highlight_selection(state, session, source)
}
