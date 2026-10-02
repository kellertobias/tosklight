//! TL-557 closure of the TL-592 passive paths: a root fixture owning several Color heads is
//! fitted per head, and every multipatch copy is fitted with its own installed calibration.
//! Descriptors, native raw seeding and continuity are keyed per destination and head.
use super::profiles::*;
use super::tests::{intent, magenta, program, warm_white};
use super::*;
use light_core::{ManualClock, SessionId};
use light_dynamics::DynamicRuntime;
use light_engine::{Engine, RenderOptions};
use light_fixture::{
    FixtureHead, FixtureProfile, InstalledColorCalibration, InstalledColorPathCalibration,
    InstalledEmitterCalibration, OpticalProvenance, OpticalSource,
};
use light_programmer::ProgrammerRegistry;

/// Duplicate every Color channel and the optical path of head 0 into a second head.
pub(super) fn two_heads(mut profile: FixtureProfile) -> FixtureProfile {
    let mode = &mut profile.modes[0];
    let first = mode.heads[0].id;
    let second = Uuid::new_v4();
    mode.heads.push(FixtureHead {
        id: second,
        name: "Cell 2".into(),
        master_shared: false,
    });
    let mut slot = mode.splits[0].footprint + 1;
    let mut ids = std::collections::HashMap::new();
    let copies: Vec<_> = mode
        .channels
        .iter()
        .filter(|c| c.head_id == first && c.attribute.0.starts_with("color."))
        .cloned()
        .collect();
    for mut channel in copies {
        let old = (channel.id, channel.functions[0].id);
        channel.id = Uuid::new_v4();
        channel.head_id = second;
        for function in &mut channel.functions {
            function.id = Uuid::new_v4();
        }
        if !channel.secondary_slots.is_empty() {
            channel.secondary_slots = vec![slot + 1];
        }
        slot += channel.resolution.bytes() as u16;
        ids.insert(old.0, (channel.id, channel.functions[0].id));
        ids.insert(old.1, (channel.id, channel.functions[0].id));
        mode.channels.push(channel);
    }
    mode.splits[0].footprint = slot - 1;
    let model = mode.color_physical.as_mut().unwrap();
    let mut path = model.paths[0].clone();
    path.id = Uuid::new_v4();
    path.head_id = second;
    path.controls = path.controls.iter().map(|id| ids[id].0).collect();
    if let OpticalSource::Additive { emitters } = &mut path.source {
        for emitter in emitters {
            emitter.id = Uuid::new_v4();
            emitter.binding.function_id = ids[&emitter.binding.channel_id].1;
            emitter.binding.channel_id = ids[&emitter.binding.channel_id].0;
        }
    }
    model.paths.push(path);
    profile.validate().unwrap();
    profile
}

/// Installed calibration scaling one emitter's output of head 0.
fn gain(profile: &FixtureProfile, emitter: usize, gain: f32) -> InstalledColorCalibration {
    let mode = &profile.modes[0];
    let path = &mode.color_physical.as_ref().unwrap().paths[0];
    let OpticalSource::Additive { emitters } = &path.source else {
        unreachable!()
    };
    InstalledColorCalibration {
        version: 1,
        revision: 1,
        paths: vec![InstalledColorPathCalibration {
            source_identity: profile
                .native_color_identity(mode.id, path.head_id)
                .unwrap(),
            emitters: vec![InstalledEmitterCalibration {
                emitter_id: emitters[emitter].id,
                output_gain: gain,
                provenance: OpticalProvenance {
                    quality: PhysicalDataQuality::Estimated,
                    ..Default::default()
                },
            }],
            measurements: vec![],
        }],
    }
}

pub(super) struct Rig {
    engine: Engine,
    clock: Arc<ManualClock>,
    pub(super) target: FixtureId,
    pub(super) adapter: ColorAdapter,
    /// Descriptor source when it differs from the installed patch (see the root-multihead test).
    compile_from: Option<light_engine::EngineSnapshot>,
}

pub(super) struct Resolved {
    pub(super) descriptor: ColorDescriptor,
    pub(super) native: Vec<u32>,
    pub(super) result: PhysicalResolution<ColorAdapter>,
}

