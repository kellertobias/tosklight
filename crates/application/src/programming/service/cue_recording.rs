use super::{ProgrammingService, support::Snapshot};
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ProgrammingCueActivationCompletion,
    ProgrammingCueActivationPolicy, ProgrammingCueActivationResult, ProgrammingCueCapturePolicy,
    ProgrammingCueCommit, ProgrammingCueCommitResult, ProgrammingCueRecordOutcome,
    ProgrammingCueRecordRequest, ProgrammingCueRecordResult, ProgrammingCueRecordingPorts,
};
use light_core::{SessionId, UserId};
use light_programmer::{
    CueRecordingCapture, CueRecordingCaptureError, CueRecordingCapturedSource, CueRecordingSource,
};
use std::sync::Arc;

use super::cue_recording_validation::{
    invalid_completion, validate_activation, validate_completion, validate_environment,
    validate_request,
};

struct RecordingIdentity {
    session_id: SessionId,
    user_id: UserId,
    desk_id: uuid::Uuid,
    request_id: String,
}

impl ProgrammingService {
    pub fn handle_cue_recording(
        &self,
        envelope: ActionEnvelope<ProgrammingCueRecordRequest>,
        ports: &dyn ProgrammingCueRecordingPorts,
    ) -> Result<ProgrammingCueRecordResult, ActionError> {
        let identity = recording_identity(&envelope)?;
        self.with_user_and_desk_gate(envelope.context.desk_id, identity.user_id, || {
            self.apply_cue_recording(envelope, ports, identity)
        })
    }

    /// Records a Cue while the caller already owns this user's user-then-desk interaction gates.
    /// Legacy command interception uses this bridge; standalone adapters must use
    /// [`Self::handle_cue_recording`].
    pub fn record_cue_within_interaction(
        &self,
        envelope: ActionEnvelope<ProgrammingCueRecordRequest>,
        ports: &dyn ProgrammingCueRecordingPorts,
    ) -> Result<ProgrammingCueRecordResult, ActionError> {
        let identity = recording_identity(&envelope)?;
        self.apply_cue_recording(envelope, ports, identity)
    }

    fn apply_cue_recording(
        &self,
        envelope: ActionEnvelope<ProgrammingCueRecordRequest>,
        ports: &dyn ProgrammingCueRecordingPorts,
        identity: RecordingIdentity,
    ) -> Result<ProgrammingCueRecordResult, ActionError> {
        ports.authorize_cue_recording(&envelope.context)?;
        self.assert_cue_owner(identity.session_id, identity.user_id)?;
        validate_request(&envelope.command)?;
        if let Some(result) = self.cached_cue_recording(&identity, &envelope.command)? {
            return Ok(result);
        }
        let environment = ports.cue_recording_environment(&envelope.context, &envelope.command)?;
        validate_environment(&envelope.command, &environment)?;
        let capture = self.capture_cue(identity.session_id, envelope.command.capture_policy)?;
        let captured_source = capture.source;
        let release_before = self.release_snapshot(&envelope, &identity, captured_source)?;
        let commit = ProgrammingCueCommit::new(envelope.command.clone(), environment, capture);
        let completion = ports.commit_cue(&envelope.context, &commit)?;
        validate_completion(&envelope.command, &commit, &completion)?;
        let runtime = activate_if_requested(&envelope, &completion, captured_source, ports)?;
        self.release_active_fallback(&envelope, &identity, release_before)?;
        let result = complete_result(
            &envelope,
            &identity.request_id,
            captured_source,
            completion,
            runtime,
        );
        self.remember_cue_recording(identity, envelope.command, result.clone());
        Ok(result)
    }

