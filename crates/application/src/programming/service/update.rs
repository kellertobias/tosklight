use super::ProgrammingService;
use crate::programming::update::{
    ProgrammingUpdateCommand, ProgrammingUpdateMenuInput, ProgrammingUpdateOutcome,
    ProgrammingUpdatePorts, ProgrammingUpdatePreviewRequest, ProgrammingUpdatePreviewResult,
    ProgrammingUpdateResult, ProgrammingUpdateTargetRequest, ProgrammingUpdateTargetsRequest,
    ProgrammingUpdateTargetsResult,
};
use crate::{ActionEnvelope, ActionError, ActionErrorKind, ActiveShowService};
use light_core::{SessionId, UserId};
use light_programmer::{
    ProgrammerUpdateCaptureError, ProgrammerUpdateContent, ProgrammerUpdateMenuCapture,
    ProgrammerUpdateSelectionCapture, ProgrammerUpdateValuesCapture,
};
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct UpdateIdentity {
    session_id: SessionId,
    user_id: UserId,
    desk_id: uuid::Uuid,
    request_id: String,
}

enum UpdateCapture {
    Values(ProgrammerUpdateValuesCapture),
    Selection(ProgrammerUpdateSelectionCapture),
}

impl UpdateCapture {
    fn user_id(&self) -> UserId {
        match self {
            Self::Values(capture) => capture.user_id,
            Self::Selection(capture) => capture.user_id,
        }
    }

    fn content(&self) -> ProgrammerUpdateContent {
        match self {
            Self::Values(capture) => capture.content(),
            Self::Selection(capture) => ProgrammerUpdateContent {
                selected_fixtures: capture.fixtures.clone(),
                ..ProgrammerUpdateContent::default()
            },
        }
    }

    fn fingerprint(&self) -> Result<String, ActionError> {
        match self {
            Self::Values(capture) => fingerprint(&capture.values),
            Self::Selection(capture) => fingerprint(&capture.fixtures),
        }
    }
}

impl ProgrammingService {
    pub fn update_targets<P: ProgrammingUpdatePorts>(
        &self,
        envelope: ActionEnvelope<ProgrammingUpdateTargetsRequest>,
        active_show: &ActiveShowService,
        ports: &P,
    ) -> Result<ProgrammingUpdateTargetsResult, ActionError> {
        let identity = update_identity(&envelope.context)?;
        self.with_user_and_desk_gate(identity.desk_id, identity.user_id, || {
            ports.authorize_programming_update(&envelope.context)?;
            self.assert_update_owner(&identity)?;
            let capture = self
                .programmers
                .capture_update_menu(identity.session_id)
                .map_err(capture_error)?;
            if capture.user_id != identity.user_id {
                return Err(foreign_capture());
            }
            let values_fingerprint = fingerprint(&capture.values)?;
            let selection_fingerprint = fingerprint(&capture.selected_fixtures)?;
            let ProgrammerUpdateMenuCapture {
                user_id,
                values_revision,
                selection_revision: _,
                values,
                selected_fixtures,
                active_preset_id,
                referenced_group_ids,
            } = capture;
            let input = ProgrammingUpdateMenuInput {
                values_fingerprint,
                selection_fingerprint,
                values: ProgrammerUpdateValuesCapture {
                    user_id,
                    revision: values_revision,
                    values,
                }
                .into_content(),
                selection: ProgrammerUpdateContent {
                    selected_fixtures,
                    ..ProgrammerUpdateContent::default()
                },
                active_preset_id,
                referenced_group_ids,
            };
            active_show.programming_update_targets(
                &envelope.context,
                identity.request_id,
                &envelope.command,
                &input,
                ports,
            )
        })
    }

