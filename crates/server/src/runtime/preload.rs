use super::*;
use light_engine::{
    PlaybackBatchAction, PlaybackBatchCommand, PlaybackBatchOutcome, PreparedPlaybackBatch,
};

#[path = "preload/programmer.rs"]
mod programmer;
#[path = "preload/response.rs"]
mod response;
#[path = "preload/transaction.rs"]
mod transaction;

pub(super) use programmer::{
    control_action_programmer_values, profile_head_owner, validate_programmer_attribute_value,
};

#[derive(Debug)]
pub(super) struct StagedPreloadPlaybackAction {
    pub(super) playback_number: u16,
    pub(super) page: Option<u8>,
    pub(super) action: String,
    pub(super) surface: String,
    pub(super) addressed_event_required: bool,
    pub(super) released_playbacks: Vec<u16>,
}

pub(super) fn preload_batch_commands(
    pending: &[light_programmer::PreloadPlaybackAction],
) -> Result<Vec<PlaybackBatchCommand>, String> {
    pending
        .iter()
        .map(|pending| {
            let action = match pending.action {
                light_programmer::PreloadPlaybackQueueAction::Toggle => PlaybackBatchAction::Toggle,
                light_programmer::PreloadPlaybackQueueAction::Go => PlaybackBatchAction::Go,
                light_programmer::PreloadPlaybackQueueAction::Back => PlaybackBatchAction::Back,
                light_programmer::PreloadPlaybackQueueAction::Off => PlaybackBatchAction::Off,
                light_programmer::PreloadPlaybackQueueAction::On => PlaybackBatchAction::On,
                light_programmer::PreloadPlaybackQueueAction::TemporaryOn => {
                    PlaybackBatchAction::SetTempButton(true)
                }
                light_programmer::PreloadPlaybackQueueAction::TemporaryOff => {
                    PlaybackBatchAction::SetTempButton(false)
                }
            };
            Ok(PlaybackBatchCommand {
                number: pending.playback_number,
                action,
                exclusion_zones: Vec::new(),
            })
        })
        .collect()
}

pub(super) fn staged_preload_actions(
    pending: &[light_programmer::PreloadPlaybackAction],
    prepared: &PreparedPlaybackBatch,
) -> Vec<StagedPreloadPlaybackAction> {
    pending
        .iter()
        .zip(prepared.outcomes())
        .map(|(pending, outcome)| staged_preload_action(pending, outcome))
        .collect()
}

fn staged_preload_action(
    pending: &light_programmer::PreloadPlaybackAction,
    outcome: &PlaybackBatchOutcome,
) -> StagedPreloadPlaybackAction {
    debug_assert_eq!(pending.playback_number, outcome.number);
    StagedPreloadPlaybackAction {
        playback_number: pending.playback_number,
        page: pending.page,
        action: pending.action.legacy_name().to_owned(),
        surface: pending.surface.name().to_owned(),
        addressed_event_required: outcome.addressed_effect.changed(),
        released_playbacks: outcome.released_playbacks.clone(),
    }
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

#[cfg(test)]
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
    commit_preload_while_show_stable(state, session)
}

pub(super) fn commit_preload_while_show_stable(
    state: &AppState,
    session: &Session,
) -> Result<serde_json::Value, String> {
    let completed = state
        .playback_service
        .run_unit_of_work(CommitPreload { state, session });
    let committed = completed.output?;
    Ok(response::preload_commit_response(
        state,
        session,
        committed,
        completed.event_sequences,
    ))
}

struct CommitPreload<'a> {
    state: &'a AppState,
    session: &'a Session,
}

impl light_application::PlaybackUnitOfWork for CommitPreload<'_> {
    type Output = Result<transaction::CommittedPreload, String>;

    fn execute(self) -> light_application::PlaybackOperation<Self::Output> {
        let result = self
            .state
            .programmers
            .with_transaction(self.session.id, || {
                transaction::commit_preload_transaction(self.state, self.session)
            });
        match result {
            Ok(mut committed) => {
                let events = std::mem::take(&mut committed.events);
                light_application::PlaybackOperation::with_events(Ok(committed), events)
            }
            Err(error) => light_application::PlaybackOperation::new(Err(error)),
        }
    }
}
