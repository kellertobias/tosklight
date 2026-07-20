use super::transaction::CommittedPreload;
use super::*;

pub(super) fn preload_commit_response(
    state: &AppState,
    session: &Session,
    committed: CommittedPreload,
    playback_event_sequences: Vec<u64>,
) -> serde_json::Value {
    let CommittedPreload {
        committed_at,
        programmer_fade_millis,
        executed,
        warnings,
        events: _,
    } = committed;
    let mut payload = serde_json::json!({
        "session_id":session.id,
        "application_timestamp":committed_at,
        "programmer_fade_millis":programmer_fade_millis,
        "playback_actions":executed,
        "playback_event_sequences":playback_event_sequences,
    });
    if !warnings.is_empty() {
        payload["warnings"] = serde_json::json!(warnings);
    }
    emit(state, "preload_committed", payload.clone());
    emit(
        state,
        "programmer_changed",
        serde_json::json!({
            "session_id":session.id,
            "user_id":session.user.id,
            "preload_committed_at":committed_at,
            "changes":if executed.is_empty() { Vec::<&str>::new() } else { vec!["preload_playback_queue"] },
        }),
    );
    if !executed.is_empty() {
        emit(
            state,
            "playback_changed",
            serde_json::json!({"session_id":session.id,"source":"preload","application_timestamp":committed_at,"actions":executed}),
        );
    }
    let mut response = serde_json::json!({
        "active":true,
        "application_timestamp":committed_at,
        "programmer_fade_millis":programmer_fade_millis,
        "playback_actions":payload["playback_actions"],
        "playback_event_sequences":payload["playback_event_sequences"],
    });
    if let Some(warnings) = payload.get("warnings") {
        response["warnings"] = warnings.clone();
    }
    response
}
