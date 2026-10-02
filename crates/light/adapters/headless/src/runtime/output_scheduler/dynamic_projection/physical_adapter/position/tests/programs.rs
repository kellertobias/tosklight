//! Actual Live destination-specific Position program regressions. Parent helpers capture
//! native/geometry frames and keep sampling plus finalization in one output transaction.
use super::*;
use light_dynamics::{
    ActivationBoundary, ActivationPolicy, DynamicDefinition, DynamicDefinitionSnapshot,
    DynamicFamilyRepresentation, DynamicInstanceOverrides, DynamicKeyframe, DynamicLane,
    DynamicLaneBody, DynamicPhaseSpreadMode, DynamicReference, DynamicRunMode, DynamicSpeed,
    DynamicTargetBinding, DynamicValue, DynamicValueSource, DynamicValueTiming,
    KeyframeConfiguration, PhaseDistribution, PhaseOrdering, ProgrammingLaneBody,
    ProgrammingLaneConfiguration, Rational,
};

pub(super) fn destination_dynamic_rig() -> (Rig, FixtureId) {
    let profile = moving_head();
    let root = FixtureId::new();
    let copy = FixtureId::new();
    let mut fixture = patched(&profile, root, 1);
    fixture.location.z = 3000;
    fixture.multipatch = vec![MultiPatchInstance {
        id: copy.0,
        universe: Some(1),
        address: Some(10),
        location: FixtureLocation {
            x: 3000,
            y: 1000,
            z: 5000,
        },
        rotation: FixtureVector {
            x: 0.,
            y: 0.,
            z: 30.,
        },
        invert_pan: true,
        position_calibration: Some(InstalledPositionCalibration {
            pan_zero_degrees: 11.,
            tilt_zero_degrees: -7.,
            ..Default::default()
        }),
        ..Default::default()
    }];
    (Rig::new(vec![fixture], root), copy)
}

pub(super) fn position_definition(
    address: DynamicValueAddress,
    values: [DynamicValue; 2],
) -> DynamicDefinition {
    let mut definition = DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 1,
        revision: 1,
        name: "TL-556 destination Position".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::Targetless,
        lanes: vec![DynamicLane {
            id: Uuid::new_v4(),
            body: DynamicLaneBody::Programming(ProgrammingLaneBody {
                address,
                configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                    points: [0., 0.5]
                        .into_iter()
                        .zip(values)
                        .map(|(position, value)| DynamicKeyframe {
                            position,
                            source: DynamicValueSource::Value { value },
                            interpolation: light_dynamics::ScalarInterpolation::Linear,
                        })
                        .collect(),
                    size: 1.,
                }),
            }),
            speed_multiplier: Rational::ONE,
            width: 1.,
            phase: None,
            random_group_id: None,
        }],
        random_groups: vec![],
        phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: Default::default(),
        phase: PhaseDistribution {
            ordering: PhaseOrdering::Selection,
            offset_degrees: 0.,
            span_degrees: 360.,
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
    definition.normalize_angle_pair();
    definition
}

pub(super) fn start_position_dynamic(
    rig: &Rig,
    base: &AttributeValue,
    definition: &DynamicDefinition,
    fade: Option<u64>,
) -> DynamicRuntime {
    start_position_dynamic_sized(rig, base, definition, fade, 1.)
}

pub(super) fn start_position_dynamic_sized(
    rig: &Rig,
    base: &AttributeValue,
    definition: &DynamicDefinition,
    fade: Option<u64>,
    controller_size: f32,
) -> DynamicRuntime {
    let snapshot = rig.engine.snapshot();
    rig.engine
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    rig.programmers.set(
        rig.session,
        rig.root,
        ProgrammingOwner::Position.key(),
        base.clone(),
    );
    assert!(rig.programmers.apply_dynamic_values(
        rig.session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: rig.root,
            attribute: ProgrammingOwner::Position.key(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::new_v4(),
                lane_id: definition.lanes[0].id,
                dynamic: DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: definition.pool_number,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(definition.clone())
                    },
                },
                overrides: DynamicInstanceOverrides {
                    size: controller_size,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.
                },
                timing: DynamicValueTiming {
                    fade_millis: fade,
                    delay_millis: None
                },
            },
        }],
        None
    ));
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
}

