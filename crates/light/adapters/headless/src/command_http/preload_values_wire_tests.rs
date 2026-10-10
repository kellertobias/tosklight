use super::*;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn captured_preload_wire_contains_exact_retained_fallback_without_normal_expansion() {
    let registry = light_programmer::ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    registry.start(session);
    assert!(registry.arm_preload(session, true));
    let mut retained = test_definition(Uuid::new_v4(), 100);
    retained.revision = 5;
    let retained = Arc::new(retained);
    let fixture = light_core::FixtureId::new();
    assert!(registry.apply_dynamic_values(
        session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: light_core::AttributeKey::intensity(),
            value: light_dynamics::DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::new_v4(),
                dynamic: light_dynamics::DynamicReference {
                    dynamic_id: Some(retained.id),
                    last_known_pool_number: retained.pool_number,
                    embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                        definition: retained.clone()
                    },
                },
                lane_id: retained.lanes[0].id,
                overrides: light_dynamics::DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: light_dynamics::Rational::ONE,
                    phase_offset_degrees: 0.0
                },
                timing: light_dynamics::DynamicValueTiming::default(),
            },
        }],
        None
    ));
    let captured = registry.preload_pending_values(session).unwrap();
    let application = application::ProgrammingPreloadValuesProjection {
        revision: registry.preload_values_revision(),
        fixture_values: captured.fixture_values,
        group_values: captured.group_values,
        dynamic_values: captured.dynamic_values,
        group_release_values: captured.group_release_values,
    };
    let before = registry.get(session).unwrap();
    let json: serde_json::Value = serde_json::from_str(
        &serde_json::to_string(&projection_from_application(&application)).unwrap(),
    )
    .unwrap();
    let reference = &json["dynamic_values"][0]["value"]["dynamic"];
    assert_eq!(reference["embedded_fallback_id"], retained.id.to_string());
    assert_eq!(reference["embedded_fallback_revision"], 5);
    assert_eq!(
        reference["embedded_fallback"],
        serde_json::to_value(super::super::dynamics_wire::definition(&retained)).unwrap()
    );
    // This retained snapshot remains revision5 even if a later library version differs.
    let mut later = (*retained).clone();
    later.revision = 6;
    later.name = "Different later definition".into();
    assert_ne!(
        reference["embedded_fallback"],
        serde_json::to_value(super::super::dynamics_wire::definition(&later)).unwrap()
    );
    let normal = serde_json::to_value(super::super::dynamics_wire::programming_value(
        &application.dynamic_values[0],
    ))
    .unwrap();
    assert!(
        normal["value"]["dynamic"]
            .get("embedded_fallback")
            .is_none()
    );
    assert_eq!(
        serde_json::to_value(registry.get(session).unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    println!(
        "PRELOAD_DYNAMIC_WIRE={}",
        serde_json::to_string(&json).unwrap()
    );
}

#[test]
fn preload_non_on_dynamic_values_keep_shared_wire_encoding() {
    let row = light_dynamics::DynamicAddressValue {
        fixture_id: light_core::FixtureId::new(),
        attribute: light_core::AttributeKey::intensity(),
        value: light_dynamics::DynamicSemanticValue::DynamicOff {
            instance_link: Uuid::new_v4(),
            timing: light_dynamics::DynamicValueTiming::default(),
        },
        programmer_order: 2,
        changed_at_millis: 50,
    };
    let application = application::ProgrammingPreloadValuesProjection {
        revision: 8,
        fixture_values: vec![],
        group_values: vec![],
        dynamic_values: vec![row.clone()],
        group_release_values: vec![],
    };
    let encoded = projection_from_application(&application);
    assert_eq!(
        encoded.dynamic_values[0],
        super::super::dynamics_wire::programming_value(&row)
    );
}

fn test_definition(id: uuid::Uuid, pool_number: u16) -> light_dynamics::DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "pool_number": pool_number,
        "revision": 1,
        "name": "Command test wave",
        "color": null,
        "icon": null,
        "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": uuid::Uuid::new_v4(),
            "attribute": "intensity",
            "mode": "keyframes",
            "keyframes": {
                "points": [
                    {"position": 0.0, "source": {"type": "value", "value": 0.25}, "interpolation": "linear"},
                    {"position": 0.5, "source": {"type": "value", "value": 0.75}, "interpolation": "linear"}
                ],
                "size": 1.0
            },
            "max_min": {
                "minimum": {"type": "value", "value": 0.25},
                "maximum": {"type": "value", "value": 0.75},
                "function": "sinus",
                "size": 1.0,
                "pwm": {
                    "attack": 0.0, "on": 0.5, "decay": 0.0, "off": 0.5,
                    "attack_interpolation": "linear", "decay_interpolation": "linear"
                }
            },
            "middle_amplitude": {
                "middle": {"type": "current"},
                "amplitude": 0.25,
                "function": "sinus",
                "size": 1.0,
                "pwm": {
                    "attack": 0.0, "on": 0.5, "decay": 0.0, "off": 0.5,
                    "attack_interpolation": "linear", "decay_interpolation": "linear"
                }
            },
            "speed_multiplier": {"numerator": 1, "denominator": 1},
            "width": 1.0,
            "random_group_id": null
        }],
        "random_groups": [],
        "phase": {
            "ordering": {"type": "selection"},
            "offset_degrees": 0.0,
            "span_degrees": 360.0,
            "block_size": 1,
            "repeats": 1,
            "wings": false,
            "anchors_degrees": []
        },
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    }))
    .unwrap()
}
