use crate::{
    ActionError, ActionErrorKind, PlaybackRuntimeIdentity, ProgrammingCueActivationCompletion,
    ProgrammingCueCommit, ProgrammingCueCommitResult, ProgrammingCueObjectProjection,
    ProgrammingCueRecordOperation, ProgrammingCueRecordRequest, ProgrammingCueRecordTarget,
    ProgrammingCueRecordingEnvironment, ProgrammingCueResolvedTarget,
    ProgrammingCueShowRevisionExpectation, ProgrammingRecordedCue,
};

const JAVASCRIPT_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn validate_request(request: &ProgrammingCueRecordRequest) -> Result<(), ActionError> {
    if request.show_id.0.is_nil() {
        return Err(invalid("Cue recording requires a valid show_id"));
    }
    validate_target(request.target)?;
    if request.operation == ProgrammingCueRecordOperation::Subtract && request.cue_number.is_none()
    {
        return Err(invalid("Cue subtract requires an explicit Cue number"));
    }
    if request.name.as_ref().is_some_and(|name| {
        name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control)
    }) {
        return Err(invalid(
            "Cue name must contain 1-256 printable bytes when supplied",
        ));
    }
    if request
        .timing
        .fade_millis
        .is_some_and(unsafe_javascript_integer)
        || request
            .timing
            .delay_millis
            .is_some_and(unsafe_javascript_integer)
    {
        return Err(invalid(
            "Cue timing must not exceed the JavaScript maximum safe integer",
        ));
    }
    Ok(())
}

const fn unsafe_javascript_integer(value: u64) -> bool {
    value > JAVASCRIPT_MAX_SAFE_INTEGER
}

fn validate_target(target: ProgrammingCueRecordTarget) -> Result<(), ActionError> {
    match target {
        ProgrammingCueRecordTarget::CuelistPool { number } if !(1..=1000).contains(&number) => {
            Err(invalid("Cuelist number must be within 1-1000"))
        }
        ProgrammingCueRecordTarget::CueList { cue_list_id } if cue_list_id.0.is_nil() => {
            Err(invalid("Cue recording requires a valid Cuelist id"))
        }
        _ => Ok(()),
    }
}

pub(super) fn validate_environment(
    request: &ProgrammingCueRecordRequest,
    environment: &ProgrammingCueRecordingEnvironment,
) -> Result<(), ActionError> {
    let target_matches = match (request.target, environment.target) {
        (
            ProgrammingCueRecordTarget::CuelistPool { number },
            ProgrammingCueResolvedTarget::CuelistPool { number: resolved },
        ) => number == resolved,
        (
            ProgrammingCueRecordTarget::SelectedPlayback,
            ProgrammingCueResolvedTarget::CueList { .. },
        ) => true,
        (
            ProgrammingCueRecordTarget::Playback { address },
            ProgrammingCueResolvedTarget::Playback { address: resolved },
        ) => address == resolved,
        (
            ProgrammingCueRecordTarget::SelectedPlayback,
            ProgrammingCueResolvedTarget::Playback { .. },
        ) => true,
        (
            ProgrammingCueRecordTarget::CueList { cue_list_id },
            ProgrammingCueResolvedTarget::CueList {
                cue_list_id: resolved,
            },
        ) => cue_list_id == resolved,
        _ => false,
    };
    if target_matches {
        Ok(())
    } else {
        Err(ActionError::new(
            ActionErrorKind::Internal,
            "Cue recording environment returned an unrelated target",
        ))
    }
}

pub(super) fn validate_completion(
    request: &ProgrammingCueRecordRequest,
    commit: &ProgrammingCueCommit,
    completion: &ProgrammingCueCommitResult,
) -> Result<(), ActionError> {
    let event_matches = completion.changed == completion.event_sequence.is_some();
    let revision_matches = match (request.expected_show_revision, completion.changed) {
        (ProgrammingCueShowRevisionExpectation::Exact(expected), true) => {
            completion.show_revision > expected
        }
        (ProgrammingCueShowRevisionExpectation::Exact(expected), false) => {
            completion.show_revision == expected
        }
        (ProgrammingCueShowRevisionExpectation::Current, _) => true,
    };
    let creation_matches = completion.created_topology
        == (completion.changed
            && completion.projections.cue_list.object_revision == 1
            && (completion.projections.page.is_some()
                || matches!(
                    request.target,
                    ProgrammingCueRecordTarget::CuelistPool { .. }
                )));
    if event_matches
        && revision_matches
        && creation_matches
        && validate_projections(commit, completion)
        && topology_matches(commit, completion)
    {
        Ok(())
    } else {
        Err(invalid_completion())
    }
}

