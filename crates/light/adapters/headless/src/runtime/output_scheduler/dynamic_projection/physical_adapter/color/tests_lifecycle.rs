//! TL-557 AC2: Record magenta and warm white on an RGB fixture through the application's real
//! Cue writer, reopen the SQLite show, then replace the destination with RGBW (White seeded
//! nonzero), CMY+wheel and a hybrid RGBW+wheel, and add a new live-Group member.
//!
//! Every step recompiles the reopened show through the show-open candidate compiler, installs
//! the compiled snapshot into an engine, plays the Cues and resolves the stored intent through
//! the Color adapter on real captured frames. The stored Cue list body, its Cue count and the
//! requested intent never change; only destination fitting does, and every destination Color
//! channel is written or parked.
use super::profiles::*;
use super::tests::{magenta, program, warm_white};
use super::*;
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, ActiveShowPorts,
    ActiveShowService, ActiveShowUnitOfWork, BackupIdentity, CueNumber, EventBus,
    ProgrammingCueActivationCompletion, ProgrammingCueActivationPolicy,
    ProgrammingCueActiveShowPorts, ProgrammingCueCapturePolicy, ProgrammingCueCommit,
    ProgrammingCueCommitResult, ProgrammingCueProjections, ProgrammingCueRecordOperation,
    ProgrammingCueRecordRequest, ProgrammingCueRecordTarget, ProgrammingCueRecordTiming,
    ProgrammingCueRecordingEnvironment, ProgrammingCueRecordingPorts, ProgrammingCueResolvedTarget,
    ProgrammingCueShowRevisionExpectation, ProgrammingService, prepare_show_candidate,
};
use light_core::{ManualClock, SessionId, ShowId};
use light_dynamics::DynamicRuntime;
use light_engine::{
    Engine, EnginePlaybackCommand, EngineSnapshot, PoolPlaybackAction, RenderOptions,
};
use light_fixture::{FixtureProfile, PortablePatchedFixtureRecord};
use light_programmer::{GroupDefinition, HighlightRegistry, ProgrammerRegistry};
use light_show::{
    FixtureProfileRevision, PortableShowCommit, PortableShowDocument, PortableShowObjectUndo,
    PortableShowTransaction, ShowStore,
};
use serde_json::Value;
use std::path::PathBuf;

/// TL-629: Focus/Zoom Group Cues across optical replacement.
mod optics_replacement;
// TL-628: saved semantic Color Dynamics across reopen, replacement and live-Group growth.
mod dynamic_programming;
// TL-560: missing persistence-matrix cells (replacement, unpatched, Preload GO, undo, Media).
mod tl560;

const PLAYBACK: u16 = 8;
const GROUP: &str = "tl557-front";

struct Ports {
    path: PathBuf,
    show_id: ShowId,
    service: ActiveShowService,
}

struct Unit {
    store: ShowStore,
    document: PortableShowDocument,
}

fn internal(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Internal, error.to_string())
}

impl ActiveShowUnitOfWork for Unit {
    fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    fn backup(&mut self, _identity: &BackupIdentity) -> Result<(), ActionError> {
        Ok(())
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        self.store
            .apply_portable_transaction(transaction)
            .map_err(internal)
    }
}

impl ActiveShowPorts for Ports {
    type UnitOfWork = Unit;
    type PreparedRuntime = EngineSnapshot;

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Unit, ActionError> {
        assert_eq!(show_id, self.show_id);
        let store = ShowStore::open(&self.path).map_err(internal)?;
        let document = store.portable_document().map_err(internal)?;
        Ok(Unit { store, document })
    }

    fn prepare_object_undo(
        &self,
        _unit: &Unit,
        _kind: &str,
        _object_id: &str,
        _expected_object_revision: u64,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        unreachable!("Cue recording does not use object Undo")
    }

    fn prepare_runtime(&self, snapshot: EngineSnapshot) -> Result<EngineSnapshot, ActionError> {
        snapshot
            .validate()
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
        Ok(snapshot)
    }

    fn install_runtime(&self, _context: &ActionContext, _prepared: EngineSnapshot) {}
}

impl ProgrammingCueActiveShowPorts for Ports {
    fn reconcile_programming_cue(&self, _projections: &ProgrammingCueProjections) {}
}

