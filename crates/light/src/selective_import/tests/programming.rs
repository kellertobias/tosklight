use super::support::*;
use crate::selective_import::*;
use light_core::{AttributeValue, programming::*};
use light_dynamics::*;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

fn target(point: Uuid) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point { point_id: point },
        [1.0, 2.0, 3.0],
    )))
}

fn definition(id: Uuid, address: DynamicValueAddress, source: DynamicValueSource) -> Value {
    let mut body = dynamic_with_dependencies(id, "unused", "unused");
    body["target_binding"] = json!({"type":"targetless"});
    body["lanes"] = json!([{
        "id": Uuid::new_v4(),
        "programming": {
            "address": address,
            "configuration": {"mode":"keyframes","configuration":{
                "points": [
                    {"position":0.0,"source":source,"interpolation":"linear"},
                    {"position":0.5,"source":{"kind":"current"},"interpolation":"linear"}
                ], "size":1.0
            }}
        },
        "speed_multiplier":{"numerator":1,"denominator":1}, "width":1.0
    }]);
    body
}

#[test]
fn typed_dynamic_import_remaps_point_preset_fallback_and_nested_group_member_keys() {
    let rig = TestRig::new();
    let fixture = portable_fixture_record(310_000, 1);
    let point = portable_fixture_record(320_000, 2);
    for record in [&fixture, &point] {
        rig.source_profile(&record.profile);
        rig.source_object(
            "patched_fixture",
            &record.fixture_id.0.to_string(),
            record.body.clone(),
        );
    }
    rig.source_object(
        "group",
        "front",
        serde_json::to_value(light_programmer::GroupDefinition {
            id: "front".into(),
            name: "Front".into(),
            ..Default::default()
        })
        .unwrap(),
    );
    let family = target(point.fixture_id.0);
    let group = AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
        owner: ProgrammingOwner::Position,
        template: family.clone(),
        members: [(fixture.fixture_id.0, family.clone())].into(),
    }));
    rig.source_object(
        "preset",
        "3.1",
        json!({
            "family":"Position", "number":1, "name":"Stage center",
            "values": {fixture.fixture_id.0.to_string(): {"position":family}},
            "group_values":{"front":{"position":group}},
            "universal_values":{"position":family},
            "future":{"point_id":point.fixture_id.0}
        }),
    );
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Target {
            reference: Some(TargetReference::Point {
                point_id: point.fixture_id.0,
            }),
        },
        component: None,
    };
    let retained = DynamicPresetTemplate {
        universal: Some(family.clone()),
        groups: vec![DynamicPresetGroupTemplate {
            group_id: "front".into(),
            value: group.clone(),
        }],
        fixtures: vec![DynamicPresetFixtureTemplate {
            fixture_id: fixture.fixture_id,
            value: family.clone(),
        }],
        fallback: Some(Box::new(DynamicPresetTemplate {
            universal: Some(family.clone()),
            ..Default::default()
        })),
    };
    let source = DynamicValueSource::Preset {
        preset_id: "3.1".into(),
        address: address.clone(),
        last_valid_by_target: vec![DynamicValueFallback {
            target: fixture.fixture_id,
            value: DynamicValue::Family(family.clone()),
        }],
        retained: Some(Arc::new(retained)),
    };
    let id = Uuid::new_v4();
    rig.source_object("dynamic", &id.to_string(), definition(id, address, source));
    let preview = rig.preview(
        rig.request("dynamic", &id.to_string())
            .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    for expected in [
        key("preset", "3.1"),
        key("group", "front"),
        key("patched_fixture", &point.fixture_id.0.to_string()),
        key("patched_fixture", &fixture.fixture_id.0.to_string()),
    ] {
        assert!(
            preview
                .dependencies
                .iter()
                .any(|dependency| dependency.dependency == expected)
        );
    }
    rig.apply(&preview).unwrap();
    let destination = |kind: &str, source: &str| {
        preview
            .objects
            .iter()
            .find(|entry| entry.source == key(kind, source))
            .unwrap()
            .destination
            .id()
            .to_owned()
    };
    let point_id = destination("patched_fixture", &point.fixture_id.0.to_string());
    let fixture_id = destination("patched_fixture", &fixture.fixture_id.0.to_string());
    let group_id = destination("group", "front");
    let preset_id = destination("preset", "3.1");
    assert_ne!(point_id, point.fixture_id.0.to_string());
    let document = rig.target_document();
    let dynamic = document
        .object("dynamic", &destination("dynamic", &id.to_string()))
        .unwrap()
        .body();
    let decoded: DynamicDefinition = serde_json::from_value(dynamic.clone()).unwrap();
    validate_definition(&decoded).unwrap();
    let lane = "/lanes/0/programming";
    let source = format!("{lane}/configuration/configuration/points/0/source");
    assert_eq!(
        dynamic
            .pointer(&format!("{lane}/address/representation/reference/point_id"))
            .unwrap(),
        &json!(point_id)
    );
    assert_eq!(
        dynamic.pointer(&format!("{source}/preset_id")).unwrap(),
        &json!(preset_id)
    );
    assert_eq!(
        dynamic
            .pointer(&format!("{source}/last_valid_by_target/0/target"))
            .unwrap(),
        &json!(fixture_id)
    );
    for suffix in [
        "/last_valid_by_target/0/value/value/value/reference/point_id",
        "/retained/universal/value/reference/point_id",
        "/retained/fallback/universal/value/reference/point_id",
        "/retained/fixtures/0/value/value/reference/point_id",
    ] {
        assert_eq!(
            dynamic.pointer(&format!("{source}{suffix}")).unwrap(),
            &json!(point_id)
        );
    }
    assert_eq!(
        dynamic
            .pointer(&format!("{source}/retained/groups/0/group_id"))
            .unwrap(),
        &json!(group_id)
    );
    assert_eq!(dynamic.pointer(&format!("{source}/retained/groups/0/value/value/members/{fixture_id}/value/reference/point_id")).unwrap(), &json!(point_id));
    let preset = document.object("preset", &preset_id).unwrap().body();
    assert_eq!(
        preset
            .pointer(&format!(
                "/values/{fixture_id}/position/value/reference/point_id"
            ))
            .unwrap(),
        &json!(point_id)
    );
    assert_eq!(preset.pointer(&format!("/group_values/{group_id}/position/value/members/{fixture_id}/value/reference/point_id")).unwrap(), &json!(point_id));
    assert_eq!(preset["future"]["point_id"], json!(point.fixture_id.0));
}

