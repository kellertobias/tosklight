use super::*;
use crate::forward::PositionInstallation;

fn profile() -> FixtureProfile {
    let mut p = FixtureProfile::blank();
    p.manufacturer = "Test".into();
    p.name = "Position hold".into();
    let mode = &mut p.modes[0];
    let head = mode.heads[0].id;
    mode.splits[0].footprint = 4;
    p.geometry = GeometryGraph::template(GeometryTemplate::MovingHead, &[head]);
    p.geometry.physical_contract = Some(GeometryPhysicalContract {
        version: 1,
        provenance: OpticalProvenance::default(),
        bracket: GeometryBracket::Fixed,
    });
    let mut bindings = Vec::new();
    for (index, role) in [PositionAxisRole::Pan, PositionAxisRole::Tilt]
        .into_iter()
        .enumerate()
    {
        let key = if index == 0 { "pan" } else { "tilt" };
        let channel: FixtureChannel = serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(), "head_id":head,"split":1,
            "fixture_attribute":key,"attribute":key,"resolution":"u16",
            "secondary_slots":[2 + index * 2],"default_raw":32768,"highlight_raw":32768,
            "functions":[{
                "id":Uuid::new_v4(),"name":key,"dmx_from":0,"dmx_to":65535,
                "attribute":key,"priority":0,
                "angular_motion":{"kind":"absolute_position"},
                "behavior":{"type":"continuous","physical_min":-270.0,"physical_max":270.0,"unit":"deg"}
            }]
        })).unwrap();
        bindings.push(MotionFunctionBinding {
            node_id: p.geometry.nodes[index + 1].id,
            channel_id: channel.id,
            function_id: channel.functions[0].id,
            role,
        });
        mode.channels.push(channel);
    }
    mode.position_physical = Some(PositionPhysicalModel {
        version: 1,
        revision: 1,
        bindings,
    });
    p.geometry.emitters.push(GeometryEmitter {
        id: Uuid::new_v4(),
        name: "Lens".into(),
        node_id: p.geometry.nodes[2].id,
        head_id: None,
        origin: Vector3 {
            x: 0.0,
            y: -300.0,
            z: 0.0,
        },
        orientation_degrees: Vector3::default(),
        beam_angle_degrees: 10.0,
        field_angle_degrees: 20.0,
        feather: 0.0,
        focus: 1.0,
        directional: true,
        layout: EmitterLayout::Point,
    });
    mode.emitter_heads = p
        .geometry
        .emitters
        .iter()
        .map(|e| EmitterHeadBinding {
            emitter_id: e.id,
            head_id: head,
        })
        .collect();
    p
}

fn signature(p: &FixtureProfile, installed: PositionInstallation<'_>) -> String {
    position_freeze_control_signature(p, p.modes[0].id, p.modes[0].channels[0].id, installed)
        .unwrap()
        .unwrap()
}

fn fixture() -> PatchedFixture {
    let p = profile();
    let definition = p.resolved_definition(p.modes[0].id).unwrap();
    let mut f: PatchedFixture = serde_json::from_value(serde_json::json!({
        "fixture_id":FixtureId::new(),"definition":definition
    }))
    .unwrap();
    reconcile_logical_heads(&mut f);
    f.freeze.targets.insert(
        f.fixture_id,
        FrozenFixtureTarget {
            families: vec![FreezeFamily::Position],
            position_native: Some(FrozenPositionOutput {
                version: 1,
                instances: vec![FrozenPositionInstance {
                    instance_id: f.fixture_id.0,
                    controls: vec![FrozenPositionControl {
                        channel_id: p.modes[0].channels[0].id,
                        signature: signature(&p, PositionInstallation::default()),
                        raw: 32768,
                    }],
                }],
            }),
            ..Default::default()
        },
    );
    f
}

#[test]
fn legacy_freeze_has_no_native_position_and_new_payload_round_trips() {
    let old: FrozenFixtureTarget = serde_json::from_value(serde_json::json!({
        "full":true,"values":{"pan":{"kind":"normalized","value":0.25}}
    }))
    .unwrap();
    assert!(old.position_native.is_none());
    assert!(
        serde_json::to_value(&old)
            .unwrap()
            .get("position_native")
            .is_none()
    );
    let f = fixture();
    let restored: PatchedFixture =
        serde_json::from_value(serde_json::to_value(&f).unwrap()).unwrap();
    assert_eq!(f.freeze, restored.freeze);
    validate_patch(&[restored]).unwrap();
}