/// The Record adapter: the application captures the Programmer and commits through the
/// centralized ActiveShow Cue writer.
impl ProgrammingCueRecordingPorts for Ports {
    fn authorize_cue_recording(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn cue_recording_environment(
        &self,
        _context: &ActionContext,
        _request: &ProgrammingCueRecordRequest,
    ) -> Result<ProgrammingCueRecordingEnvironment, ActionError> {
        Ok(ProgrammingCueRecordingEnvironment {
            target: ProgrammingCueResolvedTarget::Playback {
                playback_number: PLAYBACK,
                page_slot: None,
            },
            active_cue: None,
            cuelist_auto_off_at_zero_default: false,
            cuelist_auto_off_flash_release_default: false,
            start_after_first_recording: false,
        })
    }

    fn commit_cue(
        &self,
        context: &ActionContext,
        commit: &ProgrammingCueCommit,
    ) -> Result<ProgrammingCueCommitResult, ActionError> {
        self.service.commit_programming_cue(context, commit, self)
    }

    fn activate_recorded_cue(
        &self,
        _context: &ActionContext,
        _playback_number: u16,
        _cue_number: CueNumber,
    ) -> Option<ProgrammingCueActivationCompletion> {
        None
    }
}

/// One SQLite show under the canonical temporary directory, removed on drop, recorded from a
/// real Programmer session.
struct Show {
    ports: Ports,
    programmers: ProgrammerRegistry,
    programming: ProgrammingService,
    session: SessionId,
}

impl Drop for Show {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.ports.path.display(), suffix));
        }
    }
}

fn fixture(profile: &FixtureProfile, id: FixtureId, number: u32, address: u16) -> PatchedFixture {
    let mut fixture = patched(profile, id, address);
    fixture.fixture_number = Some(number);
    fixture.name = format!("TL-557 {number}");
    fixture
}

impl Show {
    fn new() -> Self {
        let directory = std::env::var_os("LIGHT_TMP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("tl557-ac2-{}.sqlite", Uuid::new_v4()));
        let (store, show_id) = ShowStore::create(&path, "TL-557 AC2").unwrap();
        drop(store);
        let events = EventBus::new(16);
        let programmers = ProgrammerRegistry::default();
        let session = SessionId::new();
        programmers.start(session);
        Self {
            ports: Ports {
                path,
                show_id,
                service: ActiveShowService::new(events.clone()),
            },
            programming: ProgrammingService::new(
                programmers.clone(),
                events,
                Arc::new(HighlightRegistry::default()),
            ),
            programmers,
            session,
        }
    }

    fn store(&self) -> ShowStore {
        ShowStore::open(&self.ports.path).unwrap()
    }

    /// Save/reload boundary: every read reopens the SQLite file.
    fn document(&self) -> PortableShowDocument {
        self.store().portable_document().unwrap()
    }

    fn revision(&self, kind: &str, id: &str) -> u64 {
        self.document().object(kind, id).map_or(0, |o| o.revision())
    }

    fn put(&self, kind: &str, id: &str, body: Value) {
        let expected = self.revision(kind, id);
        self.store().put_object(kind, id, &body, expected).unwrap();
    }

    /// Patch (or re-patch) a fixture record and store its immutable profile revision.
    fn patch(&self, fixture: &PatchedFixture) {
        let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
        self.store()
            .insert_fixture_profile_revision(
                &FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap())
                    .unwrap(),
            )
            .unwrap();
        let body = PortablePatchedFixtureRecord::from_runtime_fixture(fixture)
            .unwrap()
            .into_body();
        self.put("patched_fixture", &fixture.fixture_id.0.to_string(), body);
    }

    fn group(&self, members: &[FixtureId]) {
        let group = GroupDefinition {
            id: GROUP.into(),
            name: "Front".into(),
            fixtures: members.to_vec(),
            ..Default::default()
        };
        self.put("group", GROUP, serde_json::to_value(group).unwrap());
    }

    /// Record the Programmer's current values as Cue `number`, then clear the Programmer.
    fn record(&self, number: f64) {
        self.record_as(number, &format!("record-{number}"));
    }

