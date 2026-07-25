use super::*;
use light_engine::{
    PlaybackBatchAction, PlaybackBatchCommand, PlaybackBatchOutcome, PreparedPlaybackBatch,
};

#[path = "preload/authority.rs"]
mod authority;
#[path = "preload/events.rs"]
mod events;
#[path = "preload/preparation.rs"]
mod preparation;
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
    pub(super) action: light_programmer::PreloadPlaybackQueueAction,
    pub(super) surface: light_programmer::PreloadPlaybackQueueSurface,
    pub(super) addressed_effect_changed: bool,
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
                exclusion_zones: std::sync::Arc::default(),
                activation_origin: None,
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
        action: pending.action,
        surface: pending.surface,
        addressed_effect_changed: outcome.addressed_effect.changed(),
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
    let context = compatibility_context(session);
    let completed = state.playback_service.run_unit_of_work(CommitPreload {
        state,
        session,
        context,
    });
    let committed = completed.output?;
    Ok(response::preload_commit_response(
        state,
        session,
        committed,
        completed.event_sequences,
    ))
}

pub(super) fn commit_preload_lifecycle_while_show_stable(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    request: &light_application::ProgrammingPreloadLifecycleRequest,
) -> Result<light_application::ProgrammingPreloadCommitResult, light_application::ActionError> {
    let completed = state.playback_service.run_unit_of_work(CommitTypedPreload {
        state,
        session,
        context: context.clone(),
        request,
    });
    let (committed, authority) = completed.output?;
    typed_commit_result(committed, authority, completed.event_sequences)
}

struct CommitPreload<'a> {
    state: &'a AppState,
    session: &'a Session,
    context: light_application::ActionContext,
}

impl light_application::PlaybackUnitOfWork for CommitPreload<'_> {
    type Output = Result<transaction::CommittedPreload, String>;

    fn execute(self) -> light_application::PlaybackOperation<Self::Output> {
        let result = self
            .state
            .programmers
            .with_transaction(self.session.id, || {
                transaction::commit_preload_transaction(self.state, self.session, self.context)
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

struct CommitTypedPreload<'a> {
    state: &'a AppState,
    session: &'a Session,
    context: light_application::ActionContext,
    request: &'a light_application::ProgrammingPreloadLifecycleRequest,
}

impl light_application::PlaybackUnitOfWork for CommitTypedPreload<'_> {
    type Output = Result<
        (
            transaction::CommittedPreload,
            authority::PreloadCommitAuthority,
        ),
        light_application::ActionError,
    >;

    fn execute(self) -> light_application::PlaybackOperation<Self::Output> {
        let authority = match authority::validate(self.state, self.session, self.request) {
            Ok(authority) => authority,
            Err(error) => return light_application::PlaybackOperation::new(Err(error)),
        };
        let result = self
            .state
            .programmers
            .with_transaction(self.session.id, || {
                transaction::commit_preload_transaction(self.state, self.session, self.context)
            })
            .map_err(preload_commit_error);
        match result {
            Ok(mut committed) => {
                let events = std::mem::take(&mut committed.events);
                light_application::PlaybackOperation::with_events(
                    Ok((committed, authority)),
                    events,
                )
            }
            Err(error) => light_application::PlaybackOperation::new(Err(error)),
        }
    }
}

fn typed_commit_result(
    committed: transaction::CommittedPreload,
    authority: authority::PreloadCommitAuthority,
    event_sequences: Vec<u64>,
) -> Result<light_application::ProgrammingPreloadCommitResult, light_application::ActionError> {
    if committed.runtime_projections.len() != event_sequences.len() {
        return Err(light_application::ActionError::new(
            light_application::ActionErrorKind::Internal,
            "Preload runtime projections did not match their event sequences",
        ));
    }
    let playback_event_sequence_after = event_sequences
        .last()
        .copied()
        .unwrap_or(authority.playback_event_sequence);
    let runtime_changes = committed
        .runtime_projections
        .iter()
        .cloned()
        .zip(event_sequences)
        .map(
            |(projection, event_sequence)| light_application::ProgrammingPreloadRuntimeChange {
                projection,
                event_sequence,
            },
        )
        .collect();
    Ok(light_application::ProgrammingPreloadCommitResult {
        show_id: authority.show_id,
        show_revision: authority.show_revision,
        playback_event_sequence_before: authority.playback_event_sequence,
        playback_event_sequence_after,
        committed_at: committed.committed_at,
        programmer_fade_millis: committed.programmer_fade_millis,
        executed_playback_actions: committed.executed.len(),
        executed: committed.executed_projection,
        runtime_changes,
        warnings: committed.warnings,
    })
}

fn compatibility_context(session: &Session) -> light_application::ActionContext {
    light_application::ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        light_application::ActionSource::UserInterface,
    )
}

fn preload_commit_error(error: String) -> light_application::ActionError {
    light_application::ActionError::new(light_application::ActionErrorKind::Conflict, error)
}
