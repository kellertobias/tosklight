//! Portable Freeze references remap through existing fixture identities, never new hold IDs.
use super::support::*;
use crate::selective_import::*;
use light_core::{AttributeKey, AttributeValue, FixtureId, programming::*};
use light_fixture::{
    AngularMotion, AngularMotionKind, ChannelFunction, ChannelFunctionBehavior, EmitterHeadBinding,
    EmitterLayout, FixtureProfile, GeometryBracket, GeometryEmitter, GeometryGraph,
    GeometryPhysicalContract, GeometryTemplate, InstalledAxisCalibration, InstalledAxisOverrides,
    InstalledPositionCalibration, MotionFunctionBinding, OpticalProvenance, PositionAxisRole,
    PositionCalibrationContext, PositionPhysicalModel, Vector3, forward::PositionInstallation,
    position_freeze_control_signature,
};
use light_show::FixtureProfileRevision;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

fn fixture_with_channel(base: u128, number: u32) -> (PortableFixtureTestRecord, Uuid) {
    let mut record = portable_fixture_record(base, number);
    let mut profile: FixtureProfile =
        serde_json::from_value(record.profile.profile().clone()).unwrap();
    let mode = &mut profile.modes[0];
    let channel = Uuid::from_u128(base + 20);
    mode.channels.push(
        serde_json::from_value(json!({
            "id":channel, "head_id":mode.heads[0].id, "split":1,
            "fixture_attribute":"pan", "attribute":"pan", "resolution":"u32",
            "secondary_slots":[2,3,4], "default_raw":0, "highlight_raw":u32::MAX,
            "functions":[ChannelFunction::continuous("pan", AttributeKey("pan".into()), u32::MAX)]
        }))
        .unwrap(),
    );
    mode.splits[0].footprint = 4;
    record.profile =
        FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap()).unwrap();
    (record, channel)
}

fn target(point: Uuid) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point { point_id: point },
        [1., 2., 3.],
    )))
}

fn native_instance(instance: Uuid, channel: Uuid, raw: u32) -> Value {
    json!({"instance_id":instance, "controls":[{
        "channel_id":channel, "signature":"a".repeat(64), "raw":raw,
        "future_control":{"retained":true}
    }]})
}

fn install_source(rig: &TestRig, record: &PortableFixtureTestRecord) {
    rig.source_profile(&record.profile);
    rig.source_object(
        "patched_fixture",
        &record.fixture_id.0.to_string(),
        record.body.clone(),
    );
}

fn profile_key(record: &PortableFixtureTestRecord) -> ImportProfileKey {
    ImportProfileKey {
        profile_id: record.profile.id().profile_id(),
        revision: record.profile.id().revision(),
    }
}

