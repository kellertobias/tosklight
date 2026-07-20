use super::ProgrammingService;
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ProgrammingGroupCommit,
    ProgrammingGroupCommitResult, ProgrammingGroupProjection, ProgrammingGroupRecordOutcome,
    ProgrammingGroupRecordRequest, ProgrammingGroupRecordResult, ProgrammingGroupRecordingPorts,
    ProgrammingGroupRevisionExpectation,
};
use light_core::{SessionId, UserId};
use std::sync::Arc;

struct RecordingIdentity {
    session_id: SessionId,
    user_id: UserId,
    desk_id: uuid::Uuid,
    request_id: String,
}

impl ProgrammingService {
    pub fn handle_group_recording(
        &self,
        envelope: ActionEnvelope<ProgrammingGroupRecordRequest>,
        ports: &dyn ProgrammingGroupRecordingPorts,
    ) -> Result<ProgrammingGroupRecordResult, ActionError> {
        let identity = recording_identity(&envelope)?;
        self.with_user_and_desk_gate(envelope.context.desk_id, identity.user_id, || {
            self.apply_group_recording(envelope, ports, identity, false)
        })
    }

    /// Records a Group while the caller already owns this user's user-then-desk interaction gates.
    pub fn record_group_within_interaction(
        &self,
        envelope: ActionEnvelope<ProgrammingGroupRecordRequest>,
        ports: &dyn ProgrammingGroupRecordingPorts,
    ) -> Result<ProgrammingGroupRecordResult, ActionError> {
        let identity = recording_identity(&envelope)?;
        self.apply_group_recording(envelope, ports, identity, true)
    }

    fn apply_group_recording(
        &self,
        envelope: ActionEnvelope<ProgrammingGroupRecordRequest>,
        ports: &dyn ProgrammingGroupRecordingPorts,
        identity: RecordingIdentity,
        within_interaction: bool,
    ) -> Result<ProgrammingGroupRecordResult, ActionError> {
        ports.authorize_group_recording(&envelope.context)?;
        self.assert_group_owner(identity.session_id, identity.user_id)?;
        validate_request(&envelope.command)?;
        if let Some(result) = self.cached_group_recording(&identity, &envelope.command)? {
            return Ok(result);
        }
        let capture = self
            .programmers
            .capture_group_recording_selection(identity.session_id)
            .ok_or_else(missing_programmer)?;
        let commit = ProgrammingGroupCommit::new(
            &envelope.command,
            capture,
            identity.session_id,
            within_interaction,
        );
        let applied = commit.applied();
        let completion = ports.commit_group(&envelope.context, &commit)?;
        let result = complete_result(
            &envelope,
            &identity.request_id,
            applied,
            &commit,
            completion,
        )?;
        self.finish_successful_group_gesture(&envelope, &commit);
        self.remember_group_recording(identity, envelope.command, result.clone());
        Ok(result)
    }

    fn assert_group_owner(
        &self,
        session_id: SessionId,
        user_id: UserId,
    ) -> Result<(), ActionError> {
        match self.programmers.user_id(session_id) {
            Some(owner) if owner == user_id => Ok(()),
            Some(_) => Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the authenticated user",
            )),
            None => Err(missing_programmer()),
        }
    }

    fn finish_successful_group_gesture(
        &self,
        envelope: &ActionEnvelope<ProgrammingGroupRecordRequest>,
        commit: &ProgrammingGroupCommit,
    ) {
        if !commit.finish_actor_selection_gesture(&self.programmers) || commit.within_interaction()
        {
            return;
        }
        let selection = self
            .programmers
            .selection(commit.actor_session_id())
            .unwrap_or_default();
        let change = crate::ProgrammingInteractionChange::from_components(
            envelope.context.desk_id,
            None,
            Some(selection),
        );
        self.publish_interaction(&envelope.context, change);
    }

    fn cached_group_recording(
        &self,
        identity: &RecordingIdentity,
        request: &ProgrammingGroupRecordRequest,
    ) -> Result<Option<ProgrammingGroupRecordResult>, ActionError> {
        self.group_recording_replay.lock().get(
            identity.user_id,
            identity.desk_id,
            identity.session_id,
            &identity.request_id,
            request,
        )
    }

    fn remember_group_recording(
        &self,
        identity: RecordingIdentity,
        request: ProgrammingGroupRecordRequest,
        result: ProgrammingGroupRecordResult,
    ) {
        self.group_recording_replay.lock().insert(
            identity.user_id,
            identity.desk_id,
            identity.session_id,
            identity.request_id,
            request,
            result,
        );
    }
}

fn recording_identity(
    envelope: &ActionEnvelope<ProgrammingGroupRecordRequest>,
) -> Result<RecordingIdentity, ActionError> {
    let session_id = envelope.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Group recording requires an operator session",
        )
    })?;
    let user_id = envelope.context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Group recording requires an authenticated user",
        )
    })?;
    let request_id = envelope.context.request_id.as_deref().ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Group recording requires a request_id",
        )
    })?;
    super::values_validation::validate_request_id(request_id)?;
    Ok(RecordingIdentity {
        session_id,
        user_id,
        desk_id: envelope.context.desk_id,
        request_id: request_id.to_owned(),
    })
}

