use super::*;

pub(super) fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Session, ApiError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::unauthorized("missing session token"))?;
    authenticate_token(state, token)
}
pub(super) fn authenticate_token(state: &AppState, token: &str) -> Result<Session, ApiError> {
    let session = state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .cloned()
        .ok_or_else(|| ApiError::unauthorized("invalid session token"))?;
    attach_session_command_context(state, &session);
    Ok(session)
}

pub(super) fn attach_session_command_context(state: &AppState, session: &Session) {
    state
        .programmers
        .attach_command_context(session.id, SessionId(session.desk.id));
}
pub(super) fn parse_if_match(headers: &HeaderMap) -> Result<u64, ApiError> {
    let value = headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::bad_request("If-Match revision is required"))?
        .trim_matches('"');
    value
        .parse()
        .map_err(|_| ApiError::bad_request("If-Match must contain a numeric revision"))
}
pub(super) fn backup_show(state: &AppState, entry: &ShowEntry) -> Result<PathBuf, ApiError> {
    let directory = state.data_dir.join("backups");
    std::fs::create_dir_all(&directory).map_err(ApiError::io)?;
    let destination = directory.join(format!(
        "{}-{}.show",
        entry.name,
        chrono::Utc::now().timestamp_millis()
    ));
    ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .backup_to(&destination)
        .map_err(ApiError::store)?;
    let prefix = format!("{}-", entry.name);
    let mut backups = std::fs::read_dir(&directory)
        .map_err(ApiError::io)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".show"))
        })
        .collect::<Vec<_>>();
    backups.sort();
    let retention = state.configuration.read().backup_retention;
    let remove_count = backups.len().saturating_sub(retention);
    for path in backups.into_iter().take(remove_count) {
        std::fs::remove_file(path).map_err(ApiError::io)?;
    }
    Ok(destination)
}
pub(super) async fn activate_snapshot(
    state: &AppState,
    snapshot: EngineSnapshot,
    transition: &Transition,
    duration: Option<u64>,
) -> Result<(), ApiError> {
    let prepared = state
        .engine
        .prepare_snapshot(snapshot)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    activate_prepared_snapshot(state, prepared, transition, duration).await;
    Ok(())
}

pub(super) async fn activate_prepared_snapshot(
    state: &AppState,
    prepared: PreparedEngineSnapshot,
    transition: &Transition,
    duration: Option<u64>,
) {
    // A remembered Highlight selection belongs only to the current live show context. Clear the
    // transient overlay before any transition so it cannot reappear in the newly loaded show.
    state.highlight.clear_all();
    state.patch_preview_highlights.lock().clear();
    state.engine.clear_highlighted_fixtures();
    let media_fixture_ids = prepared
        .snapshot()
        .fixtures
        .iter()
        .filter(|fixture| fixture.direct_control.is_some())
        .map(|fixture| fixture.fixture_id)
        .collect::<std::collections::HashSet<_>>();
    state
        .media_status
        .write()
        .retain(|fixture, _| media_fixture_ids.contains(fixture));
    state.media_cache.lock().retain_fixtures(
        &media_fixture_ids
            .iter()
            .map(|fixture| fixture.0.to_string())
            .collect(),
    );
    let frame = Duration::from_millis(25);
    match transition {
        Transition::HoldCurrent => {
            state.output_control.lock().hold = true;
            state
                .engine
                .install_prepared_snapshot_releasing_playback(prepared);
            tokio::time::sleep(frame).await;
            state.output_control.lock().hold = false;
        }
        Transition::SafeBlackout => {
            state.output_control.lock().options.blackout = true;
            tokio::time::sleep(frame * 2).await;
            state
                .engine
                .install_prepared_snapshot_releasing_playback(prepared);
            tokio::time::sleep(frame).await;
            state.output_control.lock().options.blackout = false;
        }
        Transition::TimedFade => {
            let duration = duration.unwrap_or(1_000).clamp(100, 30_000);
            let steps = 20_u64;
            let sleep = Duration::from_millis((duration / (steps * 2)).max(1));
            for step in 1..=steps {
                state.output_control.lock().options.grand_master = 1.0 - step as f32 / steps as f32;
                tokio::time::sleep(sleep).await;
            }
            state
                .engine
                .install_prepared_snapshot_releasing_playback(prepared);
            for step in 1..=steps {
                state.output_control.lock().options.grand_master = step as f32 / steps as f32;
                tokio::time::sleep(sleep).await;
            }
        }
    }
}
