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

#[test]
fn release_is_one_recordable_undoable_instruction_without_removing_instance_tracks() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let intensity = AttributeKey::intensity();
    let controller = Uuid::new_v4();
    registry.start(session, UserId::new());
    registry.set_faded(
        session,
        fixture,
        intensity.clone(),
        AttributeValue::Normalized(0.7),
    );
    assert!(registry.set_group(
        session,
        "front".into(),
        intensity.clone(),
        AttributeValue::Normalized(0.5),
    ));
    assert!(registry.apply_dynamic_values(
        session,
        &[
            set(
                fixture,
                "intensity",
                DynamicSemanticValue::DynamicOff {
                    instance_link: controller,
                    timing: Default::default(),
                },
            ),
            set(
                fixture,
                "intensity",
                DynamicSemanticValue::FixAt {
                    value: 0.8,
                    timing: Default::default(),
                },
            ),
        ],
        None,
    ));
    let undo_before = registry.undo_depth(session).unwrap();

    assert!(registry.apply_release_values(
        session,
        &[ReleaseProgrammerFixtureValue {
            fixture_id: fixture,
            attribute: intensity.clone(),
        }],
        &[ReleaseProgrammerGroupValue {
            group_id: "front".into(),
            attribute: intensity.clone(),
        }],
    ));
    let released = registry.get(session).unwrap();
    assert!(released.values.is_empty());
    assert!(released.group_values.is_empty());
    assert_eq!(released.group_release_values.len(), 1);
    assert!(released.dynamic_values.iter().any(|value| matches!(
        value.value,
        DynamicSemanticValue::DynamicOff { instance_link, .. } if instance_link == controller
    )));
    assert!(
        released
            .dynamic_values
            .iter()
            .any(|value| matches!(value.value, DynamicSemanticValue::Release))
    );
    assert_eq!(registry.undo_depth(session), Some(undo_before + 1));

    let persisted: ProgrammerState =
        serde_json::from_value(serde_json::to_value(&released).unwrap()).unwrap();
    assert_eq!(persisted.group_release_values.len(), 1);
    assert!(
        persisted
            .dynamic_values
            .iter()
            .any(|value| matches!(value.value, DynamicSemanticValue::Release))
    );
    let mut legacy = serde_json::to_value(&released).unwrap();
    let legacy = legacy.as_object_mut().unwrap();
    legacy.remove("group_release_values");
    legacy.remove("preload_group_release_pending");
    legacy.remove("preload_group_release_active");
    let legacy: ProgrammerState = serde_json::from_value(legacy.clone().into()).unwrap();
    assert!(legacy.group_release_values.is_empty());
    assert!(legacy.preload_group_release_pending.is_empty());
    assert!(legacy.preload_group_release_active.is_empty());

    let capture = registry
        .capture_cue_recording(session, CueRecordingSource::CurrentCapture)
        .unwrap();
    assert_eq!(capture.group_release_values.len(), 1);
    assert!(
        capture
            .dynamic_values
            .iter()
            .any(|value| matches!(value.value, DynamicSemanticValue::Release))
    );

    assert!(registry.undo(session));
    let restored = registry.get(session).unwrap();
    assert_eq!(restored.values.len(), 1);
    assert_eq!(restored.group_values["front"].len(), 1);
    assert!(restored.group_release_values.is_empty());
    assert!(
        restored
            .dynamic_values
            .iter()
            .any(|value| matches!(value.value, DynamicSemanticValue::FixAt { .. }))
    );

    assert!(registry.redo(session));
    assert!(registry.clear_normal_values(session));
    let cleared = registry.get(session).unwrap();
    assert!(cleared.values.is_empty());
    assert!(cleared.group_values.is_empty());
    assert!(cleared.group_release_values.is_empty());
    assert!(cleared.dynamic_values.is_empty());
    assert!(!registry.clear_normal_values(session));
}