#[test]
fn malformed_payload_is_rejected_but_stale_ids_and_old_full_width_are_loadable() {
    let f = fixture();
    for variant in 0..7 {
        let mut bad = f.clone();
        let target = bad.freeze.targets.get_mut(&f.fixture_id).unwrap();
        let output = target.position_native.as_mut().unwrap();
        match variant {
            0 => output.version = 2,
            1 => output.instances.clear(),
            2 => output.instances.push(output.instances[0].clone()),
            3 => {
                let duplicate = output.instances[0].controls[0].clone();
                output.instances[0].controls.push(duplicate);
            }
            4 => output.instances[0].controls[0].signature = "bad".into(),
            5 => output.instances[0].instance_id = Uuid::nil(),
            _ => target.families.clear(),
        }
        assert!(validate_patch(&[bad]).is_err(), "variant {variant}");
    }
    let mut stale = f;
    let output = stale
        .freeze
        .targets
        .get_mut(&stale.fixture_id)
        .unwrap()
        .position_native
        .as_mut()
        .unwrap();
    output.instances[0].instance_id = Uuid::new_v4();
    output.instances[0].controls[0].channel_id = Uuid::new_v4();
    output.instances[0].controls[0].raw = u32::MAX;
    validate_patch(&[stale]).unwrap();
}

#[test]
fn overlapping_owners_must_agree_on_shared_native_controls() {
    let mut f = fixture();
    let other = FixtureId::new();
    f.freeze
        .targets
        .insert(other, f.freeze.targets[&f.fixture_id].clone());
    validate_position_freeze(&f).unwrap();
    f.freeze
        .targets
        .get_mut(&other)
        .unwrap()
        .position_native
        .as_mut()
        .unwrap()
        .instances[0]
        .controls[0]
        .raw += 1;
    assert!(
        validate_position_freeze(&f)
            .unwrap_err()
            .to_string()
            .contains("conflicting")
    );
}

#[test]
fn physical_signature_ignores_cosmetics_color_routing_and_every_portable_uuid() {
    let p = profile();
    let original = signature(&p, PositionInstallation::default());
    let mut changed = p.clone();
    changed.name = "Other name".into();
    changed.revision += 1;
    changed.geometry.nodes.reverse();
    changed.geometry.emitters.reverse();
    for e in &mut changed.geometry.emitters {
        e.focus = 0.37;
        e.beam_angle_degrees = 42.0;
        e.field_angle_degrees = 50.0;
        e.name = "Cosmetic".into();
    }
    changed.modes[0].channels[0].split = 2;
    changed.modes[0].channels[0].secondary_slots = vec![19];
    changed.modes[0].channels[0].fixture_attribute = AttributeKey("fixture.renamed_pan".into());
    changed.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .revision += 1;
    let mut color = changed.modes[0].channels[0].clone();
    color.id = Uuid::new_v4();
    color.attribute = AttributeKey("color.red".into());
    color.functions.clear();
    changed.modes[0].channels.push(color);
    assert_eq!(
        original,
        signature(&changed, PositionInstallation::default())
    );
    // Rewrite every UUID string consistently, including profile/mode/head/node/function refs.
    fn remap(value: &mut serde_json::Value, map: &mut std::collections::HashMap<Uuid, Uuid>) {
        match value {
            serde_json::Value::String(s) => {
                if let Ok(id) = Uuid::parse_str(s) {
                    *s = map.entry(id).or_insert_with(Uuid::new_v4).to_string();
                }
            }
            serde_json::Value::Array(rows) => {
                for row in rows {
                    remap(row, map)
                }
            }
            serde_json::Value::Object(rows) => {
                for row in rows.values_mut() {
                    remap(row, map)
                }
            }
            _ => {}
        }
    }
    let mut json = serde_json::to_value(&changed).unwrap();
    remap(&mut json, &mut Default::default());
    let remapped: FixtureProfile = serde_json::from_value(json).unwrap();
    assert_eq!(
        original,
        signature(&remapped, PositionInstallation::default())
    );
}

