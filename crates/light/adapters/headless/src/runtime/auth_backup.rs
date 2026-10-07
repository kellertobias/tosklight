use super::*;

/// The screen a request comes from, when the surface names one.
///
/// One desktop process drives the main window and every optional screen, so they share a session.
/// A request therefore says which screen it originates from, and a screen marked Not Editable
/// downgrades it to a guest. The header can only take capability away — a surface cannot grant
/// itself programming by claiming to be a different screen.
pub(super) const SCREEN_CONTEXT_HEADER: &str = "x-tosk-screen";

pub(super) fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Session, ApiError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::unauthorized("missing session token"))?;
    let mut session = authenticate_token(state, token)?;
    if requesting_screen_is_not_editable(state, headers) {
        session.capability = light_core::SurfaceCapability::PlaybackOnly;
    }
    Ok(session)
}

fn requesting_screen_is_not_editable(state: &AppState, headers: &HeaderMap) -> bool {
    headers
        .get(SCREEN_CONTEXT_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| uuid::Uuid::parse_str(value.trim()).ok())
        .and_then(|screen_id| state.installation.screen(screen_id).ok().flatten())
        .is_some_and(|screen| screen.not_editable)
}
pub(super) fn authenticate_token(state: &AppState, token: &str) -> Result<Session, ApiError> {
    let session = state
        .sessions
        .session_for_token(token)
        .ok_or_else(|| ApiError::unauthorized("invalid session token"))?;
    // A read-only visualizer never claims the desk command line, not even by reading.
    if !state.sessions.role(session.id).is_read_only() {
        attach_session_command_context(state, &session);
    }
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
