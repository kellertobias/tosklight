use super::*;

pub(super) fn parse_spread_points(tokens: &[String]) -> Result<Vec<f32>, String> {
    if tokens.len() < 3 || tokens.len().is_multiple_of(2) {
        return Err("a spread requires levels separated by THRU".into());
    }
    let mut points = Vec::with_capacity(tokens.len().div_ceil(2));
    for (index, token) in tokens.iter().enumerate() {
        if index % 2 == 1 {
            if token != "THRU" {
                return Err("spread control points must be separated by THRU".into());
            }
            continue;
        }
        let percent = if token == "FULL" {
            100.0
        } else {
            token
                .parse::<f32>()
                .map_err(|_| "spread levels must be percentages or FULL")?
        };
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err("spread levels must be within 0-100".into());
        }
        points.push(percent / 100.0);
    }
    Ok(points)
}

pub(super) fn spread_position(points: &[f32], index: usize, count: usize) -> f32 {
    if points.len() == 1 || count <= 1 {
        return points[0];
    }
    let position = index as f32 * (points.len() - 1) as f32 / (count - 1) as f32;
    let left = position.floor() as usize;
    let right = position.ceil() as usize;
    points[left] + (points[right] - points[left]) * (position - left as f32)
}

pub(super) fn parse_command_cue_number(tokens: &[String]) -> Result<f64, String> {
    if tokens.is_empty() {
        return Err("CUE requires a cue number".into());
    }
    let value = tokens.join("");
    let number = value.parse::<f64>().map_err(|_| "cue number is invalid")?;
    if !number.is_finite() || number <= 0.0 {
        return Err("cue number must be positive".into());
    }
    Ok(number)
}

pub(super) fn execute_cue_operation(
    state: &AppState,
    session: &Session,
    tokens: &[String],
) -> Result<usize, String> {
    let load = tokens.get(1).is_some_and(|token| token == "CUE");
    let start = if load { 2 } else { 1 };
    let snapshot = state.engine.snapshot();
    let (playback, cue_number) = if tokens.get(start).is_some_and(|token| token == "SET") {
        let (address, consumed) = parse_playback_address(&tokens[start..], true, &snapshot)?;
        if start + consumed != tokens.len() {
            return Err("unexpected tokens after Cue address".into());
        }
        (
            address.playback,
            address
                .cue
                .ok_or("explicit Cue address requires CUE and a Cue number")?,
        )
    } else {
        let show = state.active_show.read().clone().ok_or("no show is open")?;
        let selected = state
            .desk
            .lock()
            .selected_playback(session.desk.id, show.id)
            .map_err(|error| error.to_string())?
            .ok_or(
                "no playback is selected; select a playback or use CUE SET <playback> CUE <cue>",
            )?;
        (selected, parse_command_cue_number(&tokens[start..])?)
    };
    if !snapshot
        .playbacks
        .iter()
        .any(|definition| definition.number == playback)
    {
        return Err(format!("playback {playback} does not exist"));
    }
    state.engine.execute_playback(EnginePlaybackCommand::Pool {
        number: playback,
        action: if load {
            PoolPlaybackAction::Load(cue_number)
        } else {
            PoolPlaybackAction::GoTo(cue_number)
        },
    })?;
    emit(
        state,
        "playback_changed",
        serde_json::json!({"playback_number":playback,"action":if load {"load"} else {"go-to"},"cue_number":cue_number,"session_id":session.id}),
    );
    Ok(1)
}

pub(super) fn pending_cue_transfer_choice(command_line: &str) -> Option<serde_json::Value> {
    let tokens = command_line
        .replace(',', ".")
        .replace('.', " . ")
        .split_whitespace()
        .map(|token| token.to_ascii_uppercase())
        .collect::<Vec<_>>();
    let operation = match tokens.first()?.as_str() {
        "COPY" | "CPY" => "COPY",
        "MOVE" | "MOV" => "MOVE",
        _ => return None,
    };
    if tokens
        .get(1)
        .is_some_and(|token| matches!(token.as_str(), "PLAIN" | "STATUS"))
    {
        return None;
    }
    let at = tokens.iter().position(|token| token == "AT")?;
    if tokens.get(1).is_none_or(|token| token != "SET")
        || !tokens[1..at].iter().any(|token| token == "CUE")
        || tokens.get(at + 1).is_none_or(|token| token != "SET")
        || !tokens[at + 1..].iter().any(|token| token == "CUE")
    {
        return None;
    }
    let title = if operation == "COPY" { "Copy" } else { "Move" };
    let suffix = tokens[1..].join(" ");
    Some(serde_json::json!({
        "type":"cue_move_copy",
        "operation":operation.to_ascii_lowercase(),
        "command":command_line,
        "options":[
            {
                "id":"plain",
                "label":format!("Plain {title}"),
                "command":format!("{operation} PLAIN {suffix}")
            },
            {
                "id":"status",
                "label":format!("Status {title}"),
                "command":format!("{operation} STATUS {suffix}")
            }
        ],
        "cancel_label":"Cancel"
    }))
}

