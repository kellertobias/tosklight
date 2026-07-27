use super::*;
use light_dynamics::{DynamicSemanticValue, DynamicValueTiming};
use uuid::Uuid;

fn set(
    fixture_id: FixtureId,
    attribute: &str,
    value: DynamicSemanticValue,
) -> DynamicProgrammerValueMutation {
    DynamicProgrammerValueMutation::Set {
        fixture_id,
        attribute: AttributeKey(attribute.into()),
        value,
    }
}

#[test]
fn dynamic_tracks_are_independent_atomic_and_undoable() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let user = UserId::new();
    let fixture = FixtureId::new();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    registry.start(session, user);

    let mutations = [
        set(
            fixture,
            "pan",
            DynamicSemanticValue::DynamicOff {
                instance_link: first,
                timing: DynamicValueTiming::default(),
            },
        ),
        set(
            fixture,
            "pan",
            DynamicSemanticValue::DynamicOff {
                instance_link: second,
                timing: DynamicValueTiming {
                    fade_millis: Some(250),
                    delay_millis: None,
                },
            },
        ),
        set(
            fixture,
            "pan",
            DynamicSemanticValue::FixAt {
                value: 0.75,
                timing: DynamicValueTiming::default(),
            },
        ),
    ];
    assert!(registry.apply_dynamic_values(session, &mutations, None));
    let state = registry.get(session).unwrap();
    assert_eq!(state.dynamic_values.len(), 3);
    assert_eq!(state.undo.len(), 1);
    assert_eq!(registry.normal_values_generation(session), Some(1));
    let update = registry.capture_update_values(session).unwrap();
    assert_eq!(
        update
            .values
            .iter()
            .filter(|value| matches!(value, ProgrammerUpdateValue::Dynamic(_)))
            .count(),
        3,
        "Shift+Record Update must retain Dynamic and FAT values"
    );
    assert_eq!(update.content().dynamic_values.len(), 3);

    assert!(!registry.apply_dynamic_values(session, &mutations, None));
    assert_eq!(registry.get(session).unwrap().undo.len(), 1);
    assert!(registry.undo(session));
    assert!(registry.get(session).unwrap().dynamic_values.is_empty());
    assert!(registry.redo(session));
    assert_eq!(registry.get(session).unwrap().dynamic_values.len(), 3);
}

#[test]
fn preload_go_moves_dynamic_values_atomically_and_recording_selects_the_right_layer() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    assert!(registry.apply_dynamic_values(
        session,
        &[set(
            fixture,
            "intensity",
            DynamicSemanticValue::FixAt {
                value: 0.4,
                timing: DynamicValueTiming::default(),
            },
        )],
        None,
    ));
    assert!(registry.arm_preload(session, true));
    assert!(registry.apply_dynamic_values(
        session,
        &[set(
            fixture,
            "intensity",
            DynamicSemanticValue::FixAt {
                value: 0.8,
                timing: DynamicValueTiming {
                    fade_millis: Some(500),
                    delay_millis: Some(50),
                },
            },
        )],
        None,
    ));

    let pending = registry
        .capture_cue_recording(session, CueRecordingSource::CurrentCapture)
        .unwrap();
    assert_eq!(pending.source, CueRecordingCapturedSource::PendingPreload);
    assert!(matches!(
        pending.dynamic_values[0].value,
        DynamicSemanticValue::FixAt { value: 0.8, .. }
    ));

    assert!(registry.activate_preload(session));
    let state = registry.get(session).unwrap();
    assert!(state.preload_dynamic_pending.is_empty());
    assert_eq!(state.preload_dynamic_active.len(), 1);
    let active = registry
        .capture_cue_recording(session, CueRecordingSource::PreloadPendingOrActive)
        .unwrap();
    assert_eq!(active.source, CueRecordingCapturedSource::ActivePreload);
    assert!(matches!(
        active.dynamic_values[0].value,
        DynamicSemanticValue::FixAt { value: 0.8, .. }
    ));
}
