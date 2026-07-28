use super::*;
use light_wire::v2::{
    dynamics::{
        DynamicControllerActionOutcome, DynamicControllerLiveActionRequest,
        DynamicFixAtActionRequest, DynamicInstanceActionOutcome, DynamicOffLiveActionRequest,
        DynamicStartLiveActionRequest,
    },
    live_action::{
        CommandLineReplaceLiveActionRequest, FixtureControlLiveActionRequest, LiveAction,
        LiveActionFrame,
    },
    preload_lifecycle::ProgrammingPreloadLifecycleAction,
};

pub(super) fn dispatch_live_action(
    state: &AppState,
    session: &Session,
    frame: LiveActionFrame,
) -> WsResponse {
    dispatch_live_action_inner(state, session, frame, false)
}

/// Production Dynamic actions wait for the current active-show operation
/// instead of reporting ordinary render-loop contention as a show change.
pub(super) async fn dispatch_live_action_live(
    state: &AppState,
    session: &Session,
    frame: LiveActionFrame,
) -> WsResponse {
    if !is_dynamic_action(&frame.action) {
        return dispatch_live_action(state, session, frame);
    }
    let revision = state.output.snapshot().revision;
    if let Err(error) = validate_frame(state, session, &frame) {
        return failed_response(frame.request_id, revision, error);
    }
    let show_before = state.active_show.current().as_ref().map(|show| show.id);
    let activation = state.active_show.acquire().await;
    let show_after = state.active_show.current().as_ref().map(|show| show.id);
    if show_before != show_after {
        drop(activation);
        return failed_response(
            frame.request_id,
            revision,
            "The active show changed before the Dynamic action could run. Tap the Dynamic again."
                .into(),
        );
    }
    let response = dispatch_live_action_inner(state, session, frame, true);
    drop(activation);
    response
}

fn dispatch_live_action_inner(
    state: &AppState,
    session: &Session,
    frame: LiveActionFrame,
    dynamic_activation_held: bool,
) -> WsResponse {
    let revision = state.output.snapshot().revision;
    if let Err(error) = validate_frame(state, session, &frame) {
        return failed_response(frame.request_id, revision, error);
    }
    let request_id = frame.request_id;
    let context = interaction_context(session, &request_id);
    let ports = command_http::ServerProgrammingPorts::new(state, session, "software", true);
    let result = dispatch_action(
        state,
        session,
        frame.action,
        &context,
        &ports,
        dynamic_activation_held,
    );
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
    dynamic_activation_held: bool,
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
            dispatch_command_line_replace(state, request, context, ports)
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
            dispatch_fixture_control(state, session, request, context)
        }
        LiveAction::DynamicToggle(request) => run_interaction_with_activation(
            state,
            session,
            context,
            dynamic_activation_held,
            || ActionOutput::plain(dispatch_dynamic_toggle(state, session, context, request)),
        ),
        LiveAction::DynamicStart(request) => run_interaction_with_activation(
            state,
            session,
            context,
            dynamic_activation_held,
            || ActionOutput::plain(dispatch_dynamic_start(state, session, context, request)),
        ),
        LiveAction::DynamicOff(request) => run_interaction_with_activation(
            state,
            session,
            context,
            dynamic_activation_held,
            || ActionOutput::plain(dispatch_dynamic_off(state, session, context, request)),
        ),
        LiveAction::DynamicSize(request) => run_interaction_with_activation(
            state,
            session,
            context,
            dynamic_activation_held,
            || {
                ActionOutput::plain(dispatch_dynamic_update(
                    state,
                    session,
                    context,
                    request,
                    DynamicUpdateField::Size,
                ))
            },
        ),
        LiveAction::DynamicSpeed(request) => run_interaction_with_activation(
            state,
            session,
            context,
            dynamic_activation_held,
            || {
                ActionOutput::plain(dispatch_dynamic_update(
                    state,
                    session,
                    context,
                    request,
                    DynamicUpdateField::Speed,
                ))
            },
        ),
        LiveAction::DynamicPhase(request) => run_interaction_with_activation(
            state,
            session,
            context,
            dynamic_activation_held,
            || {
                ActionOutput::plain(dispatch_dynamic_update(
                    state,
                    session,
                    context,
                    request,
                    DynamicUpdateField::Phase,
                ))
            },
        ),
        LiveAction::DynamicFixAt(request) => run_interaction_with_activation(
            state,
            session,
            context,
            dynamic_activation_held,
            || ActionOutput::plain(dispatch_dynamic_fix_at(state, session, context, request)),
        ),
    }
}

fn is_dynamic_action(action: &LiveAction) -> bool {
    matches!(
        action,
        LiveAction::DynamicToggle(_)
            | LiveAction::DynamicStart(_)
            | LiveAction::DynamicOff(_)
            | LiveAction::DynamicSize(_)
            | LiveAction::DynamicSpeed(_)
            | LiveAction::DynamicPhase(_)
            | LiveAction::DynamicFixAt(_)
    )
}

fn dispatch_dynamic_toggle(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    request: DynamicStartLiveActionRequest,
) -> Result<serde_json::Value, String> {
    dispatch_dynamic_start_with(state, session, context, request, true)
}

fn dispatch_dynamic_start(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    request: DynamicStartLiveActionRequest,
) -> Result<serde_json::Value, String> {
    dispatch_dynamic_start_with(state, session, context, request, false)
}

