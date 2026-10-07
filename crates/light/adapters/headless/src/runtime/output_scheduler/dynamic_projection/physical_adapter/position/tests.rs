//! TL-556 references use real captured engine/native/geometry inputs. Synthetic physical
//! profiles describe authored commands, not measured motor feedback or a real lamp calibration.
pub(in crate::runtime) use super::super::color::profiles::patched;
use super::super::color::profiles::rgb;
use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use crate::runtime::output_scheduler::dynamic_projection::CapturedDynamicInputs;
use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::HybridFrameScratch;
#[derive(Default)]
pub(in crate::runtime) struct AuthoringScratch(HybridFrameScratch);
use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::prepare_captured_hybrid_frame_with_observer;
use light_core::programming::*;
use light_core::{AttributeKey, ManualClock, SessionId};
use light_dynamics::{
    DynamicOutputFrameScratch, DynamicRuntime, DynamicRuntimeError, DynamicSemanticValue,
    DynamicSpeedTransport, ProgrammingFamilyFixAt,
};
use light_engine::{Engine, EngineSnapshot, PreloadBranch, PreparedOutputFrame};
use light_fixture::forward::{CompiledPositionForward, PositionInstallation};
use light_fixture::*;
use light_programmer::{DynamicProgrammerValueMutation, ProgrammerRegistry};

mod continuations;
mod cross_mode_fades;
mod current_cohort;
mod declared_default;
mod derived_profiles;
mod envelope_cohort;
mod fit_cache;
mod fixed_peer;
mod independent_resume;
mod instance_baseline;
mod mask_cohort;
mod masks;
mod native_projection;
mod nested_resume;
mod numeric;
mod operation_cohort;
mod output_gates;
mod position_compatibility;
mod programs;
mod root_emitter;
mod shared_resume;
mod static_peer_cuts;
mod static_programs;
mod suppressed_peers;
mod tracking;
mod tracking_bench;

fn channel(head: Uuid, attribute: &str, slot: u16) -> FixtureChannel {
    let mut channel = rgb().modes[0].channels[0].clone();
    channel.id = Uuid::new_v4();
    channel.head_id = head;
    channel.attribute = AttributeKey(attribute.into());
    channel.fixture_attribute = channel.attribute.clone();
    channel.resolution = ChannelResolution::U16;
    channel.secondary_slots = vec![slot + 1];
    channel.default_raw = 32768;
    channel.highlight_raw = u16::MAX.into();
    channel.physical_min = None;
    channel.physical_max = None;
    channel.functions = vec![ChannelFunction::continuous(
        attribute,
        channel.attribute.clone(),
        u16::MAX.into(),
    )];
    channel
}

pub(in crate::runtime) fn moving_head() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "TL-556 authored U16 two-axis".into();
    let head = profile.modes[0].heads[0].id;
    profile.geometry = GeometryGraph::template(GeometryTemplate::MovingHead, &[head]);
    profile.geometry.physical_contract = Some(GeometryPhysicalContract {
        version: 1,
        provenance: OpticalProvenance::default(),
        bracket: GeometryBracket::Fixed,
    });
    let mut bindings = Vec::new();
    for (index, role) in [PositionAxisRole::Pan, PositionAxisRole::Tilt]
        .into_iter()
        .enumerate()
    {
        let mut channel = channel(
            head,
            if index == 0 { "pan" } else { "tilt" },
            1 + 2 * index as u16,
        );
        channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
            physical_min: 720.,
            physical_max: -720.,
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
        profile.modes[0].channels.push(channel);
    }
    profile.modes[0].splits[0].footprint = 4;
    profile.modes[0].position_physical = Some(PositionPhysicalModel {
        kinematics: Default::default(),
        version: 1,
        revision: 0,
        bindings,
    });
    let emitter = GeometryEmitter {
        id: Uuid::new_v4(),
        name: "Lens".into(),
        node_id: profile.geometry.nodes[2].id,
        head_id: None,
        origin: Vector3 {
            x: 0.,
            y: -600.,
            z: 100.,
        },
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
    profile.validate().unwrap();
    profile
}

pub(in crate::runtime) fn point(id: FixtureId, location: FixtureLocation) -> PatchedFixture {
    let mut profile = FixtureProfile::blank();
    let head = profile.modes[0].heads[0].id;
    profile.manufacturer = "Test".into();
    profile.name = "TL-556 Point".into();
    for (index, attribute) in [
        "point.position.x",
        "point.position.y",
        "point.position.z",
        "point.rotation.x",
        "point.rotation.y",
        "point.rotation.z",
    ]
    .iter()
    .enumerate()
    {
        profile.modes[0]
            .channels
            .push(channel(head, attribute, 1 + 2 * index as u16));
    }
    profile.modes[0].splits[0].footprint = 12;
    let mut point = patched(&profile, id, 1);
    point.fixture_number = None;
    point.universe = None;
    point.address = None;
    point.location = location;
    point
}

