//! Authoritative application-to-wire projections for the v2 Programming Update surface.

use super::programming_update_wire::{wire_cue_mode, wire_existing_mode};
use light_application as app;
use light_application::programming_update as application;
use light_core::ShowId;
use light_wire::v2::programming_update as wire;

pub(super) fn preview_response(
    show_id: ShowId,
    result: application::ProgrammingUpdatePreviewResult,
) -> Result<wire::ProgrammingUpdatePreviewResponse, app::ActionError> {
    Ok(wire::ProgrammingUpdatePreviewResponse {
        request_id: result.request_id,
        correlation_id: result.correlation_id,
        show_id: show_id.0,
        show_revision: result.show_revision.value(),
        object: wire_object(result.object)?,
        programmer_revision: result.programmer_revision,
        preview: wire_preview(result.preview)?,
    })
}

pub(super) fn targets_response(
    show_id: ShowId,
    result: application::ProgrammingUpdateTargetsResult,
) -> Result<wire::ProgrammingUpdateTargetsResponse, app::ActionError> {
    Ok(wire::ProgrammingUpdateTargetsResponse {
        request_id: result.request_id,
        correlation_id: result.correlation_id,
        show_id: show_id.0,
        show_revision: result.show_revision.value(),
        targets: result
            .entries
            .into_iter()
            .map(wire_target_entry)
            .collect::<Result<_, _>>()?,
    })
}

pub(super) fn action_outcome(
    result: application::ProgrammingUpdateResult,
) -> Result<wire::ProgrammingUpdateActionOutcome, app::ActionError> {
    let outcome = result.outcome;
    Ok(wire::ProgrammingUpdateActionOutcome::Changed {
        request_id: result.request_id,
        correlation_id: result.correlation_id,
        replayed: result.replayed,
        show_id: outcome.projection.show_id.0,
        show_revision: outcome.show_revision.value(),
        projection: wire::ProgrammingUpdateProjection {
            kind: wire_kind(outcome.projection.kind)?,
            object_id: outcome.projection.object_id.clone(),
            object_revision: outcome.projection.object_revision,
            body: outcome.projection.raw_body.clone(),
        },
        event_sequence: outcome.event_sequence,
        summary: wire_summary(outcome.summary)?,
    })
}

fn wire_target_entry(
    entry: application::ProgrammingUpdateMenuEntry,
) -> Result<wire::ProgrammingUpdateTargetEntry, app::ActionError> {
    if entry.object_revision != entry.object.object_revision {
        return Err(internal(
            "Update menu returned inconsistent object revisions",
        ));
    }
    Ok(wire::ProgrammingUpdateTargetEntry {
        request_target: wire_request_target(entry.target),
        object: wire_object(entry.object)?,
        programmer_revision: entry.programmer_revision,
        active_or_referenced: entry.active_or_referenced,
        existing_preview: wire_preview(entry.existing_preview)?,
        add_new_preview: wire_preview(entry.add_new_preview)?,
    })
}

fn wire_request_target(
    target: application::ProgrammingUpdateTargetRequest,
) -> wire::ProgrammingUpdateTarget {
    match target {
        application::ProgrammingUpdateTargetRequest::Cue {
            cue_list_id,
            playback_number,
            cue_id,
            cue_number,
            validate_active_context,
        } => wire::ProgrammingUpdateTarget::Cue {
            cue_list_id: cue_list_id.0,
            playback_number,
            cue_id,
            cue_number,
            validate_active_context,
        },
        application::ProgrammingUpdateTargetRequest::Preset { object_id } => {
            wire::ProgrammingUpdateTarget::Preset { object_id }
        }
        application::ProgrammingUpdateTargetRequest::Group { object_id } => {
            wire::ProgrammingUpdateTarget::Group { object_id }
        }
    }
}

fn wire_object(
    object: application::ProgrammingUpdateObjectReference,
) -> Result<wire::ProgrammingUpdateObjectIdentity, app::ActionError> {
    Ok(wire::ProgrammingUpdateObjectIdentity {
        kind: wire_kind(object.kind)?,
        object_id: object.object_id,
        object_revision: object.object_revision,
    })
}

fn wire_kind(
    kind: app::ActiveShowObjectKind,
) -> Result<wire::ProgrammingUpdateObjectKind, app::ActionError> {
    match kind {
        app::ActiveShowObjectKind::CueList => Ok(wire::ProgrammingUpdateObjectKind::CueList),
        app::ActiveShowObjectKind::Preset => Ok(wire::ProgrammingUpdateObjectKind::Preset),
        app::ActiveShowObjectKind::Group => Ok(wire::ProgrammingUpdateObjectKind::Group),
        app::ActiveShowObjectKind::Playback | app::ActiveShowObjectKind::PlaybackPage => {
            Err(internal("Update returned an unrelated show object kind"))
        }
    }
}

fn wire_preview(
    preview: application::UpdatePreview,
) -> Result<wire::ProgrammingUpdatePreview, app::ActionError> {
    Ok(wire::ProgrammingUpdatePreview {
        target: wire_target_identity(preview.target)?,
        mode: wire_mode(preview.mode),
        items: preview
            .items
            .into_iter()
            .map(wire_preview_item)
            .collect::<Result<_, _>>()?,
    })
}

