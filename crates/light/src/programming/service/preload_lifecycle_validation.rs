use super::{ProgrammingService, values_validation::validate_request_id};
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ProgrammingPreloadLifecycleAction,
    ProgrammingPreloadLifecycleRequest, ProgrammingPreloadRevisionExpectation,
};
use light_core::{SessionId, UserId};

pub(super) struct LifecycleIdentity {
    pub(super) session_id: SessionId,
    pub(super) user_id: UserId,
    pub(super) request_id: String,
}

impl ProgrammingService {
    pub(super) fn assert_preload_owner(
        &self,
        session: SessionId,
        user: UserId,
    ) -> Result<(), ActionError> {
        match self.programmers.user_id(session) {
            Some(owner) if owner == user => Ok(()),
            Some(_) => Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the authenticated user",
            )),
            None => Err(preload_unavailable()),
        }
    }

    pub(super) fn assert_lifecycle_revisions(
        &self,
        session_id: SessionId,
        user_id: UserId,
        request: &ProgrammingPreloadLifecycleRequest,
    ) -> Result<(), ActionError> {
        let capture = self.programmers.capture_mode_revision(user_id);
        let values = self.programmers.preload_values_revision(user_id);
        let queue = self.programmers.preload_playback_queue_revision(user_id);
        let selection = self
            .programmers
            .selection(session_id)
            .ok_or_else(preload_unavailable)?
            .revision;
        match request.action {
            ProgrammingPreloadLifecycleAction::Enter => {
                assert_expected(
                    request.expected_capture_mode_revision,
                    capture,
                    "Programmer capture-mode",
                )?;
                assert_expected(
                    request.expected_selection_revision,
                    selection,
                    "Programmer selection",
                )
            }
            ProgrammingPreloadLifecycleAction::ClearPending => {
                assert_expected(request.expected_values_revision, values, "Preload values")?;
                assert_expected(
                    request.expected_queue_revision,
                    queue,
                    "Preload playback queue",
                )
            }
            ProgrammingPreloadLifecycleAction::Go { .. }
            | ProgrammingPreloadLifecycleAction::Release => {
                assert_expected(
                    request.expected_capture_mode_revision,
                    capture,
                    "Programmer capture-mode",
                )?;
                assert_expected(request.expected_values_revision, values, "Preload values")?;
                assert_expected(
                    request.expected_queue_revision,
                    queue,
                    "Preload playback queue",
                )?;
                assert_expected(
                    request.expected_selection_revision,
                    selection,
                    "Programmer selection",
                )
            }
        }
    }

    pub(super) fn assert_go_is_armed(
        &self,
        session: SessionId,
        action: &ProgrammingPreloadLifecycleAction,
    ) -> Result<(), ActionError> {
        if !matches!(action, ProgrammingPreloadLifecycleAction::Go { .. }) {
            return Ok(());
        }
        let mode = self
            .programmers
            .capture_mode(session)
            .ok_or_else(preload_unavailable)?;
        if mode.blind {
            Ok(())
        } else {
            Err(ActionError::new(
                ActionErrorKind::Conflict,
                "Preload GO requires an armed Preload capture",
            ))
        }
    }
}

pub(super) fn lifecycle_identity(
    action: &ActionEnvelope<ProgrammingPreloadLifecycleRequest>,
) -> Result<LifecycleIdentity, ActionError> {
    let session_id = action
        .context
        .session_id
        .map(SessionId)
        .ok_or_else(|| unauthorized("Preload lifecycle actions require an operator session"))?;
    let user_id =
        action.context.user_id.map(UserId).ok_or_else(|| {
            unauthorized("Preload lifecycle actions require an authenticated user")
        })?;
    let request_id = action.context.request_id.as_deref().ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Preload lifecycle actions require a request_id",
        )
    })?;
    validate_request_id(request_id)?;
    Ok(LifecycleIdentity {
        session_id,
        user_id,
        request_id: request_id.to_owned(),
    })
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

fn unauthorized(message: &'static str) -> ActionError {
    ActionError::new(ActionErrorKind::Unauthorized, message)
}

pub(super) fn preload_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "Preload lifecycle authority is unavailable",
    )
}
