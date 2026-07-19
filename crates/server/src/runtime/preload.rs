use super::*;

#[path = "preload/programmer.rs"]
mod programmer;
#[path = "preload/transaction.rs"]
mod transaction;

pub(super) use programmer::{
    control_action_programmer_values, profile_head_owner, validate_programmer_attribute_value,
};

#[derive(Debug)]
pub(super) struct StagedPreloadPlaybackAction {
    pub(super) playback_number: u16,
    pub(super) action: String,
    pub(super) surface: String,
    pub(super) released_playbacks: Vec<u16>,
}

pub(super) fn apply_preload_playback_verb(
    playback: &mut light_playback::PlaybackEngine,
    number: u16,
    action: &str,
) -> Result<(), String> {
    match action {
        "toggle" => playback.toggle(number).map(|_| ()),
        "go" => playback.go_playback(number).map(|_| ()),
        "go-minus" => playback.back_playback(number).map(|_| ()),
        "off" => playback.off(number).map(|_| ()),
        "on" => playback.on(number).map(|_| ()),
        "temp-on" => playback.set_temp_button(number, true),
        "temp-off" => playback.set_temp_button(number, false),
        _ => Err(format!("unsupported queued Preload action {action}")),
    }
}

/// Build one complete Preload playback result without changing the live engine. A rejected verb,
/// stale definition, or timing error therefore discards only this clone, even when it follows
/// actions that would otherwise have succeeded.
pub(super) fn stage_preload_playback_batch(
    current: &light_playback::PlaybackEngine,
    definitions: &[(
        light_programmer::PreloadPlaybackAction,
        light_playback::PlaybackDefinition,
    )],
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
    exclusion_zones: &[Vec<u16>],
) -> Result<
    (
        light_playback::PlaybackEngine,
        Vec<StagedPreloadPlaybackAction>,
    ),
    String,
> {
    let mut staged = current.clone();
    let mut actions = Vec::with_capacity(definitions.len());
    for (pending, definition) in definitions {
        let previous = staged
            .runtime()
            .into_iter()
            .find(|playback| playback.playback_number == Some(definition.number))
            .map(|playback| (playback.enabled, playback.master));
        let was_enabled = previous.is_some_and(|(enabled, _)| enabled);

        apply_preload_playback_verb(&mut staged, definition.number, &pending.action)?;
        let now_enabled = staged.runtime().into_iter().any(|playback| {
            playback.playback_number == Some(definition.number) && playback.enabled
        });
        let released_playbacks = if !was_enabled && now_enabled {
            enforce_virtual_playback_exclusions_on(&mut staged, exclusion_zones, definition.number)
        } else {
            Vec::new()
        };
        staged.apply_preload_timing(
            definition.number,
            &pending.action,
            committed_at,
            programmer_fade_millis,
            previous,
        )?;
        actions.push(StagedPreloadPlaybackAction {
            playback_number: definition.number,
            action: pending.action.clone(),
            surface: pending.surface.clone(),
            released_playbacks,
        });
    }
    Ok((staged, actions))
}

pub(super) fn record_preload_persistence_failure(
    state: &AppState,
    session: &Session,
    domain: &str,
    error: ApiError,
) -> String {
    let warning = format!(
        "Preload committed but {domain} persistence failed: {}",
        error.message
    );
    emit(
        state,
        "preload_persistence_failed",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "domain":domain,
            "source":"preload",
            "accepted":true,
            "error":error.message,
        }),
    );
    warning
}

pub(super) fn commit_preload(
    state: &AppState,
    session: &Session,
) -> Result<serde_json::Value, String> {
    // Match the normal action and render order from show identity through semantic publication.
    let _activation = state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| "the active show is changing; retry Preload GO".to_owned())?;
    let _ordered = state.playback_service.operation_lock();
    let _serialized = state.playback_action_lock.lock();
    state.programmers.with_transaction(session.id, || {
        transaction::commit_preload_transaction(state, session)
    })
}
