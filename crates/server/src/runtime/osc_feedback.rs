use super::*;

pub(super) fn handle_timing_osc(state: &AppState, address: &str, arguments: &[OscArgument]) {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    let numeric = arguments.first().and_then(|v| match v {
        OscArgument::Float(v) => Some(*v),
        OscArgument::Int(v) => Some(*v as f32),
        _ => None,
    });
    if parts.len() == 4
        && parts[0] == "light"
        && parts[2] == "programmer"
        && matches!(parts[3], "prog-fade" | "cue-fade")
        && let Some(value) = numeric
    {
        let mut config = state.configuration.write();
        if parts[3] == "prog-fade" {
            config.programmer_fade_millis = (value.clamp(0.0, 1.0) * 20_000.0) as u64;
        } else {
            config.sequence_master_fade_millis = (value.clamp(0.0, 1.0) * 60_000.0) as u64;
        }
        drop(config);
        let _ = persist_server_configuration(state);
        refresh_speed_group_engine(state);
    }
    if parts.len() == 5
        && parts[0] == "light"
        && parts[2] == "speed-group"
        && parts[4] == "button"
        && let Ok(group) = parts[3].parse::<usize>()
        && group > 0
        && group <= 5
        && osc_pressed(arguments)
    {
        let index = group - 1;
        let mut controllers = state.speed_groups.lock();
        let now = application_millis(state);
        unlink_speed_group(&mut controllers, index, now);
        controllers[index].tap_learn(now);
        copy_speed_group_runtime_to_configuration(state, &controllers, &[index]);
        drop(controllers);
        state.sound_capture_owners.lock()[index] = None;
        let _ = persist_server_configuration(state);
        let snapshots = refresh_speed_group_engine(state);
        emit(
            state,
            "speed_group_action",
            serde_json::json!({"group":speed_group_name(index),"desk_alias":parts[1],"source":"osc","action":"learn","snapshot":snapshots[index]}),
        );
    }
    if parts.len() == 5
        && parts[0] == "light"
        && parts[2] == "speed-group"
        && parts[4] == "encoder"
        && let Ok(group) = parts[3].parse::<usize>()
        && let Some(value) = numeric
        && group > 0
        && group <= 5
    {
        let index = group - 1;
        let bpm = f64::from(value).clamp(0.1, 999.0);
        let now = application_millis(state);
        let mut controllers = state.speed_groups.lock();
        unlink_speed_group(&mut controllers, index, now);
        if controllers[index].set_manual_bpm(bpm).is_ok() {
            let _ = controllers[index].set_speed_master_scale(1.0);
            controllers[index].set_paused_at(false, now);
            copy_speed_group_runtime_to_configuration(state, &controllers, &[index]);
            drop(controllers);
            state.sound_capture_owners.lock()[index] = None;
            let _ = persist_server_configuration(state);
            refresh_speed_group_engine(state);
            emit(
                state,
                "speed_group_changed",
                serde_json::json!({"group":speed_group_name(index),"desk_alias":parts[1],"source":"osc","manual_bpm":bpm}),
            );
        } else {
            drop(controllers);
        }
    }
}

pub(super) fn handle_encoder_osc(state: &AppState, address: &str, arguments: &[OscArgument]) {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    let value = arguments.first().and_then(|argument| match argument {
        OscArgument::String(value) => Some(value.as_str()),
        _ => None,
    });
    let valid =
        value.is_some_and(|value| matches!(value, "up" | "down" | "left" | "right" | "press"));
    if !valid || parts.first() != Some(&"light") {
        return;
    }
    let control = if parts.len() == 4
        && parts[2] == "encode"
        && parts[3]
            .parse::<u8>()
            .is_ok_and(|number| (1..=6).contains(&number))
    {
        format!("encode/{}", parts[3])
    } else if parts.len() == 3 && parts[2] == "nav" {
        "nav".into()
    } else {
        return;
    };
    emit(
        state,
        "desk_action",
        serde_json::json!({"desk_alias":parts[1],"control":control,"value":value,"source":"osc"}),
    );
}

pub(super) fn send_osc(
    state: &AppState,
    target: SocketAddr,
    address: String,
    arguments: Vec<OscArgument>,
) {
    #[cfg(test)]
    state
        .osc_feedback_capture
        .lock()
        .push((target, address.clone(), arguments.clone()));
    if let (Some(socket), Ok(packet)) = (
        &state.osc_feedback,
        encode_osc_message(&address, &arguments),
    ) {
        let _ = socket.send_to(&packet, target);
    }
}

pub(super) fn speed_group_osc_feedback(snapshot: SpeedSnapshot) -> Vec<OscArgument> {
    vec![
        OscArgument::Int(snapshot.effective_bpm.round().clamp(0.0, 999.0) as i32),
        OscArgument::Float(0.0),
        OscArgument::Float(0.75),
        OscArgument::Float(0.95),
        OscArgument::String(
            if snapshot.phase_advancing {
                "on"
            } else {
                "off"
            }
            .into(),
        ),
    ]
}

pub(super) fn playback_color_rgb(color: &str, active: bool) -> (f32, f32, f32) {
    let component = |range: std::ops::Range<usize>| {
        u8::from_str_radix(color.get(range).unwrap_or_default(), 16).unwrap_or(0x20) as f32 / 255.0
    };
    let scale = if active { 1.0 } else { 0.35 };
    (
        component(1..3) * scale,
        component(3..5) * scale,
        component(5..7) * scale,
    )
}

pub(super) fn osc_control_desk(state: &AppState, alias: &str) -> Option<ControlDesk> {
    let store = state.desk.lock();
    if alias.eq_ignore_ascii_case("main") || alias.is_empty() {
        store.desks().ok()?.into_iter().next()
    } else {
        store.control_desk_by_alias(alias).ok().flatten()
    }
}
