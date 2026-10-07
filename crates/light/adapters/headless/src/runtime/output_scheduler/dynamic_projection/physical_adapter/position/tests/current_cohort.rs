//! Exact Current adopts one captured static mechanical cohort before logical-head sampling.
//! Original Current and one-active-owner endpoint overlays are distinct explicit cuts.
//! Independent activation envelopes are evaluated against complete mechanical endpoint cohorts.
use super::numeric::requirement_debug;
use super::programs::{commanded_angles, position_definition, program};
use super::*;
use light_dynamics::{
    DynamicDefinition, DynamicDefinitionSnapshot, DynamicFamilyRepresentation,
    DynamicInstanceOverrides, DynamicLaneBody, DynamicReference, DynamicValue, DynamicValueSource,
    DynamicValueTiming, ProgrammingLaneConfiguration, Rational,
};

pub(super) struct SharedRig {
    pub(super) rig: Rig,
    pub(super) heads: [FixtureId; 2],
    pub(super) targets: [AttributeValue; 2],
    pub(super) angles: [[f32; 2]; 2],
}

pub(super) fn shared_rig() -> SharedRig {
    shared_rig_at_pan(32768)
}
fn shared_rig_at_pan(pan_raw: u32) -> SharedRig {
    let mut profile = moving_head();
    let motor_head = profile.modes[0].heads[0].id;
    profile.modes[0].heads[0].master_shared = true;
    profile.modes[0].heads[0].name = "Shared Pan".into();
    let head_ids = [Uuid::new_v4(), Uuid::new_v4()];
    for (index, id) in head_ids.into_iter().enumerate() {
        profile.modes[0].heads.push(FixtureHead {
            id,
            name: format!("Independent Tilt {}", index + 1),
            master_shared: false,
        });
    }
    assert_eq!(profile.modes[0].channels[0].head_id, motor_head);
    profile.modes[0].channels[1].head_id = head_ids[0];
    let mut tilt_node = profile.geometry.nodes[2].clone();
    tilt_node.id = Uuid::new_v4();
    tilt_node.name = "Second independent Tilt".into();
    tilt_node.parent_id = Some(profile.geometry.nodes[1].id);
    let mut tilt_channel = profile.modes[0].channels[1].clone();
    tilt_channel.id = Uuid::new_v4();
    tilt_channel.head_id = head_ids[1];
    tilt_channel.secondary_slots = vec![6];
    for function in &mut tilt_channel.functions {
        function.id = Uuid::new_v4();
    }
    let tilt_binding = MotionFunctionBinding {
        node_id: tilt_node.id,
        channel_id: tilt_channel.id,
        function_id: tilt_channel.functions[0].id,
        role: PositionAxisRole::Tilt,
    };
    profile.modes[0].channels.push(tilt_channel);
    profile.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .bindings
        .push(tilt_binding);
    let mut second_lens = profile.geometry.emitters[0].clone();
    second_lens.id = Uuid::new_v4();
    second_lens.name = "Second independent lens".into();
    second_lens.node_id = tilt_node.id;
    second_lens.head_id = None;
    profile.geometry.nodes.push(tilt_node);
    profile.geometry.emitters.push(second_lens);
    profile.modes[0].emitter_heads = profile
        .geometry
        .emitters
        .iter()
        .zip(head_ids)
        .map(|(lens, head_id)| EmitterHeadBinding {
            emitter_id: lens.id,
            head_id,
        })
        .collect();
    profile.modes[0].splits[0].footprint = 6;
    profile.validate().unwrap();

    // Targets come from one exactly encoded native pose: Pan is common, both Tilt motors
    // differ. This avoids declaring two numerically unrelated inverse branches compatible.
    let forward = CompiledPositionForward::compile(
        &profile,
        profile.modes[0].id,
        PositionInstallation::default(),
    )
    .unwrap()
    .unwrap();
    let mut commands = forward.create_commands();
    forward
        .decode_commands(&[pan_raw, 31000, 35000], &mut commands)
        .unwrap();
    let axes = commands
        .iter()
        .map(|command| command.absolute_degrees())
        .collect::<Vec<_>>();
    let model = CompiledPositionFitting::compile(
        &profile,
        profile.modes[0].id,
        PositionInstallation::default(),
    )
    .unwrap()
    .unwrap();
    let angles = std::array::from_fn(|index| {
        let pair = model.emitter(index).unwrap().command_indices.unwrap();
        [axes[pair[0]].unwrap() as f32, axes[pair[1]].unwrap() as f32]
    });
    let mut poses = forward.create_output();
    forward
        .evaluate_pose(
            &axes,
            RigidTransform::IDENTITY,
            &mut forward.create_workspace(),
            &mut poses,
        )
        .unwrap();
    let targets = std::array::from_fn(|index| {
        let profile_world = poses[index].world.unwrap().point([0., -10., 0.]);
        let desk_world = RigidTransform::DESK_TO_PROFILE
            .inverse()
            .point(profile_world);
        target(
            TargetReference::Origin,
            desk_world.map(|value| value as f32),
        )
    });
    let root = FixtureId::new();
    let heads = [FixtureId::new(), FixtureId::new()];
    let mut fixture = patched(&profile, root, 1);
    fixture.logical_heads = head_ids
        .into_iter()
        .zip(heads)
        .enumerate()
        .map(|(index, (profile_head_id, fixture_id))| PatchedHead {
            profile_head_id: Some(profile_head_id),
            head_index: index as u16 + 1,
            fixture_id,
        })
        .collect();
    SharedRig {
        rig: Rig::new(vec![fixture], root),
        heads,
        targets,
        angles,
    }
}