pub(in crate::runtime) fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
pub(in crate::runtime) fn target(reference: TargetReference, offset: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(reference, offset)))
}

struct Rig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    root: FixtureId,
    adapter: PositionAdapter,
    clock: Arc<ManualClock>,
}
struct Resolved {
    results: Vec<PhysicalResolution<PositionAdapter>>,
    native: Vec<u32>,
    instance_native: Vec<(FixtureId, Vec<u32>)>,
    mounts: Vec<(FixtureId, RigidTransform)>,
}
impl Rig {
    fn native_baselines(
        &self,
        capture: &PreparedOutputFrame,
        scalar: &light_engine::PreparedStaticFamilyFrame,
    ) -> Vec<(FixtureId, Vec<u32>)> {
        let snapshot = capture.snapshot();
        let fixture = snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_id == self.root)
            .unwrap();
        std::iter::once(self.root.0)
            .chain(fixture.multipatch.iter().map(|copy| copy.id))
            .map(|id| {
                let mut out = light_engine::CapturedNativeRaw::default();
                scalar
                    .native_position_raw_into(
                        capture,
                        &capture.frame_token(),
                        self.root,
                        id,
                        &mut out,
                    )
                    .unwrap();
                assert_eq!(out.instance_id(), Some(id));
                (FixtureId(id), out.raw().to_vec())
            })
            .collect()
    }
    fn new(fixtures: Vec<PatchedFixture>, root: FixtureId) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        let engine = Engine::with_programming_contract_support(
            programmers.clone(),
            PROGRAMMING_CONTRACT_VERSION,
        );
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: fixtures.into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        let rig = Self {
            engine,
            programmers,
            session,
            root,
            adapter: Default::default(),
            clock,
        };
        for fixture in rig.engine.snapshot().fixtures.iter() {
            for parameter in fixture
                .definition
                .heads
                .iter()
                .flat_map(|h| h.parameters.iter())
            {
                if parameter.attribute.0.starts_with("point.") {
                    rig.set(fixture.fixture_id, parameter.attribute.0.as_ref(), 0.5);
                }
            }
        }
        rig
    }
    fn single() -> Self {
        let profile = moving_head();
        let root = FixtureId::new();
        Self::new(vec![patched(&profile, root, 1)], root)
    }
    fn set(&self, fixture: FixtureId, attribute: &str, value: f32) {
        self.programmers.set(
            self.session,
            fixture,
            AttributeKey(attribute.into()),
            AttributeValue::Normalized(value),
        );
    }
    fn capture(&self) -> PreparedOutputFrame {
        self.clock.advance_millis(25);
        self.engine.prepare_output_frame(Default::default())
    }
    fn resolve(&self, values: &[(FixtureId, AttributeValue)]) -> Resolved {
        self.resolve_with(values, None, &[])
    }
    fn resolve_with(
        &self,
        values: &[(FixtureId, AttributeValue)],
        previous: Option<&PositionContinuity>,
        protected: &[FixtureId],
    ) -> Resolved {
        let capture = self.capture();
        let token = capture.frame_token();
        let mut scalar = self.engine.prepare_static_family_frame(&capture, &[]);
        let geometry = self
            .engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let models = DynamicRuntime::default().captured_native_color_models();
        let frame = HybridFrameContext {
            capture: &capture,
            geometry: &geometry,
            native_models: models.as_ref(),
            token: &token,
            scalar: &scalar,
        };
        let descriptors = values
            .iter()
            .map(|(id, _)| {
                self.adapter
                    .compile(&capture.snapshot(), *id)
                    .unwrap()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let requests = values
            .iter()
            .zip(&descriptors)
            .map(|((target, value), descriptor)| PhysicalRequest {
                frame,
                target: *target,
                owner: ProgrammingOwner::Position,
                descriptor,
                value,
                previous,
            })
            .collect::<Vec<_>>();
        let results = self.adapter.resolve_cohort(&requests, protected).unwrap();
        for (descriptor, result) in descriptors.iter().zip(&results) {
            validate_complete_writes(&descriptor.footprint, &result.writes).unwrap();
        }
        Resolved {
            results,
            native: self.native_baselines(&capture, &scalar)[0].1.clone(),
            instance_native: self.native_baselines(&capture, &scalar),
            mounts: geometry
                .mounts()
                .mounts()
                .iter()
                .filter_map(|mount| {
                    mount
                        .world_from_fixture
                        .map(|world| (FixtureId(mount.instance_id), world))
                })
                .collect(),
        }
    }
    fn verify(&self, resolved: &Resolved) {
        let snapshot = self.engine.snapshot();
        let fixture = snapshot
            .fixtures
            .iter()
            .find(|f| f.fixture_id == self.root)
            .unwrap();
        let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
        for result in &resolved.results {
            for outcome in &result.achieved.outcomes {
                let destination = outcome.destination;
                let install = if destination == self.root {
                    PositionInstallation {
                        calibration: fixture.position_calibration.as_ref(),
                        invert_pan: fixture.invert_pan,
                        invert_tilt: fixture.invert_tilt,
                        bracket_degrees: f64::from(fixture.bracket_angle),
                    }
                } else {
                    let copy = fixture
                        .multipatch
                        .iter()
                        .find(|c| c.id == destination.0)
                        .unwrap();
                    PositionInstallation {
                        calibration: copy.position_calibration.as_ref(),
                        invert_pan: copy.invert_pan,
                        invert_tilt: copy.invert_tilt,
                        bracket_degrees: f64::from(copy.bracket_angle),
                    }
                };
                let forward =
                    CompiledPositionForward::compile(profile, profile.modes[0].id, install)
                        .unwrap()
                        .unwrap();
                let mut native = resolved
                    .instance_native
                    .iter()
                    .find(|(id, _)| *id == destination)
                    .unwrap()
                    .1
                    .clone();
                for write in result
                    .writes
                    .iter()
                    .filter(|w| w.slot.destination == destination)
                {
                    native[write.slot.channel_index as usize] = write.raw;
                }
                let mut bytes = [0u8; 512];
                profile.modes[0]
                    .compile_encoding_plan()
                    .unwrap()
                    .encode_split_by_index(
                        &mut bytes,
                        1,
                        1,
                        &(0u32..).zip(native.iter().copied()).collect::<Vec<_>>(),
                    )
                    .unwrap();
                assert!(profile.modes[0].channels.iter().all(|channel| {
                    channel.resolution == ChannelResolution::U16 && channel.split == 1
                }));
                let decoded = bytes
                    .chunks_exact(2)
                    .take(native.len())
                    .map(|pair| u32::from(u16::from_be_bytes([pair[0], pair[1]])))
                    .collect::<Vec<_>>();
                assert_eq!(
                    decoded, native,
                    "every coarse/fine pair retains the complete native commands"
                );
                let mut commands = forward.create_commands();
                forward.decode_commands(&decoded, &mut commands).unwrap();
                let axes = commands
                    .iter()
                    .map(|c| c.absolute_degrees())
                    .collect::<Vec<_>>();
                let mount = resolved
                    .mounts
                    .iter()
                    .find(|(id, _)| *id == destination)
                    .unwrap()
                    .1
                    .desk_pose_to_profile();
                let mut poses = forward.create_output();
                forward
                    .evaluate_pose(&axes, mount, &mut forward.create_workspace(), &mut poses)
                    .unwrap();
                let pose = poses
                    .iter()
                    .find(|p| p.emitter_id == outcome.result.emitter_id)
                    .unwrap();
                assert_eq!(outcome.result.pose, pose.world);
                if let Some(achieved) = outcome.result.achieved {
                    let pan = commands
                        .iter()
                        .position(|c| c.role == Some(PositionAxisRole::Pan))
                        .unwrap();
                    let tilt = commands
                        .iter()
                        .position(|c| c.role == Some(PositionAxisRole::Tilt))
                        .unwrap();
                    assert_eq!([axes[pan].unwrap(), axes[tilt].unwrap()], achieved);
                }
            }
        }
    }
}

#[test]
fn captured_position_cohort_keeps_unwrapped_requests_and_clips_only_achieved_output() {
    let rig = Rig::single();
    for (pan, tilt, expected, clipped) in [
        (450., 30., [450., 30.], false),
        (9999., -9999., [720., -720.], true),
    ] {
        let value = angles(pan, tilt);
        let resolved = rig.resolve(&[(rig.root, value.clone())]);
        rig.verify(&resolved);
        let result = &resolved.results[0];
        assert_eq!(result.requested, intent(&value).unwrap().clone());
        let fitted = &result.achieved.outcomes[0].result;
        assert_eq!(fitted.status, PositionFitStatus::Fitted);
        assert_eq!(fitted.clipped, clipped);
        for (actual, expected) in fitted.achieved.unwrap().into_iter().zip(expected) {
            assert!((actual - expected).abs() < 0.023, "{actual} != {expected}");
        }
        assert!(result.writes.iter().all(|w| !w.parked));
    }
}

#[test]
fn captured_point_local_offset_and_independent_moving_mount_keep_the_same_world_aim() {
    let profile = moving_head();
    let root = FixtureId::new();
    let aim = FixtureId::new();
    let mount = FixtureId::new();
    let mut fixture = patched(&profile, root, 1);
    fixture.position_master = Some(mount.0);
    fixture.location.z = 3000;
    let rig = Rig::new(
        vec![
            fixture,
            point(
                aim,
                FixtureLocation {
                    x: 2000,
                    y: 5000,
                    z: 1000,
                },
            ),
            point(mount, FixtureLocation::default()),
        ],
        root,
    );
    rig.set(aim, "point.rotation.z", 0.75);
    let value = target(TargetReference::Point { point_id: aim.0 }, [1., 0., 0.]);
    let before = rig.resolve(&[(root, value.clone())]);
    rig.verify(&before);
    let expected = RigidTransform::DESK_TO_PROFILE.point([2., 6., 1.]);
    assert_eq!(
        before.results[0].achieved.outcomes[0].result.requested,
        Some(PositionFitRequest::Target {
            world: Some(expected)
        })
    );
    assert_eq!(
        before.results[0].achieved.outcomes[0].result.status,
        PositionFitStatus::Fitted
    );
    assert!(
        before.results[0].achieved.outcomes[0]
            .result
            .angular_error_degrees
            .unwrap()
            < 0.04
    );
    rig.set(mount, "point.position.z", 0.505);
    let after = rig.resolve(&[(root, value.clone())]);
    rig.verify(&after);
    let old = &before.results[0].achieved.outcomes[0].result;
    let new = &after.results[0].achieved.outcomes[0].result;
    assert_eq!(new.status, PositionFitStatus::Fitted);
    assert!(new.angular_error_degrees.unwrap() < 0.04);
    assert_eq!(
        new.requested, old.requested,
        "mount motion cannot rewrite the independent target"
    );
    assert_ne!(new.achieved, old.achieved);
    assert_ne!(
        before.mounts.iter().find(|(id, _)| *id == root),
        after.mounts.iter().find(|(id, _)| *id == root)
    );
    assert_eq!(after.results[0].requested, intent(&value).unwrap().clone());
}

#[test]
fn captured_position_multipatch_fits_each_copys_inversion_and_installed_zero_once() {
    let profile = moving_head();
    let root = FixtureId::new();
    let copy = Uuid::new_v4();
    let mut fixture = patched(&profile, root, 1);
    fixture.position_calibration = Some(InstalledPositionCalibration {
        pan_zero_degrees: 31.,
        tilt_zero_degrees: -17.,
        ..Default::default()
    });
    fixture.multipatch = vec![MultiPatchInstance {
        id: copy,
        universe: Some(1),
        address: Some(10),
        invert_pan: true,
        invert_tilt: true,
        position_calibration: Some(InstalledPositionCalibration {
            pan_zero_degrees: -41.,
            tilt_zero_degrees: 23.,
            ..Default::default()
        }),
        location: FixtureLocation {
            x: 2000,
            y: 0,
            z: 0,
        },
        ..Default::default()
    }];
    let rig = Rig::new(vec![fixture], root);
    let value = angles(450., 30.);
    let resolved = rig.resolve(&[(root, value.clone())]);
    rig.verify(&resolved);
    let result = &resolved.results[0];
    assert_eq!(result.achieved.outcomes.len(), 2);
    assert_eq!(result.writes.len(), 4);
    for outcome in &result.achieved.outcomes {
        assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
        let achieved = outcome.result.achieved.unwrap();
        assert!((achieved[0] - 450.).abs() < 0.023 && (achieved[1] - 30.).abs() < 0.023);
    }
    let native = |destination| {
        result
            .writes
            .iter()
            .filter(|w| w.slot.destination == destination)
            .map(|w| w.raw)
            .collect::<Vec<_>>()
    };
    assert_ne!(native(root), native(FixtureId(copy)));
    assert_eq!(result.requested, intent(&value).unwrap().clone());
}

#[test]
fn missing_requested_shared_head_target_holds_the_other_heads_whole_position_family() {
    let mut profile = moving_head();
    // Physical motor channels are explicitly shared. The two independently selectable lens
    // heads both reference that same mechanism, without pretending one lens owns the motors.
    profile.modes[0].heads[0].master_shared = true;
    profile.modes[0].heads[0].name = "Shared motor channels".into();
    let first_head = Uuid::new_v4();
    let second_head = Uuid::new_v4();
    profile.modes[0].heads.push(FixtureHead {
        id: first_head,
        name: "First lens".into(),
        master_shared: false,
    });
    profile.modes[0].heads.push(FixtureHead {
        id: second_head,
        name: "Second lens".into(),
        master_shared: false,
    });
    profile.modes[0].emitter_heads[0].head_id = first_head;
    let mut second_emitter = profile.geometry.emitters[0].clone();
    second_emitter.id = Uuid::new_v4();
    second_emitter.name = "Shared mechanics lens".into();
    second_emitter.head_id = None;
    profile.modes[0].emitter_heads.push(EmitterHeadBinding {
        emitter_id: second_emitter.id,
        head_id: second_head,
    });
    profile.geometry.emitters.push(second_emitter);
    let root = FixtureId::new();
    let first = FixtureId::new();
    let second = FixtureId::new();
    let mut fixture = patched(&profile, root, 1);
    fixture.logical_heads = vec![
        PatchedHead {
            profile_head_id: Some(first_head),
            head_index: 1,
            fixture_id: first,
        },
        PatchedHead {
            profile_head_id: Some(second_head),
            head_index: 2,
            fixture_id: second,
        },
    ];
    let rig = Rig::new(vec![fixture], root);
    let resolved = rig.resolve(&[
        (first, angles(450., 30.)),
        (
            second,
            target(
                TargetReference::Point {
                    point_id: Uuid::new_v4(),
                },
                [0.; 3],
            ),
        ),
    ]);
    rig.verify(&resolved);
    assert_eq!(
        resolved.results[0].achieved.outcomes[0].result.status,
        PositionFitStatus::OwnershipConflict
    );
    assert_eq!(
        resolved.results[1].achieved.outcomes[0].result.status,
        PositionFitStatus::MissingTarget
    );
    for result in &resolved.results {
        assert!(result.quality.held);
        for write in &result.writes {
            assert!(write.parked);
            assert_eq!(
                write.raw,
                resolved.native[write.slot.channel_index as usize]
            );
        }
    }
}

fn prepare_live(
    rig: &Rig,
    capture: &PreparedOutputFrame,
    finalize: &PreparedOutputFrame,
    lane: &PhysicalAdapterLane<PositionAdapter>,
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    scratch: &mut HybridFrameScratch,
) -> Result<PublishedPhysicalFrame<PositionAdapter>, DynamicRuntimeError> {
    prepare_live_engine_inner(
        &rig.engine,
        rig.root,
        capture,
        finalize,
        lane,
        runtime,
        origins,
        scratch,
        &[],
    )
}

pub(in crate::runtime) fn prepare_live_engine(
    engine: &Engine,
    root: FixtureId,
    capture: &PreparedOutputFrame,
    finalize: &PreparedOutputFrame,
    lane: &PhysicalAdapterLane<PositionAdapter>,
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    scratch: &mut AuthoringScratch,
    batches: &[light_engine::ContributionBatch],
) -> Result<PublishedPhysicalFrame<PositionAdapter>, DynamicRuntimeError> {
    prepare_live_engine_inner(
        engine,
        root,
        capture,
        finalize,
        lane,
        runtime,
        origins,
        &mut scratch.0,
        batches,
    )
}

fn prepare_live_engine_inner(
    engine: &Engine,
    root: FixtureId,
    capture: &PreparedOutputFrame,
    finalize: &PreparedOutputFrame,
    lane: &PhysicalAdapterLane<PositionAdapter>,
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    scratch: &mut HybridFrameScratch,
    batches: &[light_engine::ContributionBatch],
) -> Result<PublishedPhysicalFrame<PositionAdapter>, DynamicRuntimeError> {
    let snapshot = capture.snapshot();
    let addresser = capture.frame_addresser();
    let now_millis = capture.sampled_at().timestamp_millis() as u64;
    let speeds = [DynamicSpeedTransport {
        effective_bpm: 120.,
        phase_origin_millis: 0,
        phase_reference_millis: now_millis,
        beat_phase: (now_millis as f64 / 500.).rem_euclid(1.),
        phase_advancing: true,
    }; 5];
    let inputs = CapturedDynamicInputs {
        now: capture.sampled_at(),
        speed_transports: &speeds,
        rate: 40,
        snapshot: &snapshot,
        programmer_values: capture.dynamic_programmer_values(),
        programmer_rows: Some(capture.dynamic_programmer_rows()),
        cue_values: capture.cue_dynamic_values(),
        dynamic_playbacks: capture.dynamic_playbacks(),
        playback_paused: capture.playback_dynamics_paused(),
        addresser: &addresser,
        extra_programmer_values: &[],
        programmer_reconciliation_cache: None,
        force_source_reconciliation: false,
    };
    let committed = lane.continuity(root, ProgrammingOwner::Position);
    let mut candidate = origins.clone();
    let output = runtime.with_output_frame_transaction(
        &mut DynamicOutputFrameScratch::default(),
        |runtime| {
            let prepared = prepare_captured_hybrid_frame_with_observer(
                engine,
                capture,
                batches,
                runtime,
                &mut candidate,
                &inputs,
                scratch,
                lane,
                None,
                &mut PositionFrameObserver::new(lane),
            )?;
            assert_eq!(
                lane.continuity(root, ProgrammingOwner::Position),
                committed,
                "preparation only stages Position continuity"
            );
            finalize_live_physical_frame(engine, finalize, lane, prepared)
        },
    );
    if output.is_ok() {
        *origins = candidate;
    }
    output
}

#[test]
fn position_observer_stages_continuity_until_actual_live_finalization_and_keeps_lanes_isolated() {
    let rig = Rig::single();
    let value = angles(450., 30.);
    rig.programmers.set(
        rig.session,
        rig.root,
        ProgrammingOwner::Position.key(),
        value.clone(),
    );
    assert!(
        rig.programmers.apply_dynamic_values(
            rig.session,
            &[DynamicProgrammerValueMutation::Set {
                fixture_id: rig.root,
                attribute: ProgrammingOwner::Position.key(),
                value: DynamicSemanticValue::ProgrammingFixAt {
                    mask: ProgrammingFamilyFixAt::from_family(
                        ProgrammingOwner::Position,
                        None,
                        value.clone()
                    )
                    .unwrap(),
                    timing: Default::default(),
                },
            }],
            None
        )
    );
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let before =
        PhysicalAdapterLane::preload(PositionAdapter::default(), PreloadBranch::BeforeRelease);
    let after =
        PhysicalAdapterLane::preload(PositionAdapter::default(), PreloadBranch::AfterRelease);
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    let capture = rig.capture();
    let foreign = rig.capture();
    let runtime_before = runtime.snapshot();
    let origins_before = origins.snapshot();
    assert!(
        prepare_live(
            &rig,
            &capture,
            &foreign,
            &lane,
            &mut runtime,
            &mut origins,
            &mut scratch
        )
        .is_err()
    );
    assert_eq!(runtime.snapshot(), runtime_before);
    assert_eq!(origins.snapshot(), origins_before);
    assert!(
        lane.continuity(rig.root, ProgrammingOwner::Position)
            .is_none()
    );
    assert!(lane.last_accepted().is_none());
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
    assert_eq!(published.token, capture.frame_token());
    assert_eq!(published.results.len(), 1);
    assert_eq!(published.results[0].value, value);
    assert_eq!(
        published.results[0].achieved.outcomes[0].result.status,
        PositionFitStatus::Fitted
    );
    assert_eq!(lane.last_accepted(), Some(capture.frame_token()));
    let continuity = lane
        .continuity(rig.root, ProgrammingOwner::Position)
        .unwrap();
    assert_eq!(continuity.instances.len(), 1);
    let accepted = &continuity.instances[0];
    let achieved = published.results[0].achieved.outcomes[0]
        .result
        .achieved
        .unwrap();
    let descriptor = lane
        .adapter()
        .compile(&capture.snapshot(), rig.root)
        .unwrap()
        .unwrap();
    assert_eq!(
        accepted.compatibility,
        descriptor.instances[0].compatibility
    );
    for axis in descriptor.instances[0].model.axes() {
        let expected = achieved[if axis.role == Some(PositionAxisRole::Pan) {
            0
        } else {
            1
        }];
        assert_eq!(
            accepted
                .joints
                .iter()
                .find(|(id, _)| *id == axis.node_id)
                .unwrap()
                .1,
            Some(expected)
        );
    }
    for &(index, id, _, raw) in &accepted.controls {
        let write = published.results[0]
            .writes
            .iter()
            .find(|w| w.slot.channel_index == index)
            .unwrap();
        assert_eq!((id, raw), (write.channel_id, write.raw));
    }
    for preload in [&before, &after] {
        assert!(
            preload
                .continuity(rig.root, ProgrammingOwner::Position)
                .is_none()
        );
        assert!(preload.last_accepted().is_none());
        assert!(preload.begin_frame(&capture.frame_token()).is_err());
    }
}

fn accept_angles(rig: &Rig, value: AttributeValue) -> PositionContinuity {
    rig.programmers.set(
        rig.session,
        rig.root,
        ProgrammingOwner::Position.key(),
        value.clone(),
    );
    assert!(
        rig.programmers.apply_dynamic_values(
            rig.session,
            &[DynamicProgrammerValueMutation::Set {
                fixture_id: rig.root,
                attribute: ProgrammingOwner::Position.key(),
                value: DynamicSemanticValue::ProgrammingFixAt {
                    mask: ProgrammingFamilyFixAt::from_family(
                        ProgrammingOwner::Position,
                        None,
                        value
                    )
                    .unwrap(),
                    timing: Default::default(),
                },
            }],
            None
        )
    );
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let capture = rig.capture();
    let published = prepare_live(
        rig,
        &capture,
        &capture,
        &lane,
        &mut DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION),
        &mut DynamicSourceOrigins::default(),
        &mut HybridFrameScratch::default(),
    )
    .unwrap();
    assert_eq!(
        published.results[0].achieved.outcomes[0].result.status,
        PositionFitStatus::Fitted
    );
    lane.continuity(rig.root, ProgrammingOwner::Position)
        .unwrap()
}

