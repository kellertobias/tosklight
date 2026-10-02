//! TL-600 retained Preload: a whole FixAT fade into a Direct recipe of a source foreign to the
//! patched fixture runs in BOTH retained branches through the actual paired evaluator, the real
//! Color adapter lanes and the Preload finalizer. Each branch samples the portable appearance
//! through its own captured frame and lane continuity; a pending Release changes only After.
use super::*;
use crate::runtime::dynamic_source_origins::{
    DynamicFixedSource, DynamicProgrammerSourceLane, DynamicSourceBinding,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::color::profiles::{
    patched, rgbal, rgbwauv, xyz,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::color::tests::intent;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::color::tests_direct::{
    catalogue, direct,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::color::{
    ColorAdapter, DirectReplayOutcome,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::*;
use light_fixture::FixtureProfile;

type Sidecar = PhysicalHeadResult<ColorAdapter>;
type Pair = PairedPendingHistory<PendingHybridResult<Sidecar>>;

struct ColorRig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    target: FixtureId,
    clock: Arc<ManualClock>,
    key: PendingEpisodeKey,
    publication: DynamicSnapshotPublication,
    live: RefCell<DynamicRuntime>,
    started: Instant,
    selected: Cell<u64>,
    catalogue: Arc<light_engine::NativeColorSourceCatalog>,
}

fn semantic(intent: ColorIntent) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
}

/// The two portable endpoints blended by the shared core interpolation.
fn expected(from: &AttributeValue, to: &AttributeValue, progress: f32) -> AttributeValue {
    let portable = |value: &AttributeValue| {
        let AttributeValue::ColorProgram(program) = value else {
            panic!("Color")
        };
        match program.as_ref() {
            ColorProgram::Semantic { .. } => value.clone(),
            ColorProgram::Direct { portable, .. } => {
                semantic(semantic_color_adoption(portable, None).unwrap().intent)
            }
        }
    };
    interpolate_programming_value(&portable(from), &portable(to), progress).unwrap()
}

impl ColorRig {
    fn new(profile: &FixtureProfile, retained: &[&FixtureProfile], base: &AttributeValue) -> Self {
        let mut profiles = vec![profile];
        profiles.extend_from_slice(retained);
        let catalogue = catalogue(&profiles);
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        let programmer = programmers.start(session).id;
        let target = FixtureId::new();
        let engine = Engine::with_programming_contract_support(
            programmers.clone(),
            PROGRAMMING_CONTRACT_VERSION,
        );
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![patched(profile, target, 1)].into(),
                native_color_sources: catalogue.clone(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        let publication = DynamicSnapshotPublication::new(engine.snapshot());
        let mut live = DynamicRuntime::with_native_color_models(
            PROGRAMMING_CONTRACT_VERSION,
            catalogue.clone(),
        );
        publication
            .begin_retained_history(&mut live, &engine.snapshot(), capacity(64))
            .unwrap();
        programmers.arm_preload(session, true);
        clock.advance_millis(10);
        // The Fixed composition reads an explicitly captured static underlay.
        programmers.set(session, target, ProgrammingOwner::Color.key(), base.clone());
        Self {
            engine,
            programmers,
            session,
            target,
            clock,
            key: PendingEpisodeKey {
                activation: Uuid::new_v4(),
                programmer,
                branch: PreloadBranch::BeforeRelease,
            },
            publication,
            live: RefCell::new(live),
            started: Instant::now(),
            selected: Cell::new(0),
            catalogue,
        }
    }

    fn fade_to(&self, value: AttributeValue, fade_millis: u64) {
        assert!(
            self.programmers.apply_dynamic_values(
                self.session,
                &[light_programmer::DynamicProgrammerValueMutation::Set {
                    fixture_id: self.target,
                    attribute: ProgrammingOwner::Color.key(),
                    value: DynamicSemanticValue::ProgrammingFixAt {
                        mask: ProgrammingFamilyFixAt::from_family(
                            ProgrammingOwner::Color,
                            None,
                            value
                        )
                        .unwrap(),
                        timing: DynamicValueTiming {
                            fade_millis: Some(fade_millis),
                            delay_millis: None,
                        },
                    },
                }],
                None,
            )
        );
    }

    fn release_color(&self) {
        assert!(self.programmers.apply_dynamic_values(
            self.session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id: self.target,
                attribute: ProgrammingOwner::Color.key(),
                value: DynamicSemanticValue::Release,
            }],
            None,
        ));
    }

    fn pair(&self) -> Pair {
        let mut live = self.live.borrow_mut();
        let (cold, controls) = self
            .publication
            .begin_retained_history(&mut live, &self.engine.snapshot(), capacity(64))
            .unwrap();
        let seed = |branch| PendingHistorySeed {
            key: PendingEpisodeKey { branch, ..self.key },
            runtime: live.fork_for_pending_preview(),
            origins: Default::default(),
            snapshot: self.engine.snapshot(),
            position: PendingHistoryPosition {
                inputs: self.publication.input_capture_cursor().unwrap(),
                cold,
                controls,
            },
            live_sample: live.committed_sample_boundary(),
        };
        PairedPendingHistory::new(
            seed(PreloadBranch::BeforeRelease),
            seed(PreloadBranch::AfterRelease),
        )
        .unwrap()
    }

    fn capture(&self, advance: i64) -> Arc<RetainedInputCapture> {
        self.clock.advance_millis(advance);
        let cursor = self.publication.input_capture_cursor().unwrap();
        let selected = self.selected.get();
        self.selected.set(selected + 1);
        let frame = RetainedFrameCapture::select(
            self.engine.prepare_output_frame(Default::default()),
            &self.publication,
            self.started + Duration::from_millis(selected * 40),
        );
        self.publication.retain_accepted_input(
            &self.live.borrow(),
            frame.retained().unwrap(),
            &[],
            &transports(),
            37,
            None,
        );
        self.publication
            .input_captures_since(cursor)
            .unwrap()
            .remove(0)
    }

    fn consume(
        &self,
        advance: i64,
        pair: &mut Pair,
        evaluator: &mut impl PendingPairEvaluator<PendingHybridResult<Sidecar>>,
    ) {
        let input = self.capture(advance);
        let (before, after) = pair.positions();
        let live = self.live.borrow();
        let before_controls = live.controls_since(before.controls).unwrap().unwrap();
        let after_controls = live.controls_since(after.controls).unwrap().unwrap();
        let window = pair
            .prepare_window(
                &[input],
                &[],
                &before_controls,
                &[],
                &after_controls,
                limits(),
            )
            .unwrap();
        drop(live);
        let outcome = pair.consume_window(window, evaluator);
        assert_eq!(
            outcome.successful_attempts, 1,
            "{:?}",
            outcome.failed_attempts
        );
    }
}

/// The Fixed Preload Programmer source that owns this sidecar, from its owned provenance.
fn fixed_source(
    sidecar: &Sidecar,
) -> Option<Arc<crate::runtime::dynamic_source_origins::DynamicSourceRecord>> {
    sidecar
        .provenance
        .sources
        .entries()?
        .iter()
        .find(|entry| {
            matches!(
                entry.record().binding,
                DynamicSourceBinding::Fixed {
                    source: DynamicFixedSource::Programmer {
                        lane: DynamicProgrammerSourceLane::Preload,
                        ..
                    },
                    owner: ProgrammingOwner::Color,
                    ..
                }
            )
        })
        .map(|entry| Arc::clone(entry.record()))
}

fn color(branch: &PendingHybridBranch<Sidecar>) -> Option<&Sidecar> {
    let mut rows = branch
        .sidecars
        .iter()
        .filter(|row| row.owner == ProgrammingOwner::Color);
    let row = rows.next();
    assert!(rows.next().is_none(), "at most one Color owner per branch");
    row
}

#[test]
fn retained_preload_branches_fade_into_a_foreign_direct_recipe_independently() {
    let profile = rgbwauv(Some(xyz(0.02, 0.01, 0.08)));
    let foreign = rgbal();
    let base = semantic(intent([1., 0., 0.], 0.));
    let rig = ColorRig::new(&profile, &[&foreign], &base);
    let mut pair = rig.pair();
    let lanes = PhysicalPreloadLanes::new(ColorAdapter::default(), ColorAdapter::default());
    let mut evaluator = RetainedPreloadHybridEvaluator::new(
        &rig.engine,
        rig.key.programmer,
        &lanes,
        |branch, observation: HybridFamilyObservation<'_>| lanes.observe(branch, observation),
    );
    let to = direct(&rig.catalogue, &foreign, &[0, 0, 255, 0, 0]);
    rig.fade_to(to.clone(), 1_000);
    let branches = [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease];
    for (index, progress) in [0.25f32, 0.5].into_iter().enumerate() {
        rig.consume(250, &mut pair, &mut evaluator);
        let result = &pair.last_success().unwrap().value;
        for (branch, value) in [(branches[0], &result.before), (branches[1], &result.after)] {
            assert!(
                value
                    .requirements
                    .iter()
                    .all(|row| row.owner != ProgrammingOwner::Color)
            );
            let sidecar = color(value).unwrap_or_else(|| panic!("{branch:?} {index}: Color owner"));
            assert_eq!(
                sidecar.token, value.frame_token,
                "{branch:?}: its own captured frame"
            );
            assert_eq!(sidecar.token.lane().preload_branch(), Some(branch));
            assert_eq!(
                sidecar.value,
                expected(&base, &to, progress),
                "{branch:?} {index}: portable appearance of the ORIGINAL endpoints"
            );
            let lane = lanes.lane(branch);
            assert_eq!(lane.last_accepted(), Some(value.frame_token.clone()));
            assert!(
                lane.continuity(rig.target, ProgrammingOwner::Color)
                    .is_some()
            );
        }
        // Appearance interpolation is not exact native-component attribution: the owned
        // provenance reports an unknown field transfer rather than inventing exact sources.
        for branch in [&result.before, &result.after] {
            assert!(
                color(branch)
                    .unwrap()
                    .provenance
                    .sources
                    .entries()
                    .is_none()
            );
        }
        assert_ne!(result.before.frame_token, result.after.frame_token);
        assert!(
            result
                .before
                .frame_token
                .same_capture(&result.after.frame_token)
        );
    }
    for branch in branches {
        assert_eq!(
            lanes
                .lane(branch)
                .adapter()
                .counters()
                .representation_transitions,
            2,
            "{branch:?}: each lane samples its own frames"
        );
    }
    // A pending Release changes After only; Before keeps fading and completes exactly.
    rig.release_color();
    rig.consume(250, &mut pair, &mut evaluator);
    let result = &pair.last_success().unwrap().value;
    assert_eq!(
        color(&result.before).unwrap().value,
        expected(&base, &to, 0.75)
    );
    assert!(
        color(&result.after).is_none(),
        "After reveals the static owner"
    );
    let after_lane = lanes.lane(PreloadBranch::AfterRelease);
    let [released] = after_lane
        .released()
        .try_into()
        .unwrap_or_else(|rows: Vec<_>| panic!("one released Color owner, got {}", rows.len()));
    assert_eq!(released.owner, ProgrammingOwner::Color);
    assert_eq!(
        after_lane.continuity(rig.target, ProgrammingOwner::Color),
        None
    );
    assert!(
        lanes
            .lane(PreloadBranch::BeforeRelease)
            .released()
            .is_empty()
    );
    rig.consume(250, &mut pair, &mut evaluator);
    let result = &pair.last_success().unwrap().value;
    let before = color(&result.before).unwrap();
    assert_eq!(before.value, to, "exact tagged destination at completion");
    // The completed whole write is exact again: Before's own Fixed Preload source owns it.
    let source = fixed_source(before).expect("exact Fixed Preload provenance at completion");
    assert!(result.before.origins.binding(&source.binding).is_some());
    assert!(result.after.origins.binding(&source.binding).is_none());
    assert!(matches!(
        before.quality.direct.as_ref().unwrap().replay,
        DirectReplayOutcome::Fallback { .. }
    ));
    assert!(color(&result.after).is_none());
    assert_eq!(
        lanes
            .lane(PreloadBranch::BeforeRelease)
            .adapter()
            .counters()
            .representation_transitions,
        3
    );
}