    fn record_as(&self, number: f64, request: &str) {
        let context =
            ActionContext::operator(Uuid::from_u128(1), self.session.0, ActionSource::Http)
                .with_request_id(format!("tl557-{request}"));
        let result = self
            .programming
            .handle_cue_recording(
                ActionEnvelope {
                    context,
                    command: ProgrammingCueRecordRequest {
                        show_id: self.ports.show_id,
                        target: ProgrammingCueRecordTarget::Pool {
                            playback_number: PLAYBACK,
                        },
                        operation: ProgrammingCueRecordOperation::Overwrite,
                        cue_number: Some(CueNumber::try_from_legacy_f64(number).unwrap()),
                        timing: ProgrammingCueRecordTiming::default(),
                        cue_only: false,
                        name: None,
                        capture_policy: ProgrammingCueCapturePolicy::CurrentCapture,
                        activation_policy: ProgrammingCueActivationPolicy::Hold,
                        expected_show_revision: ProgrammingCueShowRevisionExpectation::Current,
                    },
                },
                &self.ports,
            )
            .unwrap();
        assert!(
            matches!(
                result.outcome,
                light_application::ProgrammingCueRecordOutcome::Changed { .. }
            ),
            "{result:?}"
        );
        self.programmers.clear(self.session);
        self.programmers.start(self.session);
    }

    /// The only stored Cue list: `(object id, exact persisted body)`.
    fn cue_list(&self) -> (String, Value) {
        let document = self.document();
        let mut lists = document.objects_of_kind("cue_list");
        let list = lists.next().expect("one recorded Cue list");
        assert!(lists.next().is_none());
        (list.key().id().to_owned(), list.body().clone())
    }

    /// Reopen and compile through the show-open candidate compiler.
    fn compile(&self) -> EngineSnapshot {
        let document = self.document();
        prepare_show_candidate(&document, document.transaction())
            .unwrap()
            .into_parts()
            .1
    }
}

/// Engine plus adapter over one compiled snapshot.
struct Output {
    engine: Engine,
    clock: Arc<ManualClock>,
    adapter: ColorAdapter,
}

impl Output {
    fn new() -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        Self {
            engine: Engine::with_programming_contract_support(
                ProgrammerRegistry::with_clock(clock.clone()),
                light_core::programming::PROGRAMMING_CONTRACT_VERSION,
            ),
            clock,
            adapter: ColorAdapter::default(),
        }
    }

    fn go(&self, snapshot: EngineSnapshot, cue: f64) {
        self.engine.replace_snapshot(snapshot).unwrap();
        self.engine
            .execute_playback(EnginePlaybackCommand::Pool {
                number: PLAYBACK,
                action: PoolPlaybackAction::GoTo(CueNumber::try_from_legacy_f64(cue).unwrap()),
            })
            .unwrap();
    }

    /// The played Cue's composed Color owner of `target`.
    fn played(&self, target: FixtureId) -> ColorIntent {
        let values = self.engine.resolved_values();
        let value = values
            .get(&(target, ProgrammingOwner::Color.key()))
            .expect("the played Cue owns Color");
        semantic_intent(value).unwrap().clone()
    }

    /// The played Cue's exact composed Color value of `target` (Semantic or Direct).
    fn played_value(&self, target: FixtureId) -> AttributeValue {
        self.engine
            .resolved_values()
            .get(&(target, ProgrammingOwner::Color.key()))
            .expect("the played Cue owns Color")
            .clone()
    }

    /// Resolve `target`'s played value against the compiled generation's own retained
    /// original catalogue (TL-559 Direct replay).
    fn resolve_played(
        &self,
        target: FixtureId,
    ) -> (ColorDescriptor, PhysicalResolution<ColorAdapter>) {
        self.clock.advance_millis(25);
        let capture = self.engine.prepare_output_frame(RenderOptions::default());
        let token = capture.frame_token();
        let mut scalar = self.engine.prepare_static_family_frame(&capture, &[]);
        let geometry = self
            .engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let snapshot = capture.snapshot();
        let descriptor = self
            .adapter
            .compile(&snapshot, target)
            .unwrap()
            .expect("physical Color destination");
        let value = self.played_value(target);
        let result = self
            .adapter
            .resolve(PhysicalRequest {
                frame: HybridFrameContext {
                    capture: &capture,
                    geometry: &geometry,
                    native_models: snapshot.native_color_sources.as_ref(),
                    token: &token,
                    scalar: &scalar,
                },
                target,
                owner: ProgrammingOwner::Color,
                descriptor: &descriptor,
                value: &value,
                previous: None,
            })
            .unwrap();
        validate_complete_writes(&descriptor.footprint, &result.writes).unwrap();
        (descriptor, result)
    }

    /// Resolve `target`'s played intent through the lamp adapter on a real captured frame.
    fn resolve(
        &self,
        target: FixtureId,
    ) -> (ColorDescriptor, Vec<u32>, PhysicalResolution<ColorAdapter>) {
        self.resolve_with(&self.adapter, target)
    }

    fn resolve_with<A: PhysicalFamilyAdapter>(
        &self,
        adapter: &A,
        target: FixtureId,
    ) -> (A::Descriptor, Vec<u32>, PhysicalResolution<A>) {
        self.clock.advance_millis(25);
        let capture = self.engine.prepare_output_frame(RenderOptions::default());
        let token = capture.frame_token();
        let mut scalar = self.engine.prepare_static_family_frame(&capture, &[]);
        let geometry = self
            .engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let models = DynamicRuntime::default().captured_native_color_models();
        let descriptor = adapter
            .compile(&capture.snapshot(), target)
            .unwrap()
            .expect("physical Color destination");
        let native = scalar
            .native_raw(&capture, &token, target)
            .unwrap()
            .raw()
            .to_vec();
        let value = program(&self.played(target));
        let result = adapter
            .resolve(PhysicalRequest {
                frame: HybridFrameContext {
                    capture: &capture,
                    geometry: &geometry,
                    native_models: models.as_ref(),
                    token: &token,
                    scalar: &scalar,
                },
                target,
                owner: ProgrammingOwner::Color,
                descriptor: &descriptor,
                value: &value,
                previous: None,
            })
            .unwrap();
        validate_complete_writes(adapter.footprint(&descriptor), &result.writes).unwrap();
        (descriptor, native, result)
    }
}

