use super::*;
use light_wire::v2::{
    live_action::{LiveAction, LiveActionFrame},
    preload_lifecycle::ProgrammingPreloadLifecycleAction,
};

pub(super) fn dispatch_live_action(
    state: &AppState,
    session: &Session,
    frame: LiveActionFrame,
) -> WsResponse {
    let revision = state.output.snapshot().revision;
    if let Err(error) = validate_frame(state, session, &frame) {
        return failed_response(frame.request_id, revision, error);
    }
    let request_id = frame.request_id;
    let context = interaction_context(session, &request_id);
    let ports = command_http::ServerProgrammingPorts::new(state, session, "software", true);
    let result = dispatch_action(state, session, frame.action, &context, &ports);
    match result.response {
        Ok(payload) => WsResponse {
            protocol_version: 2,
            request_id,
            ok: true,
            revision: state.output.snapshot().revision,
            payload: Some(payload),
            error: None,
        },
        Err(error) => failed_response(request_id, revision, error),
    }
}

fn validate_frame(
    state: &AppState,
    session: &Session,
    frame: &LiveActionFrame,
) -> Result<(), String> {
    if frame.protocol_version != 2 {
        return Err("unsupported protocol_version".into());
    }
    if frame.session_id != session.id.0 {
        return Err("session_id does not own this connection".into());
    }
    if let Some(embedded) = frame.action.embedded_request_id()
        && embedded != frame.request_id
    {
        return Err("action payload request_id must match the frame request_id".into());
    }
    if !matches!(
        frame.action,
        LiveAction::SpeedGroup(_) | LiveAction::OutputRuntime(_)
    ) && read_desk_lock(state, session.desk.id).locked
    {
        return Err("desk is locked".into());
    }
    Ok(())
}

