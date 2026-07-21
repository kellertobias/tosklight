use super::*;

pub(super) fn cuelist_for_page_playback(
    snapshot: &EngineSnapshot,
    page_number: u8,
    slot: u8,
) -> Option<u16> {
    let number = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == page_number)?
        .slots
        .get(&slot)
        .copied()?;
    snapshot
        .playbacks
        .iter()
        .any(|definition| definition.number == number)
        .then_some(number)
}

pub(super) fn update_target_for_playback(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
) -> Result<UpdateApiTarget, String> {
    match &definition.target {
        light_playback::PlaybackTarget::CueList { cue_list_id } => {
            let context = active_update_cue_contexts(state)
                .into_iter()
                .find(|context| context.playback_number == definition.number);
            Ok(UpdateApiTarget {
                family: UpdateApiTargetFamily::Cue,
                object_id: Some(cue_list_id.0.to_string()),
                playback_number: Some(definition.number),
                cue_id: context.as_ref().map(|context| context.cue_id),
                cue_number: context.map(|context| context.cue_number),
                validate_active_context: true,
            })
        }
        light_playback::PlaybackTarget::Group { group_id } => Ok(UpdateApiTarget {
            family: UpdateApiTargetFamily::Group,
            object_id: Some(group_id.clone()),
            playback_number: Some(definition.number),
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        }),
        _ => Err(format!(
            "Playback {} is not assigned to a recordable Update target",
            definition.number
        )),
    }
}

pub(super) fn intercept_update_playback_target(
    state: &AppState,
    session: &Session,
    definition: &light_playback::PlaybackDefinition,
    touched: bool,
) -> bool {
    if !touched
        || !state
            .programmers
            .get(session.id)
            .is_some_and(|programmer| command_line_arms_update(&programmer.command_line))
    {
        return false;
    }
    let target = match update_target_for_playback(state, definition) {
        Ok(target) => target,
        Err(error) => {
            emit(
                state,
                "update_target_rejected",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "session_id":session.id,
                    "playback_number":definition.number,
                    "source":"osc",
                    "error":error,
                }),
            );
            return true;
        }
    };
    state
        .programmers
        .set_command_line(session.id, String::new());
    let _ = persist_programmer(state, session);
    emit(
        state,
        "update_target_requested",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "source":"osc",
            "target":target,
        }),
    );
    emit_update_armed_transition(state, session, true, false, "osc_target");
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id,"desk_id":session.desk.id,"source":"osc_target"}),
    );
    true
}

fn osc_playback_values(arguments: &[OscArgument]) -> (bool, Option<f32>) {
    let pressed = arguments
        .first()
        .map(|argument| match argument {
            OscArgument::Bool(value) => *value,
            OscArgument::Int(value) => *value != 0,
            OscArgument::Float(value) => *value > 0.0,
            OscArgument::String(value) => value != "0" && value != "false",
        })
        .unwrap_or(true);
    let value = arguments.first().and_then(|argument| match argument {
        OscArgument::Float(value) => Some(*value),
        OscArgument::Int(value) => Some(*value as f32 / 127.0),
        _ => None,
    });
    (pressed, value)
}

fn handle_osc_page(state: &AppState, parts: &[&str], arguments: &[OscArgument]) -> bool {
    if parts.len() != 3 || parts.first() != Some(&"light") || parts.get(2) != Some(&"page") {
        return false;
    }
    let page = arguments.first().and_then(|argument| match argument {
        OscArgument::Int(value) => u8::try_from(*value).ok(),
        OscArgument::Float(value) if value.is_finite() => Some(*value as u8),
        _ => None,
    });
    let Some(page) = page else {
        return true;
    };
    let Ok(_activation) = state.activation_lock.clone().try_lock_owned() else {
        return true;
    };
    let Some((show, desk)) = state
        .active_show
        .read()
        .clone()
        .and_then(|show| osc_control_desk(state, parts[1]).map(|desk| (show, desk)))
    else {
        return true;
    };
    let context =
        light_application::ActionContext::system(desk.id, light_application::ActionSource::Osc);
    let completed = state
        .playback_service
        .run_unit_of_work(playback_service::ChangePage {
            state,
            show: &show,
            context,
            desk_id: desk.id,
            page,
        });
    if !completed
        .output
        .is_ok_and(|availability| availability.available())
    {
        return true;
    }
    emit(
        state,
        "playback_page_changed",
        serde_json::json!({"desk_id":desk.id,"page":page}),
    );
    true
}

fn osc_playback_address(parts: &[&str]) -> Option<(PlaybackAddress, usize)> {
    if parts.len() >= 5 && parts.first() == Some(&"light") && parts.get(1) == Some(&"playback") {
        let page = parts[2].parse::<u8>().ok()?;
        let slot = parts[3].parse::<u8>().ok()?;
        Some((PlaybackAddress::ExplicitPage { page, slot }, 4))
    } else if parts.len() >= 4
        && parts.first() == Some(&"light")
        && parts
            .get(1)
            .is_some_and(|name| *name == "cuelist" || *name == "qlist" || *name == "playback")
    {
        Some((PlaybackAddress::Pool(parts[2].parse::<u16>().ok()?), 3))
    } else if parts.len() >= 5
        && parts.first() == Some(&"light")
        && parts
            .get(2)
            .is_some_and(|name| *name == "page-playback" || *name == "paged-playback")
    {
        Some((
            PlaybackAddress::CurrentPage {
                slot: parts[3].parse::<u8>().ok()?,
            },
            4,
        ))
    } else {
        None
    }
}

