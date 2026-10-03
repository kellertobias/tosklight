//! TL-556 x TL-636: genuine runtime pause/hot-edit Resume occurrences consumed by the physical
//! operation coordinator through owner-issued original Resume operand locators. Shared Pan +
//! independent Tilts with a displaced, inverted and calibrated copy are synthetic rigs: these
//! tests establish software/native coherence, not lamp calibration or operator acceptance.
use super::super::current_cohort::start_sized;
use super::super::shared_resume::resume_occurrences;
use super::*;
use light_dynamics::{
    ActivationPolicy, DynamicFamilyRepresentation, DynamicLane, DynamicSpeed,
    FamilyCompositionSample, SpeedGroup,
};

type Pairs = HashMap<(usize, FixtureId), [f64; 2]>;
const DELTAS: [f32; 2] = [10., 7.];

/// What each head carries beside its static Target baseline.
#[derive(Clone, Copy, PartialEq)]
enum Head {
    /// Its own genuine programmer Dynamic, hot-edited into a runtime Resume.
    Resume,
    /// One fully active whole Fixed Angle mask: an independently constant peer.
    Constant,
    /// Static Target plus a fading partial Fixed mask: a changing peer with its own
    /// owner-local mask stage and no Resume scope.
    Masked,
}
#[derive(Clone, Copy)]
struct Setup {
    heads: [Head; 2],
    size: f32,
    /// One LiveGroup Dynamic over both heads: one instance/controller, one shared scope.
    shared_emission: bool,
}
struct ResumeDesk {
    shared: SharedRig,
    copy: FixtureId,
    setup: Setup,
    /// (definition, heads it emits for).
    definitions: Vec<(DynamicDefinition, Vec<usize>)>,
    runtime: DynamicRuntime,
    lane: PhysicalAdapterLane<PositionAdapter>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
}
fn current_definition(base: &AttributeValue, pool: u16) -> DynamicDefinition {
    let mut definition = position_definition(
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, base).unwrap(),
        [
            DynamicValue::Family(base.clone()),
            DynamicValue::Family(base.clone()),
        ],
    );
    definition.pool_number = pool;
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational {
            numerator: 2,
            denominator: 1,
        },
    };
    set_current(&mut definition.lanes[0]);
    definition
}
fn set_current(lane: &mut DynamicLane) {
    let DynamicLaneBody::Programming(body) = &mut lane.body else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
        unreachable!()
    };
    for point in &mut configuration.points {
        point.source = DynamicValueSource::Current;
    }
}
fn set_scalar(lane: &mut DynamicLane, component: ProgrammingComponent, value: f32) {
    let DynamicLaneBody::Programming(body) = &mut lane.body else {
        unreachable!()
    };
    body.address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(component),
    };
    let ProgrammingLaneConfiguration::Keyframes(configuration) = &mut body.configuration else {
        unreachable!()
    };
    for point in &mut configuration.points {
        point.source = DynamicValueSource::Value {
            value: DynamicValue::Scalar(value),
        };
    }
}
/// Authored Target Current -> literal Pan + Tilt edit. The shared Pan stays literal and
/// compatible, so both Resume endpoints are static and every Cartesian cohort is feasible.
fn to_angles(definition: &mut DynamicDefinition, pan: f32, tilt: f32) {
    definition.revision += 1;
    set_scalar(&mut definition.lanes[0], ProgrammingComponent::Tilt, tilt);
    definition.normalize_angle_pair();
    assert_eq!(definition.lanes.len(), 2);
    set_scalar(&mut definition.lanes[1], ProgrammingComponent::Pan, pan);
}
/// Literal Angles -> original whole Target Current edit (an interrupting second Resume).
fn to_target_current(definition: &mut DynamicDefinition, base: &AttributeValue) {
    definition.revision += 1;
    definition.lanes.truncate(1);
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        unreachable!()
    };
    body.address = DynamicValueAddress::whole_family(ProgrammingOwner::Position, base).unwrap();
    set_current(&mut definition.lanes[0]);
    definition.normalize_angle_pair();
}
fn activation(sample: &FamilyCompositionSample) -> f32 {
    match sample {
        FamilyCompositionSample::Known(sample) => sample.activation_mix,
        FamilyCompositionSample::WholeExpression { activation_mix, .. }
        | FamilyCompositionSample::CoupledExpression { activation_mix, .. } => *activation_mix,
    }
}

