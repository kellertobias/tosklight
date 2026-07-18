use super::*;

pub(super) fn spawn_control_inputs(
    state: &AppState,
    cancel: CancellationToken,
) -> Vec<tokio::task::JoinHandle<()>> {
    let configuration = state.configuration.read().clone();
    let mut tasks = Vec::new();
    if state.manual_clock.is_none() {
        let feedback_state = state.clone();
        let feedback_cancel = cancel.clone();
        tasks.push(tokio::spawn(async move{let mut interval=tokio::time::interval(Duration::from_millis(500));loop{tokio::select!{_=feedback_cancel.cancelled()=>break,_=interval.tick()=>send_osc_feedback(&feedback_state,false)}}}));
        let refresh_state = state.clone();
        let refresh_cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            loop {
                tokio::select! {
                    _ = refresh_cancel.cancelled() => break,
                    _ = interval.tick() => { refresh_speed_group_engine(&refresh_state); }
                }
            }
        }));
    }
    for (address, protocol) in [
        (configuration.osc_bind, UdpInputProtocol::Osc),
        (
            configuration.art_timecode_bind,
            UdpInputProtocol::ArtTimeCode,
        ),
    ] {
        let Some(address) = address else {
            continue;
        };
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            match UdpControlInput::bind(address, protocol).await {
                Ok(input) => drive_control_input(state, cancel, input).await,
                Err(error) => tracing::error!(%address,%error,"control input could not bind"),
            }
        }));
    }
    for port in configuration.midi_inputs {
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            match MidiControlInput::open(&port) {
                Ok(input) => drive_control_input(state, cancel, input).await,
                Err(error) => tracing::error!(%port,%error,"MIDI input could not open"),
            }
        }));
    }
    if let Some(address) = configuration.rtp_midi_bind {
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            match RtpMidiInput::bind(address, "Light").await {
                Ok(input) => drive_control_input(state, cancel, input).await,
                Err(error) => tracing::error!(%address,%error,"RTP-MIDI input could not bind"),
            }
        }));
    }
    tasks
}

pub(super) async fn drive_control_input<I: ControlInput>(
    state: AppState,
    cancel: CancellationToken,
    mut input: I,
) {
    loop {
        tokio::select! { _=cancel.cancelled()=>break,event=input.next_event()=>match event { Some(event)=>handle_control_event(&state,event),None=>break } }
    }
}

pub(super) fn handle_control_event(state: &AppState, event: ControlEvent) {
    if let ControlEvent::Timecode(timecode) = &event {
        ingest_timecode(state, timecode.clone());
    }
    let input_locked = if let ControlEvent::Osc { address, .. } = &event {
        let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
        parts
            .get(1)
            .and_then(|alias| osc_control_desk(state, alias))
            .is_some_and(|desk| read_desk_lock(state, desk.id).locked)
    } else {
        false
    };
    if let ControlEvent::Osc {
        address, arguments, ..
    } = &event
        && let Some(configuration) = &state.configuration.read().osc_timecode
        && &configuration.address == address
        && let [
            OscArgument::Int(hours),
            OscArgument::Int(minutes),
            OscArgument::Int(seconds),
            OscArgument::Int(frames),
            ..,
        ] = arguments.as_slice()
        && (0..24).contains(hours)
        && (0..60).contains(minutes)
        && (0..60).contains(seconds)
        && (0..i32::from(configuration.rate.nominal_frames())).contains(frames)
    {
        ingest_timecode(
            state,
            SmpteTimecode {
                hours: *hours as u8,
                minutes: *minutes as u8,
                seconds: *seconds as u8,
                frames: *frames as u8,
                rate: configuration.rate,
                source: format!("osc:{address}"),
                received_at: chrono::Utc::now(),
            },
        );
    }
    if let ControlEvent::Osc {
        address,
        arguments,
        source,
    } = &event
    {
        if !handle_subscription_osc(state, address, arguments, source.as_deref()) && !input_locked {
            handle_playback_osc(state, address, arguments, source.as_deref());
            handle_highlight_osc(state, address, arguments, source.as_deref());
            handle_programmer_osc(state, address, arguments, source.as_deref());
            handle_timing_osc(state, address, arguments);
            handle_encoder_osc(state, address, arguments);
        }
        send_osc_feedback(state, false);
    }
    if input_locked {
        return;
    }
    let mappings = state.engine.snapshot().control_mappings.clone();
    let mut mapping_applied = false;
    for mapping in mappings.iter().filter(|mapping| mapping.matches(&event)) {
        mapping_applied = true;
        match mapping.action {
            ControlAction::CueGo { cue_list_id } => {
                let _ = state.engine.playback().write().go(cue_list_id);
            }
            ControlAction::CueBack { cue_list_id } => {
                let _ = state.engine.playback().write().back(cue_list_id);
            }
            ControlAction::CuePause { cue_list_id } => {
                let _ = state.engine.playback().write().pause(cue_list_id);
            }
            ControlAction::CueRelease { cue_list_id } => {
                state.engine.playback().write().release(cue_list_id);
            }
            ControlAction::Blackout { enabled } => {
                state.output_control.lock().options.blackout = enabled
            }
            ControlAction::GrandMaster { level } => {
                state.output_control.lock().options.grand_master = level.clamp(0.0, 1.0)
            }
            ControlAction::DeskSet => {
                emit(state, "desk_action", serde_json::json!({"action":"set"}))
            }
        }
    }
    if mapping_applied {
        let _ = persist_active_playbacks(state);
        let _ = persist_output_runtime(state);
    }
    emit(
        state,
        "control_event",
        serde_json::to_value(event)
            .unwrap_or_else(|_| serde_json::json!({"error":"serialization failed"})),
    );
}

