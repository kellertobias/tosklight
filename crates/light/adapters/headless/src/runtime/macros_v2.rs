//! Typed object intents and validation for portable command Macros.

use super::show_object_intents_v2::{ReplayAction, ReplayKey};
use super::show_objects_v2::{active_entry, object_record, validate_request_id};
use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::{macros as wire, show_objects::ShowObjectActionOutcome};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/macros/validate", post(validate_macro))
        .route("/api/v2/macros/actions", post(macro_object_action))
        .route("/api/v2/macros/runtime", get(runtime_snapshot))
        .route("/api/v2/macros/{macro_id}/run", post(run_macro))
        .route("/api/v2/macros/{macro_id}/run-line", post(run_macro_line))
        .route(
            "/api/v2/macros/executions/{execution_id}",
            get(execution_snapshot),
        )
        .route(
            "/api/v2/macros/executions/{execution_id}/undo-line",
            post(undo_run_line),
        )
        .route("/api/v2/macros/executions/cancel", post(cancel_execution))
}

async fn run_macro(
    State(state): State<AppState>,
    Path(macro_id): Path<Uuid>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::MacroRunActionRequest>,
) -> Result<Json<wire::MacroExecutionSnapshot>, ApiError> {
    let session = command_http::authenticate_desk_mutation(&state, &headers, &desk)?;
    let show_id = context.resolve(&state)?;
    let (revision, definition) = macro_for_run(&state, show_id, macro_id)?;
    if request
        .source_revision
        .is_some_and(|expected| expected != revision)
    {
        return Err(ApiError::conflict(format!(
            "Macro revision conflict: expected {}, current {revision}",
            request.source_revision.expect("checked revision")
        )));
    }
    start_execution(
        &state,
        &session,
        definition,
        revision,
        request.trigger.unwrap_or(wire::MacroTrigger::Pool),
        None,
        show_id,
    )
}

async fn run_macro_line(
    State(state): State<AppState>,
    Path(macro_id): Path<Uuid>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::MacroRunLineActionRequest>,
) -> Result<Json<wire::MacroExecutionSnapshot>, ApiError> {
    let session = command_http::authenticate_desk_mutation(&state, &headers, &desk)?;
    let show_id = context.resolve(&state)?;
    let (revision, definition) = macro_for_run(&state, show_id, macro_id)?;
    if request.source_revision != revision {
        return Err(ApiError::conflict(format!(
            "Macro revision conflict: expected {}, current {revision}",
            request.source_revision
        )));
    }
    start_execution(
        &state,
        &session,
        definition,
        revision,
        wire::MacroTrigger::Editor,
        Some(request.line),
        show_id,
    )
}

fn start_execution(
    state: &AppState,
    session: &Session,
    definition: light_application::CommandMacroDefinition,
    source_revision: u64,
    trigger: wire::MacroTrigger,
    only_line: Option<u32>,
    show_id: light_core::ShowId,
) -> Result<Json<wire::MacroExecutionSnapshot>, ApiError> {
    let initial_selection = state
        .programming
        .programmers()
        .selection(session.id)
        .map(|selection| selection.selected)
        .unwrap_or_default();
    let host = Arc::new(ServerMacroExecutionHost {
        state: state.clone(),
        session: session.clone(),
        show_id,
        context: operator_action_context(session, light_application::ActionSource::Macro),
        initial_selection,
    });
    let started = state
        .macros
        .start(
            light_application::CommandMacroRunRequest {
                definition,
                source_revision,
                context: host.context.clone(),
                trigger: application_trigger(trigger),
                only_line,
            },
            host,
        )
        .map_err(|error| ApiError::bad_request(error.message))?;
    emit(
        state,
        "macro_execution_changed",
        serde_json::json!({
            "desk_id": started.desk_id,
            "execution_id": started.execution_id,
            "macro_id": started.macro_id,
            "state": "queued",
        }),
    );
    state
        .events
        .publish(light_application::EventDraft::macro_execution_changed(
            started.clone(),
        ));
    watch_execution(
        state.clone(),
        started.desk_id,
        started.execution_id,
        started.state,
    );
    Ok(Json(execution_wire(started)))
}