#[test]
fn deleted_typed_preset_with_retained_value_imports_without_inventing_a_live_dependency() {
    let rig = TestRig::new();
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    };
    let source = DynamicValueSource::Preset {
        preset_id: "3.99".into(),
        address: address.clone(),
        last_valid_by_target: vec![],
        retained: Some(Arc::new(DynamicPresetTemplate {
            universal: Some(AttributeValue::Position(Arc::new(PositionIntent::angles(
                90.0, 30.0,
            )))),
            ..Default::default()
        })),
    };
    let id = Uuid::new_v4();
    let definition: DynamicDefinition =
        serde_json::from_value(definition(id, address, source)).unwrap();
    let mut raw = serde_json::to_value(&definition).unwrap();
    raw["lanes"][1]["future_partner"] = json!({"keep":true});
    rig.source_object("dynamic", &id.to_string(), raw);
    let preview = rig.preview(
        rig.request("dynamic", &id.to_string())
            .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert!(preview.dependencies.is_empty());
    rig.apply(&preview).unwrap();
    let destination = &preview.objects[0].destination;
    let document = rig.target_document();
    let raw = document
        .object(destination.kind(), destination.id())
        .unwrap()
        .body();
    assert_eq!(raw["lanes"][1]["future_partner"]["keep"], true);
    let mut copied: DynamicDefinition = serde_json::from_value(raw.clone()).unwrap();
    validate_definition(&copied).unwrap();
    assert!(copied.is_automatic_angle_partner(&copied.lanes[1]));
    let DynamicLaneBody::Programming(lane) = &copied.lanes[0].body else {
        panic!()
    };
    let ProgrammingLaneConfiguration::Keyframes(config) = &lane.configuration else {
        panic!()
    };
    let DynamicValueSource::Preset {
        preset_id,
        retained,
        ..
    } = &config.points[0].source
    else {
        panic!()
    };
    assert_eq!(preset_id, "3.99");
    assert!(retained.as_ref().unwrap().universal.is_some());
    copied.lanes.remove(0);
    copied.normalize_angle_pair();
    assert!(copied.lanes.is_empty());
}