fn current_definition() -> DynamicDefinition {
    let mut definition = position_definition(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        },
        [DynamicValue::Scalar(0.), DynamicValue::Scalar(0.)],
    );
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration else {
        unreachable!()
    };
    for point in &mut config.points {
        point.source = DynamicValueSource::Current;
    }
    definition
}

pub(super) fn start(
    shared: &SharedRig,
    bases: &[AttributeValue; 2],
    dynamics: &[(usize, DynamicDefinition, Option<u64>)],
) -> DynamicRuntime {
    start_sized(shared, bases, dynamics, 1.)
}

pub(super) fn start_sized(
    shared: &SharedRig,
    bases: &[AttributeValue; 2],
    dynamics: &[(usize, DynamicDefinition, Option<u64>)],
    controller_size: f32,
) -> DynamicRuntime {
    let rig = &shared.rig;
    let snapshot = rig.engine.snapshot();
    rig.engine
        .replace_snapshot(EngineSnapshot {
            dynamics: dynamics
                .iter()
                .map(|(_, definition, _)| definition.clone())
                .collect::<Vec<_>>()
                .into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    for (&head, base) in shared.heads.iter().zip(bases) {
        rig.programmers.set(
            rig.session,
            head,
            ProgrammingOwner::Position.key(),
            base.clone(),
        );
    }
    let mutations = dynamics
        .iter()
        .map(
            |(head, definition, fade)| DynamicProgrammerValueMutation::Set {
                fixture_id: shared.heads[*head],
                attribute: ProgrammingOwner::Position.key(),
                value: DynamicSemanticValue::DynamicOn {
                    instance_link: Uuid::new_v4(),
                    lane_id: definition.lanes[0].id,
                    dynamic: DynamicReference {
                        dynamic_id: Some(definition.id),
                        last_known_pool_number: definition.pool_number,
                        embedded_fallback: DynamicDefinitionSnapshot {
                            definition: Arc::new(definition.clone()),
                        },
                    },
                    overrides: DynamicInstanceOverrides {
                        size: controller_size,
                        speed_multiplier: Rational::ONE,
                        phase_offset_degrees: 0.,
                    },
                    timing: DynamicValueTiming {
                        fade_millis: *fade,
                        delay_millis: None,
                    },
                },
            },
        )
        .collect::<Vec<_>>();
    assert!(
        rig.programmers
            .apply_dynamic_values(rig.session, &mutations, None)
    );
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    runtime
        .install_definitions(dynamics.iter().map(|(_, definition, _)| definition.clone()))
        .unwrap();
    runtime
}

pub(super) fn run_frame(
    shared: &SharedRig,
    runtime: &mut DynamicRuntime,
    lane: &PhysicalAdapterLane<PositionAdapter>,
    origins: &mut DynamicSourceOrigins,
    scratch: &mut HybridFrameScratch,
) -> (PreparedOutputFrame, PublishedPhysicalFrame<PositionAdapter>) {
    let capture = shared.rig.capture();
    let continuity = shared
        .heads
        .map(|head| lane.continuity(head, ProgrammingOwner::Position));
    // The parent helper exercises the real captured Engine/geometry/native values, transactional
    // Dynamic sampling, observer composition and Live finalizer, with no fabricated frame inputs.
    let output = prepare_live(
        &shared.rig,
        &capture,
        &capture,
        lane,
        runtime,
        origins,
        scratch,
    )
    .unwrap();
    if output.requirements.is_empty() {
        for (&head, previous) in shared.heads.iter().zip(continuity) {
            if output.results.iter().any(|row| row.target == head) {
                assert!(lane.continuity(head, ProgrammingOwner::Position).is_some());
            } else {
                assert_eq!(lane.continuity(head, ProgrammingOwner::Position), previous);
            }
        }
    }
    (capture, output)
}

pub(super) fn verify_success(
    shared: &SharedRig,
    capture: &PreparedOutputFrame,
    output: &PublishedPhysicalFrame<PositionAdapter>,
    expected_rows: usize,
) {
    assert!(
        output.requirements.is_empty(),
        "{:?}",
        output
            .requirements
            .iter()
            .map(|row| (row.target, requirement_debug(&row.reason)))
            .collect::<Vec<_>>()
    );
    assert_eq!(output.results.len(), expected_rows);
    let rig = &shared.rig;
    let scalar = rig.engine.prepare_static_family_frame(capture, &[]);
    let mut raw = scalar
        .native_raw(capture, &capture.frame_token(), rig.root)
        .unwrap()
        .raw()
        .to_vec();
    let mut claims = std::collections::HashMap::new();
    for row in &output.results {
        let head = shared
            .heads
            .iter()
            .position(|id| *id == row.target)
            .unwrap();
        let baseline = scalar
            .value(row.target, &ProgrammingOwner::Position.key())
            .unwrap();
        let calculated = match &row.requested {
            PositionRequest::Program(_) => {
                assert_eq!(&program(row).base, baseline);
                Some(commanded_angles(&row.achieved.destinations[0].value))
            }
            PositionRequest::Intent(requested) => {
                assert_eq!(requested, intent(baseline).unwrap());
                assert!(matches!(
                    row.metadata.evidence,
                    FamilyProjectionEvidence::PreserveBaseline
                ));
                row.achieved.outcomes[0].result.achieved
            }
        };
        if row.quality.held {
            assert!(
                matches!(&row.requested, PositionRequest::Intent(_)),
                "only the unavailable static peer may remain held"
            );
            assert!(
                row.achieved
                    .outcomes
                    .iter()
                    .all(|outcome| outcome.result.status != PositionFitStatus::Fitted)
            );
            for write in &row.writes {
                if let Some(old) = claims.insert(write.slot.channel_index, write.raw) {
                    assert_eq!(old, write.raw);
                }
                raw[write.slot.channel_index as usize] = write.raw;
            }
            continue;
        }
        let calculated = calculated.expect("fitted static or Dynamic commanded angles");
        for axis in 0..2 {
            assert!(
                (calculated[axis] - f64::from(shared.angles[head][axis])).abs() < 0.04,
                "head {head} Current axis {axis}: {calculated:?} != {:?}",
                shared.angles[head]
            );
        }
        assert!(!row.quality.held);
        assert_eq!(row.achieved.outcomes.len(), 1);
        assert_eq!(
            row.achieved.outcomes[0].result.status,
            PositionFitStatus::Fitted
        );
        for write in &row.writes {
            assert!(!write.parked);
            if let Some(old) = claims.insert(write.slot.channel_index, write.raw) {
                assert_eq!(old, write.raw, "one shared motor cannot get two commands");
            }
            raw[write.slot.channel_index as usize] = write.raw;
        }
    }
    assert_eq!(claims.len(), if expected_rows == 2 { 3 } else { 2 });
    let snapshot = rig.engine.snapshot();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == rig.root)
        .unwrap();
    let profile = fixture.definition.profile_snapshot.as_ref().unwrap();
    let mut bytes = [0u8; 512];
    profile.modes[0]
        .compile_encoding_plan()
        .unwrap()
        .encode_split_by_index(
            &mut bytes,
            1,
            1,
            &(0u32..).zip(raw.iter().copied()).collect::<Vec<_>>(),
        )
        .unwrap();
    let decoded = (0..3)
        .map(|index| u32::from(u16::from_be_bytes([bytes[index * 2], bytes[index * 2 + 1]])))
        .collect::<Vec<_>>();
    assert_eq!(decoded, raw, "all three motors retain coarse/fine bytes");
    let forward = CompiledPositionForward::compile(
        profile,
        profile.modes[0].id,
        PositionInstallation::default(),
    )
    .unwrap()
    .unwrap();
    let mut commands = forward.create_commands();
    forward.decode_commands(&decoded, &mut commands).unwrap();
    let axes = commands
        .iter()
        .map(|command| command.absolute_degrees())
        .collect::<Vec<_>>();
    let model = CompiledPositionFitting::compile(
        profile,
        profile.modes[0].id,
        PositionInstallation::default(),
    )
    .unwrap()
    .unwrap();
    let mount = output
        .geometry
        .mounts()
        .mounts()
        .iter()
        .find(|mount| mount.instance_id == rig.root.0)
        .unwrap()
        .world_from_fixture
        .unwrap()
        .desk_pose_to_profile();
    let mut poses = forward.create_output();
    forward
        .evaluate_pose(&axes, mount, &mut forward.create_workspace(), &mut poses)
        .unwrap();
    for row in &output.results {
        let outcome = &row.achieved.outcomes[0].result;
        if row.quality.held {
            continue;
        }
        let emitter = model
            .emitters()
            .find(|emitter| emitter.emitter_id == outcome.emitter_id)
            .unwrap();
        let pair = emitter.command_indices.unwrap();
        assert_eq!(
            outcome.achieved.unwrap(),
            [axes[pair[0]].unwrap(), axes[pair[1]].unwrap()]
        );
        assert_eq!(outcome.pose, poses[emitter.emitter_index].world);
    }
}

#[test]
fn actual_live_shared_pan_independent_tilts_adopt_both_static_target_currents_as_one_cohort() {
    let shared = shared_rig();
    let mut second_definition = current_definition();
    second_definition.pool_number = 2;
    let definitions = [
        (0, current_definition(), None),
        (1, second_definition, None),
    ];
    let mut runtime = start(&shared, &shared.targets, &definitions);
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    let (capture, first) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    verify_success(&shared, &capture, &first, 2);
    let sources = first
        .sampled
        .samples
        .iter()
        .map(|row| {
            (
                row.target,
                row.lane_id,
                row.instance_id,
                row.controller_id,
                row.activated_at_millis,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(runtime.snapshot().instances.len(), 2);
    shared.rig.clock.advance_millis(100);
    let (capture, second) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    verify_success(&shared, &capture, &second, 2);
    assert_eq!(
        second
            .sampled
            .samples
            .iter()
            .map(|row| (
                row.target,
                row.lane_id,
                row.instance_id,
                row.controller_id,
                row.activated_at_millis
            ))
            .collect::<Vec<_>>(),
        sources
    );
    assert!(
        (shared.angles[0][1] - shared.angles[1][1]).abs() > 20.,
        "Tilt motors genuinely differ"
    );
    assert_eq!(shared.angles[0][0], shared.angles[1][0]);
}

#[test]
fn actual_live_shared_pan_current_includes_static_peer_and_protects_changed_shared_controls() {
    for variant in 0..4 {
        let shared = if variant == 3 {
            shared_rig_at_pan(28000)
        } else {
            shared_rig()
        };
        let peer = match variant {
            0 => angles(shared.angles[1][0], shared.angles[1][1]),
            1 | 3 => target(
                TargetReference::Point {
                    point_id: Uuid::new_v4(),
                },
                [0.; 3],
            ),
            _ => angles(shared.angles[1][0] + 90., shared.angles[1][1]),
        };
        let bases = [shared.targets[0].clone(), peer];
        let mut runtime = start(&shared, &bases, &[(0, current_definition(), None)]);
        let lane = PhysicalAdapterLane::live(PositionAdapter::default());
        let (capture, output) = run_frame(
            &shared,
            &mut runtime,
            &lane,
            &mut DynamicSourceOrigins::default(),
            &mut HybridFrameScratch::default(),
        );
        if variant <= 1 {
            verify_success(&shared, &capture, &output, 2);
            if variant == 1 {
                let baseline = shared.rig.engine.prepare_static_family_frame(&capture, &[]);
                let native = baseline
                    .native_raw(&capture, &capture.frame_token(), shared.rig.root)
                    .unwrap();
                let pan = output.results[0]
                    .writes
                    .iter()
                    .find(|write| write.slot.channel_index == 0)
                    .unwrap();
                assert_eq!(
                    pan.raw,
                    native.raw()[0],
                    "missing peer allows only unchanged shared Pan"
                );
                assert_ne!(
                    output.results[0]
                        .writes
                        .iter()
                        .find(|write| write.slot.channel_index == 1)
                        .unwrap()
                        .raw,
                    native.raw()[1],
                    "independent Tilt can still move"
                );
            }
            assert!(
                lane.continuity(shared.heads[1], ProgrammingOwner::Position)
                    .is_some(),
                "static peer now participates in the accepted final mechanical cohort"
            );
        } else {
            assert!(
                output
                    .requirements
                    .iter()
                    .any(|row| row.target == shared.heads[0]
                        && row.owner == ProgrammingOwner::Position),
                "unavailable static mechanical peer variant {variant} must withhold Current adoption; rows: {:?}",
                output
                    .results
                    .iter()
                    .map(|row| (&row.value, &row.quality, &row.achieved))
                    .collect::<Vec<_>>()
            );
            assert!(
                output
                    .results
                    .iter()
                    .flat_map(|row| &row.achieved.outcomes)
                    .all(|outcome| outcome.result.status != PositionFitStatus::Fitted)
            );
            assert!(
                lane.continuity(shared.heads[0], ProgrammingOwner::Position)
                    .is_none(),
                "a failed cohort cannot publish successful Current continuity"
            );
        }
    }
}

pub(super) fn authored_target_definition(
    shared: &SharedRig,
    head: usize,
    pool: u16,
) -> DynamicDefinition {
    let authored = shared.targets[head].clone();
    let mut definition = position_definition(
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, &authored).unwrap(),
        [
            DynamicValue::Family(authored.clone()),
            DynamicValue::Family(authored),
        ],
    );
    definition.pool_number = pool;
    definition
}

#[test]
fn actual_live_single_active_target_endpoint_includes_constant_angle_or_target_peer() {
    for peer_is_target in [false, true] {
        let shared = shared_rig();
        let bases = [
            angles(shared.angles[0][0], shared.angles[0][1]),
            if peer_is_target {
                shared.targets[1].clone()
            } else {
                angles(shared.angles[1][0], shared.angles[1][1])
            },
        ];
        let mut runtime = start(
            &shared,
            &bases,
            &[(0, authored_target_definition(&shared, 0, 1), Some(1000))],
        );
        let authored =
            serde_json::to_value(shared.rig.programmers.get(shared.rig.session).unwrap()).unwrap();
        let lane = PhysicalAdapterLane::live(PositionAdapter::default());
        let mut origins = DynamicSourceOrigins::default();
        let mut scratch = HybridFrameScratch::default();
        let (capture, first) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
        verify_success(&shared, &capture, &first, 2);
        let active = first
            .results
            .iter()
            .find(|row| row.target == shared.heads[0])
            .unwrap();
        let requested = program(active);
        assert_eq!(requested.base, bases[0]);
        assert!(
            !requested.samples.is_empty(),
            "original retained Dynamic program remains authored"
        );
        let peer = first
            .results
            .iter()
            .find(|row| row.target == shared.heads[1])
            .unwrap();
        assert!(
            matches!(&peer.requested,PositionRequest::Intent(requested) if requested == intent(&bases[1]).unwrap())
        );
        let peer_sources = peer
            .provenance
            .sources
            .entries()
            .expect("static peer retains exact captured evidence");
        assert!(!peer_sources.is_empty());
        if let Some(active_sources) = active.provenance.sources.entries() {
            assert_ne!(
                active_sources
                    .iter()
                    .map(|entry| entry.record().occurrence_id)
                    .collect::<Vec<_>>(),
                peer_sources
                    .iter()
                    .map(|entry| entry.record().occurrence_id)
                    .collect::<Vec<_>>()
            );
        } else {
            assert!(
                active.provenance.sources.unknown().is_some(),
                "an unresolved physical transfer stays explicitly unknown, never rebranded as the static peer"
            );
        }
        for head in shared.heads {
            assert!(lane.continuity(head, ProgrammingOwner::Position).is_some());
        }
        shared.rig.clock.advance_millis(100);
        let (capture, second) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
        verify_success(&shared, &capture, &second, 2);
        assert_eq!(runtime.snapshot().instances.len(), 1);
        assert_eq!(
            serde_json::to_value(shared.rig.programmers.get(shared.rig.session).unwrap()).unwrap(),
            authored,
            "speculative endpoint cohort fitting cannot rewrite programming"
        );
    }
}

// Check the accepted public output, not only fitter scratch: both owners must agree on
// every shared word, and all accepted owner controls must be those exact published words.
pub(super) fn verify_shared_native_acceptance(
    shared: &SharedRig,
    lane: &PhysicalAdapterLane<PositionAdapter>,
    output: &PublishedPhysicalFrame<PositionAdapter>,
    destinations: &[(FixtureId, usize)],
) {
    assert!(output.requirements.is_empty());
    assert_eq!(output.results.len(), 2);
    assert_eq!(lane.last_accepted(), Some(output.token.clone()));
    let mut claims = std::collections::HashMap::new();
    for head in shared.heads {
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        assert!(!row.quality.held);
        let continuity = lane.continuity(head, ProgrammingOwner::Position).unwrap();
        assert_eq!(continuity.instances.len(), destinations.len());
        assert_eq!(row.achieved.outcomes.len(), destinations.len());
        for &(destination, _) in destinations {
            let accepted = continuity
                .instances
                .iter()
                .find(|instance| instance.destination == destination)
                .unwrap();
            let outcome = row
                .achieved
                .outcomes
                .iter()
                .find(|outcome| outcome.destination == destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(!outcome.input_requirement && !outcome.missing_mount);
            let writes = row
                .writes
                .iter()
                .filter(|write| write.slot.destination == destination)
                .collect::<Vec<_>>();
            assert_eq!(writes.len(), 2);
            assert_eq!(accepted.controls.len(), writes.len());
            for write in writes {
                assert!(!write.parked);
                let control = accepted
                    .controls
                    .iter()
                    .find(|(index, id, _, _)| {
                        *index == write.slot.channel_index && *id == write.channel_id
                    })
                    .unwrap();
                assert_eq!(
                    control.3, write.raw,
                    "accept only the actual full-width published control"
                );
                if let Some(previous) =
                    claims.insert((destination, write.slot.channel_index), write.raw)
                {
                    assert_eq!(
                        previous, write.raw,
                        "both owners agree on the shared Pan control"
                    );
                }
            }
        }
    }
    assert_eq!(claims.len(), destinations.len() * 3);
    for &(destination, start) in destinations {
        let physical = output
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == destination.0)
            .unwrap();
        assert!(physical.complete);
        for channel in 0..3u32 {
            let slot = start + channel as usize * 2;
            let bytes = &output.rendered.universes[&1];
            let wire = u32::from(u16::from_be_bytes([bytes[slot], bytes[slot + 1]]));
            assert_eq!(wire, claims[&(destination, channel)]);
            assert_eq!(physical.native_raw[channel as usize], wire);
        }
    }
}

#[test]
fn actual_live_independent_shared_target_envelopes_match_cartesian_endpoint_cohorts_per_copy() {
    let shared = shared_rig();
    let bases = shared.angles.map(|axes| angles(axes[0], axes[1]));
    let snapshot = shared.rig.engine.snapshot();
    let profile = snapshot.fixtures[0]
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap();
    let forward = CompiledPositionForward::compile(
        profile,
        profile.modes[0].id,
        PositionInstallation::default(),
    )
    .unwrap()
    .unwrap();
    let targets: [AttributeValue; 2] = std::array::from_fn(|index| {
        let mut axes = [
            Some(f64::from(shared.angles[0][0])),
            Some(f64::from(shared.angles[0][1])),
            Some(f64::from(shared.angles[1][1])),
        ];
        axes[index + 1] = axes[index + 1].map(|value| value + [4., 8.][index]);
        let mut poses = forward.create_output();
        forward
            .evaluate_pose(
                &axes,
                RigidTransform::IDENTITY,
                &mut forward.create_workspace(),
                &mut poses,
            )
            .unwrap();
        let world = RigidTransform::DESK_TO_PROFILE
            .inverse()
            .point(poses[index].world.unwrap().point([0., -10., 0.]));
        target(TargetReference::Origin, world.map(|value| value as f32))
    });
    let emitter_ids = profile
        .geometry
        .emitters
        .iter()
        .map(|emitter| emitter.id)
        .collect::<Vec<_>>();
    let copy = FixtureId::new();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    fixtures[0].multipatch.push(MultiPatchInstance {
        id: copy.0,
        universe: Some(1),
        address: Some(20),
        location: FixtureLocation {
            z: 1000,
            ..Default::default()
        },
        invert_pan: true,
        position_calibration: Some(InstalledPositionCalibration {
            tilt_zero_degrees: -7.,
            ..Default::default()
        }),
        ..Default::default()
    });
    shared
        .rig
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    let definitions = std::array::from_fn::<_, 2, _>(|index| {
        let mut definition = position_definition(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &targets[index]).unwrap(),
            [
                DynamicValue::Family(targets[index].clone()),
                DynamicValue::Family(targets[index].clone()),
            ],
        );
        definition.pool_number = index as u16 + 1;
        (index, definition, Some(1000))
    });
    let mut runtime = start(&shared, &bases, &definitions);
    let authored =
        serde_json::to_value(shared.rig.programmers.get(shared.rig.session).unwrap()).unwrap();

    // Fit all four actual complete mechanical endpoint cohorts, independently of the
    // continuation implementation. Neither an equal activation time nor one owner's
    // successful fit proves that another owner's endpoint can safely be substituted.
    let mut pairs = std::collections::HashMap::new();
    for choice in 0..4usize {
        let requested: [(FixtureId, AttributeValue); 2] = std::array::from_fn(|index| {
            (
                shared.heads[index],
                if choice & (1 << index) == 0 {
                    bases[index].clone()
                } else {
                    targets[index].clone()
                },
            )
        });
        let resolved = shared.rig.resolve(&requested);
        assert_eq!(resolved.results.len(), 2);
        let mut controls = std::collections::HashMap::new();
        for (index, row) in resolved.results.iter().enumerate() {
            assert!(row.requested == *intent(&requested[index].1).unwrap());
            assert_eq!(row.achieved.outcomes.len(), 2);
            for outcome in &row.achieved.outcomes {
                assert_eq!(outcome.result.emitter_id, emitter_ids[index]);
                assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
                assert!(!outcome.input_requirement && !outcome.missing_mount);
                assert!(
                    pairs
                        .insert(
                            (choice, index, outcome.destination),
                            outcome.result.achieved.unwrap()
                        )
                        .is_none()
                );
            }
            for write in &row.writes {
                assert!(!write.parked);
                if let Some(previous) = controls.insert(
                    (write.slot.destination, write.slot.channel_index),
                    write.raw,
                ) {
                    assert_eq!(
                        previous, write.raw,
                        "Cartesian endpoints agree on the actual shared control"
                    );
                }
            }
        }
        assert_eq!(controls.len(), 6);
    }
    for destination in [shared.rig.root, copy] {
        for index in 0..2 {
            for own in [0usize, 1 << index] {
                let a = pairs[&(own, index, destination)];
                let b = pairs[&(own | (1 << (1 - index)), index, destination)];
                for axis in 0..2 {
                    assert!(
                        (a[axis] - b[axis]).abs() < 0.03,
                        "peer endpoint choice must not alter this owner's fitted endpoint: {a:?} != {b:?}"
                    );
                }
            }
        }
    }
    for index in 0..2 {
        let root_target = pairs[&(1 << index, index, shared.rig.root)];
        assert!(
            (root_target[1] - f64::from(shared.angles[index][1]) - [4., 8.][index]).abs() < 0.05
        );
        assert!(
            (root_target[1] - pairs[&(1 << index, index, copy)][1]).abs() > 1.,
            "broadcasting a root endpoint must fail this displaced/calibrated copy oracle"
        );
    }
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    shared.rig.clock.advance_millis(250);
    let (capture, output) = run_frame(&shared, &mut runtime, &lane, &mut origins, &mut scratch);
    verify_shared_native_acceptance(&shared, &lane, &output, &[(shared.rig.root, 0), (copy, 19)]);
    assert_eq!(output.token, capture.frame_token());
    assert_eq!(output.sampled.samples.len(), 2);
    let runtime_state = runtime.snapshot();
    assert_eq!(runtime_state.instances.len(), 2);
    let mut identities = std::collections::HashSet::new();
    for (index, head) in shared.heads.into_iter().enumerate() {
        let sample = output
            .sampled
            .samples
            .iter()
            .find(|sample| sample.target == head)
            .unwrap();
        let instance = runtime_state
            .instances
            .iter()
            .find(|instance| instance.targets == vec![head])
            .unwrap();
        assert_eq!(instance.controllers.len(), 1);
        assert_eq!(
            (sample.instance_id, sample.controller_id),
            (instance.id, instance.controllers[0].id)
        );
        assert!(identities.insert((sample.instance_id, sample.controller_id)));
        assert!(sample.activation_mix > 0. && sample.activation_mix < 1.);
        let mix = f64::from(sample.activation_mix);
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        let original = program(row);
        assert_eq!(original.base, bases[index]);
        assert_eq!(original.samples.len(), 1);
        let (rank, original_mix) = match &original.samples[0] {
            light_dynamics::FamilyCompositionSample::Known(value) => {
                (&value.rank, value.activation_mix)
            }
            light_dynamics::FamilyCompositionSample::WholeExpression {
                rank,
                activation_mix,
                ..
            }
            | light_dynamics::FamilyCompositionSample::CoupledExpression {
                rank,
                activation_mix,
                ..
            } => (rank, *activation_mix),
        };
        let identity = rank
            .dynamic_identity()
            .expect("retain actual Dynamic ownership");
        assert_eq!(
            (
                identity.instance_id,
                identity.controller_id,
                identity.lane_id
            ),
            (sample.instance_id, sample.controller_id, sample.lane_id)
        );
        assert_eq!(original_mix, sample.activation_mix);
        assert_eq!(row.achieved.destinations.len(), 2);
        for destination in [shared.rig.root, copy] {
            let computed = row
                .achieved
                .destinations
                .iter()
                .find(|value| value.destination == destination)
                .unwrap();
            let actual = commanded_angles(&computed.value);
            let a = pairs[&(0, index, destination)];
            let b = pairs[&(1 << index, index, destination)];
            let physical = output
                .rendered
                .physical
                .instances
                .iter()
                .find(|instance| instance.instance_id == destination.0)
                .unwrap();
            for axis in 0..2 {
                let expected = a[axis] + mix * (b[axis] - a[axis]);
                assert!(
                    (actual[axis] - expected).abs() < 0.07,
                    "owner {index} destination {destination:?} own activation exactly once: {actual:?} vs {a:?}->{b:?} at {mix}"
                );
                let command = physical.axes()[if axis == 0 { 0 } else { index + 1 }]
                    .absolute_degrees()
                    .unwrap();
                assert!(
                    (command - expected).abs() < 0.07,
                    "rendered calibrated commands match the independent oracle"
                );
            }
        }
    }
    assert_eq!(
        identities.len(),
        2,
        "owner-local envelopes do not invent a shared producer scope"
    );
    assert_eq!(
        serde_json::to_value(shared.rig.programmers.get(shared.rig.session).unwrap()).unwrap(),
        authored
    );
    assert_eq!(
        shared.rig.engine.snapshot().dynamics.as_ref(),
        &definitions
            .iter()
            .map(|(_, definition, _)| definition.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn single_active_endpoint_does_not_override_missing_or_conflicting_static_peer() {
    for missing in [false, true] {
        let shared = if missing {
            shared_rig_at_pan(28000)
        } else {
            shared_rig()
        };
        let peer = if missing {
            target(
                TargetReference::Point {
                    point_id: Uuid::new_v4(),
                },
                [0.; 3],
            )
        } else {
            angles(shared.angles[1][0] + 90., shared.angles[1][1])
        };
        let bases = [angles(shared.angles[0][0], shared.angles[0][1]), peer];
        let mut runtime = start(
            &shared,
            &bases,
            &[(0, authored_target_definition(&shared, 0, 1), Some(1000))],
        );
        let lane = PhysicalAdapterLane::live(PositionAdapter::default());
        let (_, output) = run_frame(
            &shared,
            &mut runtime,
            &lane,
            &mut DynamicSourceOrigins::default(),
            &mut HybridFrameScratch::default(),
        );
        assert!(output.requirements.iter().any(|row| row.target == shared.heads[0] && row.owner == ProgrammingOwner::Position),
            "missing/conflicting static peer must not be treated as a free shared motor");
        assert!(
            output
                .results
                .iter()
                .all(|row| row.target != shared.heads[0])
        );
        assert!(
            lane.continuity(shared.heads[0], ProgrammingOwner::Position)
                .is_none()
        );
        assert!(
            output
                .results
                .iter()
                .flat_map(|row| &row.achieved.outcomes)
                .all(|outcome| outcome.result.status != PositionFitStatus::Fitted)
        );
    }
}

#[test]
fn single_active_endpoint_does_not_treat_requirements_only_angle_peer_as_static() {
    use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::HybridFamilyRequirementReason;
    let shared = shared_rig();
    let bases = shared.angles.map(|axes| angles(axes[0], axes[1]));
    let mut missing = current_definition();
    missing.pool_number = 2;
    let DynamicLaneBody::Programming(body) = &mut missing.lanes[0].body else {
        unreachable!()
    };
    let address = body.address.clone();
    let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
        unreachable!()
    };
    for point in &mut configuration.points {
        point.source = DynamicValueSource::Preset {
            preset_id: "999.999".into(),
            address: address.clone(),
            last_valid_by_target: vec![],
            retained: None,
        };
    }
    // Pan cannot resolve, so its mandatory Current Tilt partner cannot enter as a lone lane.
    // The peer still exists as a requirements-only captured family; its static baseline is not
    // permission to substitute it for an unavailable Dynamic endpoint.
    let mut runtime = start(
        &shared,
        &bases,
        &[
            (0, authored_target_definition(&shared, 0, 1), Some(1000)),
            (1, missing, None),
        ],
    );
    let authored =
        serde_json::to_value(shared.rig.programmers.get(shared.rig.session).unwrap()).unwrap();
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let (_, output) = run_frame(
        &shared,
        &mut runtime,
        &lane,
        &mut DynamicSourceOrigins::default(),
        &mut HybridFrameScratch::default(),
    );
    assert!(
        output
            .requirements
            .iter()
            .any(|row| row.target == shared.heads[1]
                && row.owner == ProgrammingOwner::Position
                && matches!(&row.reason, HybridFamilyRequirementReason::Input(_))),
        "missing Preset produces a captured peer input requirement"
    );
    assert!(
        output
            .requirements
            .iter()
            .any(|row| row.target == shared.heads[0]
                && row.owner == ProgrammingOwner::Position
                && matches!(
                    &row.reason,
                    HybridFamilyRequirementReason::Composition(
                        TransitionRequirement::LiveJointAngles
                    )
                )),
        "requirements-only peer must block arbitrary endpoint adoption even without composable samples"
    );
    for head in shared.heads {
        assert!(output.results.iter().all(|row| row.target != head));
        assert!(lane.continuity(head, ProgrammingOwner::Position).is_none());
    }
    assert_eq!(
        serde_json::to_value(shared.rig.programmers.get(shared.rig.session).unwrap()).unwrap(),
        authored
    );
}