fn validate_request(request: &ProgrammingGroupRecordRequest) -> Result<(), ActionError> {
    if request.show_id.0.is_nil() {
        return Err(invalid("Group recording requires a valid show_id"));
    }
    if request.group_id.trim().is_empty()
        || request.group_id.len() > 256
        || request.group_id.chars().any(char::is_control)
    {
        return Err(invalid(
            "Group id must contain 1-256 printable bytes without control characters",
        ));
    }
    Ok(())
}

fn complete_result(
    envelope: &ActionEnvelope<ProgrammingGroupRecordRequest>,
    request_id: &str,
    applied: usize,
    commit: &ProgrammingGroupCommit,
    completion: ProgrammingGroupCommitResult,
) -> Result<ProgrammingGroupRecordResult, ActionError> {
    validate_completion(&envelope.command, commit, &completion)?;
    let projection = Arc::new(completion.projection);
    let outcome = match completion.event_sequence {
        Some(event_sequence) => ProgrammingGroupRecordOutcome::Changed {
            projection,
            show_revision: completion.show_revision,
            event_sequence,
        },
        None => ProgrammingGroupRecordOutcome::NoChange {
            projection,
            show_revision: completion.show_revision,
        },
    };
    Ok(ProgrammingGroupRecordResult {
        context: envelope.context.clone(),
        request_id: request_id.to_owned(),
        replayed: false,
        applied,
        outcome,
    })
}

fn validate_completion(
    request: &ProgrammingGroupRecordRequest,
    commit: &ProgrammingGroupCommit,
    completion: &ProgrammingGroupCommitResult,
) -> Result<(), ActionError> {
    validate_projection(request, commit, &completion.projection)?;
    let event_matches = completion.changed == completion.event_sequence.is_some();
    let revision_matches = revision_matches(request.expected_object_revision, completion);
    let show_revision_matches = match (request.expected_show_revision, completion.changed) {
        (Some(expected), true) => completion.show_revision > expected,
        (Some(expected), false) => completion.show_revision == expected,
        (None, _) => true,
    };
    let deletion_no_change = completion.projection.deleted && !completion.changed;
    if event_matches && revision_matches && show_revision_matches && !deletion_no_change {
        Ok(())
    } else {
        Err(invalid_completion())
    }
}

fn revision_matches(
    expected: ProgrammingGroupRevisionExpectation,
    completion: &ProgrammingGroupCommitResult,
) -> bool {
    match (expected, completion.changed) {
        (ProgrammingGroupRevisionExpectation::Exact(expected), true) => expected
            .checked_add(1)
            .is_some_and(|revision| revision == completion.projection.object_revision),
        (ProgrammingGroupRevisionExpectation::Exact(expected), false) => {
            expected > 0 && expected == completion.projection.object_revision
        }
        (ProgrammingGroupRevisionExpectation::Current, true) => {
            completion.projection.object_revision > 0
        }
        (ProgrammingGroupRevisionExpectation::Current, false) => {
            completion.projection.object_revision > 0
        }
    }
}

fn validate_projection(
    request: &ProgrammingGroupRecordRequest,
    commit: &ProgrammingGroupCommit,
    projection: &ProgrammingGroupProjection,
) -> Result<(), ActionError> {
    let body_valid = match projection.raw_body.as_deref() {
        Some(body) => {
            !projection.deleted
                && body.as_object().is_some_and(|body| {
                    body.get("id")
                        .is_none_or(|id| id.as_str() == Some(request.group_id.as_str()))
                })
        }
        None => projection.deleted,
    };
    if projection.show_id == request.show_id
        && projection.object_id == request.group_id
        && body_valid
        && projection.deleted == commit.deletes_target()
    {
        Ok(())
    } else {
        Err(invalid_completion())
    }
}

fn missing_programmer() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "the Programmer session does not exist",
    )
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn invalid_completion() -> ActionError {
    ActionError::new(
        ActionErrorKind::Internal,
        "Group recording port returned an inconsistent authoritative completion",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProgrammingGroupRecordOperation;
    use light_core::ShowId;
    use light_programmer::ProgrammerRegistry;

    #[test]
    fn legacy_body_may_omit_id_but_a_present_identity_must_match() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        registry.start(session, UserId::new());
        let request = ProgrammingGroupRecordRequest {
            show_id: ShowId::new(),
            group_id: "opaque".into(),
            operation: ProgrammingGroupRecordOperation::Overwrite,
            expected_object_revision: ProgrammingGroupRevisionExpectation::Current,
            expected_show_revision: None,
        };
        let commit = ProgrammingGroupCommit::new(
            &request,
            registry.capture_group_recording_selection(session).unwrap(),
            session,
            false,
        );
        let projection = |body| ProgrammingGroupProjection {
            show_id: request.show_id,
            object_id: request.group_id.clone(),
            object_revision: 1,
            raw_body: Some(Arc::new(body)),
            deleted: false,
        };

        assert!(
            validate_projection(
                &request,
                &commit,
                &projection(serde_json::json!({"name":"Legacy","fixtures":[]})),
            )
            .is_ok()
        );
        for invalid_id in [serde_json::json!("other"), serde_json::json!(7)] {
            assert!(
                validate_projection(
                    &request,
                    &commit,
                    &projection(serde_json::json!({
                        "id": invalid_id,
                        "name": "Invalid",
                        "fixtures": []
                    })),
                )
                .is_err()
            );
        }
    }
}