pub(super) fn commanded_angles(value: &AttributeValue) -> [f64; 2] {
    let PositionIntent::Angles {
        pan_degrees: ScalarIntent::Value(pan),
        tilt_degrees: ScalarIntent::Value(tilt),
    } = intent(value).unwrap()
    else {
        panic!("destination must materialize one complete Angle pair: {value:?}")
    };
    [f64::from(*pan), f64::from(*tilt)]
}

pub(super) fn program(result: &PhysicalHeadResult<PositionAdapter>) -> &PositionProgram {
    let PositionRequest::Program(program) = &result.requested else {
        panic!("Live observer must retain the original Position program")
    };
    program
}

pub(super) fn verify_live_native(
    rig: &Rig,
    capture: &PreparedOutputFrame,
    lane: &PhysicalAdapterLane<PositionAdapter>,
    published: &PublishedPhysicalFrame<PositionAdapter>,
) {
    let scalar = rig.engine.prepare_static_family_frame(capture, &[]);
    let instance_native = rig.native_baselines(capture, &scalar);
    let native = instance_native[0].1.clone();
    let resolved = Resolved {
        results: published
            .results
            .iter()
            .map(|row| PhysicalResolution {
                writes: row.writes.clone(),
                requested: row.requested.clone(),
                achieved: row.achieved.clone(),
                quality: row.quality.clone(),
                continuity: lane.continuity(row.target, row.owner).unwrap(),
            })
            .collect(),
        native,
        instance_native,
        mounts: published
            .geometry
            .mounts()
            .mounts()
            .iter()
            .filter_map(|mount| {
                mount
                    .world_from_fixture
                    .map(|world| (FixtureId(mount.instance_id), world))
            })
            .collect(),
    };
    rig.verify(&resolved);
    verify_live_rendered_native(rig, capture, published);
}

/// Compare fitted sidecar writes with the actual accepted renderer, preserving every other
/// channel and every universe byte from the same captured ordinary output/overlay baseline.
fn verify_live_rendered_native(
    rig: &Rig,
    capture: &PreparedOutputFrame,
    published: &PublishedPhysicalFrame<PositionAdapter>,
) {
    let baseline = rig
        .engine
        .preview_static_family_frame(
            capture,
            rig.engine.prepare_static_family_frame(capture, &[]),
        )
        .unwrap();
    let snapshot = capture.snapshot();
    let mut expected_universes = baseline.universes.clone();
    for original in &baseline.physical.instances {
        let mut expected = original.native_raw.to_vec();
        for write in published
            .results
            .iter()
            .flat_map(|row| &row.writes)
            .filter(|write| write.slot.destination.0 == original.instance_id)
        {
            expected[write.slot.channel_index as usize] = write.raw;
        }
        let actual = published
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == original.instance_id)
            .unwrap();
        assert!(actual.complete);
        assert_eq!(
            actual.native_raw.as_ref(),
            expected.as_slice(),
            "accepted native output includes fitted Position and untouched overlay baseline for {:?}",
            original.instance_id
        );
        let fixture = snapshot
            .fixtures
            .iter()
            .find(|f| f.fixture_id == original.fixture_id)
            .unwrap();
        let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
        let mode = profile.mode(fixture.definition.mode_id.unwrap()).unwrap();
        let (universe, address) = if original.instance_id == fixture.fixture_id.0 {
            (fixture.universe, fixture.address)
        } else {
            let copy = fixture
                .multipatch
                .iter()
                .find(|copy| copy.id == original.instance_id)
                .unwrap();
            (copy.universe, copy.address)
        };
        if let (Some(universe), Some(address)) = (universe, address) {
            assert!(
                fixture.split_patches.is_empty(),
                "this harness uses one ordinary split"
            );
            mode.compile_encoding_plan()
                .unwrap()
                .encode_split_by_index(
                    expected_universes.get_mut(&universe).unwrap(),
                    address,
                    1,
                    &(0u32..).zip(expected.iter().copied()).collect::<Vec<_>>(),
                )
                .unwrap();
        }
    }
    assert_eq!(
        &*published.rendered.universes, &expected_universes,
        "accepted DMX uses fitted coarse/fine bytes and preserves unrelated slots"
    );
}