pub(super) fn handle_subscription_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) -> bool {
    if address != "/light/subscribe" && address != "/light/unsubscribe" {
        return false;
    }
    let Some(client_id) = arguments.first().and_then(|v| match v {
        OscArgument::String(v) => Some(v.clone()),
        _ => None,
    }) else {
        return true;
    };
    if address == "/light/unsubscribe" {
        state.osc_subscribers.lock().remove(&client_id);
        emit(
            state,
            "hardware_connection_changed",
            serde_json::json!({"connected":!state.osc_subscribers.lock().is_empty()}),
        );
        return true;
    }
    let Some(desk_alias) = arguments.get(1).and_then(|v| match v {
        OscArgument::String(v) => Some(v.clone()),
        _ => None,
    }) else {
        return true;
    };
    let Some(port) = arguments.get(2).and_then(|v| match v {
        OscArgument::Int(v) => u16::try_from(*v).ok(),
        _ => None,
    }) else {
        return true;
    };
    let Some(command_source) = source.and_then(|v| v.parse::<SocketAddr>().ok()) else {
        return true;
    };
    let mut target = command_source;
    target.set_port(port);
    let Some(desk) = osc_control_desk(state, &desk_alias) else {
        return true;
    };
    let desk_alias = desk.osc_alias.clone();
    let existing = state.osc_subscribers.lock().get(&client_id).cloned();
    let attached_session = {
        let sessions = state.sessions.read();
        sessions
            .values()
            .find(|session| {
                session.connected
                    && session.desk.id == desk.id
                    && state.programmers.get(session.id).is_some()
            })
            .cloned()
    };
    if let Some(session) = &attached_session {
        attach_session_command_context(state, session);
    }
    let session_id = existing
        .filter(|subscriber| subscriber.desk_alias.eq_ignore_ascii_case(&desk_alias))
        .map(|subscriber| subscriber.session_id)
        .or_else(|| attached_session.map(|session| session.id))
        .unwrap_or_else(|| {
            let Some(user) = state
                .desk
                .lock()
                .users()
                .ok()
                .and_then(|u| u.into_iter().find(|u| u.enabled))
            else {
                return SessionId::new();
            };
            let id = SessionId::new();
            let session = Session {
                id,
                user: user.clone(),
                token: Uuid::new_v4().to_string(),
                connected: true,
                desk: desk.clone(),
            };
            state.programmers.start(id, user.id);
            attach_session_command_context(state, &session);
            state.sessions.write().insert(id, session);
            id
        });
    state.osc_subscribers.lock().insert(
        client_id,
        OscSubscriber {
            desk_alias,
            target,
            command_source,
            session_id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    emit(
        state,
        "hardware_connection_changed",
        serde_json::json!({"connected":true}),
    );
    send_osc_feedback(state, true);
    true
}