impl ResumeDesk {
    fn new(setup: Setup) -> Self {
        let shared = shared_rig();
        let copy = FixtureId::new();
        let snapshot = shared.rig.engine.snapshot();
        let mut fixtures = snapshot.fixtures.as_ref().clone();
        // Displaced, inverted and calibrated second copy of the same mechanical fixture.
        fixtures[0].multipatch.push(MultiPatchInstance {
            id: copy.0,
            universe: Some(1),
            address: Some(20),
            location: FixtureLocation {
                x: 0,
                y: 0,
                z: 1000,
            },
            invert_pan: true,
            position_calibration: Some(InstalledPositionCalibration {
                tilt_zero_degrees: -7.,
                ..Default::default()
            }),
            ..Default::default()
        });
        let mut groups = snapshot.groups.as_ref().clone();
        groups.push(light_programmer::GroupDefinition {
            id: "resume-operand".into(),
            fixtures: shared.heads.to_vec(),
            ..Default::default()
        });
        shared
            .rig
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures: fixtures.into(),
                groups: groups.into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        let mut definitions = Vec::new();
        if setup.shared_emission {
            let mut definition = current_definition(&shared.targets[0], 1);
            definition.target_binding = DynamicTargetBinding::LiveGroup {
                group_id: "resume-operand".into(),
            };
            definitions.push((definition, vec![0, 1]));
        } else {
            for (index, head) in setup.heads.into_iter().enumerate() {
                if head == Head::Resume {
                    let definition = current_definition(&shared.targets[index], index as u16 + 1);
                    definitions.push((definition, vec![index]));
                }
            }
        }
        let runtime = if setup.shared_emission {
            start_shared_emission(&shared, &definitions[0].0)
        } else {
            let started = definitions
                .iter()
                .map(|(definition, heads)| (heads[0], definition.clone(), Some(1000)))
                .collect::<Vec<_>>();
            start_sized(&shared, &shared.targets, &started, setup.size)
        };
        let mut desk = Self {
            shared,
            copy,
            setup,
            definitions,
            runtime,
            lane: PhysicalAdapterLane::live(PositionAdapter::default()),
            origins: Default::default(),
            scratch: Default::default(),
        };
        for (index, head) in setup.heads.into_iter().enumerate() {
            if head == Head::Constant {
                desk.fix(index, desk.constant(index), None);
            }
        }
        desk.tick();
        desk.shared.rig.clock.advance_millis(1100);
        let (_, initial) = desk.tick();
        assert!(initial.requirements.is_empty());
        desk.hot_edit(|desk, definition, heads| {
            let [pan, tilt] = desk.shared.angles[heads[0]];
            to_angles(definition, pan, tilt + DELTAS[heads[0]]);
        });
        desk
    }
    fn constant(&self, index: usize) -> AttributeValue {
        let [pan, tilt] = self.shared.angles[index];
        angles(pan, tilt)
    }
    fn mask(&self, index: usize) -> AttributeValue {
        let [pan, tilt] = self.shared.angles[index];
        angles(pan, tilt + 20.)
    }
    fn fix(&self, index: usize, value: AttributeValue, fade: Option<u64>) {
        assert!(
            self.shared.rig.programmers.apply_dynamic_values(
                self.shared.rig.session,
                &[DynamicProgrammerValueMutation::Set {
                    fixture_id: self.shared.heads[index],
                    attribute: ProgrammingOwner::Position.key(),
                    value: DynamicSemanticValue::ProgrammingFixAt {
                        mask: ProgrammingFamilyFixAt::from_family(
                            ProgrammingOwner::Position,
                            None,
                            value
                        )
                        .unwrap(),
                        timing: DynamicValueTiming {
                            fade_millis: fade,
                            delay_millis: None
                        },
                    },
                }],
                None
            )
        );
    }
    /// A second genuine owner source on head 0: a fading partial Fixed mask above the Resume.
    /// It makes the owner multi-source (refused by the existing single-source runner) and is the
    /// non-idempotent original parent suffix that must execute exactly once after the cut.
    fn add_partial_mask(&mut self) {
        self.fix(0, self.mask(0), Some(1000));
        if self.setup.heads[1] == Head::Masked {
            self.fix(1, self.mask(1), Some(1000));
        }
        self.tick();
        self.shared.rig.clock.advance_millis(250);
    }
    /// Actual global pause, authored edit, unpause: the runtime records each controller's own
    /// synchronized Resume occurrence. Nothing below rewrites an emitted graph.
    fn hot_edit(&mut self, edit: impl Fn(&Self, &mut DynamicDefinition, &[usize])) {
        self.shared
            .rig
            .engine
            .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
            .unwrap();
        let (capture, _) = self.tick();
        self.runtime
            .set_global_paused(true, capture.sampled_at().timestamp_millis() as u64);
        self.tick();
        let mut definitions = std::mem::take(&mut self.definitions);
        for (definition, heads) in &mut definitions {
            edit(self, definition, heads);
        }
        self.definitions = definitions;
        let dynamics = self
            .definitions
            .iter()
            .map(|(definition, _)| definition.clone())
            .collect::<Vec<_>>();
        let snapshot = self.shared.rig.engine.snapshot();
        self.shared
            .rig
            .engine
            .replace_snapshot(EngineSnapshot {
                dynamics: dynamics.clone().into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        self.runtime.install_definitions(dynamics).unwrap();
        let (capture, _) = self.tick();
        self.shared
            .rig
            .engine
            .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(
                false,
            ))
            .unwrap();
        self.runtime
            .set_global_paused(false, capture.sampled_at().timestamp_millis() as u64);
        self.tick();
        self.shared.rig.clock.advance_millis(250);
    }
    fn tick(&mut self) -> (PreparedOutputFrame, PublishedPhysicalFrame<PositionAdapter>) {
        let capture = self.shared.rig.capture();
        let output = prepare_live(
            &self.shared.rig,
            &capture,
            &capture,
            &self.lane,
            &mut self.runtime,
            &mut self.origins,
            &mut self.scratch,
        )
        .unwrap();
        (capture, output)
    }
    /// Each emitted head's actual runtime scopes, innermost first, as (occurrence, progress).
    fn scopes(
        &self,
        output: &PublishedPhysicalFrame<PositionAdapter>,
    ) -> HashMap<usize, Vec<(Uuid, f32)>> {
        let snapshot = self.runtime.snapshot();
        let mut scopes = HashMap::new();
        for (definition, heads) in &self.definitions {
            for &index in heads {
                let sample = output
                    .sampled
                    .samples
                    .iter()
                    .find(|sample| {
                        sample.target == self.shared.heads[index]
                            && sample.lane_id == definition.lanes[0].id
                    })
                    .unwrap();
                let instance = snapshot
                    .instances
                    .iter()
                    .find(|instance| instance.id == sample.instance_id)
                    .unwrap();
                assert_eq!(instance.controllers.len(), 1);
                assert_eq!(sample.controller_id, instance.controllers[0].id);
                let current = instance
                    .synchronized_resume_transition
                    .expect("actual runtime Resume occurrence")
                    .occurrence_id;
                let resumes = resume_occurrences(&sample.expression);
                assert!(resumes.iter().any(|(id, _)| *id == current));
                assert!(
                    resumes
                        .iter()
                        .all(|(_, progress)| (0.1..0.9).contains(progress))
                );
                scopes.insert(index, resumes);
            }
        }
        scopes
    }
    fn outgoing(&self) -> [AttributeValue; 2] {
        std::array::from_fn(|index| match self.setup.heads[index] {
            Head::Resume | Head::Masked => self.shared.targets[index].clone(),
            Head::Constant => self.constant(index),
        })
    }
    fn incoming(&self) -> [AttributeValue; 2] {
        std::array::from_fn(|index| match self.setup.heads[index] {
            Head::Resume => {
                let [pan, tilt] = self.shared.angles[index];
                angles(pan, tilt + self.setup.size * DELTAS[index])
            }
            Head::Masked => self.shared.targets[index].clone(),
            Head::Constant => self.constant(index),
        })
    }
    /// Static complete-cohort geometry oracle for every owner and both physical copies.
    fn pairs(&self, values: &[AttributeValue; 2]) -> Pairs {
        let resolved = self.shared.rig.resolve(
            &self
                .shared
                .heads
                .iter()
                .copied()
                .zip(values.iter().cloned())
                .collect::<Vec<_>>(),
        );
        let mut pairs = HashMap::new();
        for (index, resolution) in resolved.results.iter().enumerate() {
            for outcome in &resolution.achieved.outcomes {
                assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
                assert!(!outcome.input_requirement && !outcome.missing_mount);
                pairs.insert(
                    (index, outcome.destination),
                    outcome.result.achieved.unwrap(),
                );
            }
        }
        assert_eq!(pairs.len(), 4);
        pairs
    }
    fn continuity(&self) -> [Option<PositionContinuity>; 2] {
        self.shared
            .heads
            .map(|head| self.lane.continuity(head, ProgrammingOwner::Position))
    }
    /// The partial mask's original value and actual mix on this owner, if any.
    fn partial_mask(
        &self,
        output: &PublishedPhysicalFrame<PositionAdapter>,
        index: usize,
    ) -> Option<[f64; 3]> {
        let row = output
            .results
            .iter()
            .find(|row| row.target == self.shared.heads[index])?;
        program(row).samples.iter().find_map(|sample| match sample {
            FamilyCompositionSample::Known(sample)
                if sample.is_fix_at() && sample.activation_mix < 1. =>
            {
                let mix = f64::from(sample.activation_mix);
                assert!(mix > 0.);
                let [pan, tilt] = commanded_angles(&self.mask(index));
                Some([pan, tilt, mix])
            }
            _ => None,
        })
    }
}

/// One LiveGroup emission over both heads: one runtime instance and controller.
fn start_shared_emission(shared: &SharedRig, definition: &DynamicDefinition) -> DynamicRuntime {
    let rig = &shared.rig;
    let snapshot = rig.engine.snapshot();
    rig.engine
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    let link = Uuid::new_v4();
    let mutations = shared
        .heads
        .iter()
        .zip(&shared.targets)
        .map(|(&head, base)| {
            rig.programmers.set(
                rig.session,
                head,
                ProgrammingOwner::Position.key(),
                base.clone(),
            );
            DynamicProgrammerValueMutation::Set {
                fixture_id: head,
                attribute: ProgrammingOwner::Position.key(),
                value: DynamicSemanticValue::DynamicOn {
                    instance_link: link,
                    lane_id: definition.lanes[0].id,
                    dynamic: DynamicReference {
                        dynamic_id: Some(definition.id),
                        last_known_pool_number: definition.pool_number,
                        embedded_fallback: DynamicDefinitionSnapshot {
                            definition: Arc::new(definition.clone()),
                        },
                    },
                    overrides: DynamicInstanceOverrides {
                        size: 1.,
                        speed_multiplier: Rational::ONE,
                        phase_offset_degrees: 0.,
                    },
                    timing: DynamicValueTiming {
                        fade_millis: Some(1000),
                        delay_millis: None,
                    },
                },
            }
        })
        .collect::<Vec<_>>();
    assert!(
        rig.programmers
            .apply_dynamic_values(rig.session, &mutations, None)
    );
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
}

fn blend(from: [f64; 2], to: [f64; 2], progress: f64) -> [f64; 2] {
    std::array::from_fn(|axis| from[axis] + progress * (to[axis] - from[axis]))
}

/// Per-owner, per-copy native, physical-axis, DMX and accepted-continuity oracle.
fn verify_cut(
    desk: &ResumeDesk,
    capture: &PreparedOutputFrame,
    output: &PublishedPhysicalFrame<PositionAdapter>,
    expected: &Pairs,
) {
    assert!(
        output.requirements.is_empty(),
        "{:?}",
        output
            .requirements
            .iter()
            .map(|r| super::super::numeric::requirement_debug(&r.reason))
            .collect::<Vec<_>>()
    );
    assert_eq!(output.results.len(), 2);
    let mut claims = HashMap::new();
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        assert!(!row.quality.held);
        assert_eq!(program(row).base, desk.shared.targets[index]);
        assert_eq!(row.achieved.destinations.len(), 2);
        let accepted = desk
            .lane
            .continuity(head, ProgrammingOwner::Position)
            .expect("only the completed root cut installs accepted continuity");
        assert_eq!(accepted.instances.len(), 2);
        for destination in [desk.shared.rig.root, desk.copy] {
            let pair = commanded_angles(
                &row.achieved
                    .destinations
                    .iter()
                    .find(|value| value.destination == destination)
                    .unwrap()
                    .value,
            );
            let wanted = expected[&(index, destination)];
            for axis in 0..2 {
                assert!(
                    (pair[axis] - wanted[axis]).abs() < 0.07,
                    "owner {index}, {destination:?}: {pair:?} != {wanted:?}"
                );
            }
            let outcome = row
                .achieved
                .outcomes
                .iter()
                .find(|outcome| outcome.destination == destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(!outcome.input_requirement && !outcome.missing_mount);
            let physical = output
                .rendered
                .physical
                .instances
                .iter()
                .find(|instance| instance.instance_id == destination.0)
                .unwrap();
            assert!(physical.complete);
            assert!((physical.axes()[0].absolute_degrees().unwrap() - pair[0]).abs() < 0.03);
            assert!(
                (physical.axes()[index + 1].absolute_degrees().unwrap() - pair[1]).abs() < 0.03
            );
            let controls = &accepted
                .instances
                .iter()
                .find(|instance| instance.destination == destination)
                .unwrap()
                .controls;
            for write in row
                .writes
                .iter()
                .filter(|write| write.slot.destination == destination)
            {
                assert!(!write.parked);
                assert!(
                    controls
                        .iter()
                        .any(|&(channel, _, _, raw)| channel == write.slot.channel_index
                            && raw == write.raw)
                );
                if let Some(previous) =
                    claims.insert((destination, write.slot.channel_index), write.raw)
                {
                    assert_eq!(previous, write.raw, "both owners agree on the shared Pan");
                }
            }
        }
    }
    assert_eq!(claims.len(), 6, "three mechanical motors in both copies");
    for (destination, start) in [(desk.shared.rig.root, 0), (desk.copy, 19)] {
        let physical = output
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == destination.0)
            .unwrap();
        let bytes = &output.rendered.universes[&1];
        for channel in 0..3u32 {
            let raw = claims[&(destination, channel)];
            let slot = start + channel as usize * 2;
            let wire = u32::from(u16::from_be_bytes([bytes[slot], bytes[slot + 1]]));
            assert_eq!(
                wire, raw,
                "coarse/fine word for {destination:?} channel {channel}"
            );
            assert_eq!(physical.native_raw[channel as usize], raw);
        }
    }
    assert_eq!(output.token, capture.frame_token());
}

