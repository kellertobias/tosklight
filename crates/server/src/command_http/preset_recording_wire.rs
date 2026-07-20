use light_application as application;
use light_programmer::{PresetAddress, PresetFamily, PresetStoreMode};
use light_wire::v2::preset_recording as wire;

pub(super) fn address(value: wire::PresetRecordingAddress) -> Result<PresetAddress, String> {
    PresetAddress::new(family(value.family), value.number)
}

pub(super) const fn mode(value: wire::PresetRecordingMode) -> PresetStoreMode {
    match value {
        wire::PresetRecordingMode::Merge => PresetStoreMode::Merge,
        wire::PresetRecordingMode::Overwrite => PresetStoreMode::Overwrite,
    }
}

pub(super) fn outcome(
    result: application::ProgrammingPresetRecordResult,
) -> wire::PresetRecordOutcome {
    let projection = result.outcome.projection();
    let preset = wire::RecordedPresetProjection {
        id: projection.object_id.clone(),
        revision: projection.object_revision,
        body: projection.raw_body.as_ref().clone(),
    };
    let show_revision = result.outcome.show_revision().value();
    match result.outcome.event_sequence() {
        Some(event_sequence) => wire::PresetRecordOutcome::Changed {
            request_id: result.request_id,
            correlation_id: result.context.correlation_id,
            replayed: result.replayed,
            show_revision,
            preset,
            event_sequence,
        },
        None => wire::PresetRecordOutcome::NoChange {
            request_id: result.request_id,
            correlation_id: result.context.correlation_id,
            replayed: result.replayed,
            show_revision,
            preset,
        },
    }
}

const fn family(value: wire::PresetRecordingFamily) -> PresetFamily {
    match value {
        wire::PresetRecordingFamily::Mixed => PresetFamily::Mixed,
        wire::PresetRecordingFamily::Intensity => PresetFamily::Intensity,
        wire::PresetRecordingFamily::Color => PresetFamily::Color,
        wire::PresetRecordingFamily::Position => PresetFamily::Position,
        wire::PresetRecordingFamily::Beam => PresetFamily::Beam,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_rejects_zero_and_keeps_family() {
        assert!(
            address(wire::PresetRecordingAddress {
                family: wire::PresetRecordingFamily::Color,
                number: 0,
            })
            .is_err()
        );
        assert_eq!(
            address(wire::PresetRecordingAddress {
                family: wire::PresetRecordingFamily::Position,
                number: 9,
            })
            .unwrap()
            .storage_key(),
            "3.9"
        );
    }

    #[test]
    fn transport_exposes_only_supported_record_modes() {
        assert_eq!(
            mode(wire::PresetRecordingMode::Merge),
            PresetStoreMode::Merge
        );
        assert_eq!(
            mode(wire::PresetRecordingMode::Overwrite),
            PresetStoreMode::Overwrite
        );
    }
}