impl Rig {
    pub(super) fn new(fixture: PatchedFixture) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        programmers.start(SessionId::new());
        let rig = Self {
            engine: Engine::new(programmers),
            clock,
            target: fixture.fixture_id,
            adapter: ColorAdapter::default(),
            compile_from: None,
        };
        rig.install(fixture);
        rig
    }

    fn install(&self, fixture: PatchedFixture) {
        self.engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![fixture].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
    }

    pub(super) fn resolve(
        &self,
        intent: &ColorIntent,
        previous: Option<&ColorContinuity>,
    ) -> Resolved {
        self.clock.advance_millis(25);
        let capture = self.engine.prepare_output_frame(RenderOptions::default());
        let token = capture.frame_token();
        let mut scalar = self.engine.prepare_static_family_frame(&capture, &[]);
        let geometry = self
            .engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let models = DynamicRuntime::default().captured_native_color_models();
        let snapshot = capture.snapshot();
        let descriptor = self
            .adapter
            .compile(self.compile_from.as_ref().unwrap_or(&snapshot), self.target)
            .unwrap()
            .expect("physical Color destinations");
        let native = scalar
            .native_raw(&capture, &token, self.target)
            .unwrap()
            .raw()
            .to_vec();
        let value = program(intent);
        let result = self
            .adapter
            .resolve(PhysicalRequest {
                frame: HybridFrameContext {
                    capture: &capture,
                    geometry: &geometry,
                    native_models: models.as_ref(),
                    token: &token,
                    scalar: &scalar,
                },
                target: self.target,
                owner: ProgrammingOwner::Color,
                descriptor: &descriptor,
                value: &value,
                previous,
            })
            .unwrap();
        validate_complete_writes(&descriptor.footprint, &result.writes).unwrap();
        assert_eq!(&result.requested, intent, "the request is never rewritten");
        Resolved {
            descriptor,
            native,
            result,
        }
    }
}

impl Resolved {
    /// The destination's complete native vector with its writes applied.
    fn output(&self, destination: FixtureId) -> Vec<u32> {
        let mut raw = self.native.clone();
        for write in self
            .result
            .writes
            .iter()
            .filter(|w| w.slot.destination == destination)
        {
            raw[write.slot.channel_index as usize] = write.raw;
        }
        raw
    }

    pub(super) fn raws(&self, destination: FixtureId) -> Vec<u32> {
        self.result
            .writes
            .iter()
            .filter(|w| w.slot.destination == destination)
            .map(|w| w.raw)
            .collect()
    }

    /// Every destination head's published achieved output equals an independent forward
    /// evaluation of that destination's written values through that destination's own model.
    pub(super) fn verify_every_head(&self) {
        let outcomes = &self.result.quality.heads;
        assert_eq!(outcomes.len(), self.descriptor.heads.len());
        assert_eq!(self.result.achieved, outcomes[0].achieved);
        for (head, outcome) in self.descriptor.heads.iter().zip(outcomes) {
            assert_eq!(
                (outcome.destination, outcome.head_id),
                (head.destination, head.head_id)
            );
            assert!(outcome.quality.heads.is_empty());
            let model = head.fitting.forward();
            let mut forward = model.create_output();
            model
                .evaluate(&self.output(head.destination), &mut forward)
                .unwrap();
            let forward = &forward[head.head];
            assert_eq!(outcome.achieved.known_xyz, forward.known_xyz);
            assert_eq!(
                outcome.achieved.visible,
                forward.visible_complete.then_some(forward.known_xyz)
            );
        }
    }
}

/// A root target that owns two Color heads. A validated patch maps every non-shared head to a
/// logical head (and a mode has at most one master-shared head), so this topology is compiled
/// from the unpatched form while the frame comes from the validated patch of the same mode: the
/// root's native channel space and pre-master values are identical.
fn root_owning_two_heads(profile: &FixtureProfile) -> Rig {
    let root = FixtureId::new();
    let unpatched = patched(profile, root, 1);
    let mut validated = unpatched.clone();
    validated.logical_heads.push(light_fixture::PatchedHead {
        profile_head_id: Some(profile.modes[0].heads[1].id),
        head_index: 1,
        fixture_id: FixtureId::new(),
    });
    let mut rig = Rig::new(validated);
    rig.compile_from = Some(light_engine::EngineSnapshot {
        fixtures: vec![unpatched].into(),
        revision: 1,
        ..Default::default()
    });
    rig
}

