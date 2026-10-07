use super::super::{prepare_normalized_show_candidate_incremental, prepare_show_candidate};
use super::support::document_with_objects;
use light_core::{AttributeValue, FixtureId, programming::*};
use light_dynamics::*;
use light_programmer::{Preset, PresetFamily};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

fn position(pan: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, 20.0)))
}

fn preset(target: FixtureId, pan: f32) -> Preset {
    let key = ProgrammingOwner::Position.key();
    Preset {
        family: PresetFamily::Position,
        number: 1,
        universal_values: [(key.clone(), position(pan))].into(),
        values: [(target, [(key.clone(), position(pan + 10.0))].into())].into(),
        group_values: [(
            "front".into(),
            [(
                key,
                AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
                    owner: ProgrammingOwner::Position,
                    template: AttributeValue::Position(Arc::new(PositionIntent::Angles {
                        pan_degrees: ScalarIntent::Spread(vec![pan, pan + 180.0]),
                        tilt_degrees: ScalarIntent::Value(20.0),
                    })),
                    members: [(target.0, position(pan + 30.0))].into(),
                })),
            )]
            .into(),
        )]
        .into(),
        ..Default::default()
    }
}

fn dynamic() -> Value {
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    };
    let definition = DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 7,
        revision: 1,
        name: "Retained intent".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::Targetless,
        lanes: vec![DynamicLane {
            id: Uuid::new_v4(),
            body: DynamicLaneBody::Programming(ProgrammingLaneBody {
                address: address.clone(),
                configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
                    minimum: DynamicValueSource::Current,
                    maximum: DynamicValueSource::Preset {
                        preset_id: "3.1".into(),
                        address,
                        retained: None,
                        last_valid_by_target: vec![],
                    },
                    function: PeriodicFunction::LinearUp,
                    size: 1.0,
                    pwm: PwmShape::default(),
                }),
            }),
            speed_multiplier: Rational::ONE,
            width: 1.0,
            phase: None,
            random_group_id: None,
        }],
        random_groups: vec![],
        phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: DynamicSpatialMappingOverride::default(),
        phase: PhaseDistribution {
            ordering: PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: vec![],
        },
        speed: DynamicSpeed::Fixed {
            duration_millis: 1000,
        },
        overall_speed_multiplier: Rational::ONE,
        run_mode: DynamicRunMode::Loop,
        default_activation: ActivationPolicy::StartNow,
        activation_boundary: ActivationBoundary::Beat,
    };
    let mut body = serde_json::to_value(definition).unwrap();
    body["future_definition"] = json!({"keep": [3, 1]});
    body["lanes"][0]["future_lane"] = json!({"keep": true});
    body
}

fn template(body: &Value) -> Arc<DynamicPresetTemplate> {
    let definition: DynamicDefinition = serde_json::from_value(body.clone()).unwrap();
    validate_definition(&definition).unwrap();
    let DynamicLaneBody::Programming(body) = &definition.lanes[0].body else {
        panic!()
    };
    let ProgrammingLaneConfiguration::MaxMin(config) = &body.configuration else {
        panic!()
    };
    let DynamicValueSource::Preset {
        retained: Some(template),
        ..
    } = &config.maximum
    else {
        panic!("missing retained template")
    };
    Arc::clone(template)
}

fn assert_template(body: &Value, target: FixtureId, pan: f32) {
    let actual = template(body);
    let expected = preset(target, pan);
    let key = ProgrammingOwner::Position.key();
    assert_eq!(
        actual.universal.as_ref(),
        expected.universal_values.get(&key)
    );
    assert_eq!(actual.fixtures[0].value, expected.values[&target][&key]);
    assert_eq!(actual.groups[0].value, expected.group_values["front"][&key]);
    assert_eq!(body["future_definition"], json!({"keep": [3, 1]}));
    assert_eq!(body["lanes"][0]["future_lane"], json!({"keep": true}));
}

