//! TL-559 AC5 (adapter part): Semantic↔Direct takeover, fade and release of ONE Color owner
//! through the existing Live hybrid resolver, the Color adapter's frame `transition` and the
//! engine finalizer, with the generation's retained original catalogue as the model resolver.
use super::super::super::physical_adapter::color::DirectReplayOutcome;
use super::super::super::physical_adapter::color::profiles::{patched, rgbwauv, xyz};
use super::super::super::physical_adapter::color::tests::direct::{catalogue, direct};
use super::super::super::physical_adapter::color::tests::intent;
use super::super::super::physical_adapter::*;
use super::super::hybrid::*;
use super::*;
use light_engine::{NativeColorSourceCatalog, PreparedOutputFrame};
use light_fixture::FixtureProfile;

pub(super) struct Rig {
    pub(super) engine: Engine,
    pub(super) programmers: ProgrammerRegistry,
    pub(super) session: SessionId,
    pub(super) target: FixtureId,
    pub(super) clock: Arc<ManualClock>,
    pub(super) runtime: DynamicRuntime,
    pub(super) origins: DynamicSourceOrigins,
    pub(super) transaction: DynamicOutputFrameScratch,
    pub(super) hybrid: HybridFrameScratch,
    pub(super) catalogue: Arc<NativeColorSourceCatalog>,
    pub(super) profile: FixtureProfile,
    pub(super) last_static: Option<AttributeValue>,
    pub(super) requirements: Vec<TransitionRequirement>,
    /// The captured Cue Dynamic rows of the last composed frame (TL-603).
    pub(super) cue_rows: Vec<light_playback::ActiveCueDynamicValue>,
}

pub(super) fn semantic_value(intent: &ColorIntent) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: intent.clone(),
    }))
}

pub(super) fn uv_amount(value: &AttributeValue) -> Option<f32> {
    match value {
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Semantic { intent } => Some(intent.uv.amount),
            ColorProgram::Direct { portable, .. } => portable.uv.map(|uv| uv.amount),
        },
        _ => None,
    }
}

pub(super) fn is_semantic(value: &AttributeValue) -> bool {
    matches!(value, AttributeValue::ColorProgram(p) if matches!(p.as_ref(), ColorProgram::Semantic { .. }))
}

impl Rig {
    /// `base` builds the static Programmer value from the retained catalogue and the patched
    /// profile; `retained` adds further original profiles to the generation's catalogue.
    pub(super) fn new(
        base: impl FnOnce(&NativeColorSourceCatalog, &FixtureProfile) -> AttributeValue,
        retained: &[&FixtureProfile],
    ) -> Self {
        Self::with_target(FixtureId::new(), base, retained)
    }

