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
        .session_for_token(token)
        .ok_or_else(|| ApiError::unauthorized("invalid session token"))?;
    attach_session_command_context(state, &session);
    Ok(session)
}

pub(super) fn attach_session_command_context(state: &AppState, session: &Session) {
    state
        .programming
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
    let directory = state.installation.data_dir().join("backups");
    std::fs::create_dir_all(&directory).map_err(ApiError::io)?;
    let destination = directory.join(format!(
        "{}-{}.show",
        entry.name,
        chrono::Utc::now().timestamp_millis()
    ));
    ActiveShowRepository::open(&entry.path)
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
    let retention = state.installation.configuration().backup_retention;
    let remove_count = backups.len().saturating_sub(retention);
    for path in backups.into_iter().take(remove_count) {
        std::fs::remove_file(path).map_err(ApiError::io)?;
    }
    Ok(destination)
}
pub(super) async fn activate_prepared_show(
    state: &AppState,
    prepared: PreparedEngineSnapshot,
    context: &light_application::ActionContext,
    transition: &Transition,
    duration: Option<u64>,
    entry: ShowEntry,
    output_runtime: PersistedOutputRuntime,
) -> Result<(), ApiError> {
    let show = (entry, output_runtime);
    // A remembered Highlight selection belongs only to the current live show context. Clear the
    // transient overlay before any transition so it cannot reappear in the newly loaded show.
    state.highlight.clear_all();
    state.output.clear_highlighted_fixtures();
    let media_fixture_ids = prepared
        .snapshot()
        .fixtures
        .iter()
        .filter(|fixture| fixture.direct_control.is_some())
        .map(|fixture| fixture.fixture_id)
        .collect::<std::collections::HashSet<_>>();
    state.media.retain_fixtures(&media_fixture_ids);
    let frame = Duration::from_millis(25);
    match transition {
        Transition::HoldCurrent => {
            state.output.set_transition_hold(true);
            install_activated_snapshot(state, context, prepared, show).await?;
            tokio::time::sleep(frame).await;
            state.output.set_transition_hold(false);
        }
        Transition::SafeBlackout => {
            state.output.set_transition_blackout(true);
            tokio::time::sleep(frame * 2).await;
            install_activated_snapshot(state, context, prepared, show).await?;
            tokio::time::sleep(frame).await;
            state.output.set_transition_blackout(false);
        }
        Transition::TimedFade => {
            let duration = duration.unwrap_or(1_000).clamp(100, 30_000);
            let steps = 20_u64;
            let sleep = Duration::from_millis((duration / (steps * 2)).max(1));
            for step in 1..=steps {
                state
                    .output
                    .set_transition_grand_master(1.0 - step as f32 / steps as f32);
                tokio::time::sleep(sleep).await;
            }
            install_activated_snapshot(state, context, prepared, show).await?;
            for step in 1..=steps {
                state
                    .output
                    .set_transition_grand_master(step as f32 / steps as f32);
                tokio::time::sleep(sleep).await;
            }
        }
    }
    Ok(())
}

async fn install_activated_snapshot(
    state: &AppState,
    context: &light_application::ActionContext,
    prepared: PreparedEngineSnapshot,
    show: (ShowEntry, PersistedOutputRuntime),
) -> Result<(), ApiError> {
    let activation = state.active_show.acquire().await;
    let worker_state = state.clone();
    let worker_context = context.clone();
    tokio::task::spawn_blocking(move || {
        install_prepared_snapshot_with_selection_refresh(
            &worker_state,
            &worker_context,
            prepared,
            None,
            PlaybackInstallPolicy::Release,
            HighlightInstallPolicy::Clear,
        );
        let (entry, output_runtime) = show;
        invalidate_active_show_document(&worker_state);
        worker_state
            .active_show
            .replace_current(Some(entry.clone()));
        worker_state.attributes.install_entry(Some(&entry));
        worker_state.active_show.set_error(None);
        restore_output_runtime_for_show(&worker_state, entry.id, output_runtime);
        drop(activation);
    })
    .await
    .map_err(|error| ApiError::internal(format!("show activation task failed: {error}")))
}