#[test]
fn a_root_fixture_owning_two_color_heads_fits_each_head_instead_of_staying_passive() {
    let profile = two_heads(rgbw());
    let rig = root_owning_two_heads(&profile);
    for request in [magenta(), warm_white(), intent([1., 0., 1.], 0.)] {
        let resolved = rig.resolve(&request, None);
        let descriptor = &resolved.descriptor;
        assert_eq!(descriptor.heads.len(), 2, "both Color heads are fitted");
        assert_ne!(descriptor.heads[0].head_id, descriptor.heads[1].head_id);
        assert!(descriptor.heads.iter().all(|h| h.destination == rig.target));
        assert_eq!(
            resolved.result.writes.len(),
            8,
            "every Color control of both heads is written once"
        );
        let [first, second] = [0, 1].map(|head| {
            resolved
                .result
                .writes
                .iter()
                .filter(|w| {
                    descriptor.heads[head]
                        .controls
                        .iter()
                        .any(|c| c.channel_index == w.slot.channel_index)
                })
                .map(|w| w.raw)
                .collect::<Vec<_>>()
        });
        assert_eq!(first.len(), 4);
        assert_eq!(first, second, "identical cells receive identical drives");
        assert!(
            resolved
                .result
                .quality
                .heads
                .iter()
                .all(|h| h.quality.visible == VisibleFitStatus::Fitted)
        );
        resolved.verify_every_head();
        // Continuity is keyed per destination head.
        let continuity = &resolved.result.continuity;
        assert_eq!(continuity.heads.len(), 2);
        for head in descriptor.heads.iter() {
            assert_eq!(
                continuity
                    .head(head.destination, head.head_id)
                    .unwrap()
                    .controls
                    .len(),
                4
            );
        }
    }
    let counters = rig.adapter.counters();
    assert_eq!(
        counters.fitting_compiles, 1,
        "one fitter shared by both heads"
    );
    assert_eq!(counters.multi_head_targets, 3);
    assert_eq!(counters.shared_conflicts, 0);
}

#[test]
fn every_multipatch_copy_is_fitted_with_its_own_installed_calibration() {
    let profile = rgb();
    let mut fixture = patched(&profile, FixtureId::new(), 1);
    let copy = light_fixture::MultiPatchInstance {
        id: Uuid::new_v4(),
        universe: Some(1),
        address: Some(20),
        // The copy's red emitter produces half the root's output.
        color_calibration: Some(gain(&profile, 0, 0.5)),
        ..Default::default()
    };
    let copy_id = FixtureId(copy.id);
    fixture.multipatch = vec![copy];
    let rig = Rig::new(fixture);
    let mut request = magenta();
    request.relative_output = 0.3;
    let resolved = rig.resolve(&request, None);
    let descriptor = &resolved.descriptor;
    assert_eq!(descriptor.root, rig.target);
    assert_eq!(
        descriptor
            .heads
            .iter()
            .map(|h| h.destination)
            .collect::<Vec<_>>(),
        [rig.target, copy_id]
    );
    assert!(
        !Arc::ptr_eq(&descriptor.heads[0].fitting, &descriptor.heads[1].fitting),
        "the copy has its own calibrated fitter"
    );
    let (root, copy) = (resolved.raws(rig.target), resolved.raws(copy_id));
    assert_eq!((root.len(), copy.len()), (3, 3));
    assert!(
        copy[0] > root[0],
        "the dimmer copy red is driven harder: root {root:?}, copy {copy:?}"
    );
    resolved.verify_every_head();
    let [root_xyz, copy_xyz] = [0, 1].map(|i| resolved.result.quality.heads[i].achieved.known_xyz);
    for (a, b) in [
        (root_xyz.x, copy_xyz.x),
        (root_xyz.y, copy_xyz.y),
        (root_xyz.z, copy_xyz.z),
    ] {
        assert!((a - b).abs() < 2e-3, "{root_xyz:?} vs {copy_xyz:?}");
    }
    // Ignoring the copy calibration would reproduce the root's drives on the copy.
    let uncalibrated = CompiledColorFitting::compile(&profile, profile.modes[0].id, None)
        .unwrap()
        .unwrap();
    let mut forward = uncalibrated.forward().create_output();
    uncalibrated
        .forward()
        .evaluate(&resolved.output(copy_id), &mut forward)
        .unwrap();
    assert!(forward[0].known_xyz.x > copy_xyz.x * 1.2);
    let counters = rig.adapter.counters();
    assert_eq!(
        (counters.fitting_compiles, counters.copy_destinations),
        (2, 1)
    );
}

