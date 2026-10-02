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
fn identical_fix_at_preserves_an_open_selection_gesture_and_undo() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session);
    let mutation = set(
        fixture,
        "pan",
        DynamicSemanticValue::FixAt {
            value: 0.5,
            timing: Default::default(),
        },
    );
    assert!(registry.apply_dynamic_values(session, std::slice::from_ref(&mutation), None));
    registry.apply_selection_gesture(
        session,
        vec![crate::SelectionReference::Fixture {
            fixture_id: fixture,
        }],
        &Default::default(),
    );
    let before = registry.selection(session).unwrap();
    let depth = registry.undo_depth(session);
    assert!(before.gesture_open);
    assert!(!registry.apply_dynamic_values(session, &[mutation], None));
    assert_eq!(registry.selection(session).unwrap(), before);
    assert_eq!(registry.undo_depth(session), depth);
}

#[test]
fn dynamic_tracks_are_independent_atomic_and_undoable() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    registry.start(session);

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
    registry.start(session);
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
    registry.start(session);
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

fn lane_on(instance: Uuid, lane: u128) -> light_dynamics::DynamicSemanticValue {
    serde_json::from_value(serde_json::json!({
        "type": "dynamic_on", "instance_link": instance, "lane_id": Uuid::from_u128(lane),
        "dynamic": { "dynamic_id": null, "last_known_pool_number": 1,
            "embedded_fallback": { "definition": {
                "id": Uuid::from_u128(100), "pool_number": 1, "revision": 1, "name": "Position",
                "target_binding": {"type":"targetless"}, "lanes": ([1, 2].map(|id| serde_json::json!({
                    "id": Uuid::from_u128(id), "speed_multiplier": {"numerator":1,"denominator":1}, "width":1.0,
                    "programming": {"address":{"representation":{"kind":"angles"},"component": {"kind": if id == 1 { "pan" } else { "tilt" }}},
                        "configuration":{"mode":"keyframes","configuration":{"points":[
                            {"position":0.0,"source":{"kind":"value","value":{"kind":"scalar","value":45.0}},"interpolation":"linear"}
                        ],"size":1.0}}}
                }))),
                "phase": {"ordering":{"type":"selection"},"offset_degrees":0,"span_degrees":360,"block_size":1,"repeats":1,"wings":false},
                "speed":{"type":"fixed","duration_millis":1000},"default_activation":"start_now"
            }}
        }, "overrides":{"size":1.0,"speed_multiplier":{"numerator":1,"denominator":1},"phase_offset_degrees":0},
        "timing":{}
    })).unwrap()
}

#[test]
fn same_owner_lanes_survive_storage_preload_record_and_whole_instance_off() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixtures = [FixtureId::new(), FixtureId::new()];
    let instance = Uuid::new_v4();
    registry.start(session);
    let values = [
        set(fixtures[0], "position", lane_on(instance, 1)),
        set(fixtures[0], "position", lane_on(instance, 2)),
        set(fixtures[1], "position", lane_on(instance, 1)),
    ];
    assert!(registry.apply_dynamic_values(session, &values, None));
    assert_eq!(registry.get(session).unwrap().dynamic_values.len(), 3);
    assert!(!registry.apply_dynamic_values(session, &values, None));
    let stored = registry.capture_update_values(session).unwrap();
    assert_eq!(stored.content().dynamic_values.len(), 3);
    let state = registry.get(session).unwrap();
    let loaded: ProgrammerState =
        serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
    assert_eq!(loaded.dynamic_values, state.dynamic_values);
    assert!(registry.arm_preload(session, true));
    assert!(registry.apply_dynamic_values(session, &values, None));
    assert!(registry.activate_preload(session));
    assert_eq!(
        registry.get(session).unwrap().preload_dynamic_active.len(),
        3
    );
    let off = set(
        fixtures[0],
        "position",
        DynamicSemanticValue::DynamicOff {
            instance_link: instance,
            timing: Default::default(),
        },
    );
    assert!(registry.apply_dynamic_values(session, &[off], None));
    assert_eq!(registry.get(session).unwrap().dynamic_values.len(), 1);
    assert!(registry.undo(session));
    assert_eq!(registry.get(session).unwrap().dynamic_values.len(), 3);
}
