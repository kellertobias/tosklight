use super::{ProgrammingService, support::Snapshot};
use crate::programming::cue_transfer::{
    CueTransferAuthority, ProgrammingCueTransferChoiceRequest, ProgrammingCueTransferOutcome,
    ProgrammingCueTransferPorts, ProgrammingCueTransferRequest, ProgrammingCueTransferResult,
};
use crate::{ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActiveShowService};
use light_core::{SessionId, UserId};
use light_programmer::{
    CueMoveCopyChoice, CueTransferOperation, ProgrammingChoiceOption, ProgrammingChoiceOptionId,
};

use super::cue_transfer_replay::CueTransferScope;

struct TransferIdentity {
    scope: CueTransferScope,
    request_id: String,
}

impl ProgrammingService {
    /// Resolves and retains exact source/destination authority while the caller owns the
    /// Programming interaction gate that produced the pending choice.
    pub fn prepare_cue_transfer_choice_within_interaction<P: ProgrammingCueTransferPorts>(
        &self,
        context: &ActionContext,
        request: ProgrammingCueTransferChoiceRequest,
        active_show: &ActiveShowService,
        ports: &P,
    ) -> Result<CueMoveCopyChoice, ActionError> {
        let scope = transfer_scope(context)?;
        ports.authorize_cue_transfer(context)?;
        self.assert_transfer_owner(scope.session_id, scope.user_id)?;
        let authority =
            active_show.prepare_programming_cue_transfer_choice(context, &request, ports)?;
        let choice = transfer_choice(&authority, &request);
        self.cue_transfer_choices.lock().insert(scope, authority);
        Ok(choice)
    }

    pub fn handle_cue_transfer<P: ProgrammingCueTransferPorts>(
        &self,
        envelope: ActionEnvelope<ProgrammingCueTransferRequest>,
        active_show: &ActiveShowService,
        ports: &P,
    ) -> Result<ProgrammingCueTransferResult, ActionError> {
        let identity = transfer_identity(&envelope.context)?;
        self.with_user_and_desk_gate(identity.scope.desk_id, identity.scope.user_id, || {
            self.handle_transfer_locked(envelope, active_show, ports, identity)
        })
    }

    /// Compatibility command execution already owns this user's Programming gate. Its outer
    /// action clears and publishes the command line exactly once after this typed commit returns.
    pub fn cue_transfer_within_interaction<P: ProgrammingCueTransferPorts>(
        &self,
        context: &ActionContext,
        request: &ProgrammingCueTransferRequest,
        active_show: &ActiveShowService,
        ports: &P,
    ) -> Result<ProgrammingCueTransferOutcome, ActionError> {
        let scope = transfer_scope(context)?;
        ports.authorize_cue_transfer(context)?;
        self.assert_transfer_owner(scope.session_id, scope.user_id)?;
        let authority = self.authority_for(&scope, request)?;
        active_show.commit_programming_cue_transfer(context, &authority, request.mode, ports)
    }

    /// Legacy command adapters may execute an explicit PLAIN/STATUS form without first opening
    /// the modal. Resolution and commit still occur inside one ActiveShow transaction.
    pub fn current_cue_transfer_within_interaction<P: ProgrammingCueTransferPorts>(
        &self,
        context: &ActionContext,
        request: &ProgrammingCueTransferChoiceRequest,
        mode: crate::ProgrammingCueTransferMode,
        active_show: &ActiveShowService,
        ports: &P,
    ) -> Result<ProgrammingCueTransferOutcome, ActionError> {
        let scope = transfer_scope(context)?;
        ports.authorize_cue_transfer(context)?;
        self.assert_transfer_owner(scope.session_id, scope.user_id)?;
        active_show.commit_current_programming_cue_transfer(context, request, mode, ports)
    }