#[test]
fn continuity_and_native_seeding_are_keyed_per_destination_and_head() {
    let profile = wheel_only();
    let mut fixture = patched(&profile, FixtureId::new(), 1);
    let copy = light_fixture::MultiPatchInstance {
        id: Uuid::new_v4(),
        ..Default::default()
    };
    let copy_id = FixtureId(copy.id);
    fixture.multipatch = vec![copy];
    let rig = Rig::new(fixture);
    let blue = intent([0., 0., 1.], 0.);
    let first = rig.resolve(&blue, None);
    assert_eq!(first.raws(rig.target), [39]);
    assert_eq!(first.raws(copy_id), [39]);
    // Root-only continuity inside the blue slot keeps the root's wheel; the copy keeps its own.
    let mut previous = first.result.continuity.clone();
    previous.heads[0].controls[0].2 = 33;
    let kept = rig.resolve(&blue, Some(&previous));
    assert_eq!(kept.raws(rig.target), [33]);
    assert_eq!(kept.raws(copy_id), [39]);
    kept.verify_every_head();
    // The copy's continuity never leaks onto the root.
    let mut previous = first.result.continuity.clone();
    previous.heads[1].controls[0].2 = 45;
    let kept = rig.resolve(&blue, Some(&previous));
    assert_eq!(kept.raws(rig.target), [39]);
    assert_eq!(kept.raws(copy_id), [45]);
    // Replacing the destination list drops the copy's descriptor and its continuity.
    rig.install(patched(&profile, rig.target, 1));
    let replaced = rig.resolve(&blue, Some(&previous));
    assert_eq!(replaced.descriptor.heads.len(), 1);
    assert_eq!(replaced.raws(rig.target), [39]);
    assert!(replaced.raws(copy_id).is_empty());
}

/// Cell 2 shares the master head's White channel through its own (half-strength) emitter.
fn two_heads_sharing_white() -> FixtureProfile {
    let mut profile = two_heads(rgb());
    let mode = &mut profile.modes[0];
    let mut white = rgbw().modes[0].channels[4].clone();
    white.head_id = mode.heads[0].id;
    let (channel, function) = (white.id, white.functions[0].id);
    mode.channels.push(white);
    mode.splits[0].footprint += 1;
    let emitter = |scale: f32| light_fixture::OpticalEmitter {
        id: Uuid::new_v4(),
        name: "color.white".into(),
        binding: light_fixture::NativeColorBinding {
            channel_id: channel,
            function_id: function,
        },
        xyz: Some({
            let w = white_xyz();
            xyz(w.x * scale, w.y * scale, w.z * scale)
        }),
        spectrum: vec![],
        band: light_fixture::OpticalEmitterBand::Visible,
        native_reversed: false,
        maximum_level: 1.0,
        response_exponent: 1.0,
        provenance: provenance(PhysicalDataQuality::Manufacturer),
    };
    let paths = &mut mode.color_physical.as_mut().unwrap().paths;
    for (path, scale) in paths.iter_mut().zip([1.0, 0.5]) {
        path.controls.push(channel);
        if let OpticalSource::Additive { emitters } = &mut path.source {
            emitters.push(emitter(scale));
        }
    }
    profile.validate().unwrap();
    profile
}

fn white_xyz() -> Xyz {
    super::profiles::white()
}

