use super::ProgrammingService;
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, GroupManagementCommit,
    GroupManagementCommitResult, GroupManagementOperation, GroupManagementOutcome,
    GroupManagementPorts, GroupManagementProjection, GroupManagementRequest, GroupManagementResult,
    GroupPropertiesUpdate,
};
use light_core::{SessionId, UserId};
use std::sync::Arc;

struct ManagementIdentity {
    session_id: SessionId,
    user_id: UserId,
    desk_id: uuid::Uuid,
    request_id: String,
}

impl ProgrammingService {
    pub fn handle_group_management(
        &self,
        envelope: ActionEnvelope<GroupManagementRequest>,
        ports: &dyn GroupManagementPorts,
    ) -> Result<GroupManagementResult, ActionError> {
        let identity = management_identity(&envelope)?;
        self.with_user_and_desk_gate(envelope.context.desk_id, identity.user_id, || {
            self.apply_group_management(envelope, ports, identity)
        })
    }

    fn apply_group_management(
        &self,
        envelope: ActionEnvelope<GroupManagementRequest>,
        ports: &dyn GroupManagementPorts,
        identity: ManagementIdentity,
    ) -> Result<GroupManagementResult, ActionError> {
        ports.authorize_group_management(&envelope.context)?;
        self.assert_group_management_owner(identity.session_id, identity.user_id)?;
        validate_request(&envelope.command)?;
        if let Some(result) = self.cached_group_management(&identity, &envelope.command)? {
            return Ok(result);
        }
        let commit = GroupManagementCommit::new(&envelope.command);
        let completion = ports.commit_group_management(&envelope.context, &commit)?;
        let mut result = complete_result(&envelope, &identity.request_id, completion)?;
        result.persistence_warning = ports.persist_group_management(&envelope.context);
        self.remember_group_management(identity, envelope.command, result.clone());
        Ok(result)
    }

    fn assert_group_management_owner(
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
            None => Err(ActionError::new(
                ActionErrorKind::NotFound,
                "the Programmer session does not exist",
            )),
        }
    }

    fn cached_group_management(
        &self,
        identity: &ManagementIdentity,
        request: &GroupManagementRequest,
    ) -> Result<Option<GroupManagementResult>, ActionError> {
        self.group_management_replay.lock().get(
            identity.user_id,
            identity.desk_id,
            identity.session_id,
            &identity.request_id,
            request,
        )
    }

    fn remember_group_management(
        &self,
        identity: ManagementIdentity,
        request: GroupManagementRequest,
        result: GroupManagementResult,
    ) {
        self.group_management_replay.lock().insert(
            identity.user_id,
            identity.desk_id,
            identity.session_id,
            identity.request_id,
            request,
            result,
        );
    }
}

fn management_identity(
    envelope: &ActionEnvelope<GroupManagementRequest>,
) -> Result<ManagementIdentity, ActionError> {
    let session_id = envelope.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Group management requires an operator session",
        )
    })?;
    let user_id = envelope.context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Group management requires an authenticated user",
        )
    })?;
    let request_id = envelope.context.request_id.as_deref().ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Group management requires a request_id",
        )
    })?;
    super::values_validation::validate_request_id(request_id)?;
    Ok(ManagementIdentity {
        session_id,
        user_id,
        desk_id: envelope.context.desk_id,
        request_id: request_id.to_owned(),
    })
}

fn validate_request(request: &GroupManagementRequest) -> Result<(), ActionError> {
    if request.show_id.0.is_nil() {
        return Err(invalid("Group management requires a valid show_id"));
    }
    validate_group_id(&request.group_id, "Group id")?;
    if let Some(expectation) = request.operation.expected_source() {
        validate_group_id(&expectation.source_group_id, "source Group id")?;
    }
    if let GroupManagementOperation::UpdateProperties(update) = &request.operation {
        validate_properties(update)?;
    }
    Ok(())
}

fn validate_group_id(value: &str, label: &str) -> Result<(), ActionError> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(invalid(format!(
            "{label} must contain 1-256 printable bytes without control characters"
        )));
    }
    Ok(())
}

fn validate_properties(update: &GroupPropertiesUpdate) -> Result<(), ActionError> {
    if update.name.trim().is_empty() || update.name.len() > 256 {
        return Err(invalid("Group name must contain 1-256 non-blank bytes"));
    }
    for (label, value) in [("color", &update.color), ("icon", &update.icon)] {
        if let Some(value) = value
            && (value.len() > 64 || value.chars().any(char::is_control))
        {
            return Err(invalid(format!(
                "Group {label} must contain at most 64 printable bytes"
            )));
        }
    }
    Ok(())
}

fn complete_result(
    envelope: &ActionEnvelope<GroupManagementRequest>,
    request_id: &str,
    completion: GroupManagementCommitResult,
) -> Result<GroupManagementResult, ActionError> {
    validate_completion(&envelope.command, &completion)?;
    let projection = Arc::new(completion.projection);
    let outcome = match completion.event_sequence {
        Some(event_sequence) => GroupManagementOutcome::Changed {
            projection,
            show_revision: completion.show_revision,
            event_sequence,
        },
        None => GroupManagementOutcome::NoChange {
            projection,
            show_revision: completion.show_revision,
        },
    };
    Ok(GroupManagementResult {
        context: envelope.context.clone(),
        request_id: request_id.to_owned(),
        replayed: false,
        outcome,
        persistence_warning: None,
    })
}

fn validate_completion(
    request: &GroupManagementRequest,
    completion: &GroupManagementCommitResult,
) -> Result<(), ActionError> {
    validate_projection(request, &completion.projection)?;
    let event_matches = completion.changed == completion.event_sequence.is_some();
    let revision_matches = if completion.changed {
        request
            .expected_object_revision
            .checked_add(1)
            .is_some_and(|revision| revision == completion.projection.object_revision)
    } else {
        request.expected_object_revision == completion.projection.object_revision
    };
    let show_revision_matches = match (request.expected_show_revision, completion.changed) {
        (Some(expected), true) => completion.show_revision > expected,
        (Some(expected), false) => completion.show_revision == expected,
        (None, _) => true,
    };
    let change_required = !request.operation.always_changes() || completion.changed;
    let selection_expected = matches!(
        request.operation,
        GroupManagementOperation::RefreshFrozen { .. }
    );
    if event_matches
        && revision_matches
        && show_revision_matches
        && change_required
        && selection_expected == completion.selection.is_some()
    {
        Ok(())
    } else {
        Err(invalid_completion())
    }
}

fn validate_projection(
    request: &GroupManagementRequest,
    projection: &GroupManagementProjection,
) -> Result<(), ActionError> {
    let body_valid = projection.raw_body.as_object().is_some_and(|body| {
        body.get("id")
            .is_none_or(|id| id.as_str() == Some(request.group_id.as_str()))
    });
    if projection.show_id == request.show_id
        && projection.object_id == request.group_id
        && body_valid
    {
        Ok(())
    } else {
        Err(invalid_completion())
    }
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn invalid_completion() -> ActionError {
    ActionError::new(
        ActionErrorKind::Internal,
        "Group management port returned an inconsistent authoritative completion",
    )
}