#[test]
fn actual_live_native_position_preserves_unrelated_channels_and_grand_master_overlay() {
    let mut profile = moving_head();
    let mode = &mut profile.modes[0];
    for axis in &mut mode.channels {
        axis.reacts_to_grand_master = false;
    }
    let head = mode.heads[0].id;
    for (slot, attribute, raw) in [(5, "beam.focus", 77), (6, "intensity", 204)] {
        let mut control = channel(head, attribute, slot);
        control.resolution = ChannelResolution::U8;
        control.secondary_slots.clear();
        control.default_raw = raw;
        control.highlight_raw = 255;
        control.functions = vec![ChannelFunction::continuous(
            attribute,
            control.attribute.clone(),
            255,
        )];
        mode.channels.push(control);
    }
    mode.splits[0].footprint = 6;
    let root = FixtureId::new();
    let rig = Rig::new(vec![patched(&profile, root, 1)], root);
    let base = angles(130., -37.);
    let definition = position_definition(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        },
        [DynamicValue::Scalar(130.), DynamicValue::Scalar(130.)],
    );
    let mut runtime = start_position_dynamic(&rig, &base, &definition, None);
    let capture = rig
        .engine
        .prepare_output_frame(light_engine::RenderOptions {
            grand_master: 0.5,
            ..Default::default()
        });
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let published = prepare_live(
        &rig,
        &capture,
        &capture,
        &lane,
        &mut runtime,
        &mut DynamicSourceOrigins::default(),
        &mut HybridFrameScratch::default(),
    )
    .unwrap();
    assert!(published.requirements.is_empty());
    assert_eq!(
        published.results[0].writes.len(),
        2,
        "Position owns only its two motion channels"
    );
    assert_eq!(
        &published.rendered.physical.instances[0].native_raw[2..],
        &[39, 102],
        "unrelated Focus and Intensity still receive the captured Grand Master once"
    );
    verify_live_rendered_native(&rig, &capture, &published);
}