pub(super) fn command_speed_group_index(token: &str) -> Result<usize, String> {
    let group = token
        .parse::<usize>()
        .map_err(|_| "Speed Group number is invalid")?;
    if !(1..=5).contains(&group) {
        return Err("Speed Group number must be within 1-5".into());
    }
    Ok(group - 1)
}

pub(super) fn command_bpm_at(tokens: &[String]) -> Result<(f64, usize), String> {
    let whole = tokens.first().ok_or("AT requires a BPM value")?;
    let (value, consumed) = if tokens.get(1).is_some_and(|token| token == ".") {
        let fraction = tokens
            .get(2)
            .ok_or("BPM decimal requires digits after the separator")?;
        (format!("{whole}.{fraction}"), 3)
    } else {
        (whole.clone(), 1)
    };
    let bpm = value.parse::<f64>().map_err(|_| "BPM value is invalid")?;
    if !bpm.is_finite() {
        return Err("BPM value must be finite".into());
    }
    Ok((bpm, consumed))
}

pub(super) fn execute_speed_group_command(
    state: &AppState,
    tokens: &[String],
) -> Result<usize, String> {
    if tokens.len() < 5 || tokens[0] != "SPD" || tokens[1] != "GRP" || tokens[3] != "AT" {
        return Err("expected SPD GRP <1-5> AT <BPM | +/- BPM | SPD GRP <1-5>>".into());
    }
    let source = command_speed_group_index(&tokens[2])?;
    let right = &tokens[4..];
    let now = application_millis(state);
    let mut controllers = state.speed_groups.lock();
    let affected = if right.first().is_some_and(|token| token == "SPD") {
        if right.len() != 3 || right[1] != "GRP" {
            return Err("synchronization target must be SPD GRP <1-5>".into());
        }
        let target = command_speed_group_index(&right[2])?;
        synchronize_speed_groups(&mut controllers, source, target, now)
            .map_err(|error| error.message)?;
        vec![source, target]
    } else {
        let (relative, value_tokens) = match right.first().map(String::as_str) {
            Some("+") => (1.0, &right[1..]),
            Some("-") => (-1.0, &right[1..]),
            _ => (0.0, right),
        };
        let (entered, consumed) = command_bpm_at(value_tokens)?;
        if consumed != value_tokens.len() {
            return Err("unexpected tokens after BPM value".into());
        }
        let bpm = if relative == 0.0 {
            entered
        } else {
            controllers[source].manual_bpm() + relative * entered
        };
        unlink_speed_group(&mut controllers, source, now);
        controllers[source]
            .set_manual_bpm(bpm)
            .map_err(|error| error.to_string())?;
        controllers[source]
            .set_speed_master_scale(1.0)
            .map_err(|error| error.to_string())?;
        controllers[source].set_paused_at(false, now);
        vec![source]
    };
    copy_speed_group_runtime_to_configuration(state, &controllers, &affected);
    let snapshots: [SpeedSnapshot; 5] =
        std::array::from_fn(|index| controllers[index].snapshot(now));
    drop(controllers);

    {
        let mut owners = state.sound_capture_owners.lock();
        for &index in &affected {
            owners[index] = None;
        }
    }
    persist_server_configuration(state).map_err(|error| error.message)?;
    refresh_speed_group_engine(state);
    emit(
        state,
        "speed_group_command",
        serde_json::json!({
            "command":tokens.join(" "),
            "groups":affected.iter().map(|index| speed_group_name(*index)).collect::<Vec<_>>(),
            "snapshots":affected.iter().map(|index| snapshots[*index]).collect::<Vec<_>>()
        }),
    );
    Ok(affected.len())
}
