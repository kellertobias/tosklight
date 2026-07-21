use light_application as application;
use light_wire::v2::preset_recall as wire;

pub(super) fn outcome(
    result: application::ProgrammingPresetRecallResult,
) -> wire::PresetRecallOutcome {
    let programmer_revision = result.outcome.values_revision();
    let outcome = match result.outcome {
        application::ProgrammingPresetRecallOutcome::Changed {
            projection,
            values_event_sequence,
            ..
        } => wire::PresetRecallActionState::Changed {
            projection: projection
                .as_deref()
                .map(super::values_wire::values_projection),
            event_sequence: values_event_sequence,
        },
        application::ProgrammingPresetRecallOutcome::NoChange { .. } => {
            wire::PresetRecallActionState::NoChange
        }
    };
    wire::PresetRecallOutcome {
        request_id: result.request_id,
        correlation_id: result.context.correlation_id,
        replayed: result.replayed,
        show_revision: result.preset.show_revision.value(),
        programmer_revision,
        capture_mode_revision: result.capture_mode_revision,
        selection_revision: result.selection_revision,
        interaction_event_sequence: result.interaction_event_sequence,
        applied_fixtures: result.applied_fixtures as u64,
        active_context: result.active_context,
        preset: wire::RecalledPresetProjection {
            id: result.preset.object_id,
            revision: result.preset.object_revision,
            body: result.preset.raw_body.as_ref().clone(),
        },
        outcome,
        warning: result.warning,
    }
}