#[test]
fn actual_live_target_angle_activation_crossings_keep_per_copy_intermediate_values_and_original_program()
 {
    for towards_angles in [true, false] {
        let (rig, copy) = destination_dynamic_rig();
        let aim = target(TargetReference::Origin, [4., 8., 1.]);
        // The independent endpoint solve is reference evidence only; the assertions below
        // exercise real captured Dynamic sampling, destination composition and finalization.
        let endpoints = rig.resolve(&[(rig.root, aim.clone())]);
        let unanchored_target_axes = endpoints.results[0]
            .achieved
            .outcomes
            .iter()
            .map(|o| {
                assert_eq!(o.result.status, PositionFitStatus::Fitted);
                (o.destination, o.result.achieved.unwrap())
            })
            .collect::<Vec<_>>();
        let angular = angles(60., -20.);
        let (base, authored) = if towards_angles {
            (aim.clone(), angular.clone())
        } else {
            (angular.clone(), aim.clone())
        };
        let address =
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &authored).unwrap();
        let definition = position_definition(
            address,
            [
                DynamicValue::Family(authored.clone()),
                DynamicValue::Family(authored.clone()),
            ],
        );
        let lane_id = definition.lanes[0].id;
        let mut runtime = start_position_dynamic(&rig, &base, &definition, Some(1000));
        let lane = PhysicalAdapterLane::live(PositionAdapter::default());
        let mut origins = DynamicSourceOrigins::default();
        let mut scratch = HybridFrameScratch::default();
        let first = rig.capture();
        prepare_live(
            &rig,
            &first,
            &first,
            &lane,
            &mut runtime,
            &mut origins,
            &mut scratch,
        )
        .unwrap();
        // Branch anchors are established by the accepted first frame, including an Angle
        // underlay at activation mix zero. Re-evaluate the Target endpoint with those anchors;
        // a zero-anchor solve can legitimately choose a different equivalent mechanical pose.
        let accepted = lane.continuity(rig.root, ProgrammingOwner::Position);
        let endpoints = rig.resolve_with(&[(rig.root, aim.clone())], accepted.as_ref(), &[]);
        rig.verify(&endpoints);
        let target_axes = endpoints.results[0]
            .achieved
            .outcomes
            .iter()
            .map(|outcome| {
                assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
                assert!(
                    outcome.result.angular_error_degrees.unwrap() < 0.04,
                    "accepted-anchor endpoint still aims at the original world target"
                );
                (outcome.destination, outcome.result.achieved.unwrap())
            })
            .collect::<Vec<_>>();
        if let Some(accepted) = &accepted {
            let snapshot = rig.engine.snapshot();
            let profile = snapshot.fixtures[0]
                .definition
                .profile_snapshot
                .as_deref()
                .unwrap();
            let bindings = &profile.modes[0]
                .position_physical
                .as_ref()
                .unwrap()
                .bindings;
            let pan = bindings
                .iter()
                .find(|binding| binding.role == PositionAxisRole::Pan)
                .unwrap()
                .node_id;
            let tilt = bindings
                .iter()
                .find(|binding| binding.role == PositionAxisRole::Tilt)
                .unwrap()
                .node_id;
            for (destination, target) in &target_axes {
                let previous = accepted
                    .instances
                    .iter()
                    .find(|instance| instance.destination == *destination)
                    .unwrap();
                let anchor = [pan, tilt].map(|node| {
                    previous
                        .joints
                        .iter()
                        .find(|(id, _)| *id == node)
                        .unwrap()
                        .1
                        .unwrap()
                });
                let unanchored = unanchored_target_axes
                    .iter()
                    .find(|(id, _)| id == destination)
                    .unwrap()
                    .1;
                let distance = |angles: [f64; 2]| {
                    angles
                        .iter()
                        .zip(anchor)
                        .map(|(angle, anchor)| (angle - anchor).powi(2))
                        .sum::<f64>()
                };
                assert!(
                    distance(*target) <= distance(unanchored) + 1.,
                    "accepted branch is at least as close as the verified zero-anchor alternative"
                );
            }
        }
        // The independent endpoint capture above advanced the shared clock by 25 ms.
        rig.clock.advance_millis(450);
        let capture = rig.capture();
        let published = prepare_live(
            &rig,
            &capture,
            &capture,
            &lane,
            &mut runtime,
            &mut origins,
            &mut scratch,
        )
        .unwrap();
        assert!(published.requirements.is_empty());
        assert_eq!(published.results.len(), 1);
        verify_live_native(&rig, &capture, &lane, &published);
        let sample = published
            .sampled
            .samples
            .iter()
            .find(|s| s.lane_id == lane_id)
            .unwrap();
        let mix = f64::from(sample.activation_mix);
        assert!(
            mix > 0.1 && mix < 0.9,
            "test must observe an actual intermediate activation: {mix}"
        );
        let sidecar = &published.results[0];
        let logical = program(sidecar);
        assert_eq!(logical.base, base);
        assert!(!logical.samples.is_empty());
        assert_eq!(sidecar.achieved.destinations.len(), 2);
        let mut values = Vec::new();
        for destination in &sidecar.achieved.destinations {
            let target = target_axes
                .iter()
                .find(|(id, _)| *id == destination.destination)
                .unwrap()
                .1;
            let (from, to) = if towards_angles {
                (target, [60., -20.])
            } else {
                ([60., -20.], target)
            };
            let actual = commanded_angles(&destination.value);
            for axis in 0..2 {
                let expected = from[axis] + mix * (to[axis] - from[axis]);
                assert!(
                    (actual[axis] - expected).abs() < 0.05,
                    "{towards_angles} {:?} axis{axis}: {} != {expected}",
                    destination.destination,
                    actual[axis]
                );
            }
            let outcome = published.results[0]
                .achieved
                .outcomes
                .iter()
                .find(|o| o.destination == destination.destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            let achieved = outcome.result.achieved.unwrap();
            assert!(
                achieved
                    .iter()
                    .zip(actual)
                    .all(|(a, b)| (a - b).abs() < 0.023)
            );
            values.push((destination.destination, actual));
        }
        let root_angles = values.iter().find(|(id, _)| *id == rig.root).unwrap().1;
        let copy_angles = values.iter().find(|(id, _)| *id == copy).unwrap().1;
        assert!(
            root_angles
                .iter()
                .zip(copy_angles)
                .any(|(a, b)| (a - b).abs() > 1.),
            "different mounts cannot share a preconverted Angle value"
        );
        assert_eq!(
            published.results[0].value,
            sidecar
                .achieved
                .destinations
                .iter()
                .find(|d| d.destination == rig.root)
                .unwrap()
                .value
        );
        assert_eq!(
            rig.engine.snapshot().dynamics[0],
            definition,
            "destination calculations never edit the authored Dynamic"
        );
        let mut retained_sources = Vec::new();
        sample
            .expression
            .visit_source_occurrences(&mut |id| retained_sources.push(id))
            .unwrap();
        assert!(
            !retained_sources.is_empty(),
            "original authored Dynamic source identities survive fitting"
        );
    }
}