/// Refusal publishes no unheld owner row, keeps accepted continuity exactly, and writes only
/// the complete captured static baseline.
fn verify_refused(
    desk: &ResumeDesk,
    accepted: &[Option<PositionContinuity>; 2],
    capture: &PreparedOutputFrame,
    output: &PublishedPhysicalFrame<PositionAdapter>,
) {
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        for row in output.results.iter().filter(|row| row.target == head) {
            assert!(row.quality.held, "no speculative row escapes refusal");
            assert!(row.writes.iter().all(|write| write.parked));
        }
        assert_eq!(
            desk.lane.continuity(head, ProgrammingOwner::Position),
            accepted[index],
            "held diagnostics never replace accepted continuity"
        );
    }
    let baseline = desk
        .shared
        .rig
        .engine
        .preview_static_family_frame(
            capture,
            desk.shared
                .rig
                .engine
                .prepare_static_family_frame(capture, &[]),
        )
        .unwrap();
    assert_eq!(output.rendered.universes, baseline.universes);
}

/// Expected head-0 pair: its own original Resume blend, then the partial mask suffix once.
fn single_resume_expected(
    desk: &ResumeDesk,
    output: &PublishedPhysicalFrame<PositionAdapter>,
) -> Pairs {
    let scopes = desk.scopes(output);
    let from = desk.pairs(&desk.outgoing());
    let to = desk.pairs(&desk.incoming());
    let mut expected = HashMap::new();
    for index in 0..2 {
        for destination in [desk.shared.rig.root, desk.copy] {
            let key = (index, destination);
            let mut value = match desk.setup.heads[index] {
                Head::Resume => {
                    let resumes = &scopes[&index];
                    assert_eq!(resumes.len(), 1);
                    blend(from[&key], to[&key], f64::from(resumes[0].1))
                }
                Head::Constant | Head::Masked => from[&key],
            };
            if let Some([pan, tilt, mix]) = desk.partial_mask(output, index) {
                value = blend(value, [pan, tilt], mix);
            }
            expected.insert(key, value);
        }
    }
    expected
}