    pub fn preview_update<P: ProgrammingUpdatePorts>(
        &self,
        envelope: ActionEnvelope<ProgrammingUpdatePreviewRequest>,
        active_show: &ActiveShowService,
        ports: &P,
    ) -> Result<ProgrammingUpdatePreviewResult, ActionError> {
        let identity = update_identity(&envelope.context)?;
        self.with_user_and_desk_gate(identity.desk_id, identity.user_id, || {
            ports.authorize_programming_update(&envelope.context)?;
            self.assert_update_owner(&identity)?;
            let capture = self.capture_update(identity.session_id, &envelope.command.target)?;
            ensure_capture_owner(&capture, identity.user_id)?;
            let programmer_revision = capture.fingerprint()?;
            let content = capture.content();
            active_show.preview_programming_update(
                &envelope.context,
                identity.request_id,
                &envelope.command,
                &content,
                programmer_revision,
                ports,
            )
        })
    }

    pub fn handle_update<P: ProgrammingUpdatePorts>(
        &self,
        envelope: ActionEnvelope<ProgrammingUpdateCommand>,
        active_show: &ActiveShowService,
        ports: &P,
    ) -> Result<ProgrammingUpdateResult, ActionError> {
        let identity = update_identity(&envelope.context)?;
        self.with_user_and_desk_gate(identity.desk_id, identity.user_id, || {
            self.handle_update_locked(envelope, active_show, ports, identity)
        })
    }

    pub fn update_within_interaction<P: ProgrammingUpdatePorts>(
        &self,
        envelope: ActionEnvelope<ProgrammingUpdateCommand>,
        active_show: &ActiveShowService,
        ports: &P,
    ) -> Result<ProgrammingUpdateResult, ActionError> {
        let identity = update_identity(&envelope.context)?;
        if let Some(result) = self.prepare_update_attempt(&envelope, ports, &identity)? {
            return Ok(result);
        }
        self.apply_new_update(envelope, active_show, ports, identity)
    }

    fn apply_new_update<P: ProgrammingUpdatePorts>(
        &self,
        envelope: ActionEnvelope<ProgrammingUpdateCommand>,
        active_show: &ActiveShowService,
        ports: &P,
        identity: UpdateIdentity,
    ) -> Result<ProgrammingUpdateResult, ActionError> {
        let content = self.capture_expected_update(&identity, &envelope.command)?;
        let outcome = active_show.commit_programming_update(
            &envelope.context,
            &envelope.command,
            &content,
            ports,
        )?;
        Ok(self.retain_update_result(envelope, identity, outcome))
    }

    fn prepare_update_attempt<P: ProgrammingUpdatePorts>(
        &self,
        envelope: &ActionEnvelope<ProgrammingUpdateCommand>,
        ports: &P,
        identity: &UpdateIdentity,
    ) -> Result<Option<ProgrammingUpdateResult>, ActionError> {
        ports.authorize_programming_update(&envelope.context)?;
        self.assert_update_owner(identity)?;
        self.replayed_update(identity, &envelope.command)
    }

    fn replayed_update(
        &self,
        identity: &UpdateIdentity,
        command: &ProgrammingUpdateCommand,
    ) -> Result<Option<ProgrammingUpdateResult>, ActionError> {
        self.update_replay.lock().get(
            identity.user_id,
            identity.desk_id,
            identity.session_id,
            &identity.request_id,
            command,
        )
    }

    fn capture_expected_update(
        &self,
        identity: &UpdateIdentity,
        command: &ProgrammingUpdateCommand,
    ) -> Result<ProgrammerUpdateContent, ActionError> {
        let capture = self.capture_update(identity.session_id, &command.target)?;
        ensure_capture_owner(&capture, identity.user_id)?;
        let fingerprint = capture.fingerprint()?;
        if command
            .expected_programmer_revision
            .as_ref()
            .is_some_and(|expected| expected != &fingerprint)
        {
            return Err(conflict(
                "programmer content changed after the Update preview",
            ));
        }
        Ok(capture.content())
    }

    fn retain_update_result(
        &self,
        envelope: ActionEnvelope<ProgrammingUpdateCommand>,
        identity: UpdateIdentity,
        outcome: ProgrammingUpdateOutcome,
    ) -> ProgrammingUpdateResult {
        let result = ProgrammingUpdateResult {
            context: envelope.context.clone(),
            request_id: identity.request_id.clone(),
            correlation_id: envelope.context.correlation_id,
            replayed: false,
            outcome,
        };
        self.update_replay.lock().insert(
            identity.user_id,
            identity.desk_id,
            identity.session_id,
            identity.request_id,
            envelope.command,
            result.clone(),
        );
        result
    }

