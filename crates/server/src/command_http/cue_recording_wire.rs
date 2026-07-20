use light_application as application;
use light_core::{CueListId, ShowId};
use light_show::PortableShowRevision;
use light_wire::v2::cue_recording as wire;

const JAVASCRIPT_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn application_command(
    show_id: ShowId,
    expected_show_revision: u64,
    request: wire::CueRecordRequest,
) -> Result<(String, application::ProgrammingCueRecordRequest), String> {
    validate_request(&request)?;
    let request_id = request.request_id;
    let command = application::ProgrammingCueRecordRequest {
        show_id,
        target: target(request.target)?,
        operation: operation(request.operation),
        cue_number: request.cue_number.map(application::CueNumber::new),
        timing: application::ProgrammingCueRecordTiming {
            fade_millis: request.timing.fade_millis,
            delay_millis: request.timing.delay_millis,
        },
        cue_only: request.cue_only,
        name: request.name,
        capture_policy: capture_policy(request.capture_policy),
        activation_policy: activation_policy(request.activation_policy),
        expected_show_revision: application::ProgrammingCueShowRevisionExpectation::Exact(
            PortableShowRevision::from_value(expected_show_revision),
        ),
    };
    Ok((request_id, command))
}

pub(super) fn outcome(
    result: application::ProgrammingCueRecordResult,
) -> Result<wire::CueRecordOutcome, application::ActionError> {
    let common = OutcomeCommon::from_result(&result)?;
    Ok(match &result.outcome {
        application::ProgrammingCueRecordOutcome::Changed {
            show_event_sequence,
            runtime,
            ..
        } => wire::CueRecordOutcome::Changed {
            request_id: result.request_id,
            correlation_id: result.correlation_id,
            replayed: result.replayed,
            captured_source: common.captured_source,
            show_revision: common.show_revision,
            recorded_cue: common.recorded_cue,
            projections: common.projections,
            show_event_sequence: *show_event_sequence,
            runtime: runtime.as_deref().map(runtime_outcome).map(Box::new),
        },
        application::ProgrammingCueRecordOutcome::NoChange { .. } => {
            wire::CueRecordOutcome::NoChange {
                request_id: result.request_id,
                correlation_id: result.correlation_id,
                replayed: result.replayed,
                captured_source: common.captured_source,
                show_revision: common.show_revision,
                recorded_cue: common.recorded_cue,
                projections: common.projections,
            }
        }
    })
}

struct OutcomeCommon {
    captured_source: wire::CueRecordCapturedSource,
    show_revision: u64,
    recorded_cue: wire::RecordedCueProjection,
    projections: wire::CueRecordProjections,
}

impl OutcomeCommon {
    fn from_result(
        result: &application::ProgrammingCueRecordResult,
    ) -> Result<Self, application::ActionError> {
        let projections = result.outcome.projections();
        let recorded = recorded_cue(&result.outcome);
        Ok(Self {
            captured_source: captured_source(result.captured_source),
            show_revision: result.outcome.show_revision().value(),
            recorded_cue: wire::RecordedCueProjection {
                id: recorded.id,
                number: recorded.number.value(),
                deleted: recorded.deleted,
            },
            projections: wire::CueRecordProjections {
                cue_list: object_projection(
                    &projections.cue_list,
                    application::ActiveShowObjectKind::CueList,
                )?,
                playback: projections
                    .playback
                    .as_ref()
                    .map(|projection| {
                        object_projection(projection, application::ActiveShowObjectKind::Playback)
                    })
                    .transpose()?,
                page: projections
                    .page
                    .as_ref()
                    .map(|projection| {
                        object_projection(
                            projection,
                            application::ActiveShowObjectKind::PlaybackPage,
                        )
                    })
                    .transpose()?,
            },
        })
    }
}

fn recorded_cue(
    outcome: &application::ProgrammingCueRecordOutcome,
) -> application::ProgrammingRecordedCue {
    match outcome {
        application::ProgrammingCueRecordOutcome::Changed { recorded_cue, .. }
        | application::ProgrammingCueRecordOutcome::NoChange { recorded_cue, .. } => *recorded_cue,
    }
}

fn object_projection(
    projection: &application::ProgrammingCueObjectProjection,
    expected_kind: application::ActiveShowObjectKind,
) -> Result<wire::RecordedCueObjectProjection, application::ActionError> {
    if projection.kind != expected_kind {
        return Err(application::ActionError::new(
            application::ActionErrorKind::Internal,
            "Cue recording returned an inconsistent authoritative projection",
        ));
    }
    Ok(wire::RecordedCueObjectProjection {
        id: projection.object_id.clone(),
        revision: projection.object_revision,
        body: std::sync::Arc::clone(&projection.raw_body),
    })
}

fn runtime_outcome(
    result: &application::ProgrammingCueActivationResult,
) -> wire::CueRecordRuntimeOutcome {
    wire::CueRecordRuntimeOutcome {
        projection: super::super::playback_v2::runtime_projection(&result.projection),
        event_sequence: result.event_sequence,
    }
}

