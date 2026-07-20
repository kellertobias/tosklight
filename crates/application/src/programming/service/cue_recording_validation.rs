use crate::{
    ActionError, ActionErrorKind, PlaybackRuntimeIdentity, ProgrammingCueActivationCompletion,
    ProgrammingCueCommit, ProgrammingCueCommitResult, ProgrammingCueObjectProjection,
    ProgrammingCuePageSlot, ProgrammingCueRecordOperation, ProgrammingCueRecordRequest,
    ProgrammingCueRecordTarget, ProgrammingCueRecordingEnvironment, ProgrammingCueResolvedTarget,
    ProgrammingCueShowRevisionExpectation, ProgrammingRecordedCue,
};

const JAVASCRIPT_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn validate_request(request: &ProgrammingCueRecordRequest) -> Result<(), ActionError> {
    if request.show_id.0.is_nil() {
        return Err(invalid("Cue recording requires a valid show_id"));
    }
    validate_target(request.target)?;
    if let Some(number) = request.cue_number {
        let number = number.value();
        if !number.is_finite() || number <= 0.0 {
            return Err(invalid("Cue number must be finite and positive"));
        }
    }
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
        ProgrammingCueRecordTarget::Pool { playback_number }
            if !(1..=1_000).contains(&playback_number) =>
        {
            Err(invalid("Playback number must be within 1-1000"))
        }
        ProgrammingCueRecordTarget::PageSlot { page, slot }
            if !(1..=127).contains(&page) || !(1..=127).contains(&slot) =>
        {
            Err(invalid("Playback page and slot must be within 1-127"))
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
            ProgrammingCueRecordTarget::Pool { playback_number },
            ProgrammingCueResolvedTarget::Playback {
                playback_number: resolved,
                page_slot: None,
            },
        ) => playback_number == resolved,
        (
            ProgrammingCueRecordTarget::SelectedPlayback,
            ProgrammingCueResolvedTarget::Playback {
                page_slot: None, ..
            },
        ) => true,
        (
            ProgrammingCueRecordTarget::PageSlot { page, slot },
            ProgrammingCueResolvedTarget::Playback {
                page_slot: Some(resolved),
                ..
            }
            | ProgrammingCueResolvedTarget::EmptyPageSlot(resolved),
        ) => resolved == ProgrammingCuePageSlot { page, slot },
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
    if event_matches
        && revision_matches
        && validate_projections(commit, completion)
        && topology_matches(commit, completion)
        && completion.recorded_cue.number.value().is_finite()
        && completion.recorded_cue.number.value() > 0.0
    {
        Ok(())
    } else {
        Err(invalid_completion())
    }
}

pub(super) fn validate_activation(
    playback: u16,
    completion: &ProgrammingCueCommitResult,
    activation: &ProgrammingCueActivationCompletion,
) -> Result<(), ActionError> {
    let projection = &activation.projection;
    let current = projection.current_cue();
    if projection.requested == PlaybackRuntimeIdentity::Playback(playback)
        && projection.playback_number == Some(playback)
        && activation
            .event_sequence
            .is_none_or(|sequence| sequence > 0)
        && current.is_some_and(|cue| {
            cue.id == completion.recorded_cue.id
                && cue.number.to_bits() == completion.recorded_cue.number.value().to_bits()
        })
    {
        Ok(())
    } else {
        Err(invalid_completion())
    }
}

fn topology_matches(commit: &ProgrammingCueCommit, result: &ProgrammingCueCommitResult) -> bool {
    let projections = &result.projections;
    let cue_list = serde_json::from_value::<light_playback::CueList>(
        projections.cue_list.raw_body.as_ref().clone(),
    );
    let Ok(cue_list) = cue_list else {
        return false;
    };
    match commit.environment().target {
        ProgrammingCueResolvedTarget::CueList { cue_list_id } => {
            cue_list.id == cue_list_id
                && projections.playback.is_none()
                && projections.page.is_none()
                && result.concrete_playback_number.is_none()
        }
        ProgrammingCueResolvedTarget::Playback {
            playback_number,
            page_slot,
        } => {
            playback_matches(projections.playback.as_ref(), playback_number, cue_list.id)
                && page_slot_matches(projections.page.as_ref(), page_slot, playback_number)
                && result.concrete_playback_number == Some(playback_number)
        }
        ProgrammingCueResolvedTarget::EmptyPageSlot(page_slot) => {
            let Some(playback_number) = result.concrete_playback_number else {
                return false;
            };
            playback_matches(projections.playback.as_ref(), playback_number, cue_list.id)
                && page_slot_matches(projections.page.as_ref(), Some(page_slot), playback_number)
        }
    }
}

fn playback_matches(
    projection: Option<&ProgrammingCueObjectProjection>,
    number: u16,
    cue_list_id: light_core::CueListId,
) -> bool {
    let Some(projection) = projection else {
        return false;
    };
    serde_json::from_value::<light_playback::PlaybackDefinition>(
        projection.raw_body.as_ref().clone(),
    )
    .is_ok_and(|playback| {
        playback.number == number
            && playback.target == light_playback::PlaybackTarget::CueList { cue_list_id }
    })
}

fn page_slot_matches(
    projection: Option<&ProgrammingCueObjectProjection>,
    page_slot: Option<ProgrammingCuePageSlot>,
    playback_number: u16,
) -> bool {
    match (projection, page_slot) {
        (None, None) => true,
        (Some(projection), Some(page_slot)) => {
            serde_json::from_value::<light_playback::PlaybackPage>(
                projection.raw_body.as_ref().clone(),
            )
            .is_ok_and(|page| {
                page.number == page_slot.page
                    && page.slots.get(&page_slot.slot) == Some(&playback_number)
            })
        }
        _ => false,
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
        && cue_presence_matches(&projections.cue_list.raw_body, result.recorded_cue)
        && projections.playback.as_ref().is_none_or(|projection| {
            projection.kind == crate::ActiveShowObjectKind::Playback && object_is_valid(projection)
        })
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

fn cue_presence_matches(body: &serde_json::Value, recorded: ProgrammingRecordedCue) -> bool {
    let present = body
        .get("cues")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|cues| {
            cues.iter().any(|cue| {
                cue.get("id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|id| uuid::Uuid::parse_str(id).ok())
                    == Some(recorded.id)
                    && cue.get("number").and_then(serde_json::Value::as_f64)
                        == Some(recorded.number.value())
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
