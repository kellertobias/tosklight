use super::ProgrammingService;
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ProgrammingPresetCommit,
    ProgrammingPresetCommitResult, ProgrammingPresetProjection, ProgrammingPresetRecordOutcome,
    ProgrammingPresetRecordRequest, ProgrammingPresetRecordResult, ProgrammingPresetRecordingPorts,
    ProgrammingPresetRevisionExpectation,
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
    /// Records one action-time snapshot through the same user and desk ordering boundary as every
    /// normal Programmer mutation.
    pub fn handle_preset_recording(
        &self,
        envelope: ActionEnvelope<ProgrammingPresetRecordRequest>,
        ports: &dyn ProgrammingPresetRecordingPorts,
    ) -> Result<ProgrammingPresetRecordResult, ActionError> {
        let identity = recording_identity(&envelope)?;
        self.with_user_and_desk_gate(envelope.context.desk_id, identity.user_id, || {
            self.apply_preset_recording(envelope, ports, identity)
        })
    }

    /// Executes Preset recording while an existing [`ProgrammingService`] interaction already
    /// owns this user's user-then-desk gates.
    ///
    /// This is the bridge for the legacy command parser currently invoked inside
    /// `ProgrammingPorts::execute`. Calling it without those gates is a programming error; new
    /// standalone adapters must use [`Self::handle_preset_recording`].
    pub fn record_preset_within_interaction(
        &self,
        envelope: ActionEnvelope<ProgrammingPresetRecordRequest>,
        ports: &dyn ProgrammingPresetRecordingPorts,
    ) -> Result<ProgrammingPresetRecordResult, ActionError> {
        let identity = recording_identity(&envelope)?;
        self.apply_preset_recording(envelope, ports, identity)
    }

    fn apply_preset_recording(
        &self,
        envelope: ActionEnvelope<ProgrammingPresetRecordRequest>,
        ports: &dyn ProgrammingPresetRecordingPorts,
        identity: RecordingIdentity,
    ) -> Result<ProgrammingPresetRecordResult, ActionError> {
        ports.authorize_preset_recording(&envelope.context)?;
        self.assert_preset_owner(identity.session_id, identity.user_id)?;
        validate_recording_request(&envelope.command)?;
        if let Some(result) = self.cached_preset_recording(&identity, &envelope.command)? {
            return Ok(result);
        }
        self.assert_normal_capture(identity.session_id)?;
        let captured = self.capture_preset(identity.session_id, &envelope.command)?;
        let commit = ProgrammingPresetCommit::new(&envelope.command, captured);
        let completion = ports.commit_preset(&envelope.context, &commit)?;
        let result = complete_result(&envelope, &identity.request_id, completion)?;
        self.remember_preset_recording(identity, envelope.command, result.clone());
        Ok(result)
    }

    fn assert_preset_owner(
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

    fn assert_normal_capture(&self, session_id: SessionId) -> Result<(), ActionError> {
        let mode = self.programmers.capture_mode(session_id).ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::NotFound,
                "the Programmer session does not exist",
            )
        })?;
        if mode.redirects_normal_values_to_preload() {
            Err(ActionError::new(
                ActionErrorKind::Conflict,
                "normal Preset recording is unavailable while Programmer capture is redirected to Preload",
            ))
        } else {
            Ok(())
        }
    }

    fn capture_preset(
        &self,
        session_id: SessionId,
        request: &ProgrammingPresetRecordRequest,
    ) -> Result<light_programmer::Preset, ActionError> {
        let preset = self
            .programmers
            .capture_normal_preset(session_id, request.address, request.name.clone())
            .ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::NotFound,
                    "the Programmer session does not exist",
                )
            })?;
        if preset.values.is_empty() && preset.group_values.is_empty() {
            Err(ActionError::new(
                ActionErrorKind::Invalid,
                "the normal Programmer has no values in the requested Preset family",
            ))
        } else {
            Ok(preset)
        }
    }

    fn cached_preset_recording(
        &self,
        identity: &RecordingIdentity,
        request: &ProgrammingPresetRecordRequest,
    ) -> Result<Option<ProgrammingPresetRecordResult>, ActionError> {
        self.preset_recording_replay.lock().get(
            identity.user_id,
            identity.desk_id,
            identity.session_id,
            &identity.request_id,
            request,
        )
    }

    fn remember_preset_recording(
        &self,
        identity: RecordingIdentity,
        request: ProgrammingPresetRecordRequest,
        result: ProgrammingPresetRecordResult,
    ) {
        self.preset_recording_replay.lock().insert(
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
    envelope: &ActionEnvelope<ProgrammingPresetRecordRequest>,
) -> Result<RecordingIdentity, ActionError> {
    let session_id = envelope.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preset recording requires an operator session",
        )
    })?;
    let user_id = envelope.context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preset recording requires an authenticated user",
        )
    })?;
    let request_id = envelope.context.request_id.as_deref().ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Preset recording requires a request_id",
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

