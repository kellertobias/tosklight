use super::ProgrammingService;
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ProgrammingPorts, ProgrammingPriorityActionState,
    ProgrammingPriorityChange, ProgrammingPriorityProjection, ProgrammingPriorityRequest,
    ProgrammingPriorityResult, ProgrammingPriorityRevisionExpectation, ProgrammingPrioritySnapshot,
};
use light_core::{SessionId, UserId};

impl ProgrammingService {
    pub fn priority_snapshot(
        &self,
        context: &crate::ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingPrioritySnapshot, ActionError> {
        let (session, user_id) = priority_identity(context)?;
        self.with_user_and_desk_gate(context.desk_id, user_id, || {
            ports.authorize(context)?;
            self.assert_priority_owner(session, user_id)?;
            let event_sequence = self.events.latest_sequence();
            let revision = self.programmers.priority_revision(user_id);
            Ok(ProgrammingPrioritySnapshot {
                event_sequence,
                projection: self.priority_projection(session, user_id, revision)?,
            })
        })
    }

    pub fn handle_priority(
        &self,
        action: ActionEnvelope<ProgrammingPriorityRequest>,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingPriorityResult, ActionError> {
        let (session, user_id, request_id) = priority_context(&action)?;
        self.with_user_and_desk_gate(action.context.desk_id, user_id, || {
            ports.authorize(&action.context)?;
            self.assert_priority_owner(session, user_id)?;
            if let Some(cached) = self.priority_replay.lock().get(
                user_id,
                action.context.desk_id,
                session,
                &request_id,
                &action.command,
            )? {
                return Ok(cached);
            }
            let revision_before =
                self.assert_priority_revision(user_id, action.command.expected_revision)?;
            let changed = self
                .programmers
                .update_priority(session, action.command.priority)
                .ok_or_else(priority_unavailable)?;
            let revision = if changed {
                self.programmers.advance_priority_revision(user_id)
            } else {
                revision_before
            };
            let projection = self.priority_projection(session, user_id, revision)?;
            let warning = changed
                .then(|| ports.persist(&action.context, "programmer.priority"))
                .flatten();
            let outcome = if changed {
                let event_sequence = self.publish_priority(
                    &action.context,
                    ProgrammingPriorityChange::Upsert {
                        projection: projection.clone(),
                    },
                );
                ProgrammingPriorityActionState::Changed { event_sequence }
            } else {
                ProgrammingPriorityActionState::NoChange
            };
            let result = ProgrammingPriorityResult {
                context: action.context.clone(),
                request_id: request_id.clone(),
                projection,
                outcome,
                replayed: false,
                warning,
            };
            self.priority_replay.lock().insert(
                user_id,
                action.context.desk_id,
                session,
                request_id,
                action.command,
                result.clone(),
            );
            Ok(result)
        })
    }

    fn assert_priority_owner(
        &self,
        session: SessionId,
        user_id: UserId,
    ) -> Result<(), ActionError> {
        match self.programmers.user_id(session) {
            Some(owner) if owner == user_id => Ok(()),
            Some(_) => Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the authenticated user",
            )),
            None => Err(priority_unavailable()),
        }
    }

    fn assert_priority_revision(
        &self,
        user_id: UserId,
        expected: ProgrammingPriorityRevisionExpectation,
    ) -> Result<u64, ActionError> {
        let actual = self.programmers.priority_revision(user_id);
        match expected {
            ProgrammingPriorityRevisionExpectation::Current => Ok(actual),
            ProgrammingPriorityRevisionExpectation::Exact(expected) if actual == expected => {
                Ok(actual)
            }
            ProgrammingPriorityRevisionExpectation::Exact(expected) => Err(ActionError::new(
                ActionErrorKind::Conflict,
                format!(
                    "Programmer priority revision conflict: expected {expected}, actual {actual}"
                ),
            )
            .at_revision(actual)),
        }
    }

    pub(in crate::programming) fn priority_projection(
        &self,
        session: SessionId,
        user_id: UserId,
        revision: u64,
    ) -> Result<ProgrammingPriorityProjection, ActionError> {
        let (owner, priority, changed_at) = self
            .programmers
            .priority_state(session)
            .ok_or_else(priority_unavailable)?;
        if owner != user_id {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the authenticated user",
            ));
        }
        Ok(ProgrammingPriorityProjection {
            user_id,
            revision,
            priority,
            changed_at,
        })
    }
}

fn priority_context(
    action: &ActionEnvelope<ProgrammingPriorityRequest>,
) -> Result<(SessionId, UserId, String), ActionError> {
    let session = action.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer priority actions require an operator session",
        )
    })?;
    let user_id = action.context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer priority actions require an authenticated user",
        )
    })?;
    let request_id = action.context.request_id.as_deref().ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Programmer priority actions require a request_id",
        )
    })?;
    super::values_validation::validate_request_id(request_id)?;
    Ok((session, user_id, request_id.to_owned()))
}

fn priority_identity(context: &crate::ActionContext) -> Result<(SessionId, UserId), ActionError> {
    let session = context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer priority snapshots require an operator session",
        )
    })?;
    let user_id = context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer priority snapshots require an authenticated user",
        )
    })?;
    Ok((session, user_id))
}

fn priority_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "Programmer priority is unavailable",
    )
}