#[test]
fn actual_live_authored_pan_target_current_tilt_is_destination_bound_and_keeps_source_and_phase() {
    let (rig, copy) = destination_dynamic_rig();
    let base = target(TargetReference::Origin, [4., 8., 1.]);
    let endpoints = rig.resolve(&[(rig.root, base.clone())]);
    let target_tilts = endpoints.results[0]
        .achieved
        .outcomes
        .iter()
        .map(|o| {
            assert_eq!(o.result.status, PositionFitStatus::Fitted);
            (o.destination, o.result.achieved.unwrap()[1])
        })
        .collect::<Vec<_>>();
    assert!((target_tilts[0].1 - target_tilts[1].1).abs() > 1.);
    let definition = position_definition(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        },
        [DynamicValue::Scalar(90.), DynamicValue::Scalar(180.)],
    );
    assert_eq!(
        definition.lanes.len(),
        2,
        "Tilt Current remains an explicit complete-pair lane"
    );
    let pan_lane = definition.lanes[0].id;
    let mut runtime = start_position_dynamic(&rig, &base, &definition, None);
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    let first_capture = rig.capture();
    let first = prepare_live(
        &rig,
        &first_capture,
        &first_capture,
        &lane,
        &mut runtime,
        &mut origins,
        &mut scratch,
    )
    .unwrap();
    assert!(first.requirements.is_empty());
    verify_live_native(&rig, &first_capture, &lane, &first);
    let first_pan = first
        .sampled
        .samples
        .iter()
        .find(|s| s.lane_id == pan_lane)
        .unwrap();
    let identity = (
        first_pan.instance_id,
        first_pan.controller_id,
        first_pan.activated_at_millis,
    );
    let mut authored_sources = Vec::new();
    first_pan
        .expression
        .visit_source_occurrences(&mut |id| authored_sources.push(id))
        .unwrap();
    assert_eq!(authored_sources.len(), 1);
    let phase_anchor = runtime.snapshot().instances[0].started_at_millis;
    let accepted = lane
        .continuity(rig.root, ProgrammingOwner::Position)
        .unwrap();
    // Current remains the same captured Target intention, but its mechanical branch follows
    // the last accepted complete pair. Authored Pan can move that nearest branch between frames.
    let second_endpoint = rig.resolve_with(&[(rig.root, base.clone())], Some(&accepted), &[]);
    rig.verify(&second_endpoint);
    let second_target_tilts = second_endpoint.results[0]
        .achieved
        .outcomes
        .iter()
        .map(|outcome| {
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(
                outcome.result.angular_error_degrees.unwrap() < 0.04,
                "the accepted-branch Current endpoint still aims at the unchanged original Target"
            );
            assert_eq!(
                outcome.result.requested,
                Some(PositionFitRequest::Target {
                    world: Some(RigidTransform::DESK_TO_PROFILE.point([4., 8., 1.])),
                })
            );
            (outcome.destination, outcome.result.achieved.unwrap()[1])
        })
        .collect::<Vec<_>>();
    assert!(
        (second_target_tilts[0].1 - second_target_tilts[1].1).abs() > 1.,
        "the next captured Current endpoint remains independently resolved for each copy"
    );
    // Reference capture consumed 25ms; preserve the same 250ms oscillator interval.
    rig.clock.advance_millis(200);
    let second_capture = rig.capture();
    let second = prepare_live(
        &rig,
        &second_capture,
        &second_capture,
        &lane,
        &mut runtime,
        &mut origins,
        &mut scratch,
    )
    .unwrap();
    assert!(second.requirements.is_empty());
    verify_live_native(&rig, &second_capture, &lane, &second);
    let second_pan = second
        .sampled
        .samples
        .iter()
        .find(|s| s.lane_id == pan_lane)
        .unwrap();
    assert_eq!(
        (
            second_pan.instance_id,
            second_pan.controller_id,
            second_pan.activated_at_millis
        ),
        identity
    );
    let mut second_sources = Vec::new();
    second_pan
        .expression
        .visit_source_occurrences(&mut |id| second_sources.push(id))
        .unwrap();
    assert_eq!(second_sources, authored_sources);
    assert_eq!(
        runtime.snapshot().instances[0].started_at_millis,
        phase_anchor,
        "destination solving cannot restart the shared oscillator"
    );
    for (frame_index, published) in [&first, &second].into_iter().enumerate() {
        let sidecar = &published.results[0];
        let logical = program(sidecar);
        assert_eq!(
            logical.base, base,
            "original captured Target is retained before destination adoption"
        );
        assert_eq!(sidecar.achieved.destinations.len(), 2);
        assert_eq!(
            logical.samples.len(),
            1,
            "the original complete Angle pair competes as one family"
        );
        let mut pans = Vec::new();
        for destination in &sidecar.achieved.destinations {
            let actual = commanded_angles(&destination.value);
            let expected_tilts = if frame_index == 0 {
                &target_tilts
            } else {
                &second_target_tilts
            };
            let expected_tilt = expected_tilts
                .iter()
                .find(|(id, _)| *id == destination.destination)
                .unwrap()
                .1;
            assert!(
                (actual[1] - expected_tilt).abs() < 0.05,
                "frame {frame_index} destination {:?}: Tilt {} != captured branch {expected_tilt}; accepted joints {:?}",
                destination.destination,
                actual[1],
                accepted
                    .instances
                    .iter()
                    .find(|instance| instance.destination == destination.destination)
                    .map(|instance| &instance.joints)
            );
            assert!(
                destination
                    .provenance
                    .controls
                    .as_ref()
                    .is_some_and(|controls| controls.iter().any(|control| {
                        control.rank.dynamic_identity().is_some_and(|id| {
                            id.instance_id == identity.0
                                && id.controller_id == identity.1
                                && definition.lanes.iter().any(|lane| lane.id == id.lane_id)
                        })
                    })),
                "atomic pair coverage keeps its original instance/controller and highest-ranked pair lane"
            );
            let outcome = published.results[0]
                .achieved
                .outcomes
                .iter()
                .find(|o| o.destination == destination.destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(
                outcome
                    .result
                    .achieved
                    .unwrap()
                    .iter()
                    .zip(actual)
                    .all(|(a, b)| (a - b).abs() < 0.023)
            );
            pans.push((destination.destination, actual[0]));
        }
        assert_eq!(
            pans.iter().find(|(id, _)| *id == rig.root).unwrap().1,
            pans.iter().find(|(id, _)| *id == copy).unwrap().1,
            "one authored Pan waveform and phase feeds every destination"
        );
        for partner in published
            .sampled
            .samples
            .iter()
            .filter(|sample| sample.expression.angle_current_address().is_some())
        {
            partner
                .expression
                .visit_source_occurrences(&mut |_| {
                    panic!("Current Tilt must not acquire Pan authorship")
                })
                .unwrap();
        }
    }
    let first_angle = commanded_angles(&first.results[0].achieved.destinations[0].value)[0];
    let second_angle = commanded_angles(&second.results[0].achieved.destinations[0].value)[0];
    assert!(
        (second_angle - first_angle - 45.).abs() < 0.1,
        "shared waveform advances 250 ms without destination phase resets: {first_angle} -> {second_angle}"
    );
    assert_eq!(rig.engine.snapshot().dynamics[0], definition);
}