fn color_channels(profile: &FixtureProfile) -> Vec<u32> {
    (0u32..)
        .zip(&profile.modes[0].channels)
        .filter(|(_, c)| c.attribute.0.starts_with("color."))
        .map(|(index, _)| index)
        .collect()
}

/// RGBW whose White channel defaults to 204 (80 %): the destination starts with White on.
fn rgbw_white_on() -> FixtureProfile {
    let mut profile = rgbw();
    profile.modes[0].channels[4].default_raw = 204;
    profile.validate().unwrap();
    profile
}

/// Every destination Color channel is written exactly once, the request is the stored intent,
/// and the achieved output is the forward evaluation of the written values.
fn assert_complete(
    profile: &FixtureProfile,
    target: FixtureId,
    stored: &ColorIntent,
    (descriptor, native, result): &(ColorDescriptor, Vec<u32>, PhysicalResolution<ColorAdapter>),
) -> Vec<u32> {
    let name = &profile.name;
    validate_complete_writes(&descriptor.footprint, &result.writes).unwrap();
    assert_eq!(
        &result.requested, stored,
        "{name}: request is the stored intent"
    );
    let mut written: Vec<_> = result.writes.iter().map(|w| w.slot.channel_index).collect();
    written.sort_unstable();
    assert_eq!(
        written,
        color_channels(profile),
        "{name}: every Color channel"
    );
    assert!(result.writes.iter().all(|w| w.slot.destination == target));
    let mut output = native.clone();
    for write in &result.writes {
        output[write.slot.channel_index as usize] = write.raw;
    }
    let head = descriptor.primary();
    let mut forward = head.fitting.forward().create_output();
    head.fitting
        .forward()
        .evaluate(&output, &mut forward)
        .unwrap();
    assert_eq!(
        result.achieved.known_xyz, forward[head.head].known_xyz,
        "{name}"
    );
    output
}

