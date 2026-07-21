use super::*;

pub(super) fn ws_preset_apply(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    #[derive(Deserialize)]
    struct Input {
        #[serde(default)]
        preset_id: Option<String>,
        #[serde(default)]
        family: Option<light_programmer::PresetFamily>,
        #[serde(default)]
        number: Option<u32>,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let requested_address = match (input.family, input.number, input.preset_id.as_deref()) {
        (Some(family), Some(number), _) => light_programmer::PresetAddress::new(family, number)?,
        (_, _, Some(id)) => light_programmer::PresetAddress::parse(id)?,
        _ => return Err("preset.apply requires family and number".into()),
    };
    let show_id = state
        .active_show
        .read()
        .as_ref()
        .map(|show| show.id)
        .ok_or("no active show is loaded")?;
    let current = light_application::ProgrammingPresetRecallRevisionExpectation::Current;
    let result = state
        .programming
        .handle_preset_recall(
            light_application::ActionEnvelope {
                context: context.clone(),
                command: light_application::ProgrammingPresetRecallRequest {
                    show_id,
                    address: requested_address,
                    expected_preset_revision: current,
                    expected_show_revision: current,
                    expected_values_revision: current,
                    expected_capture_mode_revision: current,
                    expected_selection_revision: current,
                },
            },
            ports,
        )
        .map_err(|error| error.message)?;
    let values_changed = result.outcome.values_event_sequence().is_some();
    Ok(WsTypedProgrammingAction {
        payload: serde_json::json!({
            "applied":result.applied_fixtures,
            "programmer":state.programmers.get(session.id),
        }),
        values_changed,
        replayed: result.replayed,
    })
}

pub(super) fn ws_programmer_mode(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        blind: Option<bool>,
        preview: Option<bool>,
        highlight: Option<bool>,
        active_context: Option<Option<String>>,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    state.programmers.set_modes(
        session.id,
        input.blind,
        input.preview,
        None,
        input.active_context,
    );
    persist_programmer(state, session).map_err(|e| e.message)?;
    let mut highlight_state = None;
    if let Some(enabled) = input.highlight {
        let programmer = state
            .programmers
            .get(session.id)
            .ok_or("programmer does not exist")?;
        let selection = state
            .programmers
            .selection(session.id)
            .ok_or("programmer selection does not exist")?;
        let snapshot = state.engine.snapshot();
        let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
        let groups = highlight_groups(&snapshot);
        let transition = state
            .highlight
            .action_guarded(
                session.desk.id,
                session.user.id,
                Some(&session.user.name),
                if enabled {
                    HighlightAction::On
                } else {
                    HighlightAction::Off
                },
                &selection,
                &fixtures,
                &groups,
                programmer.blind || programmer.preview,
            )
            .map_err(|error| error.to_string())?;
        apply_highlight_selection_write(state, session, transition.working_selection.as_ref())
            .map_err(|error| error.message)?;
        sync_highlight_output(state);
        emit(
            state,
            "highlight_changed",
            serde_json::json!({"desk_id":session.desk.id,"user_id":session.user.id,"state":&transition.state}),
        );
        highlight_state = Some(transition.state);
    }
    Ok(serde_json::json!({"updated":true,"highlight":highlight_state}))
}