#[test]
fn numeric_typed_sources_including_random_import_live_preset_dependencies() {
    for mode in ["max_min", "middle_amplitude", "random"] {
        let rig = TestRig::new();
        rig.source_object("preset", "2.5", json!({
            "family":"Color", "number":5, "name":"UV",
            "universal_values":{"color":AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                intent:ColorIntent { uv:UvIntent { amount:0.8 }, ..Default::default() }
            }))}
        }));
        let address = DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Retain,
            },
            component: Some(ProgrammingComponent::Color(ColorComponent::Uv)),
        };
        let source = DynamicValueSource::Preset {
            preset_id: "2.5".into(),
            address: address.clone(),
            last_valid_by_target: vec![],
            retained: None,
        };
        let id = Uuid::new_v4();
        let mut body = definition(id, address, source.clone());
        let pwm = PwmShape::default();
        body["lanes"][0]["programming"]["configuration"] = match mode {
            "max_min" => json!({"mode":mode,"configuration":{
                "minimum":source, "maximum":source, "function":"sinus", "size":1.0,"pwm":pwm
            }}),
            "middle_amplitude" => json!({"mode":mode,"configuration":{
                "middle":source,"amplitude":{"kind":"scalar","value":0.2},
                "function":"sinus","size":1.0,"pwm":pwm,"invert_waveform":false
            }}),
            _ => {
                let group_id = Uuid::new_v4();
                body["lanes"][0]["random_group_id"] = json!(group_id);
                body["random_groups"] = json!([DynamicRandomGroup {
                    id: group_id,
                    seed: 7,
                    range: DynamicRandomRange::Programming {
                        low: source.clone(),
                        high: source
                    },
                    decision_interval_millis: 500,
                    start_probability: 0.5,
                    mean_duration_millis: 300,
                    duration_spread_millis: 0,
                    attack_ratio: 0.2,
                    decay_ratio: 0.2,
                }]);
                json!({"mode":"random"})
            }
        };
        rig.source_object("dynamic", &id.to_string(), body);
        let preview = rig.preview(
            rig.request("dynamic", &id.to_string())
                .with_mode(ImportLoadMode::AddToEnd),
        );
        assert!(preview.can_apply(), "{mode}: {:?}", preview.blockers);
        assert!(
            preview
                .dependencies
                .iter()
                .any(|dependency| dependency.dependency == key("preset", "2.5")),
            "{mode}"
        );
        rig.apply(&preview).unwrap();
        let document = rig.target_document();
        let destination = |kind: &str| {
            preview
                .objects
                .iter()
                .find(|entry| entry.source.kind() == kind)
                .unwrap()
                .destination
                .id()
        };
        let body = document
            .object("dynamic", destination("dynamic"))
            .unwrap()
            .body();
        let decoded: DynamicDefinition = serde_json::from_value(body.clone()).unwrap();
        validate_definition(&decoded).unwrap();
        let paths = match mode {
            "max_min" => vec![
                "/lanes/0/programming/configuration/configuration/minimum",
                "/lanes/0/programming/configuration/configuration/maximum",
            ],
            "middle_amplitude" => vec!["/lanes/0/programming/configuration/configuration/middle"],
            _ => vec![
                "/random_groups/0/programming_range/low",
                "/random_groups/0/programming_range/high",
            ],
        };
        for path in paths {
            assert_eq!(
                body.pointer(&format!("{path}/preset_id")).unwrap(),
                &json!(destination("preset"))
            );
        }
    }
}