#[test]
fn typed_preset_edit_delete_and_reopen_retain_authored_templates_losslessly() {
    let target = FixtureId::new();
    let (store, document) = document_with_objects(&[
        ("dynamic", "7", dynamic()),
        (
            "preset",
            "3.1",
            serde_json::to_value(preset(target, -720.0)).unwrap(),
        ),
    ]);
    let (transaction, initial) = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts();
    assert_template(
        document
            .candidate(&transaction)
            .unwrap()
            .object("dynamic", "7")
            .unwrap()
            .body(),
        target,
        -720.0,
    );
    assert!(
        serde_json::to_string(document.object("dynamic", "7").unwrap().body())
            .unwrap()
            .find("retained")
            .is_none()
    );
    store.apply_portable_transaction(transaction).unwrap();
    let document = store.portable_document().unwrap();
    assert!(
        prepare_show_candidate(&document, document.transaction())
            .unwrap()
            .into_parts()
            .0
            .is_empty()
    );

    // Only the live Preset was explicitly edited. Its dependent Dynamic must join the transaction.
    let mut edit = document.transaction();
    edit.put(
        "preset",
        "3.1",
        serde_json::to_value(preset(target, 450.0)).unwrap(),
    );
    let (edit, next) = prepare_normalized_show_candidate_incremental(&document, edit, &initial)
        .unwrap()
        .into_parts();
    assert!(
        edit.changed_object_keys()
            .any(|key| key.kind() == "dynamic" && key.id() == "7")
    );
    assert_template(
        document
            .candidate(&edit)
            .unwrap()
            .object("dynamic", "7")
            .unwrap()
            .body(),
        target,
        450.0,
    );
    store.apply_portable_transaction(edit).unwrap();
    let document = store.portable_document().unwrap();
    let mut delete = document.transaction();
    delete.delete("preset", "3.1");
    let (delete, next) = prepare_normalized_show_candidate_incremental(&document, delete, &next)
        .unwrap()
        .into_parts();
    assert_eq!(next.required_programming_contract(), 1);
    assert_template(
        document
            .candidate(&delete)
            .unwrap()
            .object("dynamic", "7")
            .unwrap()
            .body(),
        target,
        450.0,
    );
    store.apply_portable_transaction(delete).unwrap();

    let reopened = store.portable_document().unwrap();
    let (migration, snapshot) = prepare_show_candidate(&reopened, reopened.transaction())
        .unwrap()
        .into_parts();
    assert!(migration.is_empty());
    assert_eq!(snapshot.required_programming_contract(), 1);
    assert_template(
        reopened.object("dynamic", "7").unwrap().body(),
        target,
        450.0,
    );
}

#[test]
fn deletion_captures_latest_source_even_before_first_retention_and_preserves_undo_body() {
    let target = FixtureId::new();
    let original = dynamic();
    let (store, document) = document_with_objects(&[
        ("dynamic", "7", original.clone()),
        (
            "preset",
            "3.1",
            serde_json::to_value(preset(target, 90.0)).unwrap(),
        ),
    ]);
    let mut delete = document.transaction();
    delete.delete("preset", "3.1");
    let (delete, _) = prepare_show_candidate(&document, delete)
        .unwrap()
        .into_parts();
    assert_template(
        document
            .candidate(&delete)
            .unwrap()
            .object("dynamic", "7")
            .unwrap()
            .body(),
        target,
        90.0,
    );
    store.apply_portable_transaction(delete).unwrap();
    let document = store.portable_document().unwrap();
    let mut undo = document.transaction();
    undo.put("dynamic", "7", original.clone());
    let (undo, _) =
        super::super::prepare_show_candidate_preserving_object(&document, undo, "dynamic", "7")
            .unwrap()
            .into_parts();
    assert_eq!(
        document
            .candidate(&undo)
            .unwrap()
            .object("dynamic", "7")
            .unwrap()
            .body(),
        &original
    );
}

