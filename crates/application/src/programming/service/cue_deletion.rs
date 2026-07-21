use super::{ProgrammingService, cue_deletion_replay::CueDeletionScope};
use crate::programming::cue_deletion::ResolvedCueDeletionRequest;
use crate::programming::cue_list_resolution::CueListAddress;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActiveShowService,
    ProgrammingCueDeletionAddress, ProgrammingCueDeletionPorts, ProgrammingCueDeletionRequest,
    ProgrammingCueDeletionResult,
};
use light_core::{SessionId, UserId};

struct CueDeletionIdentity {
    scope: CueDeletionScope,
    request_id: String,
}

impl ProgrammingService {
    pub fn handle_cue_deletion<P: ProgrammingCueDeletionPorts>(
        &self,
        envelope: ActionEnvelope<ProgrammingCueDeletionRequest>,
        active_show: &ActiveShowService,
        ports: &P,
    ) -> Result<ProgrammingCueDeletionResult, ActionError> {
        let identity = deletion_identity(&envelope.context)?;
        self.with_user_and_desk_gate(identity.scope.desk_id, identity.scope.user_id, || {
            self.handle_deletion_locked(envelope, active_show, ports, identity)
        })
    }

    /// Command execution already owns this user's Programming gate and its outer replay cache.
    /// This boundary therefore performs the typed deletion without re-entering either gate.
    pub fn delete_cue_within_interaction<P: ProgrammingCueDeletionPorts>(
        &self,
        context: &ActionContext,
        request: &ProgrammingCueDeletionRequest,
        active_show: &ActiveShowService,
        ports: &P,
    ) -> Result<crate::ProgrammingCueDeletionOutcome, ActionError> {
        let identity = deletion_identity(context)?;
        ports.authorize_cue_deletion_identity(context)?;
        self.assert_deletion_owner(identity.scope.session_id, identity.scope.user_id)?;
        validate_request(request)?;
        let resolved = resolve_request(context, request, ports)?;
        active_show.delete_programming_cue(context, &resolved, ports)
    }

    fn handle_deletion_locked<P: ProgrammingCueDeletionPorts>(
        &self,
        envelope: ActionEnvelope<ProgrammingCueDeletionRequest>,
        active_show: &ActiveShowService,
        ports: &P,
        identity: CueDeletionIdentity,
    ) -> Result<ProgrammingCueDeletionResult, ActionError> {
        ports.authorize_cue_deletion_identity(&envelope.context)?;
        self.assert_deletion_owner(identity.scope.session_id, identity.scope.user_id)?;
        if let Some(result) = self.cue_deletion_replay.lock().get(
            &identity.scope,
            &identity.request_id,
            &envelope.command,
            envelope.context.expected_revision,
        )? {
            return Ok(result);
        }
        validate_request(&envelope.command)?;
        let resolved = resolve_request(&envelope.context, &envelope.command, ports)?;
        let mut outcome =
            active_show.delete_programming_cue(&envelope.context, &resolved, ports)?;
        outcome.persistence_warning = ports.persist_cue_deletion(&envelope.context);
        let result = ProgrammingCueDeletionResult {
            context: envelope.context.clone(),
            request_id: identity.request_id.clone(),
            correlation_id: envelope.context.correlation_id,
            replayed: false,
            outcome,
        };
        self.cue_deletion_replay.lock().insert(
            identity.scope,
            identity.request_id,
            envelope.command,
            envelope.context.expected_revision,
            result.clone(),
        );
        Ok(result)
    }

    fn assert_deletion_owner(
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
}

fn validate_request(request: &ProgrammingCueDeletionRequest) -> Result<(), ActionError> {
    let cue = request.cue_number.value();
    if !cue.is_finite() || cue <= 0.0 {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "Cue number must be finite and greater than zero",
        ));
    }
    match request.address {
        ProgrammingCueDeletionAddress::Pool { playback_number }
            if !(1..=light_playback::MAX_PLAYBACKS).contains(&playback_number) =>
        {
            invalid("playback number must be within 1-1000")
        }
        ProgrammingCueDeletionAddress::CurrentPage {
            expected_page,
            slot,
        } if !(1..=light_playback::MAX_PLAYBACK_PAGES).contains(&expected_page)
            || !(1..=light_playback::MAX_PAGE_SLOTS).contains(&slot) =>
        {
            invalid("page and slot must be within 1-127")
        }
        ProgrammingCueDeletionAddress::PageSlot { page, slot }
            if !(1..=light_playback::MAX_PLAYBACK_PAGES).contains(&page)
                || !(1..=light_playback::MAX_PAGE_SLOTS).contains(&slot) =>
        {
            invalid("page and slot must be within 1-127")
        }
        _ => Ok(()),
    }
}

fn invalid(message: &'static str) -> Result<(), ActionError> {
    Err(ActionError::new(ActionErrorKind::Invalid, message))
}

fn resolve_request<P: ProgrammingCueDeletionPorts>(
    context: &ActionContext,
    request: &ProgrammingCueDeletionRequest,
    ports: &P,
) -> Result<ResolvedCueDeletionRequest, ActionError> {
    let address = match request.address {
        ProgrammingCueDeletionAddress::Pool { playback_number } => {
            CueListAddress::Pool { playback_number }
        }
        ProgrammingCueDeletionAddress::PageSlot { page, slot } => {
            CueListAddress::PageSlot { page, slot }
        }
        ProgrammingCueDeletionAddress::CurrentPage {
            expected_page,
            slot,
        } => {
            let current = ports.current_cue_deletion_page(context, request.show_id)?;
            if current != expected_page {
                return Err(ActionError::new(
                    ActionErrorKind::Conflict,
                    "the desk page changed before the Cue deletion was applied",
                )
                .at_related_revision(u64::from(current)));
            }
            CueListAddress::PageSlot {
                page: expected_page,
                slot,
            }
        }
    };
    Ok(ResolvedCueDeletionRequest {
        show_id: request.show_id,
        address,
        cue_number: request.cue_number,
        expectation: request.expectation.clone(),
    })
}

fn deletion_identity(context: &ActionContext) -> Result<CueDeletionIdentity, ActionError> {
    let request_id = context.request_id.clone().ok_or_else(|| {
        ActionError::new(ActionErrorKind::Invalid, "Cue deletion requires request_id")
    })?;
    if request_id.trim().is_empty()
        || request_id.len() > 128
        || request_id.chars().any(char::is_control)
    {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "request_id must contain 1-128 printable bytes",
        ));
    }
    let user_id = context.user_id.ok_or_else(unauthenticated)?;
    let session_id = context.session_id.ok_or_else(unauthenticated)?;
    Ok(CueDeletionIdentity {
        scope: CueDeletionScope {
            user_id: UserId(user_id),
            desk_id: context.desk_id,
            session_id: SessionId(session_id),
        },
        request_id,
    })
}

fn unauthenticated() -> ActionError {
    ActionError::new(
        ActionErrorKind::Unauthorized,
        "Cue deletion requires an authenticated operator session",
    )
}