    fn handle_update_locked<P: ProgrammingUpdatePorts>(
        &self,
        envelope: ActionEnvelope<ProgrammingUpdateCommand>,
        active_show: &ActiveShowService,
        ports: &P,
        identity: UpdateIdentity,
    ) -> Result<ProgrammingUpdateResult, ActionError> {
        if let Some(result) = self.prepare_update_attempt(&envelope, ports, &identity)? {
            return Ok(result);
        }
        let before = self.update_interaction_snapshot(&identity)?;
        let result = self.apply_new_update(envelope.clone(), active_show, ports, identity.clone());
        let after = self.update_interaction_snapshot(&identity)?;
        self.publish_update_interaction(&envelope.context, &identity, &before, &after);
        result
    }

    fn publish_update_interaction(
        &self,
        context: &crate::ActionContext,
        identity: &UpdateIdentity,
        before: &super::Snapshot,
        after: &super::Snapshot,
    ) {
        let change = super::interaction_change(
            &self.programmers,
            identity.desk_id,
            identity.session_id,
            before,
            after,
        );
        self.publish_interaction(context, change);
    }

    fn update_interaction_snapshot(
        &self,
        identity: &UpdateIdentity,
    ) -> Result<super::Snapshot, ActionError> {
        super::Snapshot::read(
            &self.programmers,
            identity.desk_id,
            identity.session_id,
            identity.user_id,
        )
    }

    fn capture_update(
        &self,
        session: SessionId,
        target: &ProgrammingUpdateTargetRequest,
    ) -> Result<UpdateCapture, ActionError> {
        let capture = match target {
            ProgrammingUpdateTargetRequest::Cue { .. }
            | ProgrammingUpdateTargetRequest::Preset { .. } => UpdateCapture::Values(
                self.programmers
                    .capture_update_values(session)
                    .map_err(capture_error)?,
            ),
            ProgrammingUpdateTargetRequest::Group { .. } => UpdateCapture::Selection(
                self.programmers
                    .capture_update_selection(session)
                    .map_err(capture_error)?,
            ),
        };
        Ok(capture)
    }

    fn assert_update_owner(&self, identity: &UpdateIdentity) -> Result<(), ActionError> {
        match self.programmers.user_id(identity.session_id) {
            Some(owner) if owner == identity.user_id => Ok(()),
            Some(_) => Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the authenticated user",
            )),
            None => Err(missing_programmer()),
        }
    }
}

fn update_identity(context: &crate::ActionContext) -> Result<UpdateIdentity, ActionError> {
    let session_id = context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Update requires an operator session",
        )
    })?;
    let user_id = context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Update requires an authenticated user",
        )
    })?;
    let request_id = context.request_id.as_deref().ok_or_else(|| {
        ActionError::new(ActionErrorKind::Invalid, "Update requires a request_id")
    })?;
    super::values_validation::validate_request_id(request_id)?;
    Ok(UpdateIdentity {
        session_id,
        user_id,
        desk_id: context.desk_id,
        request_id: request_id.to_owned(),
    })
}

fn capture_error(error: ProgrammerUpdateCaptureError) -> ActionError {
    match error {
        ProgrammerUpdateCaptureError::MissingSession => missing_programmer(),
    }
}

fn ensure_capture_owner(capture: &UpdateCapture, user_id: UserId) -> Result<(), ActionError> {
    if capture.user_id() == user_id {
        Ok(())
    } else {
        Err(foreign_capture())
    }
}

fn fingerprint(value: &impl serde::Serialize) -> Result<String, ActionError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| invalid(format!("could not fingerprint Programmer: {error}")))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn foreign_capture() -> ActionError {
    ActionError::new(
        ActionErrorKind::Forbidden,
        "the captured Programmer belongs to another user",
    )
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

fn conflict(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message)
}
