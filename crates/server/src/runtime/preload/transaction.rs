use super::*;

pub(super) fn commit_preload_transaction(
    state: &AppState,
    session: &Session,
) -> Result<CommittedPreload, String> {
    let preparation::PreparedPreloadCommit {
        pending,
        committed_at,
        programmer_fade_millis,
        prepared_playback,
        staged_actions,
        identities,
        before,
        context,
    } = preparation::prepare_preload_commit(state, session)?;
    let playback_runtime_changed = prepared_playback.effect().durable();
    install_preload_commit(state, session, pending, committed_at, prepared_playback)?;
    let changes =
        events::preload_change_events(state, &context, &identities, before, &staged_actions)?;
    events::emit_exclusions(state, session, &changes);
    let executed = executed_preload_actions(staged_actions, committed_at, programmer_fade_millis);
    let warnings = persist_preload_commit(state, session, playback_runtime_changed);
    Ok(CommittedPreload {
        committed_at,
        programmer_fade_millis,
        executed,
        warnings,
        events: changes.drafts,
    })
}

pub(super) struct CommittedPreload {
    pub(super) committed_at: chrono::DateTime<chrono::Utc>,
    pub(super) programmer_fade_millis: u64,
    pub(super) executed: Vec<serde_json::Value>,
    pub(super) warnings: Vec<String>,
    pub(super) events: Vec<light_application::EventDraft>,
}

fn install_preload_commit(
    state: &AppState,
    session: &Session,
    pending: Vec<light_programmer::PreloadPlaybackAction>,
    committed_at: chrono::DateTime<chrono::Utc>,
    prepared_playback: light_engine::PreparedPlaybackBatch,
) -> Result<(), String> {
    state
        .programmers
        .activate_preload_at(session.id, committed_at);
    let drained = state.programmers.take_preload_playback_actions(session.id);
    if drained != pending {
        return Err("the Preload queue changed while GO was being prepared".into());
    }
    state
        .engine
        .install_prepared_playback_batch(prepared_playback)
}

fn executed_preload_actions(
    actions: Vec<StagedPreloadPlaybackAction>,
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
) -> Vec<serde_json::Value> {
    actions
        .into_iter()
        .map(|action| executed_preload_action(action, committed_at, programmer_fade_millis))
        .collect()
}

fn executed_preload_action(
    action: StagedPreloadPlaybackAction,
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
) -> serde_json::Value {
    let mut executed = serde_json::json!({
        "playback_number":action.playback_number,
        "action":action.action.legacy_name(),
        "surface":action.surface.name(),
        "started_at":committed_at,
        "fallback_millis":programmer_fade_millis
    });
    if let Some(page) = action.page {
        executed["page"] = page.into();
    }
    executed
}

fn persist_preload_commit(
    state: &AppState,
    session: &Session,
    active_playbacks_changed: bool,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if let Err(error) = persist_programmer(state, session) {
        warnings.push(record_preload_persistence_failure(
            state,
            session,
            "programmer",
            error,
        ));
    }
    if active_playbacks_changed && let Err(error) = persist_active_playbacks(state) {
        warnings.push(record_preload_persistence_failure(
            state,
            session,
            "active playbacks",
            error,
        ));
    }
    warnings
}