    fn assert_cue_owner(&self, session: SessionId, user_id: UserId) -> Result<(), ActionError> {
        match self.programmers.user_id(session) {
            Some(owner) if owner == user_id => Ok(()),
            Some(_) => Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the authenticated user",
            )),
            None => Err(missing_programmer()),
        }
    }

    fn capture_cue(
        &self,
        session: SessionId,
        policy: ProgrammingCueCapturePolicy,
    ) -> Result<CueRecordingCapture, ActionError> {
        let source = match policy {
            ProgrammingCueCapturePolicy::CurrentCapture => CueRecordingSource::CurrentCapture,
            ProgrammingCueCapturePolicy::PendingOrActivePreload => {
                CueRecordingSource::PreloadPendingOrActive
            }
        };
        self.programmers
            .capture_cue_recording(session, source)
            .map_err(|error| match error {
                CueRecordingCaptureError::MissingSession => missing_programmer(),
            })
    }

    fn release_snapshot(
        &self,
        envelope: &ActionEnvelope<ProgrammingCueRecordRequest>,
        identity: &RecordingIdentity,
        source: CueRecordingCapturedSource,
    ) -> Result<Option<Snapshot>, ActionError> {
        if source != CueRecordingCapturedSource::ActivePreload {
            return Ok(None);
        }
        Snapshot::read(
            &self.programmers,
            envelope.context.desk_id,
            identity.session_id,
            identity.user_id,
        )
        .map(Some)
    }

    fn release_active_fallback(
        &self,
        envelope: &ActionEnvelope<ProgrammingCueRecordRequest>,
        identity: &RecordingIdentity,
        before: Option<Snapshot>,
    ) -> Result<(), ActionError> {
        let Some(before) = before else {
            return Ok(());
        };
        let lifecycle_before = self.active_lifecycle_programmer(identity.user_id);
        self.programmers.release_preload(identity.session_id);
        let after = Snapshot::read(
            &self.programmers,
            envelope.context.desk_id,
            identity.session_id,
            identity.user_id,
        )?;
        self.publish_released_preload(&envelope.context, identity, &before, &after)?;
        self.publish_lifecycle_for_context(&envelope.context, lifecycle_before);
        Ok(())
    }

    fn publish_released_preload(
        &self,
        context: &crate::ActionContext,
        identity: &RecordingIdentity,
        before: &Snapshot,
        after: &Snapshot,
    ) -> Result<(), ActionError> {
        self.publish_capture_mode(
            context,
            self.capture_mode_change(identity.user_id, before.capture_mode, after.capture_mode),
        );
        let values = self.preload_values_change(
            identity.user_id,
            identity.session_id,
            before.preload_values_generation,
            after.preload_values_generation,
        )?;
        self.publish_preload_values(context, values);
        let queue = self.preload_playback_queue_change(
            identity.user_id,
            identity.session_id,
            before.preload_playback_queue_generation,
            after.preload_playback_queue_generation,
        )?;
        self.publish_preload_playback_queue(context, queue);
        Ok(())
    }

    fn cached_cue_recording(
        &self,
        identity: &RecordingIdentity,
        request: &ProgrammingCueRecordRequest,
    ) -> Result<Option<ProgrammingCueRecordResult>, ActionError> {
        self.cue_recording_replay.lock().get(
            identity.user_id,
            identity.desk_id,
            identity.session_id,
            &identity.request_id,
            request,
        )
    }

    fn remember_cue_recording(
        &self,
        identity: RecordingIdentity,
        request: ProgrammingCueRecordRequest,
        result: ProgrammingCueRecordResult,
    ) {
        self.cue_recording_replay.lock().insert(
            identity.user_id,
            identity.desk_id,
            identity.session_id,
            identity.request_id,
            request,
            result,
        );
    }
}

fn activate_if_requested(
    envelope: &ActionEnvelope<ProgrammingCueRecordRequest>,
    completion: &ProgrammingCueCommitResult,
    source: CueRecordingCapturedSource,
    ports: &dyn ProgrammingCueRecordingPorts,
) -> Result<Option<ProgrammingCueActivationResult>, ActionError> {
    if !should_activate(&envelope.command, completion, source) {
        return Ok(None);
    }
    let playback = completion
        .concrete_playback_number
        .ok_or_else(invalid_completion)?;
    let Some(activation) =
        ports.activate_recorded_cue(&envelope.context, playback, completion.recorded_cue.number)
    else {
        return Ok(None);
    };
    validate_activation(playback, completion, &activation)?;
    Ok(emitted_activation(activation))
}

fn emitted_activation(
    completion: ProgrammingCueActivationCompletion,
) -> Option<ProgrammingCueActivationResult> {
    completion
        .event_sequence
        .map(|event_sequence| ProgrammingCueActivationResult {
            projection: completion.projection,
            event_sequence,
        })
}

fn should_activate(
    request: &ProgrammingCueRecordRequest,
    completion: &ProgrammingCueCommitResult,
    source: CueRecordingCapturedSource,
) -> bool {
    completion.changed
        && !completion.recorded_cue.deleted
        && source == CueRecordingCapturedSource::Normal
        && request.activation_policy == ProgrammingCueActivationPolicy::GoToIfNormal
        && completion.concrete_playback_number.is_some()
}

fn recording_identity(
    envelope: &ActionEnvelope<ProgrammingCueRecordRequest>,
) -> Result<RecordingIdentity, ActionError> {
    let session_id = envelope.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Cue recording requires an operator session",
        )
    })?;
    let user_id = envelope.context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Cue recording requires an authenticated user",
        )
    })?;
    let request_id = envelope.context.request_id.as_deref().ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Cue recording requires a request_id",
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

fn complete_result(
    envelope: &ActionEnvelope<ProgrammingCueRecordRequest>,
    request_id: &str,
    captured_source: CueRecordingCapturedSource,
    completion: ProgrammingCueCommitResult,
    runtime: Option<ProgrammingCueActivationResult>,
) -> ProgrammingCueRecordResult {
    let projections = Arc::new(completion.projections);
    let outcome = match completion.event_sequence {
        Some(show_event_sequence) => ProgrammingCueRecordOutcome::Changed {
            projections,
            recorded_cue: completion.recorded_cue,
            show_revision: completion.show_revision,
            show_event_sequence,
            runtime: runtime.map(Arc::new),
        },
        None => ProgrammingCueRecordOutcome::NoChange {
            projections,
            recorded_cue: completion.recorded_cue,
            show_revision: completion.show_revision,
        },
    };
    ProgrammingCueRecordResult {
        context: envelope.context.clone(),
        request_id: request_id.to_owned(),
        correlation_id: envelope.context.correlation_id,
        replayed: false,
        captured_source,
        outcome,
    }
}

fn missing_programmer() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "the Programmer session does not exist",
    )
}