#[test]
fn unrelated_preset_edit_does_not_recompile_a_source_with_an_omitted_empty_fallback() {
    let mut body = dynamic();
    body.pointer_mut("/lanes/0/programming/configuration/configuration/maximum")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("last_valid_by_target");
    let (_, document) = document_with_objects(&[("dynamic", "7", body.clone())]);
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut edit = document.transaction();
    let mut unrelated = preset(FixtureId::new(), 90.0);
    unrelated.number = 2;
    edit.put("preset", "3.2", serde_json::to_value(unrelated).unwrap());
    let (edit, _) = prepare_normalized_show_candidate_incremental(&document, edit, &previous)
        .unwrap()
        .into_parts();
    assert!(
        !edit
            .changed_object_keys()
            .any(|key| key.kind() == "dynamic")
    );
    assert_eq!(
        document
            .candidate(&edit)
            .unwrap()
            .object("dynamic", "7")
            .unwrap()
            .body(),
        &body
    );
}

#[test]
fn deleting_a_malformed_preset_does_not_fabricate_a_last_valid_source() {
    let target = FixtureId::new();
    let mut invalid = preset(target, 90.0);
    invalid.values.get_mut(&target).unwrap().insert(
        light_core::AttributeKey::intensity(),
        AttributeValue::Spread(vec![0.0, 1.0]),
    );
    assert!(invalid.validate_programming().is_err());
    let mut original = dynamic();
    // Keep this fallback test independent of the explicit Current partner migration.
    let normalized: DynamicDefinition = serde_json::from_value(original.clone()).unwrap();
    original["lanes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::to_value(&normalized.lanes[1]).unwrap());
    let (_, document) = document_with_objects(&[
        ("dynamic", "7", original.clone()),
        ("preset", "3.1", serde_json::to_value(invalid).unwrap()),
    ]);
    let mut delete = document.transaction();
    delete.delete("preset", "3.1");
    let (delete, _) = prepare_show_candidate(&document, delete)
        .unwrap()
        .into_parts();
    assert_eq!(
        document
            .candidate(&delete)
            .unwrap()
            .object("dynamic", "7")
            .unwrap()
            .body(),
        &original
    );
}

#[test]
fn incompatible_preset_edit_and_delete_retain_the_previous_group_angle_template() {
    let target = FixtureId::new();
    let (store, document) = document_with_objects(&[
        ("dynamic", "7", dynamic()),
        (
            "preset",
            "3.1",
            serde_json::to_value(preset(target, -180.0)).unwrap(),
        ),
    ]);
    let (initial, snapshot) = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts();
    store.apply_portable_transaction(initial).unwrap();
    let document = store.portable_document().unwrap();
    let point = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [0.0, 5.0, 0.0],
    )));
    let changed = Preset {
        family: PresetFamily::Position,
        number: 1,
        group_values: [(
            "front".into(),
            [(ProgrammingOwner::Position.key(), point.clone())].into(),
        )]
        .into(),
        ..Default::default()
    };
    let mut edit = document.transaction();
    edit.put("preset", "3.1", serde_json::to_value(changed).unwrap());
    let (edit, snapshot) =
        prepare_normalized_show_candidate_incremental(&document, edit, &snapshot)
            .unwrap()
            .into_parts();
    store.apply_portable_transaction(edit).unwrap();
    let document = store.portable_document().unwrap();
    let mut delete = document.transaction();
    delete.delete("preset", "3.1");
    let (delete, _) = prepare_normalized_show_candidate_incremental(&document, delete, &snapshot)
        .unwrap()
        .into_parts();
    let retained = template(
        document
            .candidate(&delete)
            .unwrap()
            .object("dynamic", "7")
            .unwrap()
            .body(),
    );
    assert_eq!(retained.groups[0].value, point);
    let fallback = retained.fallback.as_ref().unwrap();
    assert!(
        fallback.universal.is_none() && fallback.fixtures.is_empty(),
        "removed scopes must not reappear"
    );
    assert_eq!(
        fallback.groups[0].value,
        preset(target, -180.0).group_values["front"][&ProgrammingOwner::Position.key()]
    );
    assert!(fallback.fallback.is_none());
}