#[test]
fn multi_source_resume_fits_complete_cohort_per_copy() {
    let mut desk = ResumeDesk::new(Setup {
        heads: [Head::Resume, Head::Constant],
        size: 1.,
        shared_emission: false,
    });
    desk.add_partial_mask();
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    let row = output
        .results
        .iter()
        .find(|row| row.target == desk.shared.heads[0])
        .unwrap();
    let samples = &program(row).samples;
    assert_eq!(samples.len(), 2, "Resume plus an independent second source");
    assert!(
        samples.iter().any(|sample| activation(sample) < 1.),
        "the existing single-source runner refuses this owner"
    );
    assert_eq!(evidence.resume_cuts, 1, "one original Resume cut");
    assert_eq!(
        evidence.parents, 2,
        "both physical copies of the owner join it"
    );
    assert_eq!(evidence.endpoint_cohorts, 2);
    assert_eq!(
        evidence.completed, 1,
        "the new Resume operand consumer published"
    );
    assert_eq!(evidence.resume_active_unissued, 0);
    let from = desk.pairs(&desk.outgoing());
    assert!(
        (from[&(0, desk.copy)][1] - from[&(0, desk.shared.rig.root)][1]).abs() > 1.,
        "the displaced, calibrated copy needs its own Outgoing inverse"
    );
    let expected = single_resume_expected(&desk, &output);
    assert!(
        desk.partial_mask(&output, 0).is_some(),
        "parent suffix is present"
    );
    verify_cut(&desk, &capture, &output, &expected);
}