    /// As `new`, programming a caller-chosen target (paired reference rigs share it).
    pub(super) fn with_target(
        target: FixtureId,
        base: impl FnOnce(&NativeColorSourceCatalog, &FixtureProfile) -> AttributeValue,
        retained: &[&FixtureProfile],
    ) -> Self {
        let profile = rgbwauv(Some(xyz(0.02, 0.01, 0.08)));
        let mut profiles = vec![&profile];
        profiles.extend_from_slice(retained);
        let catalogue = catalogue(&profiles);
        let base = base(&catalogue, &profile);
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        programmers.set(session, target, ProgrammingOwner::Color.key(), base);
        let engine = Engine::with_programming_contract_support(
            programmers.clone(),
            PROGRAMMING_CONTRACT_VERSION,
        );
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![patched(&profile, target, 1)].into(),
                native_color_sources: catalogue.clone(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        let runtime = DynamicRuntime::with_native_color_models(
            PROGRAMMING_CONTRACT_VERSION,
            catalogue.clone(),
        );
        Self {
            engine,
            programmers,
            session,
            target,
            clock,
            runtime,
            origins: Default::default(),
            transaction: Default::default(),
            hybrid: Default::default(),
            catalogue,
            profile,
            last_static: None,
            requirements: Vec::new(),
            cue_rows: Vec::new(),
        }
    }

    /// Replace the show's Dynamic pool, Cuelists and Playbacks; patch and catalogue are kept.
    pub(super) fn install_show(
        &self,
        dynamics: Vec<DynamicDefinition>,
        cue_lists: Vec<light_playback::CueList>,
        playbacks: Vec<light_playback::PlaybackDefinition>,
    ) {
        let mut snapshot = (*self.engine.snapshot()).clone();
        snapshot.dynamics = dynamics.into();
        snapshot.cue_lists = cue_lists.into();
        snapshot.playbacks = playbacks.into();
        snapshot.revision += 1;
        self.engine.replace_snapshot(snapshot).unwrap();
    }

    /// One real Playback pool action at the current clock.
    pub(super) fn playback(&self, number: u16, action: light_engine::PoolPlaybackAction) {
        self.engine
            .execute_playback(light_engine::EnginePlaybackCommand::Pool { number, action })
            .unwrap();
    }

    /// Leave Color to the Cuelists: the Programmer no longer holds a static Color value.
    pub(super) fn release_programmer_color(&self) {
        assert!(self.programmers.release_fixture_attribute(
            self.session,
            self.target,
            &ProgrammingOwner::Color.key()
        ));
    }

    /// Fade `value` in as a typed Fixed Color mask over whatever the owner currently shows.
    pub(super) fn fade_to(&self, value: AttributeValue, fade_millis: u64) {
        assert!(
            self.programmers.apply_dynamic_values(
                self.session,
                &[light_programmer::DynamicProgrammerValueMutation::Set {
                    fixture_id: self.target,
                    attribute: ProgrammingOwner::Color.key(),
                    value: DynamicSemanticValue::ProgrammingFixAt {
                        mask: light_dynamics::ProgrammingFamilyFixAt::from_family(
                            ProgrammingOwner::Color,
                            None,
                            value,
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

    pub(super) fn release(&self) {
        assert!(self.programmers.apply_dynamic_values(
            self.session,
            &[light_programmer::DynamicProgrammerValueMutation::Release {
                fixture_id: self.target,
                attribute: ProgrammingOwner::Color.key(),
                instance_link: None,
            }],
            None,
        ));
    }

    pub(super) fn capture(&self, advance: i64) -> PreparedOutputFrame {
        self.clock.advance_millis(advance);
        self.engine.prepare_output_frame(Default::default())
    }

    /// One Live frame; returns the single Color sidecar.
    pub(super) fn frame(
        &mut self,
        advance: i64,
        lane: &PhysicalAdapterLane<ColorAdapter>,
    ) -> Option<PhysicalHeadResult<ColorAdapter>> {
        let capture = self.capture(advance);
        let engine = &self.engine;
        let snapshot = capture.snapshot();
        let addresser = capture.frame_addresser();
        let speeds = [DynamicSpeedTransport {
            effective_bpm: 120.,
            phase_origin_millis: 0,
            phase_reference_millis: 0,
            beat_phase: 0.,
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
        self.cue_rows = capture.cue_dynamic_values().to_vec();
        let mut candidate = self.origins.clone();
        let scratch = &mut self.hybrid;
        let published = self
            .runtime
            .with_output_frame_transaction(&mut self.transaction, |runtime| {
                let prepared = prepare_captured_hybrid_frame(
                    engine,
                    &capture,
                    &[],
                    runtime,
                    &mut candidate,
                    &inputs,
                    scratch,
                    lane,
                    None,
                    |observation| lane.observe(observation),
                )?;
                finalize_live_physical_frame(engine, &capture, lane, prepared)
            })
            .unwrap();
        self.origins = candidate;
        self.requirements = published
            .requirements
            .iter()
            .filter_map(|r| match r.reason {
                HybridFamilyRequirementReason::Composition(requirement) => Some(requirement),
                _ => None,
            })
            .collect();
        assert_eq!(
            self.requirements.len(),
            published.requirements.len(),
            "only composition requirements are expected"
        );
        let static_value = published
            .rendered
            .resolved_values
            .value(self.target, &ProgrammingOwner::Color.key())
            .cloned();
        let mut results = published.results;
        assert!(results.len() <= 1, "at most one complete Color owner");
        let Some(result) = results.pop() else {
            // No typed contribution: the static owner renders unchanged.
            self.last_static = static_value;
            return None;
        };
        assert_eq!(result.token, capture.frame_token());
        assert_eq!(
            published
                .rendered
                .resolved_values
                .value(self.target, &ProgrammingOwner::Color.key()),
            Some(&result.value),
            "the engine received the same composed value"
        );
        Some(result)
    }

    pub(super) fn typed(
        &mut self,
        advance: i64,
        lane: &PhysicalAdapterLane<ColorAdapter>,
    ) -> PhysicalHeadResult<ColorAdapter> {
        self.frame(advance, lane).expect("typed Color owner")
    }

    pub(super) fn uv_write(&self, result: &PhysicalHeadResult<ColorAdapter>) -> u32 {
        let uv = self.profile.modes[0].channels.last().unwrap().id;
        result
            .writes
            .iter()
            .find(|w| w.channel_id == uv)
            .unwrap()
            .raw
    }
}

/// Semantic base → Direct takeover fade → completion → release. Current is adopted once per
/// frame into the recipe's own verified source (fitted under the frame token, captured by the
/// original model); the composer then interpolates native controls, so every sample is one
/// complete Direct value replayed exactly and UV rises monotonically. Completion is the exact
/// recipe; release reveals the Semantic base without a stale UV.
#[test]
fn semantic_to_direct_takeover_fade_completion_and_release_keep_one_owner() {
    let base = ColorIntent {
        relative_output: 0.5,
        ..intent([0., 0., 1.], 0.)
    };
    let mut rig = Rig::new(|_, _| semantic_value(&base), &[]);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    assert!(rig.frame(10, &lane).is_none());
    assert_eq!(rig.last_static, Some(semantic_value(&base)));

    let recipe = direct(&rig.catalogue, &rig.profile, &[200, 40, 0, 0, 0, 255]);
    rig.fade_to(recipe.clone(), 1_000);
    let mut previous_uv = 0;
    for step in 0..3 {
        let mid = rig.typed(250, &lane);
        assert!(rig.requirements.is_empty());
        assert!(!is_semantic(&mid.value), "step {step}: native fade");
        let status = mid.quality.direct.as_ref().unwrap();
        assert_eq!(status.replay, DirectReplayOutcome::Exact);
        let uv = rig.uv_write(&mid);
        assert!(
            uv > previous_uv && uv < 255,
            "step {step}: UV fades in ({uv})"
        );
        previous_uv = uv;
    }
    assert!(lane.adapter().counters().representation_adoptions >= 3);
    let done = rig.typed(500, &lane);
    assert_eq!(done.value, recipe, "exact Direct recipe at completion");
    assert_eq!(
        done.writes.iter().map(|w| w.raw).collect::<Vec<_>>(),
        [200, 40, 0, 0, 0, 255]
    );

    rig.release();
    for _ in 0..8 {
        if let Some(result) = rig.frame(250, &lane) {
            assert_eq!(result.value, semantic_value(&base));
        }
    }
    assert_eq!(rig.last_static, Some(semantic_value(&base)));
}

/// Direct base (UV full) → Semantic fade: Current is adopted into Semantic through its source
/// estimate and blended by the core complete-family interpolation. UV fades between the two
/// endpoint amounts and is 0 at completion, never kept from the Direct recipe.
#[test]
fn direct_to_semantic_fade_blends_portable_appearance_and_never_keeps_the_previous_uv() {
    let mut rig = Rig::new(
        |catalogue, profile| direct(catalogue, profile, &[200, 40, 0, 0, 0, 255]),
        &[],
    );
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    assert!(rig.frame(10, &lane).is_none());
    let blue = ColorIntent {
        relative_output: 0.8,
        ..intent([0., 0.3, 1.], 0.)
    };
    rig.fade_to(semantic_value(&blue), 1_000);
    let mut previous_uv = 1.;
    for step in 0..3 {
        let mid = rig.typed(250, &lane);
        assert!(rig.requirements.is_empty());
        assert!(is_semantic(&mid.value), "step {step}: portable blend");
        let uv = uv_amount(&mid.value).unwrap();
        assert!(
            uv < previous_uv && uv > 0.,
            "step {step}: UV fades out ({uv})"
        );
        previous_uv = uv;
        let expected = (f64::from(uv) * 255.).round() as u32;
        assert!(rig.uv_write(&mid).abs_diff(expected) <= 1);
    }
    let done = rig.typed(500, &lane);
    assert_eq!(done.value, semantic_value(&blue));
    assert_eq!(rig.uv_write(&done), 0, "previous UV never leaks");
}

/// The adapter's frame `transition` (reached by the composer's Required/endpoint-crossfade
/// paths) through the lane's `HybridFrameResolver::resolve` on a real captured frame:
/// exact endpoints, portable interior blend with UV between the endpoint amounts, a held source
/// for unknown appearance, and `Scale` staying passive.
#[test]
fn frame_transition_blends_semantic_and_direct_with_exact_endpoints() {
    let foreign = rgbwauv(None);
    let rig = Rig::new(
        |_, _| semantic_value(&intent([1., 0., 0.], 0.)),
        &[&foreign],
    );
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    let capture = rig.capture(10);
    let token = capture.frame_token();
    lane.begin_frame(&token).unwrap();
    let mut scalar = rig.engine.prepare_static_family_frame(&capture, &[]);
    let geometry = rig
        .engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let frame = HybridFrameContext {
        capture: &capture,
        geometry: &geometry,
        native_models: rig.catalogue.as_ref(),
        token: &token,
        scalar: &scalar,
    };
    let semantic = semantic_value(&intent([0., 0., 1.], 0.));
    let uv_full = direct(&rig.catalogue, &rig.profile, &[200, 40, 0, 0, 0, 255]);
    let resolve = |from: &AttributeValue, to: &AttributeValue, operation| {
        lane.resolve(
            frame,
            rig.target,
            TransitionRequirement::ColorAppearance,
            from,
            to,
            operation,
        )
        .map(|(value, _)| value)
    };
    let at = |progress| FamilyExpressionOperation::Transition { progress };
    assert_eq!(resolve(&uv_full, &semantic, at(0.)).unwrap(), uv_full);
    assert_eq!(resolve(&uv_full, &semantic, at(1.)).unwrap(), semantic);
    for (progress, expected) in [(0.25f32, 0.75f32), (0.5, 0.5), (0.75, 0.25)] {
        let mid = resolve(&uv_full, &semantic, at(progress)).unwrap();
        assert!(is_semantic(&mid));
        assert!(
            (uv_amount(&mid).unwrap() - expected).abs() < 1e-4,
            "{progress}"
        );
    }
    let mid = resolve(&semantic, &uv_full, at(0.5)).unwrap();
    assert!((uv_amount(&mid).unwrap() - 0.5).abs() < 1e-4);
    // Unknown appearance holds the source until completion.
    let unknown = direct(&rig.catalogue, &foreign, &[0, 0, 0, 0, 0, 128]);
    assert_eq!(resolve(&semantic, &unknown, at(0.5)).unwrap(), semantic);
    assert_eq!(resolve(&semantic, &unknown, at(1.)).unwrap(), unknown);
    assert!(matches!(
        resolve(
            &semantic,
            &uv_full,
            FamilyExpressionOperation::Scale { factor: 0.5 }
        ),
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance
        ))
    ));
    let counters = lane.adapter().counters();
    assert_eq!(
        (
            counters.representation_transitions,
            counters.representation_holds
        ),
        (5, 1)
    );
}