fn validate_recording_request(request: &ProgrammingPresetRecordRequest) -> Result<(), ActionError> {
    if request.show_id.0.is_nil() {
        return Err(invalid("Preset recording requires a valid show_id"));
    }
    light_programmer::PresetAddress::new(request.address.family, request.address.number)
        .map_err(invalid)?;
    if request.name.trim().is_empty()
        || request.name.len() > 256
        || request.name.chars().any(char::is_control)
    {
        return Err(invalid("Preset name must contain 1-256 printable bytes"));
    }
    Ok(())
}

fn complete_result(
    envelope: &ActionEnvelope<ProgrammingPresetRecordRequest>,
    request_id: &str,
    completion: ProgrammingPresetCommitResult,
) -> Result<ProgrammingPresetRecordResult, ActionError> {
    validate_completion(&envelope.command, &completion)?;
    let projection = Arc::new(completion.projection);
    let outcome = match completion.event_sequence {
        Some(event_sequence) => ProgrammingPresetRecordOutcome::Changed {
            projection,
            show_revision: completion.show_revision,
            event_sequence,
        },
        None => ProgrammingPresetRecordOutcome::NoChange {
            projection,
            show_revision: completion.show_revision,
        },
    };
    Ok(ProgrammingPresetRecordResult {
        context: envelope.context.clone(),
        request_id: request_id.to_owned(),
        replayed: false,
        outcome,
    })
}

fn validate_completion(
    request: &ProgrammingPresetRecordRequest,
    completion: &ProgrammingPresetCommitResult,
) -> Result<(), ActionError> {
    validate_projection(request, &completion.projection)?;
    let event_matches = completion.changed == completion.event_sequence.is_some();
    let revision_matches = match (request.expected_object_revision, completion.changed) {
        (ProgrammingPresetRevisionExpectation::Exact(expected), true) => {
            completion.projection.object_revision > expected
        }
        (ProgrammingPresetRevisionExpectation::Exact(expected), false) => {
            completion.projection.object_revision == expected
        }
        (ProgrammingPresetRevisionExpectation::Current, true) => {
            completion.projection.object_revision > 0
        }
        (ProgrammingPresetRevisionExpectation::Current, false) => true,
    };
    let show_revision_matches = match (request.expected_show_revision, completion.changed) {
        (Some(expected), true) => completion.show_revision > expected,
        (Some(expected), false) => completion.show_revision == expected,
        (None, _) => true,
    };
    if event_matches && revision_matches && show_revision_matches {
        Ok(())
    } else {
        Err(invalid_completion())
    }
}

fn validate_projection(
    request: &ProgrammingPresetRecordRequest,
    projection: &ProgrammingPresetProjection,
) -> Result<(), ActionError> {
    let object_id_valid = !projection.object_id.trim().is_empty()
        && projection.object_id.len() <= 256
        && !projection.object_id.chars().any(char::is_control);
    let body = projection.raw_body.as_object();
    let body_identity_valid = body.is_some_and(|body| {
        body.get("name").and_then(serde_json::Value::as_str) == Some(request.name.as_str())
            && body.get("number").and_then(serde_json::Value::as_u64)
                == Some(u64::from(request.address.number))
            && body.get("family").and_then(serde_json::Value::as_str)
                == Some(preset_family_name(request.address.family))
    });
    if projection.show_id == request.show_id
        && projection.address == request.address
        && object_id_valid
        && body_identity_valid
    {
        Ok(())
    } else {
        Err(invalid_completion())
    }
}

const fn preset_family_name(family: light_programmer::PresetFamily) -> &'static str {
    match family {
        light_programmer::PresetFamily::Mixed => "Mixed",
        light_programmer::PresetFamily::Intensity => "Intensity",
        light_programmer::PresetFamily::Color => "Color",
        light_programmer::PresetFamily::Position => "Position",
        light_programmer::PresetFamily::Beam => "Beam",
    }
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn invalid_completion() -> ActionError {
    ActionError::new(
        ActionErrorKind::Internal,
        "Preset recording port returned an inconsistent authoritative completion",
    )
}
