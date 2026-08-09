use light_application as application;
use light_wire::v2::preset_recall as wire;

pub(crate) fn outcome(
    result: application::ProgrammingPresetRecallResult,
) -> wire::PresetRecallOutcome {
    let programmer_revision = result.outcome.values_revision();
    let target = match result.target {
        application::ProgrammingPresetRecallTarget::Programmer => None,
        application::ProgrammingPresetRecallTarget::Preload => {
            Some(wire::PresetRecallTarget::Preload)
        }
    };
    let (outcome, preload_projection, preload_event_sequence) = match result.outcome {
        application::ProgrammingPresetRecallOutcome::Changed {
            projection,
            values_event_sequence,
            ..
        } => (
            wire::PresetRecallActionState::Changed {
                projection: projection
                    .as_deref()
                    .map(super::values_wire::values_projection),
                event_sequence: values_event_sequence,
            },
            None,
            None,
        ),
        application::ProgrammingPresetRecallOutcome::PreloadChanged {
            projection,
            preload_values_event_sequence,
            ..
        } => (
            wire::PresetRecallActionState::Changed {
                projection: None,
                event_sequence: None,
            },
            projection
                .as_deref()
                .map(super::preload_values_wire::projection_from_application),
            preload_values_event_sequence,
        ),
        application::ProgrammingPresetRecallOutcome::NoChange { .. } => {
            (wire::PresetRecallActionState::NoChange, None, None)
        }
    };
    wire::PresetRecallOutcome {
        correlation_id: result.context.correlation_id,
        disposition: match result.disposition {
            application::ProgrammingPresetRecallDisposition::Recalled => {
                wire::PresetRecallDisposition::Recalled
            }
            application::ProgrammingPresetRecallDisposition::TargetsSelected => {
                wire::PresetRecallDisposition::TargetsSelected
            }
        },
        show_revision: result.preset.show_revision.value(),
        programmer_revision,
        target,
        preload_values_revision: (result.target
            == application::ProgrammingPresetRecallTarget::Preload)
            .then_some(result.preload_values_revision),
        preload_projection,
        preload_event_sequence,
        capture_mode_revision: result.capture_mode_revision,
        selection_revision: result.selection_revision,
        interaction_event_sequence: result.interaction_event_sequence,
        applied_fixtures: result.applied_fixtures as u64,
        selected_targets: result.selected_targets as u64,
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