#[test]
fn recorded_semantic_cues_survive_save_reload_fixture_replacement_and_new_live_group_members() {
    let show = Show::new();
    let (a, b) = (FixtureId::new(), FixtureId::new());
    show.patch(&fixture(&rgb(), a, 1, 1));
    show.group(&[a]);
    let color = ProgrammingOwner::Color.key();
    show.programmers
        .set(show.session, a, color.clone(), program(&magenta()));
    show.record(1.0);
    show.programmers
        .set_group(show.session, GROUP.into(), color, program(&warm_white()));
    show.record(2.0);
    let (list_id, recorded) = show.cue_list();
    assert_eq!(recorded["cues"].as_array().unwrap().len(), 2);
    assert_eq!(
        recorded.pointer("/cues/1/group_changes/0/group_id"),
        Some(&Value::String(GROUP.into())),
        "warm white is stored against the live Group, not its members"
    );
    let stored = [(1.0, magenta()), (2.0, warm_white())];

    let output = Output::new();
    let replacements = [rgb(), rgbw_white_on(), cmy_wheel(), hybrid()];
    for (step, profile) in replacements.iter().enumerate() {
        show.patch(&fixture(profile, a, 1, 1));
        if step == 1 {
            // A member added to the live Group after recording.
            show.patch(&fixture(profile, b, 2, 40));
            show.group(&[a, b]);
        } else if step > 1 {
            show.patch(&fixture(profile, b, 2, 40));
        }
        let (id, body) = show.cue_list();
        assert_eq!(id, list_id);
        assert_eq!(
            body, recorded,
            "{}: the stored Cue list is unchanged",
            profile.name
        );
        let snapshot = show.compile();
        assert_eq!(snapshot.cue_lists.len(), 1);
        assert_eq!(snapshot.cue_lists[0].cues.len(), 2, "no Cue reprogramming");
        for (cue, intent) in &stored {
            output.go(show.compile(), *cue);
            let members: &[FixtureId] = if step == 0 || *cue == 1.0 {
                &[a]
            } else {
                &[a, b]
            };
            for &target in members {
                assert_eq!(
                    &output.played(target),
                    intent,
                    "{}: played intent",
                    profile.name
                );
                let resolved = output.resolve(target);
                let written = assert_complete(profile, target, intent, &resolved);
                if step == 1 {
                    assert_eq!(
                        resolved.1[4], 204,
                        "White starts nonzero on the new destination"
                    );
                    assert_ne!(written[4], 204, "White is fitted, never left at the seed");
                }
            }
        }
    }
    let counters = output.adapter.counters();
    assert!(
        counters.fitting_compiles >= 4,
        "each replacement recompiles the fitter"
    );
}

/// TL-557 AC5 through the actual Record path: a semantic Cue on a Media layer reaches the
/// personality's tint/Grayscale controls after reload; re-recording the Cue (an Update of its
/// content) changes the stored intent and the controls, never the Cue count. White Blend 100 %
/// keeps the tint; the layer dimmer (Intensity) is never a Color write.
#[test]
fn media_layer_cues_carry_semantic_intent_through_record_rerecord_and_reload() {
    use super::super::media_color::tests::{media_fixture, shipped_media_server};
    let show = Show::new();
    let (root, layer, other) = (FixtureId::new(), FixtureId::new(), FixtureId::new());
    show.patch(&media_fixture(
        &shipped_media_server(),
        root,
        &[layer, other],
    ));
    let color = ProgrammingOwner::Color.key();
    let mut red = super::tests::intent([1., 0., 0.], 0.);
    red.white_blend = 1.;
    let mut amber = super::tests::intent([1., 0.735, 0.], 0.);
    amber.white_blend = 0.25;
    let output = Output::new();
    let media = MediaColorAdapter::default();
    for (step, (intent, expected)) in [(&red, [0, 255, 255, 255]), (&amber, [0, 128, 255, 64])]
        .into_iter()
        .enumerate()
    {
        show.programmers
            .set(show.session, layer, color.clone(), program(intent));
        show.record_as(1.0, &format!("media-{step}"));
        let (_, body) = show.cue_list();
        assert_eq!(body["cues"].as_array().unwrap().len(), 1, "no new Cue");
        output.go(show.compile(), 1.0);
        assert_eq!(&output.played(layer), intent, "the stored intent is played");
        let (descriptor, _, result) = output.resolve_with(&media, layer);
        assert_eq!(&result.requested, intent);
        assert_eq!(
            result.writes.iter().map(|w| w.raw).collect::<Vec<_>>(),
            expected,
            "C, M, Y, Grayscale"
        );
        assert_eq!(
            descriptor.footprint.len(),
            4,
            "the layer dimmer is not Color"
        );
    }
}