#[test]
fn equal_progress_independent_scopes_do_not_correlate() {
    let mut desk = ResumeDesk::new(Setup {
        heads: [Head::Resume, Head::Resume],
        size: 1.,
        shared_emission: false,
    });
    desk.add_partial_mask();
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    let scopes = desk.scopes(&output);
    assert_ne!(scopes[&0][0].0, scopes[&1][0].0, "two runtime occurrences");
    assert_eq!(
        scopes[&0][0].1, scopes[&1][0].1,
        "equal progress is deliberately not scope authority"
    );
    // Root cut A (owner 0), then B nested in each A child; the root then cuts B, whose
    // children replay owner 0's inherited Full goal and re-enter memoized A x B leaves.
    assert_eq!(evidence.resume_cuts, 6);
    assert_eq!(evidence.parents, 12, "two copies per cut, never four");
    assert_eq!(
        evidence.endpoint_cohorts, 8,
        "four Cartesian A x B leaves plus four single-scope children"
    );
    assert_eq!(evidence.completed, 1);
    assert_eq!(evidence.resume_active_unissued, 0);
    let expected = single_resume_expected(&desk, &output);
    verify_cut(&desk, &capture, &output, &expected);
}

#[test]
fn resume_nested_in_resume_operand_uses_inherited_goal() {
    let mut desk = ResumeDesk::new(Setup {
        heads: [Head::Resume, Head::Masked],
        size: 0.5,
        shared_emission: false,
    });
    desk.add_partial_mask();
    // Interrupt again while the first Resume is interior: its retained graph becomes the
    // Outgoing operand of a second Resume back to the original whole Target Current.
    let base = desk.shared.targets[0].clone();
    desk.hot_edit(|_, definition, _| to_target_current(definition, &base));
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    let resumes = desk.scopes(&output)[&0].clone();
    assert_eq!(resumes.len(), 2, "first Resume survives inside the second");
    // Owner 1's partial mask over its Target prefix is a MaskAdoption stage: one child.
    // Root: R1 cut (children R1o/R1i, each cutting owner 1's mask), R2 cut (children R2o/R2i).
    // R2o's Resume operand driver re-pends R1 and issues the nested locator (nested cut 1);
    // after it, R2o's mask child re-pends R1 again (nested cut 2, memoized leaves). The root
    // mask child replays owner 0's inherited Full goal: R1 and R2 cut again over memoized
    // children. Resume cuts 6, mask cuts 7, two parents (one owner's copies) per cut.
    assert_eq!(evidence.resume_cuts, 6);
    assert_eq!(evidence.resume_nested_cuts, 2);
    assert_eq!(evidence.parents, 26);
    assert_eq!(evidence.endpoint_cohorts, 13);
    assert_eq!(evidence.completed, 1);
    let snapshot = desk.runtime.snapshot();
    let second = snapshot.instances[0]
        .synchronized_resume_transition
        .unwrap()
        .occurrence_id;
    let (inner, outer) = match resumes.iter().position(|(id, _)| *id == second) {
        Some(1) => (resumes[0].1, resumes[1].1),
        Some(0) => (resumes[1].1, resumes[0].1),
        _ => unreachable!(),
    };
    let from = desk.pairs(&desk.outgoing());
    let mask = desk.partial_mask(&output, 0).unwrap();
    let [pan, tilt] = desk.shared.angles[0];
    let edited = [f64::from(pan), f64::from(tilt + DELTAS[0])];
    let mut expected = HashMap::new();
    for destination in [desk.shared.rig.root, desk.copy] {
        let key = (0, destination);
        // The incoming Size scales the literal edit from each copy's own adopted Current.
        let incoming = blend(from[&key], edited, f64::from(desk.setup.size));
        let first = blend(from[&key], incoming, f64::from(inner));
        let second = blend(first, from[&key], f64::from(outer));
        expected.insert(key, blend(second, [mask[0], mask[1]], mask[2]));
        let peer = desk.partial_mask(&output, 1).unwrap();
        expected.insert(
            (1, destination),
            blend(from[&(1, destination)], [peer[0], peer[1]], peer[2]),
        );
    }
    verify_cut(&desk, &capture, &output, &expected);
}