#[test]
fn fresh_native_edit_invalidates_accepted_branch_and_hold_never_replays_old_motor_commands() {
    let rig = Rig::single();
    let accepted = accept_angles(&rig, angles(450., 30.));
    let old_pan = accepted.instances[0]
        .controls
        .iter()
        .find(|(index, _, _, _)| *index == 0)
        .unwrap()
        .3;
    rig.set(rig.root, "pan", 0.1);
    let missing = target(
        TargetReference::Point {
            point_id: Uuid::new_v4(),
        },
        [0.; 3],
    );
    for protected in [false, true] {
        let protected = if protected { vec![rig.root] } else { vec![] };
        let resolved =
            rig.resolve_with(&[(rig.root, missing.clone())], Some(&accepted), &protected);
        rig.verify(&resolved);
        let result = &resolved.results[0];
        assert!(result.quality.held);
        assert_ne!(resolved.native[0], old_pan);
        for write in &result.writes {
            assert_eq!(
                write.raw, resolved.native[write.slot.channel_index as usize],
                "fresh scalar edit wins over accepted semantic motor output"
            );
            assert!(write.parked);
        }
        assert_ne!(
            result.continuity.instances[0].joints,
            accepted.instances[0].joints
        );
    }
}

#[test]
fn changed_installed_calibration_and_inversion_reject_old_continuity_with_same_native_ids() {
    let rig = Rig::single();
    let accepted = accept_angles(&rig, angles(450., 30.));
    let snapshot = rig.engine.snapshot();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    fixtures[0].invert_pan = true;
    fixtures[0].position_calibration = Some(InstalledPositionCalibration {
        pan_zero_degrees: 42.,
        tilt_zero_degrees: -13.,
        ..Default::default()
    });
    rig.engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    let missing = target(
        TargetReference::Point {
            point_id: Uuid::new_v4(),
        },
        [0.; 3],
    );
    let resolved = rig.resolve_with(&[(rig.root, missing)], Some(&accepted), &[]);
    rig.verify(&resolved);
    let result = &resolved.results[0];
    assert_eq!(
        result.achieved.outcomes[0].result.status,
        PositionFitStatus::MissingTarget
    );
    assert!(result.quality.held);
    assert_ne!(
        result.continuity.instances[0].compatibility,
        accepted.instances[0].compatibility
    );
    assert_ne!(
        result.continuity.instances[0].joints,
        accepted.instances[0].joints
    );
    for write in &result.writes {
        assert_eq!(
            write.raw,
            resolved.native[write.slot.channel_index as usize]
        );
        assert!(write.parked);
    }
}