/// UV-only black and a pinned wheel constraint persist through the actual Record path and a
/// reload. On a replacement that cannot honour them (no UV emitter, another wheel) the stored
/// intent is unchanged and the result is passive status, never a block or an approximation.
#[test]
fn uv_only_black_and_wheel_constraints_persist_and_degrade_passively_on_replacement() {
    use light_core::NativeColorValue;
    use light_core::programming::ColorWheelConstraint;
    use light_fixture::forward::{ColorConstraintStatus, UvFitStatus};
    let show = Show::new();
    let (uv, wheel) = (FixtureId::new(), FixtureId::new());
    let wheel_profile = wheel_only();
    show.patch(&fixture(&rgbwauv(None), uv, 1, 1));
    show.patch(&fixture(&wheel_profile, wheel, 2, 40));
    let mode = &wheel_profile.modes[0];
    let channel = &mode.channels[1];
    let mut pinned = super::tests::intent([0., 0., 1.], 0.);
    pinned.wheel_constraints = vec![ColorWheelConstraint {
        source: wheel_profile
            .native_color_identity(mode.id, mode.heads[0].id)
            .unwrap(),
        value: NativeColorValue {
            channel_id: channel.id,
            function_id: channel.functions[0].id,
            raw: 20,
        },
    }];
    let uv_black = super::tests::uv_only_black();
    let color = ProgrammingOwner::Color.key();
    show.programmers
        .set(show.session, uv, color.clone(), program(&uv_black));
    show.programmers
        .set(show.session, wheel, color, program(&pinned));
    show.record(1.0);
    let (_, recorded) = show.cue_list();

    let output = Output::new();
    output.go(show.compile(), 1.0);
    let (_, _, result) = output.resolve(uv);
    assert_eq!(result.requested, uv_black);
    assert_eq!(
        result.writes.iter().map(|w| w.raw).collect::<Vec<_>>(),
        [
            0,
            0,
            0,
            0,
            0,
            (f64::from(uv_black.uv.amount) * 255.).round() as u32
        ],
        "UV-only black: every visible emitter parked, UV driven"
    );
    assert_eq!(result.quality.uv, UvFitStatus::Applied);
    let (_, _, result) = output.resolve(wheel);
    assert_eq!(
        result.requested, pinned,
        "the constraint is stored and played"
    );
    assert_eq!(result.writes[0].raw, 20);
    assert_eq!(
        result.quality.constraints[0].status,
        ColorConstraintStatus::Applied
    );

    // Replacements that cannot honour UV or the pinned wheel.
    show.patch(&fixture(&rgb(), uv, 1, 1));
    show.patch(&fixture(&cmy_wheel(), wheel, 2, 40));
    assert_eq!(show.cue_list().1, recorded, "the stored Cue is unchanged");
    output.go(show.compile(), 1.0);
    let (_, _, result) = output.resolve(uv);
    assert_eq!(
        result.requested, uv_black,
        "UV intent retained, not approximated"
    );
    assert_eq!(
        result.writes.iter().map(|w| w.raw).collect::<Vec<_>>(),
        [0, 0, 0],
        "no violet substitute"
    );
    assert_eq!(result.quality.uv, UvFitStatus::Unsupported);
    let (_, _, result) = output.resolve(wheel);
    assert_eq!(result.requested, pinned);
    assert_eq!(
        result.writes.len(),
        4,
        "every CMY and wheel control is decided"
    );
    assert_eq!(
        result.quality.constraints[0].status,
        ColorConstraintStatus::SourceMismatch
    );

    // The retained request becomes usable again as soon as a UV-capable lamp is patched back.
    show.patch(&fixture(&rgbwauv(None), uv, 1, 1));
    assert_eq!(show.cue_list().1, recorded);
    output.go(show.compile(), 1.0);
    let (_, _, result) = output.resolve(uv);
    assert_eq!(result.quality.uv, UvFitStatus::Applied);
    assert_eq!(
        result.writes.last().unwrap().raw,
        (f64::from(uv_black.uv.amount) * 255.).round() as u32
    );
}