pub(super) fn osc_playback_session(
    state: &AppState,
    source: Option<&str>,
    action_alias: &str,
    action_desk: Option<&ControlDesk>,
) -> Result<Option<Session>, ()> {
    let source = source.and_then(|source| source.parse::<SocketAddr>().ok());
    let subscribed = state
        .osc_subscribers
        .lock()
        .values()
        .find(|subscriber| Some(subscriber.command_source) == source)
        .cloned();
    if let Some(subscriber) = subscribed {
        let desk = action_desk.ok_or(())?;
        if !subscriber.desk_alias.eq_ignore_ascii_case(action_alias) {
            return Err(());
        }
        let session = state
            .sessions
            .read()
            .get(&subscriber.session_id)
            .filter(|session| session.connected && session.desk.id == desk.id)
            .cloned()
            .ok_or(())?;
        if !subscriber
            .desk_alias
            .eq_ignore_ascii_case(&session.desk.osc_alias)
        {
            return Err(());
        }
        return Ok(Some(session));
    }
    Ok(action_desk.and_then(|desk| {
        state
            .sessions
            .read()
            .values()
            .find(|session| session.connected && session.desk.id == desk.id)
            .cloned()
    }))
}

pub(super) fn handle_playback_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) {
    // Preserve the three established OSC address families as distinct typed intents:
    //
    // - `/light/playback/{page}/{slot}` always targets that explicit page.
    // - `/light/{desk}/page-playback/{slot}` resolves the desk's current page under the
    //   PlaybackService operation gate.
    // - `/light/playback/{number}` and its Cuelist aliases address the global pool directly.
    //
    // Keeping this distinction until the application boundary prevents page changes from
    // retargeting explicit hardware input while retaining current-page behavior for desk wings.
    // Parsing stops at typed intent here; address resolution and mutation ordering stay in the
    // application service, alongside the HTTP and compatibility WebSocket paths.
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    let (pressed, value) = osc_playback_values(arguments);
    let source_socket = source.and_then(|source| source.parse::<SocketAddr>().ok());
    if handle_osc_page(state, &parts, arguments) {
        return;
    }
    let Some((playback_address, action_index)) = osc_playback_address(&parts) else {
        return;
    };
    let Ok(_activation) = state.activation_lock.clone().try_lock_owned() else {
        return;
    };
    let button = (parts[action_index] == "button")
        .then(|| parts.get(action_index + 1)?.parse::<u8>().ok())
        .flatten();
    let input = PoolPlaybackInput {
        value: value.map(|value| value.clamp(0.0, 1.0)),
        pressed: Some(pressed),
        button,
        surface: Some("osc".into()),
        ..PoolPlaybackInput::default()
    };
    let path_alias = if parts
        .get(2)
        .is_some_and(|part| *part == "page-playback" || *part == "paged-playback")
    {
        Some(parts[1])
    } else {
        None
    };
    let subscribed = source_socket.and_then(|source| {
        state
            .osc_subscribers
            .lock()
            .values()
            .find(|subscriber| subscriber.command_source == source)
            .cloned()
    });
    let action_alias = path_alias
        .map(str::to_owned)
        .or_else(|| {
            subscribed
                .as_ref()
                .map(|subscriber| subscriber.desk_alias.clone())
        })
        .unwrap_or_else(|| "main".into());
    let action_desk = subscribed
        .as_ref()
        .and_then(|subscriber| {
            state
                .sessions
                .read()
                .get(&subscriber.session_id)
                .map(|session| session.desk.clone())
        })
        .or_else(|| osc_control_desk(state, &action_alias));
    let Ok(session) = osc_playback_session(state, source, &action_alias, action_desk.as_ref())
    else {
        return;
    };
    let action = if parts[action_index] == "fader" {
        "master"
    } else {
        parts[action_index]
    };
    let suppression_input =
        session
            .as_ref()
            .map(|session| osc_cue_record_suppression::OscSuppressionInput {
                session_id: session.id,
                source: source_socket,
                address,
                continuous: action == "master",
                pressed,
            });
    if suppression_input.is_some_and(|input| {
        state
            .osc_cue_record_suppression
            .lock()
            .suppresses_input(input, Instant::now())
    }) {
        return;
    }
    if let Some(session) = session.as_ref()
        && command_http::intercept_armed_cue_playback(
            state,
            session,
            playback_address.clone(),
            action == "master" || pressed,
        )
    {
        if let Some(input) = suppression_input {
            state
                .osc_cue_record_suppression
                .lock()
                .remember_intercept(input, Instant::now());
        }
        return;
    }
    let Ok(result) = playback_service::osc_action(
        state,
        session.as_ref(),
        action_desk.as_ref(),
        playback_address,
        action,
        &input,
    ) else {
        return;
    };
    let changed = matches!(
        result.execution,
        PlaybackExecution::Pool { changed: true, .. }
    );
    if changed && let Some(number) = result.resolved.playback_number() {
        emit(
            state,
            "playback_changed",
            serde_json::json!({"playback_number":number,"action":action,"source":"osc","session_id":session.map(|session|session.id)}),
        );
    }
}

pub(super) fn ingest_timecode(state: &AppState, timecode: SmpteTimecode) {
    let current = state.timecode_router.lock().ingest(timecode).cloned();
    if let Some(timecode) = current {
        let fps = u64::from(timecode.rate.nominal_frames());
        let seconds = u64::from(timecode.hours) * 3600
            + u64::from(timecode.minutes) * 60
            + u64::from(timecode.seconds);
        state
            .engine
            .set_timecode_frame(Some(seconds * fps + u64::from(timecode.frames)));
    }
}
