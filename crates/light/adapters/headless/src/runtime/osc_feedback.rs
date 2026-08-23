use super::*;

pub(super) fn send_action_timing_feedback(
    state: &AppState,
    desk_alias: &str,
    target: SocketAddr,
    timing: &ActionTimingProjection,
) {
    send_osc(
        state,
        target,
        format!("/light/{desk_alias}/feedback/action"),
        vec![
            OscArgument::String(timing.request_id.clone()),
            OscArgument::Bool(timing.succeeded),
            OscArgument::Int(i32::try_from(timing.received_output_tick).unwrap_or(i32::MAX)),
            OscArgument::Int(i32::try_from(timing.acknowledged_output_tick).unwrap_or(i32::MAX)),
            OscArgument::Int(i32::from(timing.output_frame_hz)),
            OscArgument::Int(i32::try_from(timing.budget_ticks).unwrap_or(i32::MAX)),
            OscArgument::Bool(timing.acknowledgement_within_budget),
        ],
    );
}

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
        && matches!(parts[3], "prog-fade" | "cue-fade" | "release-fade")
        && let Some(value) = numeric
    {
        state.installation.update_configuration(|configuration| {
            if parts[3] == "prog-fade" {
                configuration.programmer_fade_millis = (value.clamp(0.0, 1.0) * 20_000.0) as u64;
            } else if parts[3] == "cue-fade" {
                configuration.sequence_master_fade_millis =
                    (value.clamp(0.0, 1.0) * 60_000.0) as u64;
            } else {
                configuration.release_fade_millis = (value.clamp(0.0, 1.0) * 60_000.0) as u64;
            }
        });
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
        let now = application_millis(state);
        let _ = state.output.tap_speed_group(index, now);
        copy_speed_group_runtime_to_configuration(state, &[index]);
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
        if state
            .output
            .set_manual_speed_group(index, bpm, now, false)
            .is_ok()
        {
            copy_speed_group_runtime_to_configuration(state, &[index]);
            let _ = persist_server_configuration(state);
            refresh_speed_group_engine(state);
            emit(
                state,
                "speed_group_changed",
                serde_json::json!({"group":speed_group_name(index),"desk_alias":parts[1],"source":"osc","manual_bpm":bpm}),
            );
        }
    }
}

pub(super) fn handle_encoder_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) {
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
    let request_id = arguments.get(1).and_then(|argument| match argument {
        OscArgument::String(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    });
    let source = source.and_then(|value| value.parse::<SocketAddr>().ok());
    let Some((subscriber, session)) = programmer_osc_session(state, source) else {
        return;
    };
    if !subscriber.desk_alias.eq_ignore_ascii_case(parts[1]) {
        return;
    }
    emit(
        state,
        "desk_action",
        serde_json::json!({
            "desk_alias":parts[1],
            "desk_id":session.desk.id,
            "session_id":session.id,
            "request_id":request_id,
            "control":control,
            "value":value,
            "source":"osc"
        }),
    );
}

pub(super) fn send_osc(
    state: &AppState,
    target: SocketAddr,
    address: String,
    arguments: Vec<OscArgument>,
) {
    state.integrations.send_osc(target, address, arguments);
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

/// The canonical path a remote-control-only guest connects on. Playback only: no Record, no
/// Update, no Assign. Its counterpart is `desk`, which is not named here because every path that
/// is not this one is a desk-button surface.
pub(super) const OSC_REMOTE_ALIAS: &str = "remote";

/// What a surface connecting on `alias` is allowed to do.
///
/// The path is the capability. `remote` is the guest path; everything else — `desk`, the legacy
/// `main`, and any alias a saved hardware configuration still names — is a surface of the one
/// programming user.
pub(super) fn osc_surface_capability(alias: &str) -> light_core::SurfaceCapability {
    if alias.eq_ignore_ascii_case(OSC_REMOTE_ALIAS) {
        light_core::SurfaceCapability::PlaybackOnly
    } else {
        light_core::SurfaceCapability::Programming
    }
}

/// The desk an OSC alias addresses.
///
/// There is one desk, so every alias addresses it. A saved hardware configuration naming an alias
/// from before the collapse is not foreign — there is nothing left for it to be foreign to — so it
/// resolves rather than being refused and left silently unable to connect.
pub(super) fn osc_control_desk(state: &AppState, alias: &str) -> Option<ControlDesk> {
    state
        .installation
        .control_desk_by_alias(alias)
        .ok()
        .flatten()
        .or_else(|| state.installation.desks().ok()?.into_iter().next())
}