/// TL-559 AC6 through the actual Record path: tagged Direct values recorded per fixture and
/// against a live Group survive SQLite save/reload byte-for-byte, replay exactly on the
/// original fixture, fall back on a replacement type and on a new Group member, and a re-record
/// (Update of the Cue content) replaces the value without adding Cues. A semantic Cue in the
/// same list keeps its ordinary portability.
#[test]
fn recorded_direct_cues_keep_tagged_identity_through_reload_replacement_group_and_rerecord() {
    use super::tests_direct::{direct, identity};
    let show = Show::new();
    let (a, b) = (FixtureId::new(), FixtureId::new());
    let source = rgbw();
    show.patch(&fixture(&source, a, 1, 1));
    show.group(&[a]);
    let catalogue = Arc::clone(&show.compile().native_color_sources);
    let orange = direct(&catalogue, &source, &[65535, 90, 0, 0]);
    let black = direct(&catalogue, &source, &[0, 0, 0, 0]);
    let color = ProgrammingOwner::Color.key();
    show.programmers
        .set(show.session, a, color.clone(), orange.clone());
    show.record(1.0);
    show.programmers
        .set_group(show.session, GROUP.into(), color.clone(), black.clone());
    show.record(2.0);
    show.programmers
        .set(show.session, a, color.clone(), program(&magenta()));
    show.record(3.0);
    let (list_id, recorded) = show.cue_list();
    let stored = |cue: &str, path: &str| {
        recorded
            .pointer(&format!("/cues/{cue}/{path}/0/value/value"))
            .cloned()
            .unwrap()
    };
    assert_eq!(
        stored("0", "changes")["kind"],
        "direct",
        "tagged per-fixture value"
    );
    assert_eq!(
        stored("1", "group_changes")["kind"],
        "direct",
        "tagged Group value"
    );
    assert_eq!(stored("2", "changes")["kind"], "semantic");
    assert_eq!(
        serde_json::from_value::<ColorProgram>(stored("0", "changes")).unwrap(),
        **super::tests_direct::direct_program(&orange),
        "exact recipe, pinned identity and estimate are stored"
    );

    let output = Output::new();
    output.go(show.compile(), 1.0);
    assert_eq!(
        output.played_value(a),
        orange,
        "exact tagged value after reload"
    );
    let (_, result) = output.resolve_played(a);
    assert_eq!(
        result.quality.direct.as_ref().unwrap().replay,
        DirectReplayOutcome::Exact
    );
    assert_eq!(
        result.writes.iter().map(|w| w.raw).collect::<Vec<_>>(),
        [65535, 90, 0, 0]
    );

    // Replacement type and a new live-Group member: stored Cues unchanged, fallback fitting.
    let replacement = rgbal();
    show.patch(&fixture(&replacement, a, 1, 1));
    show.patch(&fixture(&replacement, b, 2, 40));
    show.group(&[a, b]);
    assert_eq!(show.cue_list(), (list_id.clone(), recorded.clone()));
    output.go(show.compile(), 1.0);
    assert_eq!(output.played_value(a), orange);
    let (_, result) = output.resolve_played(a);
    let status = result.quality.direct.as_ref().unwrap();
    assert!(matches!(
        status.replay,
        DirectReplayOutcome::Fallback { .. }
    ));
    assert_eq!(
        status.origin,
        DirectEstimateOrigin::Forward,
        "original retained"
    );
    output.go(show.compile(), 2.0);
    for member in [a, b] {
        assert_eq!(
            output.played_value(member),
            black,
            "Group value on every member"
        );
        let (_, result) = output.resolve_played(member);
        assert!(
            result.writes.iter().all(|w| w.raw == 0),
            "known black, never white"
        );
    }
    output.go(show.compile(), 3.0);
    assert_eq!(
        output.played(a),
        magenta(),
        "semantic portability is separate"
    );

    // Re-record Cue 1 with another Direct value: same Cue count, new tagged value.
    show.patch(&fixture(&source, a, 1, 1));
    let dim = direct(
        &show.compile().native_color_sources,
        &source,
        &[1000, 0, 7, 3],
    );
    show.programmers.set(show.session, a, color, dim.clone());
    show.record_as(1.0, "direct-rerecord");
    let (_, rerecorded) = show.cue_list();
    assert_eq!(rerecorded["cues"].as_array().unwrap().len(), 3);
    output.go(show.compile(), 1.0);
    assert_eq!(output.played_value(a), dim);
    let AttributeValue::ColorProgram(program) = &dim else {
        unreachable!()
    };
    let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
        unreachable!()
    };
    assert_eq!(recipe.source, identity(&source), "pinned identity survives");
}