#[test]
fn a_master_shared_slot_is_written_once_and_a_disagreeing_head_reports_the_written_value() {
    let profile = two_heads_sharing_white();
    let rig = root_owning_two_heads(&profile);
    let resolved = rig.resolve(&magenta(), None);
    let descriptor = &resolved.descriptor;
    assert_eq!(descriptor.heads.len(), 2);
    assert_eq!(
        descriptor.heads[1]
            .writes_control
            .iter()
            .filter(|w| !**w)
            .count(),
        1,
        "cell 2 does not write the White slot the master head owns"
    );
    assert_eq!(
        resolved.result.writes.len(),
        7,
        "the shared White is written once"
    );
    resolved.verify_every_head();
    let second = &resolved.result.quality.heads[1].quality;
    assert!(second.shared_conflict, "{:?}", resolved.raws(rig.target));
    assert!(
        second
            .limitations
            .contains(ColorFitLimitations::SHARED_CONTROL)
    );
    assert!(!resolved.result.quality.heads[0].quality.shared_conflict);
    assert_eq!(rig.adapter.counters().shared_conflicts, 1);
}

/// Valid installed observations can exceed the forward compiler's smaller bounded capacity.
fn over_capacity_calibration(profile: &FixtureProfile) -> InstalledColorCalibration {
    let mut calibration = gain(profile, 0, 1.0);
    let path = &profile.modes[0].color_physical.as_ref().unwrap().paths[0];
    let mode = &profile.modes[0];
    calibration.paths[0].measurements = (1..=6000)
        .map(|red| light_fixture::ColorRecipeMeasurement {
            recipe: path
                .controls
                .iter()
                .enumerate()
                .map(|(index, id)| {
                    let channel = mode.channels.iter().find(|c| c.id == *id).unwrap();
                    light_core::NativeColorValue {
                        channel_id: *id,
                        function_id: channel.functions[0].id,
                        raw: if index == 0 { red } else { 0 },
                    }
                })
                .collect(),
            xyz: xyz(0.1, 0.1, 0.1),
            provenance: provenance(PhysicalDataQuality::Measured),
        })
        .collect();
    calibration.validate().unwrap();
    assert!(matches!(
        calibration.status(profile, mode.id),
        light_fixture::InstalledColorCalibrationStatus::Current
    ));
    calibration
}

#[test]
fn an_unsupported_copy_or_root_holds_the_complete_color_owner() {
    let profile = rgb();
    for root_unsupported in [false, true] {
        let calibration = over_capacity_calibration(&profile);
        let mut fixture = patched(&profile, FixtureId::new(), 1);
        let mut copy = light_fixture::MultiPatchInstance {
            id: Uuid::new_v4(),
            ..Default::default()
        };
        if root_unsupported {
            fixture.color_calibration = Some(calibration);
        } else {
            copy.color_calibration = Some(calibration);
        }
        fixture.multipatch.push(copy);
        assert!(compile_fitting(&fixture, None).is_err() == root_unsupported);
        assert!(
            compile_fitting(&fixture, Some(&fixture.multipatch[0])).is_err() != root_unsupported
        );
        let snapshot = light_engine::EngineSnapshot {
            fixtures: vec![fixture.clone()].into(),
            ..Default::default()
        };
        let adapter = ColorAdapter::default();
        assert!(
            adapter
                .compile(&snapshot, fixture.fixture_id)
                .unwrap()
                .is_none(),
            "never drop an unsupported copy or publish a copy as the root primary"
        );
        assert_eq!(adapter.counters().fitting_failures, 1);
    }
}

#[test]
fn shared_color_controls_across_validated_logical_targets_hold_passively() {
    let profile = two_heads_sharing_white();
    let root = FixtureId::new();
    let child = FixtureId::new();
    let mut fixture = patched(&profile, root, 1);
    fixture.logical_heads.push(light_fixture::PatchedHead {
        profile_head_id: Some(profile.modes[0].heads[1].id),
        head_index: 1,
        fixture_id: child,
    });
    let rig = Rig::new(fixture); // Engine validates the actual logical-head topology.
    let snapshot = rig.engine.snapshot();
    for target in [root, child] {
        assert!(
            rig.adapter.compile(&snapshot, target).unwrap().is_none(),
            "shared controls need destination-wide arbitration, not duplicate writes"
        );
    }
}
