use super::*;
use light_application::{
    ActionError, ActionErrorKind, EventFilter, EventObject, EventReplay,
    ProgrammingPreloadLifecycleAction, ProgrammingPreloadLifecycleRequest,
    ProgrammingPreloadRevisionExpectation,
};
use light_show::ShowStore;

pub(super) struct PreloadCommitAuthority {
    pub(super) show_id: light_core::ShowId,
    pub(super) show_revision: u64,
    pub(super) playback_event_sequence: u64,
}

pub(super) fn validate(
    state: &AppState,
    session: &Session,
    request: &ProgrammingPreloadLifecycleRequest,
) -> Result<PreloadCommitAuthority, ActionError> {
    let ProgrammingPreloadLifecycleAction::Go {
        show_id,
        expected_show_revision,
        expected_playback_event_sequence,
    } = request.action
    else {
        return Err(invalid("the Preload commit port requires a GO action"));
    };
    let active = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| conflict("Preload GO requires an active Show"))?;
    if active.id != show_id {
        return Err(conflict("the requested Show is not active"));
    }
    let store = ShowStore::open(&active.path)
        .map_err(|error| internal(format!("failed to open the active Show: {error}")))?;
    let stored_id = store
        .id()
        .map_err(|error| internal(format!("failed to read the active Show identity: {error}")))?;
    if stored_id != show_id {
        return Err(conflict(
            "the active Show authority changed during Preload GO",
        ));
    }
    let show_revision = store
        .portable_revision()
        .map_err(|error| internal(format!("failed to read the active Show revision: {error}")))?
        .value();
    assert_expected(expected_show_revision, show_revision, "active Show")?;
    let playback_event_sequence = state.application_events.latest_sequence();
    validate_runtime_cursor(
        state,
        session,
        expected_playback_event_sequence,
        playback_event_sequence,
        show_revision,
    )?;
    Ok(PreloadCommitAuthority {
        show_id,
        show_revision,
        playback_event_sequence,
    })
}

fn validate_runtime_cursor(
    state: &AppState,
    session: &Session,
    expected: ProgrammingPreloadRevisionExpectation,
    current: u64,
    show_revision: u64,
) -> Result<(), ActionError> {
    let ProgrammingPreloadRevisionExpectation::Exact(cursor) = expected else {
        return Ok(());
    };
    if cursor > current {
        return Err(revision_conflict(
            "Playback event cursor is ahead of the server",
            current,
            show_revision,
        ));
    }
    let pending = state
        .programmers
        .preload_playback_actions(session.id)
        .ok_or_else(|| conflict("Preload playback queue is unavailable"))?;
    if pending.is_empty() {
        return Ok(());
    }
    let filter = pending
        .iter()
        .fold(EventFilter::default(), |filter, action| {
            let desk_id = action.origin_desk_id.unwrap_or(session.desk.id);
            filter
                .with_object(EventObject::playback(action.playback_number))
                .with_object(EventObject::playback_view(desk_id))
        });
    match state.application_events.replay(cursor, &filter) {
        EventReplay::Events(events) if events.is_empty() => Ok(()),
        EventReplay::Events(_) => Err(revision_conflict(
            "a queued Playback or origin desk changed after the expected cursor",
            current,
            show_revision,
        )),
        EventReplay::Gap(_) => Err(revision_conflict(
            "the expected Playback cursor is outside retained history",
            current,
            show_revision,
        )),
    }
}

fn assert_expected(
    expected: ProgrammingPreloadRevisionExpectation,
    actual: u64,
    authority: &str,
) -> Result<(), ActionError> {
    match expected {
        ProgrammingPreloadRevisionExpectation::Current => Ok(()),
        ProgrammingPreloadRevisionExpectation::Exact(expected) if expected == actual => Ok(()),
        ProgrammingPreloadRevisionExpectation::Exact(expected) => Err(ActionError::new(
            ActionErrorKind::Conflict,
            format!("{authority} revision conflict: expected {expected}, actual {actual}"),
        )
        .at_revision(actual)),
    }
}

/// Cursor conflicts report the current event cursor as the primary revision and the portable
/// Show revision as the related authority participating in the atomic GO precondition.
fn revision_conflict(message: &'static str, cursor: u64, show_revision: u64) -> ActionError {
    conflict(message)
        .at_revision(cursor)
        .at_related_revision(show_revision)
}

fn invalid(message: &'static str) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn conflict(message: &'static str) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message)
}

fn internal(message: String) -> ActionError {
    ActionError::new(ActionErrorKind::Internal, message)
}