fn captured_operation<T>(
    rig: &Rig,
    operation: impl FnOnce(HybridFrameContext<'_>, &PositionDescriptor) -> T,
) -> T {
    let capture = rig.capture();
    let token = capture.frame_token();
    let mut scalar = rig.engine.prepare_static_family_frame(&capture, &[]);
    let geometry = rig
        .engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let models = DynamicRuntime::default().captured_native_color_models();
    let descriptor = rig
        .adapter
        .compile(&capture.snapshot(), rig.root)
        .unwrap()
        .unwrap();
    operation(
        HybridFrameContext {
            capture: &capture,
            geometry: &geometry,
            native_models: models.as_ref(),
            token: &token,
            scalar: &scalar,
        },
        &descriptor,
    )
}

#[test]
fn captured_target_adoption_preserves_accepted_unwrapped_branch_and_single_target_resolve_is_passive()
 {
    let rig = Rig::single();
    let accepted = accept_angles(&rig, angles(450., 30.));
    let resolved = rig.resolve(&[(rig.root, angles(450., 30.))]);
    let world = resolved.results[0].achieved.outcomes[0]
        .result
        .pose
        .unwrap()
        .point([0., -10., 0.]);
    let desk = RigidTransform::DESK_TO_PROFILE.inverse().point(world);
    let original = target(TargetReference::Origin, desk.map(|value| value as f32));
    let original_before = original.clone();
    let address = DynamicValueAddress {
        representation: light_dynamics::DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    };
    let adopted = captured_operation(&rig, |frame, descriptor| {
        assert!(matches!(
            rig.adapter.resolve(PhysicalRequest {
                frame,
                target: rig.root,
                owner: ProgrammingOwner::Position,
                descriptor,
                value: &original,
                previous: Some(&accepted),
            }),
            Err(TransitionError::Requires(
                TransitionRequirement::LiveJointAngles
            ))
        ));
        rig.adapter
            .adopt_with_continuity(
                frame,
                descriptor,
                rig.root,
                &original,
                &address,
                Some(&accepted),
            )
            .unwrap()
    });
    let PositionIntent::Angles {
        pan_degrees: ScalarIntent::Value(pan),
        tilt_degrees: ScalarIntent::Value(tilt),
    } = intent(&adopted).unwrap()
    else {
        panic!("captured Target adopts commanded Angles")
    };
    assert!(
        (*pan - 450.).abs() < 0.05,
        "nearest accepted turn survives adoption: {pan}"
    );
    assert!((*tilt - 30.).abs() < 0.05);
    assert_eq!(
        original, original_before,
        "adoption cannot rewrite stored Target intent"
    );
}

#[test]
fn different_point_reference_transition_resolves_both_endpoints_from_each_captured_frame() {
    let profile = moving_head();
    let root = FixtureId::new();
    let first = FixtureId::new();
    let second = FixtureId::new();
    let rig = Rig::new(
        vec![
            patched(&profile, root, 1),
            point(
                first,
                FixtureLocation {
                    x: 0,
                    y: 5000,
                    z: 0,
                },
            ),
            point(
                second,
                FixtureLocation {
                    x: 4000,
                    y: 7000,
                    z: 1000,
                },
            ),
        ],
        root,
    );
    rig.set(second, "point.rotation.z", 0.75);
    let from = target(TargetReference::Point { point_id: first.0 }, [0.; 3]);
    let to = target(TargetReference::Point { point_id: second.0 }, [1., 0., 0.]);
    let saved = (from.clone(), to.clone());
    let midpoint = || {
        captured_operation(&rig, |frame, descriptor| {
            rig.adapter
                .transition(
                    frame,
                    descriptor,
                    root,
                    TransitionRequirement::LiveTargetPoints,
                    &from,
                    &to,
                    FamilyExpressionOperation::Transition { progress: 0.5 },
                )
                .unwrap()
                .0
        })
    };
    let before = midpoint();
    assert_eq!(before, target(TargetReference::Origin, [2., 6.5, 0.5]));
    rig.set(second, "point.position.x", 0.51);
    let after = midpoint();
    let PositionIntent::Target {
        reference: TargetReference::Origin,
        offset_metres,
    } = intent(&after).unwrap()
    else {
        panic!("world transition is a derived Origin value")
    };
    let expected = [3., 6.5, 0.5];
    for (value, expected) in offset_metres.iter().zip(expected) {
        let ScalarIntent::Value(value) = value else {
            panic!("resolved scalar offset")
        };
        assert!((*value - expected).abs() < 0.0001);
    }
    assert_ne!(
        after, before,
        "a moving referenced endpoint is re-evaluated in the next capture"
    );
    assert_eq!(
        (from, to),
        saved,
        "derived midpoint never replaces referenced source intents"
    );
}