#[test]
fn cue_and_playback_import_keep_generated_lane_references_with_the_duplicated_dynamic() {
    let rig = TestRig::new();
    let fixture = portable_fixture_record(330_000, 1);
    rig.source_profile(&fixture.profile);
    rig.source_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body.clone(),
    );
    rig.source_object("preset", "3.8", json!({
        "family":"Position", "number":8, "name":"Pan",
        "universal_values":{"position":AttributeValue::Position(Arc::new(PositionIntent::angles(45.0, 20.0)))}
    }));
    let id = Uuid::new_v4();
    let cue_id = Uuid::new_v4();
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    };
    let source = DynamicValueSource::Preset {
        preset_id: "3.8".into(),
        address: address.clone(),
        last_valid_by_target: vec![],
        retained: None,
    };
    let mut definition: DynamicDefinition =
        serde_json::from_value(definition(id, address, source)).unwrap();
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![fixture.fixture_id],
    };
    let partner = definition.lanes[1].id;
    let reference = DynamicReference {
        dynamic_id: Some(id),
        last_known_pool_number: 7,
        embedded_fallback: DynamicDefinitionSnapshot {
            definition: Arc::new(definition.clone()),
        },
    };
    let mut fallback = serde_json::to_value(&reference).unwrap();
    fallback["embedded_fallback"]["definition"]["lanes"][1]["future_partner"] =
        json!({"keep":true});
    rig.source_object(
        "dynamic",
        &id.to_string(),
        serde_json::to_value(definition).unwrap(),
    );
    rig.source_object(
        "cue_list",
        &cue_id.to_string(),
        json!({
            "id":cue_id,"name":"Angles","priority":10,"mode":"sequence","looped":false,
            "cues":[{"id":Uuid::new_v4(),"number":"1","name":"Pan and Current Tilt",
                "changes":[],"fade_millis":0,"delay_millis":0,"trigger":{"type":"manual"},
                "dynamic_changes":[{"fixture_id":fixture.fixture_id,"attribute":"position",
                    "value":{"type":"dynamic_on","instance_link":Uuid::new_v4(),"dynamic":fallback,
                    "lane_id":partner,"overrides":DynamicInstanceOverrides {
                        size:1.0, speed_multiplier:Rational { numerator:1, denominator:1 }, phase_offset_degrees:0.0
                    },"timing":{}}
                }]
            }]
        }),
    );
    rig.source_object(
        "playback",
        "1",
        json!({
                "number":1,"name":"Angles","target":{"type":"dynamic","assignment":{
                "dynamic":fallback
            }}
        }),
    );
    let preview = rig.preview(
        SelectiveShowImportRequest::new(
            rig.source_id,
            rig.target_id,
            [key("cue_list", &cue_id.to_string()), key("playback", "1")],
        )
        .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    rig.apply(&preview).unwrap();
    let destination = |kind: &str| {
        preview
            .objects
            .iter()
            .find(|entry| entry.source.kind() == kind)
            .unwrap()
            .destination
            .id()
    };
    let document = rig.target_document();
    let live = document
        .object("dynamic", destination("dynamic"))
        .unwrap()
        .body();
    let cue = document
        .object("cue_list", destination("cue_list"))
        .unwrap()
        .body();
    let playback = document
        .object("playback", destination("playback"))
        .unwrap()
        .body();
    let generated = &live["lanes"][1]["id"];
    assert_ne!(generated, &json!(partner));
    assert_eq!(
        cue["cues"][0]["dynamic_changes"][0]["value"]["lane_id"],
        *generated
    );
    for reference in [
        cue.pointer("/cues/0/dynamic_changes/0/value/dynamic")
            .unwrap(),
        playback.pointer("/target/assignment/dynamic").unwrap(),
    ] {
        assert_eq!(reference["dynamic_id"], live["id"]);
        let fallback = &reference["embedded_fallback"]["definition"];
        assert_eq!(fallback["id"], live["id"]);
        assert_eq!(&fallback["lanes"][1]["id"], generated);
        assert_eq!(fallback["lanes"][1]["future_partner"]["keep"], true);
        assert_eq!(
            fallback
                .pointer(
                    "/lanes/0/programming/configuration/configuration/points/0/source/preset_id"
                )
                .unwrap(),
            &json!(destination("preset"))
        );
        validate_definition(
            &serde_json::from_value::<DynamicDefinition>(fallback.clone()).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn targetless_playback_scope_migration_reidentifies_the_required_current_partner() {
    let rig = TestRig::new();
    let fixture = portable_fixture_record(340_000, 1);
    rig.source_profile(&fixture.profile);
    rig.source_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body.clone(),
    );
    let original_id = Uuid::new_v4();
    let definition: DynamicDefinition = serde_json::from_value(definition(
        original_id,
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        },
        DynamicValueSource::Value {
            value: DynamicValue::Scalar(45.0),
        },
    ))
    .unwrap();
    rig.source_object("playback", "1", json!({
        "number":1,"name":"Targetless scope","target":{"type":"dynamic","assignment":{
            "dynamic":DynamicReference { dynamic_id:None,last_known_pool_number:7,
                embedded_fallback:DynamicDefinitionSnapshot { definition:Arc::new(definition) } },
            "target_scope":{"type":"frozen_targets","targets":[fixture.fixture_id]}
        }}
    }));
    let preview = rig.preview(rig.request("playback", "1"));
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    rig.apply(&preview).unwrap();
    let document = rig.target_document();
    let raw = document.object("playback", "1").unwrap().body();
    assert!(
        raw.pointer("/target/assignment/target_scope")
            .unwrap()
            .is_null()
    );
    let mut migrated: DynamicDefinition = serde_json::from_value(
        raw.pointer("/target/assignment/dynamic/embedded_fallback/definition")
            .unwrap()
            .clone(),
    )
    .unwrap();
    assert_ne!(migrated.id, original_id);
    assert!(migrated.is_automatic_angle_partner(&migrated.lanes[1]));
    migrated.lanes.remove(0);
    migrated.normalize_angle_pair();
    assert!(migrated.lanes.is_empty());
}