fn dispatch_action(
    state: &AppState,
    session: &Session,
    action: LiveAction,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> ActionOutput {
    match action {
        LiveAction::SpeedGroup(request) => runtime_output(ws_speed_group_action(
            state,
            session,
            &action_request(request, context),
        )),
        LiveAction::OutputRuntime(request) => runtime_output(ws_output_runtime_action(
            state,
            session,
            &action_request(request, context),
        )),
        LiveAction::DmxOverride(request) => ActionOutput::plain(ws_dmx_override(
            state,
            session,
            &action_request(request, context),
        )),
        LiveAction::PatchPreviewHighlight(request) => ActionOutput::plain(
            ws_patch_preview_highlight(state, session, &action_request(request, context)),
        ),
        LiveAction::ProgrammingSelection(request) => {
            typed_programming(ws_programmer_selection_action(
                state,
                &action_request(request, context),
                context,
                ports,
            ))
        }
        LiveAction::ProgrammingValues(request) => typed_programming(ws_programmer_values_action(
            state,
            &action_request(request, context),
            context,
            ports,
        )),
        LiveAction::ProgrammerCaptureMode(request) => {
            run_interaction(state, session, context, || {
                let response =
                    ws_programmer_capture_mode(state, session, request).and_then(|outcome| {
                        serde_json::to_value(outcome).map_err(|error| error.to_string())
                    });
                ActionOutput::plain(response)
            })
        }
        LiveAction::ProgrammerPriority(request) => typed_programming(
            ws_programmer_priority_action(state, &action_request(request, context), context, ports),
        ),
        LiveAction::ProgrammerPreloadLifecycle(request) => {
            let needs_activation =
                matches!(request.action, ProgrammingPreloadLifecycleAction::Go { .. });
            run_typed_with_optional_activation(state, needs_activation, || {
                ws_programmer_preload_lifecycle_action(
                    state,
                    &action_request(request, context),
                    context,
                    ports,
                )
            })
        }
        LiveAction::ProgrammerPreloadValues(request) => run_typed_with_activation(state, || {
            ws_programmer_preload_values_action(
                state,
                &action_request(request, context),
                context,
                ports,
            )
        }),
        LiveAction::PresetRecall(request) => run_typed_with_activation(state, || {
            ws_preset_recall_action(state, &action_request(request, context), context, ports)
        }),
        LiveAction::CommandLineReplace(request) => {
            let request = WsActionRequest {
                request_id: context.request_id.clone().unwrap_or_default(),
                payload: serde_json::json!({
                    "request_id":context.request_id,
                    "expected_revision":request.expected_revision,
                    "text":request.text,
                }),
            };
            typed_programming(ws_programmer_command_line_replace(
                state, &request, context, ports,
            ))
        }
        LiveAction::Highlight(request) => run_interaction(state, session, context, || {
            ActionOutput::plain(ws_highlight_action(
                state,
                session,
                &action_request(request, context),
            ))
        }),
        LiveAction::Playback(request) => run_interaction(state, session, context, || {
            ActionOutput::plain(ws_playback_action(
                state,
                session,
                &action_request(request, context),
                Some(context),
            ))
        }),
        LiveAction::CommandLineSet(request) => run_interaction(state, session, context, || {
            ActionOutput::plain(ws_programmer_command_line(
                state,
                session,
                &action_request(request, context),
            ))
        }),
        LiveAction::CommandTarget(request) => run_interaction(state, session, context, || {
            ActionOutput::plain(ws_programmer_command_target(
                state,
                session,
                &action_request(request, context),
            ))
        }),
        LiveAction::CommandLineExecute(request) => run_interaction(state, session, context, || {
            ActionOutput::plain(ws_programmer_execute(
                state,
                session,
                &action_request(request, context),
                Some(context),
            ))
        }),
        LiveAction::ProgrammerUndo => run_interaction(state, session, context, || {
            let changed = state.programming.undo(session.id);
            let response = persist_programmer(state, session)
                .map(|()| serde_json::json!({"changed":changed}))
                .map_err(|error| error.message);
            ActionOutput::plain(response)
        }),
        LiveAction::ProgrammingAlign(request) => run_interaction(state, session, context, || {
            ActionOutput::plain(ws_programmer_align(
                state,
                session,
                &action_request(request, context),
            ))
        }),
        LiveAction::FixtureControl(request) => {
            run_interaction(
                state,
                session,
                context,
                || match ws_programmer_control_action(
                    state,
                    session,
                    &action_request(request, context),
                ) {
                    Ok(result) => ActionOutput {
                        response: Ok(result.payload),
                    },
                    Err(error) => ActionOutput::plain(Err(error)),
                },
            )
        }
    }
}

fn action_request<T: serde::Serialize>(
    value: T,
    context: &light_application::ActionContext,
) -> WsActionRequest {
    WsActionRequest {
        request_id: context.request_id.clone().unwrap_or_default(),
        payload: serde_json::to_value(value).expect("typed live actions always serialize"),
    }
}

fn runtime_output(result: Result<WsTypedRuntimeAction, String>) -> ActionOutput {
    match result {
        Ok(result) => ActionOutput {
            response: Ok(result.payload),
        },
        Err(error) => ActionOutput::plain(Err(error)),
    }
}

fn typed_programming(result: Result<WsTypedProgrammingAction, String>) -> ActionOutput {
    match result {
        Ok(result) => ActionOutput {
            response: Ok(result.payload),
        },
        Err(error) => ActionOutput::plain(Err(error)),
    }
}

fn run_typed_with_activation(
    state: &AppState,
    action: impl FnOnce() -> Result<WsTypedProgrammingAction, String>,
) -> ActionOutput {
    let _activation = match try_programming_activation(state) {
        Ok(activation) => activation,
        Err(error) => return ActionOutput::plain(Err(error)),
    };
    typed_programming(action())
}

fn run_typed_with_optional_activation(
    state: &AppState,
    needs_activation: bool,
    action: impl FnOnce() -> Result<WsTypedProgrammingAction, String>,
) -> ActionOutput {
    if needs_activation {
        run_typed_with_activation(state, action)
    } else {
        typed_programming(action())
    }
}

fn run_interaction(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    action: impl FnOnce() -> ActionOutput,
) -> ActionOutput {
    let _activation = match try_programming_activation(state) {
        Ok(activation) => activation,
        Err(error) => return ActionOutput::plain(Err(error)),
    };
    let before = tracked_state(state, session);
    match state.programming.run_external_interaction(
        context,
        &command_http::ServerProgrammingPorts::new(state, session, "software", true),
        action,
    ) {
        Ok(completed) => {
            let output = completed.output;
            if output.response.is_ok() {
                reconcile_interaction(state, session, &before);
            }
            output
        }
        Err(error) => ActionOutput::plain(Err(error.message)),
    }
}

fn failed_response(request_id: String, revision: u64, error: String) -> WsResponse {
    WsResponse {
        protocol_version: 2,
        request_id,
        ok: false,
        revision,
        payload: None,
        error: Some(error),
    }
}

struct ActionOutput {
    response: Result<serde_json::Value, String>,
}

impl ActionOutput {
    fn plain(response: Result<serde_json::Value, String>) -> Self {
        Self { response }
    }
}

pub(super) struct WsTypedProgrammingAction {
    pub(super) payload: serde_json::Value,
}

struct WsTrackedState {
    interaction: Option<light_programmer::ProgrammerInteractionVersion>,
}

impl WsTrackedState {
    fn selection_revision(&self) -> Option<u64> {
        self.interaction
            .as_ref()
            .map(|state| state.selection_revision)
    }

    fn capture_mode(&self) -> Option<light_programmer::ProgrammerCaptureMode> {
        self.interaction.as_ref().map(|state| state.capture_mode)
    }
}

fn tracked_state(state: &AppState, session: &Session) -> WsTrackedState {
    WsTrackedState {
        interaction: state.programming.interaction_version(session.id),
    }
}

fn reconcile_interaction(state: &AppState, session: &Session, before: &WsTrackedState) {
    let after = tracked_state(state, session);
    if before.capture_mode() != after.capture_mode() {
        reconcile_highlight_capture_mode(state, session, "programmer_capture_mode");
    } else if before.selection_revision() != after.selection_revision() {
        reconcile_highlight_selection(state, session, "programmer_selection");
    }
}

fn interaction_context(session: &Session, request_id: &str) -> light_application::ActionContext {
    light_application::ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        light_application::ActionSource::UserInterface,
    )
    .with_request_id(request_id)
}