#[test]
fn physical_signature_changes_for_mapping_mechanics_and_effective_installation() {
    let p = profile();
    let original = signature(&p, PositionInstallation::default());
    for variant in 0..5 {
        let mut changed = p.clone();
        match variant {
            0 => changed.geometry.nodes[1].pivot.x += 10.0,
            1 => changed.modes[0].channels[0].invert = true,
            2 => {
                changed.modes[0].channels[0].canonical_transform =
                    CanonicalTransform::InvertNormalized
            }
            3 => {
                if let ChannelFunctionBehavior::Continuous { physical_max, .. } =
                    &mut changed.modes[0].channels[0].functions[0].behavior
                {
                    *physical_max += 90.0
                }
            }
            _ => changed.modes[0].channels[0].resolution = ChannelResolution::U24,
        }
        assert_ne!(
            original,
            signature(&changed, PositionInstallation::default()),
            "variant {variant}"
        );
    }
    let mut calibration = InstalledPositionCalibration {
        pan_zero_degrees: 12.0,
        ..Default::default()
    };
    let first = signature(
        &p,
        PositionInstallation {
            calibration: Some(&calibration),
            ..Default::default()
        },
    );
    assert_ne!(original, first);
    calibration.source = Some("Changed evidence only".into());
    calibration.revision += 1;
    assert_eq!(
        first,
        signature(
            &p,
            PositionInstallation {
                calibration: Some(&calibration),
                ..Default::default()
            }
        )
    );
    assert_ne!(
        original,
        signature(
            &p,
            PositionInstallation {
                invert_pan: true,
                ..Default::default()
            }
        )
    );
}

#[test]
fn effective_axis_overrides_hash_values_not_source_proof_and_stale_sets_fall_back() {
    let p = profile();
    let node = p.modes[0].position_physical.as_ref().unwrap().bindings[0].node_id;
    let mut calibration = InstalledPositionCalibration {
        pan_zero_degrees: 12.0,
        axis_overrides: Some(InstalledAxisOverrides {
            version: 1,
            source_identity: p
                .position_calibration_identity(p.modes[0].id)
                .unwrap()
                .unwrap(),
            axes: vec![InstalledAxisCalibration {
                node_id: node,
                zero_degrees: 42.0,
                invert: true,
            }],
        }),
        ..Default::default()
    };
    let explicit = signature(
        &p,
        PositionInstallation {
            calibration: Some(&calibration),
            ..Default::default()
        },
    );
    let mut remapped = p.clone();
    remapped.id = FixtureId::new();
    remapped.modes[0].id = Uuid::new_v4();
    let mut remapped_calibration = calibration.clone();
    remapped_calibration
        .axis_overrides
        .as_mut()
        .unwrap()
        .source_identity = remapped
        .position_calibration_identity(remapped.modes[0].id)
        .unwrap()
        .unwrap();
    assert_eq!(
        explicit,
        signature(
            &remapped,
            PositionInstallation {
                calibration: Some(&remapped_calibration),
                ..Default::default()
            }
        )
    );
    let same_effective = InstalledPositionCalibration {
        pan_zero_degrees: 42.0,
        ..Default::default()
    };
    // Patch inversion also controls the legacy/native input path and remains part of identity,
    // even when an axis override replaces its forward calibration sense.
    assert_ne!(
        explicit,
        signature(
            &p,
            PositionInstallation {
                calibration: Some(&calibration),
                invert_pan: true,
                ..Default::default()
            }
        )
    );
    calibration
        .axis_overrides
        .as_mut()
        .unwrap()
        .source_identity
        .profile_id = Uuid::new_v4();
    let fallback = signature(
        &p,
        PositionInstallation {
            calibration: Some(&calibration),
            ..Default::default()
        },
    );
    calibration.axis_overrides = None;
    assert_eq!(
        fallback,
        signature(
            &p,
            PositionInstallation {
                calibration: Some(&calibration),
                ..Default::default()
            }
        )
    );
    assert_ne!(fallback, explicit);
    assert_ne!(
        explicit,
        signature(
            &p,
            PositionInstallation {
                calibration: Some(&same_effective),
                ..Default::default()
            }
        )
    );
}