fn physical_fixture(base: u128) -> (PortableFixtureTestRecord, FixtureProfile) {
    let (mut record, _) = fixture_with_channel(base, 1);
    let mut profile: FixtureProfile =
        serde_json::from_value(record.profile.profile().clone()).unwrap();
    let head = profile.modes[0].heads[0].id;
    profile.geometry = GeometryGraph::template(GeometryTemplate::MovingHead, &[head]);
    profile.geometry.physical_contract = Some(GeometryPhysicalContract {
        version: 1,
        provenance: OpticalProvenance::default(),
        bracket: GeometryBracket::Fixed,
    });
    let mut tilt = profile.modes[0].channels[0].clone();
    tilt.id = Uuid::from_u128(base + 21);
    tilt.attribute = AttributeKey("tilt".into());
    tilt.fixture_attribute = tilt.attribute.clone();
    tilt.secondary_slots = vec![6, 7, 8];
    tilt.functions[0].id = Uuid::from_u128(base + 22);
    tilt.functions[0].attribute = tilt.attribute.clone();
    profile.modes[0].channels.push(tilt);
    profile.modes[0].splits[0].footprint = 8;
    let mut bindings = Vec::new();
    for (index, role) in [PositionAxisRole::Pan, PositionAxisRole::Tilt]
        .into_iter()
        .enumerate()
    {
        let channel = &mut profile.modes[0].channels[index];
        channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
            physical_min: -270.,
            physical_max: 270.,
            unit: Some("deg".into()),
        };
        channel.functions[0].angular_motion = Some(AngularMotion {
            kind: AngularMotionKind::AbsolutePosition,
            max_speed_degrees_per_second: None,
            acceleration_degrees_per_second_squared: None,
            deceleration_degrees_per_second_squared: None,
        });
        bindings.push(MotionFunctionBinding {
            node_id: profile.geometry.nodes[index + 1].id,
            channel_id: channel.id,
            function_id: channel.functions[0].id,
            role,
        });
    }
    profile.modes[0].position_physical = Some(PositionPhysicalModel {
        kinematics: Default::default(),
        version: 1,
        revision: 1,
        bindings,
    });
    let emitter = GeometryEmitter {
        id: Uuid::from_u128(base + 23),
        name: "Lens".into(),
        node_id: profile.geometry.nodes[2].id,
        head_id: None,
        origin: Vector3::default(),
        orientation_degrees: Vector3::default(),
        beam_angle_degrees: 10.,
        field_angle_degrees: 20.,
        feather: 0.,
        focus: 1.,
        directional: true,
        layout: EmitterLayout::Point,
    };
    profile.modes[0].emitter_heads = vec![EmitterHeadBinding {
        emitter_id: emitter.id,
        head_id: head,
    }];
    profile.geometry.emitters = vec![emitter];
    record.profile =
        FixtureProfileRevision::from_profile(serde_json::to_value(&profile).unwrap()).unwrap();
    (record, profile)
}