fn watch_execution(
    state: AppState,
    desk_id: Uuid,
    execution_id: Uuid,
    mut last_state: light_application::CommandMacroExecutionState,
) {
    std::thread::Builder::new()
        .name(format!("macro-execution-{execution_id}"))
        .spawn(move || {
            loop {
                let Some(snapshot) = state.macros.execution(desk_id, execution_id) else {
                    return;
                };
                if snapshot.state != last_state {
                    last_state = snapshot.state;
                    state
                        .events
                        .publish(light_application::EventDraft::macro_execution_changed(
                            snapshot.clone(),
                        ));
                    emit(
                        &state,
                        "macro_execution_changed",
                        serde_json::to_value(execution_wire(snapshot.clone()))
                            .expect("Macro execution snapshots serialize"),
                    );
                }
                if snapshot.state.is_terminal() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        })
        .expect("spawn Macro execution event watcher");
}

/// Queue a Macro through the same authenticated execution service used by pool/editor actions.
/// Every playback surface reaches this function through the authoritative playback dispatcher.
pub(super) fn start_macro_from_playback(
    state: &AppState,
    session: &Session,
    macro_id: Uuid,
    playback_number: u16,
) -> Result<(), ApiError> {
    let show_id = state
        .active_show
        .current()
        .as_ref()
        .map(|show| show.id)
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let (revision, definition) = macro_for_run(state, show_id, macro_id)?;
    let _started = start_execution(
        state,
        session,
        definition,
        revision,
        wire::MacroTrigger::Playback { playback_number },
        None,
        show_id,
    )?;
    Ok(())
}

/// Ordered desk-WebSocket entry point for Macro pool and editor execution.
pub(super) fn run_macro_live_action(
    state: &AppState,
    session: &Session,
    action: wire::MacroLiveAction,
) -> Result<serde_json::Value, String> {
    let show_id = state
        .active_show
        .current()
        .as_ref()
        .map(|show| show.id)
        .ok_or_else(|| "no show is open".to_owned())?;
    match action {
        wire::MacroLiveAction::Run {
            macro_id,
            source_revision,
            trigger,
        } => {
            let (revision, definition) =
                macro_for_run(state, show_id, macro_id).map_err(|error| error.message)?;
            if source_revision.is_some_and(|expected| expected != revision) {
                return Err(format!(
                    "Macro revision conflict: expected {}, current {revision}",
                    source_revision.expect("checked revision")
                ));
            }
            serde_json::to_value(
                start_execution(state, session, definition, revision, trigger, None, show_id)
                    .map_err(|error| error.message)?
                    .0,
            )
            .map_err(|error| error.to_string())
        }
        wire::MacroLiveAction::RunLine {
            macro_id,
            source_revision,
            line,
        } => {
            let (revision, definition) =
                macro_for_run(state, show_id, macro_id).map_err(|error| error.message)?;
            if source_revision != revision {
                return Err(format!(
                    "Macro revision conflict: expected {source_revision}, current {revision}"
                ));
            }
            serde_json::to_value(
                start_execution(
                    state,
                    session,
                    definition,
                    revision,
                    wire::MacroTrigger::Editor,
                    Some(line),
                    show_id,
                )
                .map_err(|error| error.message)?
                .0,
            )
            .map_err(|error| error.to_string())
        }
    }
}

/// Starts a numbered Macro from the shared command line without allowing Macro-to-Macro calls.
#[allow(private_interfaces)]
pub(crate) fn start_macro_from_command_line(
    state: &AppState,
    session: &Session,
    macro_number: u16,
) -> Result<Uuid, ApiError> {
    let show_id = state
        .active_show
        .current()
        .as_ref()
        .map(|show| show.id)
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let entry = active_entry(state, show_id)?;
    let (_, objects) = ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .objects_with_portable_revision("macro")
        .map_err(ApiError::store)?;
    let object = objects
        .into_iter()
        .find(|object| {
            serde_json::from_value::<light_application::CommandMacroDefinition>(object.body.clone())
                .is_ok_and(|definition| definition.number == macro_number)
        })
        .ok_or_else(|| ApiError::not_found(format!("Macro {macro_number} does not exist")))?;
    let definition = serde_json::from_value(object.body)
        .map_err(|error| ApiError::internal(format!("stored Macro is invalid: {error}")))?;
    let started = start_execution(
        state,
        session,
        definition,
        object.revision,
        wire::MacroTrigger::CommandLine,
        None,
        show_id,
    )?;
    Ok(started.0.execution_id)
}

async fn runtime_snapshot(
    State(state): State<AppState>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<Json<wire::MacroRuntimeSnapshot>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    context.verify(&state)?;
    Ok(Json(runtime_wire(state.macros.snapshot(session.desk.id))))
}

async fn execution_snapshot(
    State(state): State<AppState>,
    Path(execution_id): Path<Uuid>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<Json<wire::MacroExecutionSnapshot>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    context.verify(&state)?;
    state
        .macros
        .execution(session.desk.id, execution_id)
        .map(execution_wire)
        .map(Json)
        .ok_or_else(|| ApiError::not_found("Macro execution does not exist"))
}

async fn cancel_execution(
    State(state): State<AppState>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::MacroCancelActionRequest>,
) -> Result<Json<wire::MacroExecutionSnapshot>, ApiError> {
    let session = command_http::authenticate_desk_mutation(&state, &headers, &desk)?;
    context.verify(&state)?;
    state
        .macros
        .cancel(session.desk.id, request.execution_id)
        .map(execution_wire)
        .map(Json)
        .map_err(|error| ApiError::not_found(error.message))
}

async fn undo_run_line(
    State(state): State<AppState>,
    Path(execution_id): Path<Uuid>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<Json<wire::MacroRunLineUndoOutcome>, ApiError> {
    let session = command_http::authenticate_desk_mutation(&state, &headers, &desk)?;
    let show_id = context.resolve(&state)?;
    let execution = state
        .macros
        .execution(session.desk.id, execution_id)
        .ok_or_else(|| ApiError::not_found("Macro execution does not exist"))?;
    if execution.user_id != session.user.id.0 || execution.session_id != session.id.0 {
        return Err(ApiError::forbidden(
            "Run-line Undo belongs to the initiating operator session",
        ));
    }
    if execution.state != light_application::CommandMacroExecutionState::Succeeded
        || execution.trigger != light_application::CommandMacroTrigger::Editor
        || execution.line.is_none()
    {
        return Err(ApiError::conflict(
            "Only a successfully completed editor Run line can be undone",
        ));
    }
    let runtime = state.macros.snapshot(session.desk.id);
    if runtime.recent.first().map(|entry| entry.execution_id) != Some(execution_id) {
        return Err(ApiError::conflict(
            "Undo last run is unavailable because a newer Macro execution exists",
        ));
    }
    let (current_revision, _) = macro_for_run(&state, show_id, execution.macro_id)?;
    if current_revision != execution.source_revision {
        return Err(ApiError::conflict(
            "Undo last run is unavailable because the Macro revision changed",
        ));
    }
    let line = execution
        .line
        .expect("Run-line execution has a source line");
    let statement = execution
        .statement
        .expect("Run-line execution has a source statement");
    let expected_request_id = format!(
        "macro:{execution_id}:{}:{}:{line}:{statement}",
        execution.macro_id, execution.source_revision
    );
    let newest = state
        .programming
        .command_history(session.desk.id)
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::conflict("Undo last run has no compatible command history"))?;
    if newest.session_id != session.id
        || newest.source != "macro"
        || newest.status != "accepted"
        || newest.request_id.as_deref() != Some(expected_request_id.as_str())
    {
        return Err(ApiError::conflict(
            "Undo last run is unavailable because another command or edit intervened",
        ));
    }
    let mut action_context =
        operator_action_context(&session, light_application::ActionSource::Macro);
    action_context.request_id = Some(format!("macro-undo:{execution_id}"));
    let result = command_http::run_service_with_source(
        &state,
        &session,
        action_context,
        light_application::ProgrammingCommand::Undo,
        "macro",
    )
    .map_err(|error| ApiError::conflict(error.message))?;
    let changed = matches!(
        &result.outcome,
        light_application::ProgrammingOutcome::Accepted {
            action: light_application::ProgrammingAction::Undone,
            ..
        }
    );
    if !changed {
        return Err(ApiError::conflict(
            "The command family has no compatible Undo entry",
        ));
    }
    if let Some(warning) = command_http::publish_service_result(
        &state,
        &session,
        &result,
        "macro",
        result.context.request_id.as_deref(),
        None,
    ) {
        return Err(ApiError::internal(warning));
    }
    Ok(Json(wire::MacroRunLineUndoOutcome {
        execution_id,
        changed,
        message: "Undid the exact newest Run-line application change".into(),
    }))
}

async fn validate_macro(
    State(state): State<AppState>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::MacroValidationRequest>,
) -> Result<Json<wire::MacroValidation>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    context.verify(&state)?;
    let action_context = operator_action_context(&session, light_application::ActionSource::Http);
    Ok(Json(analyze_source_for_context(
        &request.source,
        request.cursor,
        &state,
        &session,
        &action_context,
    )))
}

