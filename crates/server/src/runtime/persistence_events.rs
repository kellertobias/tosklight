use super::*;

pub(super) fn persist_programmer(state: &AppState, session: &Session) -> Result<(), ApiError> {
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    state
        .desk
        .lock()
        .save_session(&PersistedSession {
            id: session.id,
            user_id: session.user.id,
            token: session.token.clone(),
            programmer_json: serde_json::to_string(&programmer)
                .map_err(|error| ApiError::internal(error.to_string()))?,
            connected: programmer.connected,
            updated_at: chrono::Utc::now().to_rfc3339(),
        })
        .map_err(ApiError::store)
}

pub(super) fn active_playbacks_setting(show_id: light_core::ShowId) -> String {
    format!("active_playbacks:{}", show_id.0)
}

pub(super) fn output_runtime_setting(show_id: light_core::ShowId) -> String {
    format!("output_runtime:{}", show_id.0)
}

pub(super) fn persist_output_runtime(state: &AppState) -> Result<(), ApiError> {
    let Some(show) = state.active_show.read().clone() else {
        return Ok(());
    };
    let (grand_master, blackout) = {
        let control = state.output_control.lock();
        (control.options.grand_master, control.options.blackout)
    };
    let runtime = PersistedOutputRuntime {
        grand_master,
        blackout,
        dynamics_paused_at: state.engine.playback_dynamics().paused_since,
        group_masters: state
            .engine
            .snapshot()
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.master))
            .collect(),
    };
    let serialized =
        serde_json::to_string(&runtime).map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .desk
        .lock()
        .set_setting(&output_runtime_setting(show.id), &serialized)
        .map_err(ApiError::store)
}

pub(super) fn persist_active_playbacks(state: &AppState) -> Result<(), ApiError> {
    let Some(show) = state.active_show.read().clone() else {
        return Ok(());
    };
    let runtime = state.engine.playback_runtime();
    let serialized =
        serde_json::to_string(&runtime).map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .desk
        .lock()
        .set_setting(&active_playbacks_setting(show.id), &serialized)
        .map_err(ApiError::store)
}
pub(super) fn emit(state: &AppState, kind: &str, payload: serde_json::Value) {
    let event = Event {
        revision: state.event_revision.fetch_add(1, Ordering::Relaxed) + 1,
        kind: kind.into(),
        payload,
    };
    {
        let mut audit = state.audit_events.lock();
        if audit.len() == 2048 {
            audit.pop_front();
        }
        audit.push_back(event.clone());
    }
    let _ = state.events.send(event);
}

pub(super) fn record_command_history(
    state: &AppState,
    session: &Session,
    command: &str,
    status: &str,
    feedback: &str,
    source: &str,
    request_id: Option<&str>,
) {
    let (retained_command, sensitive) = command_audit_projection(command);
    if retained_command.is_empty() {
        return;
    }
    let retained_feedback = if sensitive {
        "Sensitive input omitted".into()
    } else {
        feedback.chars().take(1_000).collect::<String>()
    };
    let entry = CommandHistoryEntry {
        id: Uuid::new_v4().to_string(),
        desk_id: session.desk.id,
        session_id: session.id,
        command: retained_command,
        status: status.into(),
        feedback: retained_feedback,
        source: source.into(),
        request_id: request_id.map(str::to_owned),
        at: chrono::Utc::now().to_rfc3339(),
    };
    {
        let mut histories = state.command_history.lock();
        let history = histories.entry(session.desk.id).or_default();
        history.push_front(entry.clone());
        history.truncate(COMMAND_HISTORY_LIMIT);
    }
    emit(
        state,
        "command_history",
        serde_json::to_value(entry).expect("command history entries serialize"),
    );
}

pub(super) fn command_audit_projection(command: &str) -> (String, bool) {
    let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ");
    let upper = normalized.to_ascii_uppercase();
    let sensitive = [
        "PASSWORD",
        "PASSCODE",
        "TOKEN",
        "SECRET",
        "AUTHORIZATION",
        "API_KEY",
    ]
    .iter()
    .any(|term| upper.split_whitespace().any(|token| token.contains(term)));
    if sensitive {
        ("[REDACTED SENSITIVE COMMAND]".into(), true)
    } else {
        (normalized.chars().take(512).collect(), false)
    }
}
pub(super) fn validate_show_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 100 || name.contains(['/', '\\']) {
        Err(ApiError::bad_request(
            "show name must be a plain name up to 100 characters",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn available_show_name(state: &AppState, stem: &str) -> Result<String, ApiError> {
    let existing = state
        .desk
        .lock()
        .library()
        .map_err(ApiError::store)?
        .into_iter()
        .map(|show| show.name.to_lowercase())
        .collect::<HashSet<_>>();
    for number in 1..=10_000 {
        let candidate = if number == 1 {
            stem.to_owned()
        } else {
            format!("{stem} {number}")
        };
        let path = state
            .data_dir
            .join("shows")
            .join(format!("{candidate}.show"));
        if !existing.contains(&candidate.to_lowercase()) && !path.exists() {
            return Ok(candidate);
        }
    }
    Err(ApiError::conflict("no available show name remains"))
}

pub(super) fn revision_copy_name(
    state: &AppState,
    source_name: &str,
    revision: u64,
    copied_on: chrono::NaiveDate,
) -> Result<String, ApiError> {
    let existing = state
        .desk
        .lock()
        .library()
        .map_err(ApiError::store)?
        .into_iter()
        .map(|show| show.name.to_lowercase())
        .collect::<HashSet<_>>();
    let stem_suffix = format!("-rev-{revision}-{copied_on}");
    for number in 1..=10_000 {
        let disambiguator = if number == 1 {
            String::new()
        } else {
            format!("-{number}")
        };
        let available = 100usize.saturating_sub(stem_suffix.len() + disambiguator.len());
        let mut boundary = source_name.len().min(available);
        while !source_name.is_char_boundary(boundary) {
            boundary -= 1;
        }
        let candidate = format!(
            "{}{}{}",
            &source_name[..boundary],
            stem_suffix,
            disambiguator
        );
        let path = state
            .data_dir
            .join("shows")
            .join(format!("{candidate}.show"));
        if !existing.contains(&candidate.to_lowercase()) && !path.exists() {
            return Ok(candidate);
        }
    }
    Err(ApiError::conflict(
        "no unused name is available for the revision copy",
    ))
}