#[test]
fn active_scope_peer_completed_synchronously_is_refused() {
    let mut desk = ResumeDesk::new(Setup {
        heads: [Head::Resume, Head::Resume],
        size: 1.,
        shared_emission: true,
    });
    // A fully active whole Fixed mask completes owner 0 synchronously above its Resume. Its
    // original branch keeps the shared scope Active, yet it issues no Resume locator.
    desk.fix(0, desk.constant(0), None);
    desk.tick();
    desk.shared.rig.clock.advance_millis(40);
    let (_, accepted_output) = desk.tick();
    let accepted = desk.continuity();
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    let scopes = desk.scopes(&output);
    assert_eq!(scopes[&0][0].0, scopes[&1][0].0, "one shared runtime scope");
    assert_eq!(evidence.resume_active_unissued, 1);
    assert_eq!(evidence.resume_cuts, 0);
    assert_eq!(evidence.completed, 0);
    verify_refused(&desk, &accepted, &capture, &output);
    let _ = accepted_output;
}

#[test]
fn rejected_finalizer_preserves_continuity_then_retries_resume() {
    let mut desk = ResumeDesk::new(Setup {
        heads: [Head::Resume, Head::Constant],
        size: 1.,
        shared_emission: false,
    });
    desk.add_partial_mask();
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(evidence.completed, 1);
    let expected = single_resume_expected(&desk, &output);
    verify_cut(&desk, &capture, &output, &expected);
    let (occurrence, progress) = desk.scopes(&output)[&0][0];
    let accepted = desk.continuity();
    let runtime = desk.runtime.snapshot();
    let tracking = desk.lane.adapter().tracking.borrow().snapshot().unwrap();
    desk.shared.rig.clock.advance_millis(50);
    let next = desk.shared.rig.capture();
    assert!(
        prepare_live(
            &desk.shared.rig,
            &next,
            &capture,
            &desk.lane,
            &mut desk.runtime,
            &mut desk.origins,
            &mut desk.scratch
        )
        .is_err()
    );
    assert_eq!(
        desk.runtime.snapshot(),
        runtime,
        "producer sampling rolls back"
    );
    assert_eq!(
        desk.continuity(),
        accepted,
        "accepted continuity is preserved"
    );
    assert!(
        Arc::ptr_eq(
            &tracking,
            &desk.lane.adapter().tracking.borrow().snapshot().unwrap()
        ),
        "a speculative cut never stages tracking geometry"
    );
    let ((capture, retry), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(
        evidence.completed, 1,
        "the retry is the only new publication"
    );
    let (same, later) = desk.scopes(&retry)[&0][0];
    assert_eq!(same, occurrence);
    assert!(later > progress);
    let expected = single_resume_expected(&desk, &retry);
    verify_cut(&desk, &capture, &retry, &expected);
}