    fn handle_transfer_locked<P: ProgrammingCueTransferPorts>(
        &self,
        envelope: ActionEnvelope<ProgrammingCueTransferRequest>,
        active_show: &ActiveShowService,
        ports: &P,
        identity: TransferIdentity,
    ) -> Result<ProgrammingCueTransferResult, ActionError> {
        ports.authorize_cue_transfer(&envelope.context)?;
        self.assert_transfer_owner(identity.scope.session_id, identity.scope.user_id)?;
        if let Some(result) = self.cue_transfer_replay.lock().get(
            &identity.scope,
            &identity.request_id,
            &envelope.command,
        )? {
            return Ok(result);
        }
        let authority = self.authority_for(&identity.scope, &envelope.command)?;
        let before = Snapshot::read(
            &self.programmers,
            identity.scope.desk_id,
            identity.scope.session_id,
            identity.scope.user_id,
        )?;
        let mut outcome = active_show.commit_programming_cue_transfer(
            &envelope.context,
            &authority,
            envelope.command.mode,
            ports,
        )?;
        self.clear_transfer_choice(identity.scope.session_id)?;
        outcome.persistence_warning = ports.persist_cue_transfer(&envelope.context);
        let after = Snapshot::read(
            &self.programmers,
            identity.scope.desk_id,
            identity.scope.session_id,
            identity.scope.user_id,
        )?;
        outcome.command_line = after.command_line.clone();
        outcome.interaction_event_sequence = self.publish_interaction(
            &envelope.context,
            super::interaction_change(
                &self.programmers,
                identity.scope.desk_id,
                identity.scope.session_id,
                &before,
                &after,
            ),
        );
        let result = ProgrammingCueTransferResult {
            context: envelope.context.clone(),
            request_id: identity.request_id.clone(),
            choice_id: envelope.command.choice_id,
            correlation_id: envelope.context.correlation_id,
            replayed: false,
            outcome,
        };
        self.cue_transfer_replay.lock().insert(
            identity.scope,
            identity.request_id,
            envelope.command,
            result.clone(),
        );
        Ok(result)
    }

    fn authority_for(
        &self,
        scope: &CueTransferScope,
        request: &ProgrammingCueTransferRequest,
    ) -> Result<CueTransferAuthority, ActionError> {
        let command = self
            .programmers
            .command_line_state(scope.session_id)
            .ok_or_else(missing_programmer)?;
        if command.revision != request.expected_command_line_revision {
            return Err(conflict_related_revision(
                "the command line changed after the Cue transfer choice was shown",
                command.revision,
            ));
        }
        let pending = command.pending_choice.ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Conflict,
                "Cue transfer choice is no longer pending",
            )
        })?;
        if pending.choice_id != request.choice_id || pending.show_id != request.show_id.0 {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "Cue transfer choice authority changed",
            ));
        }
        self.cue_transfer_choices
            .lock()
            .get(scope, request.choice_id)
    }

    fn clear_transfer_choice(&self, session_id: SessionId) -> Result<(), ActionError> {
        self.programmers
            .complete_command_execution(session_id, Some(""), None)
            .ok_or_else(missing_programmer)?;
        Ok(())
    }

    fn assert_transfer_owner(
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
}

fn transfer_choice(
    authority: &CueTransferAuthority,
    request: &ProgrammingCueTransferChoiceRequest,
) -> CueMoveCopyChoice {
    let title = match authority.operation {
        CueTransferOperation::Copy => "Copy",
        CueTransferOperation::Move => "Move",
    };
    CueMoveCopyChoice {
        choice_id: authority.choice_id,
        show_id: authority.show_id.0,
        show_revision: authority.show_revision.value(),
        operation: authority.operation,
        command: request.command.clone(),
        options: vec![
            ProgrammingChoiceOption {
                id: ProgrammingChoiceOptionId::Plain,
                label: format!("Plain {title}"),
                command: request.plain_command.clone(),
            },
            ProgrammingChoiceOption {
                id: ProgrammingChoiceOptionId::Status,
                label: format!("Status {title}"),
                command: request.status_command.clone(),
            },
        ],
        cancel_label: "Cancel".into(),
    }
}

fn transfer_identity(context: &ActionContext) -> Result<TransferIdentity, ActionError> {
    let request_id = context.request_id.clone().ok_or_else(|| {
        ActionError::new(ActionErrorKind::Invalid, "Cue transfer requires request_id")
    })?;
    if request_id.is_empty() || request_id.len() > 128 {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "request_id must contain 1-128 bytes",
        ));
    }
    Ok(TransferIdentity {
        scope: transfer_scope(context)?,
        request_id,
    })
}

fn transfer_scope(context: &ActionContext) -> Result<CueTransferScope, ActionError> {
    Ok(CueTransferScope {
        user_id: UserId(context.user_id.ok_or_else(unauthenticated)?),
        desk_id: context.desk_id,
        session_id: SessionId(context.session_id.ok_or_else(unauthenticated)?),
    })
}

fn unauthenticated() -> ActionError {
    ActionError::new(
        ActionErrorKind::Unauthorized,
        "Cue transfer requires an authenticated operator session",
    )
}

fn missing_programmer() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "the Programmer session does not exist",
    )
}

fn conflict_related_revision(message: impl Into<String>, revision: u64) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message).at_related_revision(revision)
}