fn dispatch_dynamic_start_with(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    request: DynamicStartLiveActionRequest,
    toggle: bool,
) -> Result<serde_json::Value, String> {
    let ports = super::dynamics_adapter::ServerDynamicsPorts { state, session };
    let command = light_application::DynamicStartCommand {
        dynamic_id: request.dynamic_id,
        targets: request
            .request
            .targets
            .into_iter()
            .map(light_core::FixtureId)
            .collect(),
        overrides: dynamic_overrides(request.request.overrides),
        timing: dynamic_timing(request.request.timing),
    };
    let result = if toggle {
        state.dynamics.toggle(context, command, &ports)
    } else {
        state.dynamics.start(context, command, &ports)
    }
    .map_err(|error| error.message)?;
    persist_programmer(state, session).map_err(|error| error.message)?;
    persist_output_runtime(state).map_err(|error| error.message)?;
    serde_json::to_value(DynamicInstanceActionOutcome {
        request_id: request.request.request_id,
        runtime_instance_id: result.runtime_instance_id,
        controller_id: result.controller_id,
        targets: result.targets.into_iter().map(|target| target.0).collect(),
        started: result.started,
    })
    .map_err(|error| error.to_string())
}

fn dispatch_dynamic_off(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    request: DynamicOffLiveActionRequest,
) -> Result<serde_json::Value, String> {
    let ports = super::dynamics_adapter::ServerDynamicsPorts { state, session };
    let result = state
        .dynamics
        .off(
            context,
            light_application::DynamicOffCommand {
                controller_id: request.controller_id,
                timing: dynamic_timing(request.request.timing),
            },
            &ports,
        )
        .map_err(|error| error.message)?;
    persist_programmer(state, session).map_err(|error| error.message)?;
    persist_output_runtime(state).map_err(|error| error.message)?;
    serde_json::to_value(DynamicInstanceActionOutcome {
        request_id: request.request.request_id,
        runtime_instance_id: result.runtime_instance_id,
        controller_id: result.controller_id,
        targets: result.targets.into_iter().map(|target| target.0).collect(),
        started: false,
    })
    .map_err(|error| error.to_string())
}

enum DynamicUpdateField {
    Size,
    Speed,
    Phase,
}

fn dispatch_dynamic_update(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    request: DynamicControllerLiveActionRequest,
    field: DynamicUpdateField,
) -> Result<serde_json::Value, String> {
    let ports = super::dynamics_adapter::ServerDynamicsPorts { state, session };
    let (size, speed_multiplier, phase_offset_degrees) = match field {
        DynamicUpdateField::Size => (Some(request.request.value), None, None),
        DynamicUpdateField::Speed => (None, Some(request.request.value), None),
        DynamicUpdateField::Phase => (None, None, Some(request.request.value)),
    };
    state
        .dynamics
        .update_controller(
            context,
            light_application::DynamicControllerUpdate {
                controller_id: request.controller_id,
                size,
                speed_multiplier,
                phase_offset_degrees,
                undo_group: request.request.undo_group,
            },
            &ports,
        )
        .map_err(|error| error.message)?;
    persist_programmer(state, session).map_err(|error| error.message)?;
    persist_output_runtime(state).map_err(|error| error.message)?;
    serde_json::to_value(DynamicControllerActionOutcome {
        request_id: request.request.request_id,
        controller_id: request.controller_id,
        changed: true,
    })
    .map_err(|error| error.to_string())
}

fn dispatch_dynamic_fix_at(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    request: DynamicFixAtActionRequest,
) -> Result<serde_json::Value, String> {
    let ports = super::dynamics_adapter::ServerDynamicsPorts { state, session };
    state
        .dynamics
        .fix_at(
            context,
            light_application::DynamicFixAtCommand {
                targets: request
                    .targets
                    .into_iter()
                    .map(light_core::FixtureId)
                    .collect(),
                attribute: light_core::AttributeKey(request.attribute),
                value: request.value,
                timing: dynamic_timing(request.timing),
            },
            &ports,
        )
        .map_err(|error| error.message)?;
    persist_programmer(state, session).map_err(|error| error.message)?;
    persist_output_runtime(state).map_err(|error| error.message)?;
    serde_json::to_value(DynamicControllerActionOutcome {
        request_id: request.request_id,
        controller_id: uuid::Uuid::nil(),
        changed: true,
    })
    .map_err(|error| error.to_string())
}

fn dynamic_overrides(
    value: light_wire::v2::dynamics::DynamicInstanceOverridesProjection,
) -> light_dynamics::DynamicInstanceOverrides {
    light_dynamics::DynamicInstanceOverrides {
        size: value.size,
        speed_multiplier: light_dynamics::Rational {
            numerator: value.speed_multiplier.numerator,
            denominator: value.speed_multiplier.denominator,
        },
        phase_offset_degrees: value.phase_offset_degrees,
    }
}

fn dynamic_timing(
    value: light_wire::v2::dynamics::DynamicValueTimingProjection,
) -> light_dynamics::DynamicValueTiming {
    light_dynamics::DynamicValueTiming {
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

fn dispatch_command_line_replace(
    state: &AppState,
    request: CommandLineReplaceLiveActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> ActionOutput {
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

fn dispatch_fixture_control(
    state: &AppState,
    session: &Session,
    request: FixtureControlLiveActionRequest,
    context: &light_application::ActionContext,
) -> ActionOutput {
    run_interaction(
        state,
        session,
        context,
        || match ws_programmer_control_action(state, session, &action_request(request, context)) {
            Ok(result) => ActionOutput {
                response: Ok(result.payload),
            },
            Err(error) => ActionOutput::plain(Err(error)),
        },
    )
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
    run_interaction_with_activation(state, session, context, false, action)
}

fn run_interaction_with_activation(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    activation_held: bool,
    action: impl FnOnce() -> ActionOutput,
) -> ActionOutput {
    let _activation = if activation_held {
        None
    } else {
        match try_programming_activation(state) {
            Ok(activation) => Some(activation),
            Err(error) => return ActionOutput::plain(Err(error)),
        }
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