async fn macro_object_action(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::MacroObjectActionRequest>,
) -> Result<Json<ShowObjectActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::Macro(request.action.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (mutation, previous) = macro_mutation(&store, &request.action)?;
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![mutation],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action).await?;
    let change = result
        .changes
        .first()
        .ok_or_else(|| ApiError::internal("Macro mutation returned no object change"))?;
    let object = if change.deleted {
        previous.ok_or_else(|| ApiError::internal("deleted Macro had no previous object"))?
    } else {
        ActiveShowRepository::open(&entry.path)
            .map_err(ApiError::store)?
            .object_with_portable_revision("macro", &change.object_id)
            .map_err(ApiError::store)?
            .1
            .ok_or_else(|| ApiError::internal("committed Macro is missing"))?
    };
    let outcome = ShowObjectActionOutcome {
        request_id: request.request_id,
        replayed: false,
        show_id: show_id.0,
        show_revision: result.show_revision.value(),
        object: object_record(object),
        event_sequence: Some(result.event_sequence),
    };
    emit(
        &state,
        "macro_object_changed",
        serde_json::json!({
            "show_id": show_id,
            "macro_id": change.object_id,
            "object_revision": change.object_revision,
            "deleted": change.deleted,
        }),
    );
    state
        .replay
        .insert_show_object_intent(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

fn macro_mutation(
    store: &ActiveShowRepository,
    action: &wire::MacroObjectAction,
) -> Result<
    (
        light_application::ActiveShowObjectMutation,
        Option<light_show::VersionedObject>,
    ),
    ApiError,
> {
    match action {
        wire::MacroObjectAction::Create { definition } => {
            let definition = application_definition(definition.clone())?;
            let id = definition.id.to_string();
            let previous = store
                .object_with_portable_revision("macro", &id)
                .map_err(ApiError::store)?
                .1;
            if previous.is_some() {
                return Err(ApiError::conflict("Macro id already exists"));
            }
            ensure_pool_identity_free(store, &definition, None)?;
            Ok((
                put_active_show_object(
                    light_application::ActiveShowObjectKind::Macro,
                    id,
                    0,
                    serde_json::to_value(definition)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                )?,
                None,
            ))
        }
        wire::MacroObjectAction::Update {
            macro_id,
            expected_revision,
            patch,
        } => {
            let previous = load_macro(store, *macro_id)?;
            if previous.revision != *expected_revision {
                return Err(ApiError::conflict(format!(
                    "Macro revision conflict: expected {expected_revision}, current {}",
                    previous.revision
                )));
            }
            let mut definition =
                serde_json::from_value::<light_application::CommandMacroDefinition>(
                    previous.body.clone(),
                )
                .map_err(|error| ApiError::internal(format!("stored Macro is invalid: {error}")))?;
            if let Some(number) = patch.number {
                definition.number = number;
            }
            if let Some(name) = &patch.name {
                definition.name.clone_from(name);
            }
            if let Some(source) = &patch.source {
                definition.source.clone_from(source);
            }
            if let Some(presentation) = &patch.presentation {
                definition.presentation = light_application::MacroPresentation {
                    color: presentation.color.clone(),
                    icon: presentation.icon.clone(),
                };
            }
            definition.name = definition.name.trim().to_owned();
            definition.validate().map_err(ApiError::bad_request)?;
            ensure_pool_identity_free(store, &definition, Some(*macro_id))?;
            Ok((
                put_active_show_object(
                    light_application::ActiveShowObjectKind::Macro,
                    macro_id.to_string(),
                    *expected_revision,
                    serde_json::to_value(definition)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                )?,
                Some(previous),
            ))
        }
        wire::MacroObjectAction::Copy {
            source_macro_id,
            expected_revision,
            pool_number,
        } => {
            let source = load_macro(store, *source_macro_id)?;
            if source.revision != *expected_revision {
                return Err(ApiError::conflict(format!(
                    "Macro revision conflict: expected {expected_revision}, current {}",
                    source.revision
                )));
            }
            let mut definition =
                serde_json::from_value::<light_application::CommandMacroDefinition>(source.body)
                    .map_err(|error| {
                        ApiError::internal(format!("stored Macro is invalid: {error}"))
                    })?;
            definition.id = Uuid::new_v4();
            definition.number = *pool_number;
            definition.name = copied_macro_name(&definition.name);
            definition.validate().map_err(ApiError::bad_request)?;
            ensure_pool_identity_free(store, &definition, None)?;
            Ok((
                put_active_show_object(
                    light_application::ActiveShowObjectKind::Macro,
                    definition.id.to_string(),
                    0,
                    serde_json::to_value(definition)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                )?,
                None,
            ))
        }
        wire::MacroObjectAction::Delete {
            macro_id,
            expected_revision,
        } => {
            let previous = load_macro(store, *macro_id)?;
            if previous.revision != *expected_revision {
                return Err(ApiError::conflict(format!(
                    "Macro revision conflict: expected {expected_revision}, current {}",
                    previous.revision
                )));
            }
            Ok((
                delete_active_show_object(
                    light_application::ActiveShowObjectKind::Macro,
                    macro_id.to_string(),
                    *expected_revision,
                ),
                Some(previous),
            ))
        }
    }
}

fn copied_macro_name(source: &str) -> String {
    const SUFFIX: &str = " Copy";
    let mut prefix_bytes = light_application::MAX_MACRO_NAME_BYTES - SUFFIX.len();
    while !source.is_char_boundary(prefix_bytes.min(source.len())) {
        prefix_bytes = prefix_bytes.saturating_sub(1);
    }
    format!("{}{}", &source[..prefix_bytes.min(source.len())], SUFFIX)
}

fn application_definition(
    definition: wire::MacroDefinition,
) -> Result<light_application::CommandMacroDefinition, ApiError> {
    let definition = light_application::CommandMacroDefinition {
        id: definition.id,
        number: definition.number,
        name: definition.name.trim().to_owned(),
        source: definition.source,
        presentation: light_application::MacroPresentation {
            color: definition.presentation.color,
            icon: definition.presentation.icon,
        },
    };
    definition.validate().map_err(ApiError::bad_request)?;
    Ok(definition)
}

fn load_macro(
    store: &ActiveShowRepository,
    id: Uuid,
) -> Result<light_show::VersionedObject, ApiError> {
    store
        .object_with_portable_revision("macro", &id.to_string())
        .map_err(ApiError::store)?
        .1
        .ok_or_else(|| ApiError::not_found("Macro does not exist"))
}

fn ensure_pool_identity_free(
    store: &ActiveShowRepository,
    definition: &light_application::CommandMacroDefinition,
    replacing: Option<Uuid>,
) -> Result<(), ApiError> {
    let (_, objects) = store
        .objects_with_portable_revision("macro")
        .map_err(ApiError::store)?;
    for object in objects {
        if replacing.is_some_and(|id| object.id == id.to_string()) {
            continue;
        }
        let Ok(existing) =
            serde_json::from_value::<light_application::CommandMacroDefinition>(object.body)
        else {
            continue;
        };
        if existing.number == definition.number {
            return Err(ApiError::conflict(format!(
                "Macro {} already uses pool number {}",
                existing.name, definition.number
            )));
        }
        if existing.name.eq_ignore_ascii_case(&definition.name) {
            return Err(ApiError::conflict(format!(
                "Macro name {} is already in use",
                definition.name
            )));
        }
    }
    Ok(())
}

fn analyze_source(source: &str, cursor: Option<u32>) -> wire::MacroValidation {
    let definitions = source_definitions(source);
    let mut diagnostics = source
        .lines()
        .enumerate()
        .filter_map(|(index, raw)| {
            let command = raw.trim();
            if command.is_empty() {
                return None;
            }
            if command.starts_with('#') || command.starts_with("//") {
                return Some(wire::MacroLineDiagnostic {
                    line: (index + 1) as u32,
                    status: wire::MacroLineStatus::Valid,
                    message: "Comment".into(),
                    tokens: vec![wire::MacroToken {
                        start: 0,
                        end: raw.len() as u32,
                        kind: wire::MacroTokenKind::Comment,
                        expansion: None,
                    }],
                });
            }
            let statements = command
                .split(';')
                .map(str::trim)
                .filter(|statement| !statement.is_empty())
                .collect::<Vec<_>>();
            let mut status = wire::MacroLineStatus::Valid;
            let mut message = if statements.len() > 1 {
                format!("{} valid commands", statements.len())
            } else {
                "Valid command line".into()
            };
            for statement in statements {
                if statement.starts_with("#") || statement.starts_with("//") {
                    break;
                }
                if starts_with_macro_keyword(statement, "DEFINE")
                    || statement.eq_ignore_ascii_case(light_application::RESTORE_SELECTION_COMMAND)
                {
                    continue;
                }
                let expanded = expand_statement_for_validation(statement, &definitions);
                let result = expanded
                    .as_deref()
                    .map_err(Clone::clone)
                    .and_then(tokenize_programmer_command);
                match result {
                    Ok((tokens, _)) if tokens.is_empty() => {
                        status = wire::MacroLineStatus::Invalid;
                        message = "Command is empty".into();
                        break;
                    }
                    Ok((tokens, _)) if interaction_required(&tokens) => {
                        status = wire::MacroLineStatus::InteractionRequired;
                        message = "Command requires an operator choice or destination".into();
                        break;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        status = wire::MacroLineStatus::Invalid;
                        message = error;
                        break;
                    }
                }
            }
            Some(wire::MacroLineDiagnostic {
                line: (index + 1) as u32,
                status,
                message,
                tokens: highlight_tokens(raw, &definitions),
            })
        })
        .collect::<Vec<_>>();
    if let Err(error) = light_application::compile_macro_source(source) {
        if let Some(diagnostic) = diagnostics
            .iter_mut()
            .find(|diagnostic| diagnostic.line == error.line as u32)
        {
            diagnostic.status = wire::MacroLineStatus::Invalid;
            diagnostic.message = error.message;
        } else {
            diagnostics.push(wire::MacroLineDiagnostic {
                line: error.line as u32,
                status: wire::MacroLineStatus::Invalid,
                message: error.message,
                tokens: Vec::new(),
            });
        }
    }
    wire::MacroValidation {
        valid: diagnostics
            .iter()
            .all(|diagnostic| matches!(diagnostic.status, wire::MacroLineStatus::Valid)),
        diagnostics,
        suggestions: macro_suggestions(source, cursor, &definitions),
    }
}

fn analyze_source_for_context(
    source: &str,
    cursor: Option<u32>,
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
) -> wire::MacroValidation {
    let mut validation = analyze_source(source, cursor);
    if !validation.valid {
        return validation;
    }
    let executable = light_application::compile_macro_source(source)
        .expect("valid Macro source compiles")
        .lines;
    let commands = executable
        .iter()
        .map(|line| line.command.as_str())
        .collect::<Vec<_>>();
    let initial_selection = state
        .programming
        .programmers()
        .selection(session.id)
        .map(|selection| selection.selected)
        .unwrap_or_default();
    if let Err((index, error)) = super::prevalidate_macro_commands_from(
        state,
        session,
        &commands,
        &initial_selection,
        context,
    ) {
        if let Some(line) = executable.get(index)
            && let Some(diagnostic) = validation
                .diagnostics
                .iter_mut()
                .find(|diagnostic| diagnostic.line == line.number as u32)
        {
            diagnostic.status = wire::MacroLineStatus::Invalid;
            diagnostic.message = error;
            validation.valid = false;
        }
    }
    validation
}

fn interaction_required(tokens: &[String]) -> bool {
    matches!(tokens, [target] if matches!(target.as_str(), "RECORD" | "REC" | "DELETE" | "DEL" | "MOVE" | "MOV" | "COPY" | "CPY" | "SET"))
        || matches!(
            tokens.last().map(String::as_str),
            Some("AT" | "CUE" | "GROUP" | "SET")
        )
}

fn highlight_tokens(
    raw: &str,
    definitions: &std::collections::BTreeMap<String, String>,
) -> Vec<wire::MacroToken> {
    let keywords = [
        "RECORD", "REC", "UPDATE", "DELETE", "DEL", "MOVE", "MOV", "COPY", "CPY", "SET", "AT",
        "THRU", "TIME", "DELAY", "FULL", "OFF", "GO", "PAUSE",
    ];
    let targets = [
        "FIXTURE", "GROUP", "CUE", "DYNAMIC", "PRESET", "PLAYBACK", "SET",
    ];
    let timings = ["TIME", "DELAY"];
    source_tokens(raw)
        .into_iter()
        .map(|(start, end, token)| {
            let normalized = token
                .trim_matches(|character: char| matches!(character, '+' | '-' | ',' | '.'))
                .to_ascii_uppercase();
            let expansion = definitions.get(token).cloned();
            let kind = if expansion.is_some() || valid_define_token(token) {
                wire::MacroTokenKind::Definition
            } else if timings.contains(&normalized.as_str()) {
                wire::MacroTokenKind::Timing
            } else if targets.contains(&normalized.as_str()) {
                wire::MacroTokenKind::Target
            } else if keywords.contains(&normalized.as_str()) {
                wire::MacroTokenKind::Keyword
            } else if matches!(token, "+" | "-" | ".") {
                wire::MacroTokenKind::Operator
            } else if normalized.parse::<f64>().is_ok() {
                wire::MacroTokenKind::Number
            } else {
                wire::MacroTokenKind::Text
            };
            wire::MacroToken {
                start: start as u32,
                end: end as u32,
                kind,
                expansion,
            }
        })
        .collect()
}

fn source_tokens(raw: &str) -> Vec<(usize, usize, &str)> {
    let mut tokens = Vec::new();
    let mut start = None;
    for (index, character) in raw.char_indices() {
        if character.is_whitespace() || character == ';' {
            if let Some(token_start) = start.take() {
                tokens.push((token_start, index, &raw[token_start..index]));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(token_start) = start {
        tokens.push((token_start, raw.len(), &raw[token_start..]));
    }
    tokens
}

fn source_definitions(source: &str) -> std::collections::BTreeMap<String, String> {
    let mut definitions = std::collections::BTreeMap::new();
    for line in source.lines() {
        for statement in line.split(';') {
            let statement = statement.trim();
            if statement.starts_with('#') || statement.starts_with("//") {
                break;
            }
            let mut parts = statement.trim().splitn(3, char::is_whitespace);
            if !parts
                .next()
                .is_some_and(|keyword| keyword.eq_ignore_ascii_case("DEFINE"))
            {
                continue;
            }
            let identifier = parts.next().unwrap_or_default();
            let expansion = parts.next().unwrap_or_default().trim();
            if valid_define_token(identifier) && !expansion.is_empty() {
                definitions.insert(identifier.to_owned(), expansion.to_owned());
            }
        }
    }
    definitions
}

fn starts_with_macro_keyword(command: &str, keyword: &str) -> bool {
    command
        .split_whitespace()
        .next()
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(keyword))
}

fn valid_define_token(token: &str) -> bool {
    token.starts_with('_')
        && token.len() > 1
        && token
            .bytes()
            .skip(1)
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn expand_statement_for_validation(
    statement: &str,
    definitions: &std::collections::BTreeMap<String, String>,
) -> Result<String, String> {
    let mut source = definitions
        .iter()
        .map(|(identifier, expansion)| format!("DEFINE {identifier} {expansion}"))
        .collect::<Vec<_>>();
    source.push(statement.to_owned());
    light_application::compile_macro_source(&source.join("\n"))
        .map_err(|error| error.message)?
        .lines
        .last()
        .map(|line| line.command.clone())
        .ok_or_else(|| "Command is empty".into())
}

fn macro_suggestions(
    source: &str,
    cursor: Option<u32>,
    definitions: &std::collections::BTreeMap<String, String>,
) -> Vec<wire::MacroSuggestion> {
    let Some(cursor) = cursor else {
        return Vec::new();
    };
    let cursor_byte = utf16_to_byte(source, cursor);
    let bytes = source.as_bytes();
    let mut start = cursor_byte;
    while start > 0 && suggestion_character(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = cursor_byte;
    while end < bytes.len() && suggestion_character(bytes[end]) {
        end += 1;
    }
    let prefix = source[start..cursor_byte].to_ascii_uppercase();
    if prefix.is_empty() {
        return Vec::new();
    }
    let statement_start = source[..cursor_byte]
        .rfind(['\n', ';'])
        .map_or(0, |separator| separator + 1);
    let before = source[statement_start..start].trim();
    let previous = before
        .split_whitespace()
        .next_back()
        .map(str::to_ascii_uppercase);
    let context = if before.is_empty() {
        MacroSuggestionContext::CommandStart
    } else if previous.as_deref() == Some("AT") {
        MacroSuggestionContext::Value
    } else {
        MacroSuggestionContext::Continuation
    };
    let mut choices = suggestion_choices(context)
        .into_iter()
        .map(|(label, insert_text, detail)| {
            (label.to_owned(), insert_text.to_owned(), detail.to_owned())
        })
        .collect::<Vec<_>>();
    choices.extend(definitions.iter().map(|(identifier, expansion)| {
        (
            identifier.clone(),
            identifier.clone(),
            format!("Expands to {expansion}"),
        )
    }));
    choices
        .into_iter()
        .filter(|(label, _, _)| label.to_ascii_uppercase().starts_with(&prefix))
        .map(|(label, insert_text, detail)| wire::MacroSuggestion {
            label,
            insert_text,
            detail,
            replace_start: byte_to_utf16(source, start),
            replace_end: byte_to_utf16(source, end),
        })
        .collect()
}

#[derive(Clone, Copy)]
enum MacroSuggestionContext {
    CommandStart,
    Value,
    Continuation,
}

fn suggestion_choices(
    context: MacroSuggestionContext,
) -> Vec<(&'static str, &'static str, &'static str)> {
    match context {
        MacroSuggestionContext::CommandStart => vec![
            ("FIXTURE", "FIXTURE ", "Select fixtures by number or range"),
            ("GROUP", "GROUP ", "Address a stored Group"),
            ("PRESET", "PRESET ", "Recall or address a Preset"),
            ("CUE", "CUE ", "Address a Cue"),
            ("DYNAMIC", "DYNAMIC ", "Address a Dynamic"),
            ("PLAYBACK", "PLAYBACK ", "Address a Playback"),
            (
                "RECORD",
                "RECORD ",
                "Record through the authoritative command grammar",
            ),
            ("UPDATE", "UPDATE ", "Update an existing show object"),
            (
                "DELETE",
                "DELETE ",
                "Delete through the authoritative command grammar",
            ),
            ("MOVE", "MOVE ", "Move a pool object"),
            ("COPY", "COPY ", "Copy a pool object"),
            ("SET", "SET ", "Open or address configuration"),
            (
                "RESTORE SELECTION",
                "RESTORE SELECTION",
                "Restore this Macro run's initiating selection",
            ),
            (
                "DEFINE",
                "DEFINE _",
                "Define an underscore-prefixed command substitution",
            ),
        ],
        MacroSuggestionContext::Value => vec![
            ("FULL", "FULL ", "Use the canonical full-level value"),
            ("PRESET", "PRESET ", "Apply a Preset value"),
        ],
        MacroSuggestionContext::Continuation => vec![
            ("AT", "AT ", "Apply a value to the current target"),
            ("THRU", "THRU ", "Continue an address range"),
            ("TIME", "TIME ", "Set command timing"),
            ("DELAY", "DELAY ", "Set command delay"),
        ],
    }
}

fn suggestion_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn utf16_to_byte(value: &str, offset: u32) -> usize {
    let mut utf16 = 0u32;
    for (byte, character) in value.char_indices() {
        if utf16 >= offset {
            return byte;
        }
        utf16 += character.len_utf16() as u32;
        if utf16 > offset {
            return byte;
        }
    }
    value.len()
}

fn byte_to_utf16(value: &str, byte: usize) -> u32 {
    value[..byte].encode_utf16().count() as u32
}

fn macro_for_run(
    state: &AppState,
    show_id: light_core::ShowId,
    macro_id: Uuid,
) -> Result<(u64, light_application::CommandMacroDefinition), ApiError> {
    let entry = active_entry(state, show_id)?;
    let object = load_macro(
        &ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?,
        macro_id,
    )?;
    let definition = serde_json::from_value(object.body)
        .map_err(|error| ApiError::internal(format!("stored Macro is invalid: {error}")))?;
    Ok((object.revision, definition))
}

struct ServerMacroExecutionHost {
    state: AppState,
    session: Session,
    show_id: light_core::ShowId,
    context: light_application::ActionContext,
    initial_selection: Vec<light_core::FixtureId>,
}

impl light_application::CommandMacroExecutionHost for ServerMacroExecutionHost {
    fn prevalidate(
        &self,
        lines: &[light_application::CommandMacroOwnedLine],
    ) -> Result<(), light_application::CommandMacroExecutionError> {
        let source = lines
            .iter()
            .map(|line| line.command.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let validation = analyze_source(&source, None);
        if validation.valid {
            let commands = lines
                .iter()
                .map(|line| line.command.as_str())
                .collect::<Vec<_>>();
            if let Err((index, error)) = super::prevalidate_macro_commands_from(
                &self.state,
                &self.session,
                &commands,
                &self.initial_selection,
                &self.context,
            ) {
                let line = lines.get(index).map_or(0, |line| line.number);
                return Err(light_application::CommandMacroExecutionError::new(format!(
                    "Macro line {line} cannot run: {error}"
                )));
            }
            return Ok(());
        }
        let message = validation
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.status != wire::MacroLineStatus::Valid)
            .map_or_else(
                || "Macro contains an invalid command".into(),
                |diagnostic| {
                    format!(
                        "Macro line {} cannot run: {}",
                        diagnostic.line, diagnostic.message
                    )
                },
            );
        Err(light_application::CommandMacroExecutionError::new(message))
    }

    fn execute(
        &self,
        execution_id: Uuid,
        macro_id: Uuid,
        macro_revision: u64,
        line: &light_application::CommandMacroOwnedLine,
    ) -> Result<(), light_application::CommandMacroExecutionError> {
        if self.state.active_show.current().map(|show| show.id) != Some(self.show_id) {
            return Err(light_application::CommandMacroExecutionError::new(
                "The active show changed while the Macro was running",
            ));
        }
        let _activation = self.state.active_show.acquire_blocking();
        if self.state.active_show.current().map(|show| show.id) != Some(self.show_id) {
            return Err(light_application::CommandMacroExecutionError::new(
                "The active show changed while the Macro was waiting to run",
            ));
        }
        let mut context = self.context.clone();
        context.request_id = Some(format!(
            "macro:{execution_id}:{macro_id}:{macro_revision}:{}:{}",
            line.number, line.statement
        ));
        let command = if line
            .command
            .eq_ignore_ascii_case(light_application::RESTORE_SELECTION_COMMAND)
        {
            light_application::ProgrammingCommand::RestoreSelection {
                fixtures: self.initial_selection.clone(),
            }
        } else {
            light_application::ProgrammingCommand::Execute {
                command: Some(line.command.clone()),
                policy: light_application::ExecutionPolicy::Compatibility,
            }
        };
        let result = command_http::run_service_with_source(
            &self.state,
            &self.session,
            context,
            command,
            "macro",
        )
        .map_err(|error| light_application::CommandMacroExecutionError::new(error.message))?;
        if let Some(warning) = command_http::publish_service_result(
            &self.state,
            &self.session,
            &result,
            "macro",
            result.context.request_id.as_deref(),
            Some(&line.command),
        ) {
            return Err(light_application::CommandMacroExecutionError::new(warning));
        }
        match result.outcome {
            light_application::ProgrammingOutcome::Accepted { .. } => Ok(()),
            light_application::ProgrammingOutcome::ChoiceRequired { .. } => {
                Err(light_application::CommandMacroExecutionError::new(format!(
                    "Macro line {} requires operator interaction",
                    line.number
                )))
            }
            light_application::ProgrammingOutcome::Rejected { error } => {
                Err(light_application::CommandMacroExecutionError::new(format!(
                    "Macro line {} failed: {error}",
                    line.number
                )))
            }
        }
    }

    fn execute_sequence(
        &self,
        execution_id: Uuid,
        macro_id: Uuid,
        macro_revision: u64,
        lines: &[light_application::CommandMacroOwnedLine],
        is_cancelled: &dyn Fn() -> bool,
        on_line: &mut dyn FnMut(&light_application::CommandMacroOwnedLine),
    ) -> Result<
        light_application::CommandMacroSequenceOutcome,
        light_application::CommandMacroExecutionError,
    > {
        if self.state.active_show.current().map(|show| show.id) != Some(self.show_id) {
            return Err(light_application::CommandMacroExecutionError::new(
                "The active show changed while the Macro was running",
            ));
        }
        let _activation = self.state.active_show.acquire_blocking();
        let actions = lines
            .iter()
            .map(|line| {
                let mut context = self.context.clone();
                context.request_id = Some(format!(
                    "macro:{execution_id}:{macro_id}:{macro_revision}:{}:{}",
                    line.number, line.statement
                ));
                let command = if line
                    .command
                    .eq_ignore_ascii_case(light_application::RESTORE_SELECTION_COMMAND)
                {
                    light_application::ProgrammingCommand::RestoreSelection {
                        fixtures: self.initial_selection.clone(),
                    }
                } else {
                    light_application::ProgrammingCommand::Execute {
                        command: Some(line.command.clone()),
                        policy: light_application::ExecutionPolicy::Compatibility,
                    }
                };
                light_application::ActionEnvelope { context, command }
            })
            .collect::<Vec<_>>();
        let ports =
            command_http::ServerProgrammingPorts::new(&self.state, &self.session, "macro", true);
        let mut index = 0usize;
        let (results, cancelled) = self
            .state
            .programming
            .handle_sequence_while(&actions, &ports, || {
                if is_cancelled() {
                    return false;
                }
                if let Some(line) = lines.get(index) {
                    on_line(line);
                }
                index += 1;
                true
            })
            .map_err(|error| light_application::CommandMacroExecutionError::new(error.message))?;

        for (line, result) in lines.iter().zip(&results) {
            if let Some(warning) = command_http::publish_service_result(
                &self.state,
                &self.session,
                result,
                "macro",
                result.context.request_id.as_deref(),
                Some(&line.command),
            ) {
                return Err(light_application::CommandMacroExecutionError::new(warning));
            }
            match &result.outcome {
                light_application::ProgrammingOutcome::Accepted { .. } => {}
                light_application::ProgrammingOutcome::ChoiceRequired { .. } => {
                    return Err(light_application::CommandMacroExecutionError::new(format!(
                        "Macro line {} requires operator interaction",
                        line.number
                    )));
                }
                light_application::ProgrammingOutcome::Rejected { error } => {
                    return Err(light_application::CommandMacroExecutionError::new(format!(
                        "Macro line {} failed: {error}",
                        line.number
                    )));
                }
            }
        }
        Ok(if cancelled {
            light_application::CommandMacroSequenceOutcome::Cancelled
        } else {
            light_application::CommandMacroSequenceOutcome::Succeeded
        })
    }
}

fn application_trigger(trigger: wire::MacroTrigger) -> light_application::CommandMacroTrigger {
    match trigger {
        wire::MacroTrigger::Pool => light_application::CommandMacroTrigger::Pool,
        wire::MacroTrigger::Editor => light_application::CommandMacroTrigger::Editor,
        wire::MacroTrigger::Playback { playback_number } => {
            light_application::CommandMacroTrigger::Playback { playback_number }
        }
        wire::MacroTrigger::CommandLine => light_application::CommandMacroTrigger::CommandLine,
        wire::MacroTrigger::Http => light_application::CommandMacroTrigger::Http,
        wire::MacroTrigger::WebSocket => light_application::CommandMacroTrigger::WebSocket,
        wire::MacroTrigger::Osc => light_application::CommandMacroTrigger::Osc,
        wire::MacroTrigger::Hardware => light_application::CommandMacroTrigger::Hardware,
        wire::MacroTrigger::Schedule => light_application::CommandMacroTrigger::Schedule,
        wire::MacroTrigger::Timecode => light_application::CommandMacroTrigger::Timecode,
    }
}

fn trigger_wire(trigger: light_application::CommandMacroTrigger) -> wire::MacroTrigger {
    match trigger {
        light_application::CommandMacroTrigger::Pool => wire::MacroTrigger::Pool,
        light_application::CommandMacroTrigger::Editor => wire::MacroTrigger::Editor,
        light_application::CommandMacroTrigger::Playback { playback_number } => {
            wire::MacroTrigger::Playback { playback_number }
        }
        light_application::CommandMacroTrigger::CommandLine => wire::MacroTrigger::CommandLine,
        light_application::CommandMacroTrigger::Http => wire::MacroTrigger::Http,
        light_application::CommandMacroTrigger::WebSocket => wire::MacroTrigger::WebSocket,
        light_application::CommandMacroTrigger::Osc => wire::MacroTrigger::Osc,
        light_application::CommandMacroTrigger::Hardware => wire::MacroTrigger::Hardware,
        light_application::CommandMacroTrigger::Schedule => wire::MacroTrigger::Schedule,
        light_application::CommandMacroTrigger::Timecode => wire::MacroTrigger::Timecode,
    }
}

pub(super) fn execution_wire(
    snapshot: light_application::CommandMacroExecutionSnapshot,
) -> wire::MacroExecutionSnapshot {
    wire::MacroExecutionSnapshot {
        execution_id: snapshot.execution_id,
        macro_id: snapshot.macro_id,
        macro_number: snapshot.macro_number,
        macro_name: snapshot.macro_name,
        source_revision: snapshot.source_revision,
        desk_id: snapshot.desk_id,
        user_id: snapshot.user_id,
        session_id: snapshot.session_id,
        state: match snapshot.state {
            light_application::CommandMacroExecutionState::Queued => {
                wire::MacroExecutionState::Queued
            }
            light_application::CommandMacroExecutionState::Validating => {
                wire::MacroExecutionState::Validating
            }
            light_application::CommandMacroExecutionState::Running => {
                wire::MacroExecutionState::Running
            }
            light_application::CommandMacroExecutionState::Succeeded => {
                wire::MacroExecutionState::Succeeded
            }
            light_application::CommandMacroExecutionState::Failed => {
                wire::MacroExecutionState::Failed
            }
            light_application::CommandMacroExecutionState::Cancelled => {
                wire::MacroExecutionState::Cancelled
            }
        },
        line: snapshot.line,
        statement: snapshot.statement,
        command: snapshot.command,
        message: snapshot.message,
        trigger: trigger_wire(snapshot.trigger),
        started_at: snapshot.started_at,
        finished_at: snapshot.finished_at,
    }
}

fn runtime_wire(
    snapshot: light_application::CommandMacroRuntimeSnapshot,
) -> wire::MacroRuntimeSnapshot {
    wire::MacroRuntimeSnapshot {
        desk_id: snapshot.desk_id,
        active: snapshot.active.into_iter().map(execution_wire).collect(),
        recent: snapshot.recent.into_iter().map(execution_wire).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_preserves_lines_and_rejects_interactions() {
        let validation = analyze_source(
            "# note\nGROUP 1 AT 50 TIME 2\nRECORD GROUP\nSET GROUP 10 AT 1 . 4",
            None,
        );
        assert!(!validation.valid);
        assert_eq!(validation.diagnostics.len(), 4);
        assert_eq!(validation.diagnostics[0].line, 1);
        assert_eq!(
            validation.diagnostics[2].status,
            wire::MacroLineStatus::InteractionRequired
        );
    }

    #[test]
    fn validation_offers_authoritative_fixture_completion_at_the_cursor() {
        let validation = analyze_source("F", Some(1));
        assert_eq!(
            validation
                .suggestions
                .iter()
                .map(|suggestion| suggestion.label.as_str())
                .collect::<Vec<_>>(),
            ["FIXTURE"]
        );
        let suggestion = &validation.suggestions[0];
        assert_eq!(suggestion.insert_text, "FIXTURE ");
        assert_eq!((suggestion.replace_start, suggestion.replace_end), (0, 1));

        let value = analyze_source("FIXTURE 1 AT F", Some(14));
        assert_eq!(
            value
                .suggestions
                .iter()
                .map(|suggestion| suggestion.label.as_str())
                .collect::<Vec<_>>(),
            ["FULL"]
        );
    }

    #[test]
    fn validation_expands_underscore_definitions_across_semicolon_statements() {
        let validation = analyze_source("DEFINE _front FIXTURE 1; _front AT 50", None);
        assert!(validation.valid, "{:?}", validation.diagnostics);
        assert!(validation.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .tokens
                .iter()
                .any(|token| token.expansion.as_deref() == Some("FIXTURE 1"))
        }));
    }

    #[test]
    fn validation_rejects_bare_definition_identifiers() {
        let validation = analyze_source("DEFINE front FIXTURE 1\nfront AT 50", None);
        assert!(!validation.valid);
        assert!(
            validation
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("underscore"))
        );
    }

    #[test]
    fn highlighting_preserves_definition_expansion_before_a_semicolon() {
        let validation = analyze_source("DEFINE _front FIXTURE 1\n_front; AT 50", None);
        assert!(
            validation.diagnostics[1]
                .tokens
                .iter()
                .any(|token| token.expansion.as_deref() == Some("FIXTURE 1")
                    && token.start == 0
                    && token.end == 6)
        );
    }

    #[test]
    fn copied_names_stay_valid_at_utf8_and_byte_limits() {
        let ascii = copied_macro_name(&"a".repeat(light_application::MAX_MACRO_NAME_BYTES));
        assert_eq!(ascii.len(), light_application::MAX_MACRO_NAME_BYTES);
        assert!(ascii.ends_with(" Copy"));
        let unicode = copied_macro_name(&"é".repeat(64));
        assert!(unicode.is_char_boundary(unicode.len()));
        assert!(unicode.len() <= light_application::MAX_MACRO_NAME_BYTES);
        assert!(unicode.ends_with(" Copy"));
    }
}