fn validate_request(request: &wire::CueRecordRequest) -> Result<(), String> {
    if let Some(number) = request.cue_number
        && (!number.is_finite() || number <= 0.0)
    {
        return Err("cue_number must be finite and greater than zero".into());
    }
    if let Some(name) = request.name.as_deref()
        && (name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control))
    {
        return Err("name must contain 1-256 printable bytes when supplied".into());
    }
    validate_timing("fade_millis", request.timing.fade_millis)?;
    validate_timing("delay_millis", request.timing.delay_millis)?;
    Ok(())
}

fn validate_timing(name: &str, value: Option<u64>) -> Result<(), String> {
    if value.is_some_and(|value| value > JAVASCRIPT_MAX_SAFE_INTEGER) {
        return Err(format!(
            "{name} must not exceed the JavaScript maximum safe integer"
        ));
    }
    Ok(())
}

fn target(value: wire::CueRecordTarget) -> Result<application::ProgrammingCueRecordTarget, String> {
    Ok(match value {
        wire::CueRecordTarget::Pool { playback_number }
            if (1..=light_playback::MAX_PLAYBACKS).contains(&playback_number) =>
        {
            application::ProgrammingCueRecordTarget::Pool { playback_number }
        }
        wire::CueRecordTarget::Pool { .. } => {
            return Err("playback_number must be within 1-1000".into());
        }
        wire::CueRecordTarget::SelectedPlayback => {
            application::ProgrammingCueRecordTarget::SelectedPlayback
        }
        wire::CueRecordTarget::PageSlot { page, slot }
            if (1..=light_playback::MAX_PLAYBACK_PAGES).contains(&page)
                && (1..=light_playback::MAX_PAGE_SLOTS).contains(&slot) =>
        {
            application::ProgrammingCueRecordTarget::PageSlot { page, slot }
        }
        wire::CueRecordTarget::PageSlot { .. } => {
            return Err("page and slot must be within 1-127".into());
        }
        wire::CueRecordTarget::CueList { cue_list_id } if !cue_list_id.is_nil() => {
            application::ProgrammingCueRecordTarget::CueList {
                cue_list_id: CueListId(cue_list_id),
            }
        }
        wire::CueRecordTarget::CueList { .. } => {
            return Err("cue_list_id must not be nil".into());
        }
    })
}

const fn operation(value: wire::CueRecordOperation) -> application::ProgrammingCueRecordOperation {
    match value {
        wire::CueRecordOperation::Overwrite => {
            application::ProgrammingCueRecordOperation::Overwrite
        }
        wire::CueRecordOperation::Merge => application::ProgrammingCueRecordOperation::Merge,
        wire::CueRecordOperation::Subtract => application::ProgrammingCueRecordOperation::Subtract,
    }
}

const fn capture_policy(
    value: wire::CueRecordCapturePolicy,
) -> application::ProgrammingCueCapturePolicy {
    match value {
        wire::CueRecordCapturePolicy::CurrentCapture => {
            application::ProgrammingCueCapturePolicy::CurrentCapture
        }
        wire::CueRecordCapturePolicy::PendingOrActivePreload => {
            application::ProgrammingCueCapturePolicy::PendingOrActivePreload
        }
    }
}

const fn activation_policy(
    value: wire::CueRecordActivationPolicy,
) -> application::ProgrammingCueActivationPolicy {
    match value {
        wire::CueRecordActivationPolicy::Hold => application::ProgrammingCueActivationPolicy::Hold,
        wire::CueRecordActivationPolicy::GoToIfNormal => {
            application::ProgrammingCueActivationPolicy::GoToIfNormal
        }
    }
}

const fn captured_source(
    value: light_programmer::CueRecordingCapturedSource,
) -> wire::CueRecordCapturedSource {
    match value {
        light_programmer::CueRecordingCapturedSource::Normal => {
            wire::CueRecordCapturedSource::Normal
        }
        light_programmer::CueRecordingCapturedSource::PendingPreload => {
            wire::CueRecordCapturedSource::PendingPreload
        }
        light_programmer::CueRecordingCapturedSource::ActivePreload => {
            wire::CueRecordCapturedSource::ActivePreload
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_mapping_rejects_invalid_addresses() {
        assert!(target(wire::CueRecordTarget::Pool { playback_number: 0 }).is_err());
        assert!(target(wire::CueRecordTarget::PageSlot { page: 1, slot: 0 }).is_err());
        assert!(
            target(wire::CueRecordTarget::CueList {
                cue_list_id: uuid::Uuid::nil()
            })
            .is_err()
        );
    }

    #[test]
    fn request_validation_rejects_invalid_cue_number_and_name() {
        let mut request = request();
        request.cue_number = Some(f64::NAN);
        assert!(validate_request(&request).is_err());
        request.cue_number = Some(1.0);
        request.name = Some("\n".into());
        assert!(validate_request(&request).is_err());
    }

    fn request() -> wire::CueRecordRequest {
        wire::CueRecordRequest {
            request_id: "cue-1".into(),
            target: wire::CueRecordTarget::SelectedPlayback,
            operation: wire::CueRecordOperation::Overwrite,
            cue_number: None,
            timing: wire::CueRecordTiming::default(),
            cue_only: false,
            name: None,
            capture_policy: wire::CueRecordCapturePolicy::CurrentCapture,
            activation_policy: wire::CueRecordActivationPolicy::Hold,
        }
    }
}