pub(super) fn validate_activation(
    playback: light_playback::PlaybackAddress,
    completion: &ProgrammingCueCommitResult,
    activation: &ProgrammingCueActivationCompletion,
) -> Result<(), ActionError> {
    let projection = &activation.projection;
    let current = projection.current_cue();
    if projection.identity == PlaybackRuntimeIdentity::Playback(playback)
        && activation
            .event_sequence
            .is_none_or(|sequence| sequence > 0)
        && current.is_some_and(|cue| {
            cue.id == completion.recorded_cue.id && cue.number == completion.recorded_cue.number
        })
    {
        Ok(())
    } else {
        Err(invalid_completion())
    }
}

fn topology_matches(commit: &ProgrammingCueCommit, result: &ProgrammingCueCommitResult) -> bool {
    let projections = &result.projections;
    let Ok(cue_list) = serde_json::from_value::<light_playback::CueList>(
        projections.cue_list.raw_body.as_ref().clone(),
    ) else {
        return false;
    };
    match commit.environment().target {
        ProgrammingCueResolvedTarget::CuelistPool { number } => {
            (cue_list.pool_number == Some(number) || cue_list.legacy_pool_aliases.contains(&number))
                && projections.page.is_none()
                && result.concrete_playback_address.is_none()
        }
        ProgrammingCueResolvedTarget::CueList { cue_list_id } => {
            cue_list.id == cue_list_id
                && projections.page.is_none()
                && result.concrete_playback_address.is_none()
        }
        ProgrammingCueResolvedTarget::Playback { address } => {
            result.concrete_playback_address == Some(address)
                && projections.page.as_ref().is_some_and(|projection| {
                    serde_json::from_value::<light_playback::PlaybackPage>(
                        projection.raw_body.as_ref().clone(),
                    )
                    .is_ok_and(|page| {
                        page.number == address.page()
                            && page
                                .assignments
                                .get(&address.position())
                                .is_some_and(|playback| {
                                    playback.target
                                        == light_playback::PlaybackTarget::CueList {
                                            cue_list_id: cue_list.id,
                                        }
                                })
                    })
                })
        }
    }
}

fn validate_projections(
    commit: &ProgrammingCueCommit,
    result: &ProgrammingCueCommitResult,
) -> bool {
    let projections = &result.projections;
    projections.show_id == commit.show_id
        && projections.cue_list.kind == crate::ActiveShowObjectKind::CueList
        && object_is_valid(&projections.cue_list)
        && cue_presence_matches(&projections.cue_list.raw_body, &result.recorded_cue)
        && projections.page.as_ref().is_none_or(|projection| {
            projection.kind == crate::ActiveShowObjectKind::PlaybackPage
                && object_is_valid(projection)
        })
}

fn object_is_valid(projection: &ProgrammingCueObjectProjection) -> bool {
    projection.object_revision > 0
        && !projection.object_id.trim().is_empty()
        && !projection.object_id.chars().any(char::is_control)
        && projection.raw_body.is_object()
}

fn cue_presence_matches(body: &serde_json::Value, recorded: &ProgrammingRecordedCue) -> bool {
    let present = body
        .get("cues")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|cues| {
            cues.iter().any(|cue| {
                cue.get("id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|id| uuid::Uuid::parse_str(id).ok())
                    == Some(recorded.id)
                    && cue.get("number").and_then(serde_json::Value::as_str)
                        == Some(recorded.number.to_string().as_str())
            })
        });
    present != recorded.deleted
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

pub(super) fn invalid_completion() -> ActionError {
    ActionError::new(
        ActionErrorKind::Internal,
        "Cue recording port returned an inconsistent authoritative completion",
    )
}