fn wire_target_identity(
    target: application::UpdateTargetIdentity,
) -> Result<wire::ProgrammingUpdateTargetIdentity, app::ActionError> {
    let family = match target.family {
        application::UpdateTargetFamily::Cue => wire::ProgrammingUpdateTargetFamily::Cue,
        application::UpdateTargetFamily::Preset => wire::ProgrammingUpdateTargetFamily::Preset,
        application::UpdateTargetFamily::Group => wire::ProgrammingUpdateTargetFamily::Group,
        application::UpdateTargetFamily::Other { .. } => {
            return Err(internal("Update returned an unsupported target family"));
        }
    };
    Ok(wire::ProgrammingUpdateTargetIdentity {
        family,
        object_id: target.object_id,
        name: target.name,
        playback_number: target.playback_number,
        cue: target.cue.map(|cue| wire::ProgrammingUpdateCueIdentity {
            id: cue.id,
            number: cue.number,
        }),
    })
}

fn wire_mode(mode: application::UpdateMode) -> wire::ProgrammingUpdateMode {
    match mode {
        application::UpdateMode::Cue(mode) => wire::ProgrammingUpdateMode::Cue(wire_cue_mode(mode)),
        application::UpdateMode::ExistingContent(mode) => {
            wire::ProgrammingUpdateMode::ExistingContent(wire_existing_mode(mode))
        }
    }
}

fn wire_preview_item(
    item: application::UpdatePreviewItem,
) -> Result<wire::ProgrammingUpdatePreviewItem, app::ActionError> {
    Ok(wire::ProgrammingUpdatePreviewItem {
        address: match item.address {
            application::UpdateAddress::FixtureAttribute {
                fixture_id,
                attribute,
            } => wire::ProgrammingUpdateAddress::FixtureAttribute {
                fixture_id: fixture_id.0,
                attribute: attribute.0,
            },
            application::UpdateAddress::GroupAttribute {
                group_id,
                attribute,
            } => wire::ProgrammingUpdateAddress::GroupAttribute {
                group_id,
                attribute: attribute.0,
            },
            application::UpdateAddress::GroupMembership { fixture_id } => {
                wire::ProgrammingUpdateAddress::GroupMembership {
                    fixture_id: fixture_id.0,
                }
            }
        },
        outcome: wire_item_outcome(item.outcome)?,
    })
}

fn wire_item_outcome(
    outcome: application::UpdateItemOutcome,
) -> Result<wire::ProgrammingUpdateItemOutcome, app::ActionError> {
    use application::UpdateItemOutcome as Input;
    use wire::ProgrammingUpdateItemOutcome as Output;
    Ok(match outcome {
        Input::ChangeAtSource { source } => Output::ChangeAtSource {
            source: wire_cue_source(source)?,
        },
        Input::ChangeInCurrentCue { cue } => Output::ChangeInCurrentCue {
            cue: wire_cue_source(cue)?,
        },
        Input::AddToCurrentCue { cue } => Output::AddToCurrentCue {
            cue: wire_cue_source(cue)?,
        },
        Input::AddNewToCurrentCue { cue } => Output::AddNewToCurrentCue {
            cue: wire_cue_source(cue)?,
        },
        Input::UpdateExisting => Output::UpdateExisting,
        Input::AddNew => Output::AddNew,
        Input::Unchanged { source } => Output::Unchanged {
            source: source.map(wire_cue_source).transpose()?,
        },
        Input::Ignored { reason } => Output::Ignored {
            reason: wire_ignore_reason(reason),
        },
    })
}

fn wire_ignore_reason(
    reason: application::UpdateIgnoreReason,
) -> wire::ProgrammingUpdateIgnoreReason {
    match reason {
        application::UpdateIgnoreReason::NewAddress => {
            wire::ProgrammingUpdateIgnoreReason::NewAddress
        }
        application::UpdateIgnoreReason::NotInCurrentCue => {
            wire::ProgrammingUpdateIgnoreReason::NotInCurrentCue
        }
        application::UpdateIgnoreReason::NotInActiveTrackedState => {
            wire::ProgrammingUpdateIgnoreReason::NotInActiveTrackedState
        }
        application::UpdateIgnoreReason::NewGroupMember => {
            wire::ProgrammingUpdateIgnoreReason::NewGroupMember
        }
    }
}

fn wire_summary(
    summary: application::UpdateResult,
) -> Result<wire::ProgrammingUpdateSummary, app::ActionError> {
    Ok(wire::ProgrammingUpdateSummary {
        target: wire_target_identity(summary.target)?,
        revision_before: summary.revision_before,
        revision_after: summary.revision_after,
        eligible_count: count(summary.eligible_count)?,
        changed_count: count(summary.changed_count)?,
        added_count: count(summary.added_count)?,
        ignored_count: count(summary.ignored_count)?,
        changed_cues: summary
            .changed_cues
            .into_iter()
            .map(wire_cue_source)
            .collect::<Result<_, _>>()?,
        programmer_values_retained: summary.programmer_values_retained,
    })
}

fn wire_cue_source(
    source: application::CueSource,
) -> Result<wire::ProgrammingUpdateCueSource, app::ActionError> {
    Ok(wire::ProgrammingUpdateCueSource {
        cue_id: source.cue_id,
        cue_number: source.cue_number,
        cue_index: count(source.cue_index)?,
    })
}

fn count(value: usize) -> Result<u64, app::ActionError> {
    value
        .try_into()
        .map_err(|_| internal("Update count exceeds the wire representation"))
}

fn internal(message: impl Into<String>) -> app::ActionError {
    app::ActionError::new(app::ActionErrorKind::Internal, message)
}