#[test]
fn profile_import_rebases_root_and_copy_axis_proofs_without_repairing_stale_geometry() {
    // A matching Duplicate is compatible. A stale digest, foreign profile proof, or changed
    // Keep geometry must retain its stale meaning, rather than being silently repaired.
    for case in 0..4 {
        let rig = TestRig::new();
        let (mut fixture, profile) = physical_fixture(980_000 + case * 100);
        let context = PositionCalibrationContext::new(&profile, profile.modes[0].id)
            .unwrap()
            .unwrap();
        let mut calibrations = Vec::new();
        for index in 0..2 {
            let mut proof = context.identity.clone();
            if case == 1 {
                proof.geometry_digest = "b".repeat(64);
            }
            if case == 3 {
                proof.profile_id = Uuid::from_u128(999_999);
            }
            calibrations.push(InstalledPositionCalibration {
                pan_zero_degrees: 3.,
                tilt_zero_degrees: 4.,
                axis_overrides: Some(InstalledAxisOverrides {
                    version: 1,
                    source_identity: proof,
                    axes: vec![
                        InstalledAxisCalibration {
                            node_id: profile.geometry.nodes[1].id,
                            zero_degrees: 37. + index as f32,
                            invert: index == 0,
                        },
                        InstalledAxisCalibration {
                            node_id: profile.geometry.nodes[2].id,
                            zero_degrees: -12. - index as f32,
                            invert: index == 1,
                        },
                    ],
                }),
                ..Default::default()
            });
        }
        fixture.body["position_calibration"] = json!(calibrations[0]);
        fixture.body["multipatch"][0]["position_calibration"] = json!(calibrations[1]);
        fixture.body["multipatch"][0]["invert_pan"] = json!(true);
        let channel = profile.modes[0].channels[0].id;
        let signatures = calibrations
            .iter()
            .enumerate()
            .map(|(index, calibration)| {
                position_freeze_control_signature(
                    &profile,
                    profile.modes[0].id,
                    channel,
                    PositionInstallation {
                        calibration: Some(calibration),
                        invert_pan: index == 1,
                        ..Default::default()
                    },
                )
                .unwrap()
                .unwrap()
            })
            .collect::<Vec<_>>();
        fixture.body["freeze"] = json!({"targets":{
            fixture.fixture_id.0.to_string():{"full":false,"families":["position"],"values":{},
                "position_native":{"version":1,"instances":[
                    {"instance_id":fixture.fixture_id.0,"controls":[{
                        "channel_id":channel,"signature":signatures[0],"raw":u32::MAX - 1}]},
                    {"instance_id":fixture.multipatch_id,"controls":[{
                        "channel_id":channel,"signature":signatures[1],"raw":1}]}
                ]}}
        }});
        install_source(&rig, &fixture);
        let mut conflict = fixture.profile.profile().clone();
        if case == 2 {
            conflict["geometry"]["nodes"][1]["pivot"]["x"] = json!(123.);
        } else {
            conflict["manufacturer"] = json!("Occupied revision");
        }
        rig.target_profile(&FixtureProfileRevision::from_profile(conflict).unwrap());
        let resolution = if case == 2 {
            ImportProfileConflictResolution::KeepDestination
        } else {
            ImportProfileConflictResolution::Duplicate
        };
        let preview = rig.preview(
            rig.request("patched_fixture", &fixture.fixture_id.0.to_string())
                .with_mode(ImportLoadMode::AddToEnd)
                .resolve_profile(profile_key(&fixture), resolution),
        );
        assert!(preview.can_apply(), "case {case}: {:?}", preview.blockers);
        rig.apply(&preview).unwrap();
        let document = rig.target_document();
        let object = preview
            .objects
            .iter()
            .find(|entry| entry.source.kind() == "patched_fixture")
            .unwrap();
        let body = document
            .object("patched_fixture", object.destination.id())
            .unwrap()
            .body();
        let destination = preview
            .profiles
            .iter()
            .find(|entry| entry.source == profile_key(&fixture))
            .unwrap()
            .destination;
        let imported: FixtureProfile = serde_json::from_value(
            document
                .fixture_profile_revision(destination.profile_id, destination.revision)
                .unwrap()
                .profile()
                .clone(),
        )
        .unwrap();
        let imported_context = PositionCalibrationContext::new(&imported, imported.modes[0].id)
            .unwrap()
            .unwrap();
        let held =
            &body["freeze"]["targets"][object.destination.id()]["position_native"]["instances"];
        for (index, pointer) in [
            "/position_calibration",
            "/multipatch/0/position_calibration",
        ]
        .into_iter()
        .enumerate()
        {
            let calibration: InstalledPositionCalibration =
                serde_json::from_value(body.pointer(pointer).unwrap().clone()).unwrap();
            let mut expected = serde_json::to_value(&calibrations[index]).unwrap();
            if case != 3 {
                expected["axis_overrides"]["source_identity"]["profile_id"] =
                    json!(destination.profile_id.0);
            }
            assert_eq!(body.pointer(pointer).unwrap(), &expected);
            let overrides = calibration.axis_overrides.as_ref().unwrap();
            assert_eq!(
                overrides.validate_for_context(&imported_context).is_ok(),
                case == 0
            );
            if case == 0 {
                for (axis_index, role) in [PositionAxisRole::Pan, PositionAxisRole::Tilt]
                    .into_iter()
                    .enumerate()
                {
                    assert_eq!(
                        calibration
                            .effective_axis(
                                &imported_context,
                                imported.geometry.nodes[axis_index + 1].id,
                                role,
                                index == 1,
                                false
                            )
                            .unwrap(),
                        calibrations[index]
                            .effective_axis(
                                &context,
                                profile.geometry.nodes[axis_index + 1].id,
                                role,
                                index == 1,
                                false
                            )
                            .unwrap()
                    );
                }
            }
            let actual_signature = position_freeze_control_signature(
                &imported,
                imported.modes[0].id,
                channel,
                PositionInstallation {
                    calibration: Some(&calibration),
                    invert_pan: index == 1,
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap();
            assert_eq!(actual_signature == signatures[index], case != 2);
            assert_eq!(
                held[index]["controls"][0]["signature"],
                json!(signatures[index])
            );
            assert_eq!(
                held[index]["controls"][0]["raw"],
                json!(if index == 0 { u32::MAX - 1 } else { 1 })
            );
        }
    }
}

#[test]
fn duplicate_freeze_remaps_root_head_copy_and_target_with_profile_collision() {
    let rig = TestRig::new();
    let (mut fixture, channel) = fixture_with_channel(930_000, 1);
    let point = portable_fixture_record(940_000, 2);
    fixture.body["multipatch"][0]["invert_pan"] = json!(true);
    let native = json!({"version":1,"instances":[
        native_instance(fixture.fixture_id.0, channel, u32::MAX - 1),
        native_instance(fixture.multipatch_id, channel, 1)
    ]});
    fixture.body["freeze"] = json!({"targets":{
        fixture.fixture_id.0.to_string():{
            "full":true,"families":[],"values":{"position":target(point.fixture_id.0)},
            "position_native":native.clone(),"future_target":{"retained":true}
        },
        fixture.head_id.0.to_string():{
            "full":false,"families":["position"],
            "values":{"position":target(point.fixture_id.0)},"position_native":native
        }
    }});
    install_source(&rig, &fixture);
    install_source(&rig, &point);
    // Occupied fixture identity and immutable profile revision both require actual duplication.
    let mut occupied = fixture.body.clone();
    occupied["name"] = json!("Destination fixture");
    rig.target_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        occupied,
    );
    let mut conflicting = fixture.profile.profile().clone();
    conflicting["manufacturer"] = json!("Destination model");
    rig.target_profile(&FixtureProfileRevision::from_profile(conflicting).unwrap());
    let preview = rig.preview(
        rig.request("patched_fixture", &fixture.fixture_id.0.to_string())
            .with_mode(ImportLoadMode::AddToEnd)
            .resolve_profile(
                profile_key(&fixture),
                ImportProfileConflictResolution::Duplicate,
            ),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let destination = |id: FixtureId| {
        preview
            .objects
            .iter()
            .find(|entry| entry.source == key("patched_fixture", &id.0.to_string()))
            .unwrap()
            .destination
            .id()
            .to_owned()
    };
    let root = destination(fixture.fixture_id);
    let point_id = destination(point.fixture_id);
    assert_ne!(root, fixture.fixture_id.0.to_string());
    assert_ne!(point_id, point.fixture_id.0.to_string());
    rig.apply(&preview).unwrap();
    let document = rig.target_document();
    let imported = document.object("patched_fixture", &root).unwrap().body();
    let head = imported["logical_heads"][0]["fixture_id"].as_str().unwrap();
    let copy = imported["multipatch"][0]["id"].as_str().unwrap();
    assert_ne!(head, fixture.head_id.0.to_string());
    assert_ne!(copy, fixture.multipatch_id.to_string());
    assert_eq!(imported["multipatch"][0]["invert_pan"], true);
    let mut expected_targets = serde_json::Map::new();
    for (old, new) in [
        (fixture.fixture_id.0.to_string(), root.as_str()),
        (fixture.head_id.0.to_string(), head),
    ] {
        let mut frozen = fixture.body["freeze"]["targets"][&old].clone();
        frozen["values"]["position"]["value"]["reference"]["point_id"] = json!(point_id);
        frozen["position_native"]["instances"][0]["instance_id"] = json!(root);
        frozen["position_native"]["instances"][1]["instance_id"] = json!(copy);
        expected_targets.insert(new.into(), frozen);
    }
    assert_eq!(
        imported["freeze"],
        json!({"targets":expected_targets}),
        "raw words, signatures, channel IDs and extension data must remain exact"
    );
    let mapped_profile = preview
        .profiles
        .iter()
        .find(|entry| entry.source == profile_key(&fixture))
        .unwrap()
        .destination;
    assert_ne!(mapped_profile.profile_id, profile_key(&fixture).profile_id);
    assert_eq!(imported["profile_id"], json!(mapped_profile.profile_id.0));
    let copied = document
        .fixture_profile_revision(mapped_profile.profile_id, mapped_profile.revision)
        .unwrap();
    assert_eq!(
        copied.profile()["modes"][0]["channels"][0]["id"],
        json!(channel)
    );
}

#[test]
fn stale_freeze_owner_point_instance_and_channel_remain_loadable_and_unrebound() {
    let rig = TestRig::new();
    let (mut fixture, _) = fixture_with_channel(950_000, 1);
    let missing_owner = Uuid::from_u128(960_001);
    let missing_point = Uuid::from_u128(960_002);
    let missing_channel = Uuid::from_u128(960_003);
    // This ID belongs to another source fixture: it must still remain a stale instance here.
    let other = portable_fixture_record(970_000, 2);
    let native = json!({"version":1,"instances":[
        native_instance(other.fixture_id.0, missing_channel, u32::MAX)
    ]});
    fixture.body["freeze"] = json!({"targets":{
        fixture.fixture_id.0.to_string():{
            "full":false,"families":["position"],
            "values":{"position":target(missing_point)},"position_native":native.clone()
        },
        missing_owner.to_string():{
            "full":false,"families":["position"],
            "values":{"position":target(missing_point)},"position_native":native
        }
    }});
    install_source(&rig, &fixture);
    install_source(&rig, &other);
    let preview = rig.preview(
        SelectiveShowImportRequest::new(
            rig.source_id,
            rig.target_id,
            [
                key("patched_fixture", &fixture.fixture_id.0.to_string()),
                key("patched_fixture", &other.fixture_id.0.to_string()),
            ],
        )
        .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let root = preview
        .objects
        .iter()
        .find(|entry| entry.source == key("patched_fixture", &fixture.fixture_id.0.to_string()))
        .unwrap()
        .destination
        .id()
        .to_owned();
    rig.apply(&preview).unwrap();
    let document = rig.target_document();
    let imported = document.object("patched_fixture", &root).unwrap().body();
    let original_targets = fixture.body["freeze"]["targets"].as_object().unwrap();
    let mut expected = original_targets.clone();
    let owned = expected.remove(&fixture.fixture_id.0.to_string()).unwrap();
    expected.insert(root, owned);
    assert_eq!(imported["freeze"], json!({"targets":expected}));
    assert!(
        rig.ports.installed.lock().is_some(),
        "actual runtime preparation must accept stale holds"
    );
}

#[test]
fn frozen_direct_color_keeps_strict_pinned_profile_validation() {
    let rig = TestRig::new();
    let source = super::programming_native::native_fixture();
    let mut body = source.fixture_body.clone();
    body["freeze"] = json!({"targets":{
        source.fixture_id.0.to_string():{
            "full":false,"families":["color"],"values":{"color":{
                "kind":"color_program","value":{"kind":"direct",
                    "recipe":{"source":source.identity,"channels":source.channels,"spreads":[]},
                    "portable":{"model_revision":1,"visible":null,
                        "uv":{"amount":0.7,"quality":"estimated"},
                        "quality":"estimated","limitations":["UV response unknown"]}
                }
            }}
        }
    }});
    rig.source_profile(&source.revision);
    rig.source_object("patched_fixture", &source.fixture_id.0.to_string(), body);
    let mut changed = source.revision.profile().clone();
    changed["manufacturer"] = json!("Incompatible destination");
    rig.target_profile(&FixtureProfileRevision::from_profile(changed).unwrap());
    let preview = rig.preview(
        rig.request("patched_fixture", &source.fixture_id.0.to_string())
            .resolve_profile(
                ImportProfileKey {
                    profile_id: source.revision.id().profile_id(),
                    revision: source.revision.id().revision(),
                },
                ImportProfileConflictResolution::KeepDestination,
            ),
    );
    assert!(
        !preview.can_apply(),
        "Freeze must not relax Direct source compatibility"
    );
    assert!(
        preview.blockers.iter().any(|blocker| matches!(blocker,
            ImportBlocker::ReferenceRewrite { message, .. }
                if message.contains("destination profile changes the pinned native Color model")
        )),
        "{:?}",
        preview.blockers
    );
    assert!(rig.apply(&preview).is_err());
    assert!(
        rig.target_document()
            .object("patched_fixture", &source.fixture_id.0.to_string())
            .is_none()
    );
}
