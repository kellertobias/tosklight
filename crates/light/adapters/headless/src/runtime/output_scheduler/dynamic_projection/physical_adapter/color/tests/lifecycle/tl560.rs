//! TL-560 persistence-matrix cells that were MISSING in
//! `docs/testing/33-semantic-intent-persistence-coverage.md`.
//!
//! Every test records through the real application writers of the parent harness
//! (`ProgrammingService::handle_cue_recording` → `ActiveShowService::commit_programming_cue`,
//! `ActiveShowService::commit_programming_preset`, …), reopens the SQLite show, compiles it
//! through `prepare_show_candidate` and inspects the exact stored payloads. Where output applies,
//! the reopened snapshot is installed into a fresh `AppState` built by
//! `test_state_with_family_adapters` (contract 1 plus the explicit all-family Live opt-in) and
//! rendered through `OutputResource::render_with_playback_events`, the production Live boundary
//! (`dynamic_output_frame` → family lanes → `render_static_family_frame`). The test reads the
//! published family writes and the actual encoded universe bytes of that same frame.
//!
//! Production `SUPPORTED_PROGRAMMING_CONTRACT` stays 0; these desks opt in explicitly.
use super::*;
use crate::runtime::AppState;
use crate::runtime::tests::test_state_with_family_adapters;
use light_core::programming::PROGRAMMING_CONTRACT_VERSION;
use light_engine::RenderResult;

/// Focus Cue record, reopen and replacement independent of Zoom.
mod focus;
/// Media colour through Preset, Update and live Group.
mod media_color;
/// Media colour Cue across replacement by another personality.
mod media_replacement;
/// Position Angles and Point Target through replacement by shipped profiles.
mod position_replacement;
/// Preload GO commit for Target, Direct, UV and Zoom.
mod preload_go;
/// Preload GO commit for Focus and Media colour.
mod preload_go_focus_media;
/// Preset recall of semantic UV and of Focus.
mod preset_recall;
/// Magenta/warm-white Presets and visible colour plus UV across replacement.
mod replacement_presets;
/// Regression: a static typed Zoom reaches the Live family frame (defect found by TL-560).
mod static_zoom;
/// Unpatched fixtures keep semantic programming; only DMX is suppressed.
mod unpatched;
/// Unpatched Target, Direct, UV, Focus, Zoom and Media colour.
mod unpatched_families;

/// One fresh desk process: an `AppState` at contract 1 with the family Live path engaged.
pub(super) struct Desk {
    pub state: AppState,
    pub clock: Arc<ManualClock>,
    /// The desk's own Programmer registry (`state.programming` and the engine share it).
    pub programmers: ProgrammerRegistry,
    data_dir: PathBuf,
}

impl Drop for Desk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

/// One rendered Live frame and the family writes it published.
pub(super) struct DeskFrame {
    pub rendered: RenderResult,
    pub writes: Vec<(ProgrammingOwner, FixtureId, NativeControlWrite)>,
}

impl DeskFrame {
    /// The written raw values of `owner` on `target`, by channel index.
    pub fn family(&self, owner: ProgrammingOwner, target: FixtureId) -> Vec<(u32, u32)> {
        let mut writes: Vec<_> = self
            .writes
            .iter()
            .filter(|(o, t, _)| *o == owner && *t == target)
            .map(|(_, _, write)| (write.slot.channel_index, write.raw))
            .collect();
        writes.sort_unstable();
        writes
    }

    /// The final native output of one physical instance in this frame.
    pub fn native(&self, instance: FixtureId) -> &[u32] {
        &self
            .rendered
            .physical
            .instances
            .iter()
            .find(|output| output.instance_id == instance.0)
            .unwrap_or_else(|| panic!("physical instance {instance:?}"))
            .native_raw
    }

    /// The universe bytes of this frame, if anything was output on `universe`.
    pub fn universe(&self, universe: u16) -> Option<&[u8]> {
        self.rendered
            .universes
            .get(&universe)
            .map(|frame| frame.as_ref())
    }

    /// The fixture's native output encoded at its own patch equals the rendered bytes, and every
    /// published family write of the fixture is in that native output. Returns the native output.
    pub fn assert_encoded(&self, fixture: &PatchedFixture) -> Vec<u32> {
        let native = self.native(fixture.fixture_id).to_vec();
        for (owner, _, write) in self
            .writes
            .iter()
            .filter(|(_, _, w)| w.slot.destination == fixture.fixture_id)
        {
            assert_eq!(
                native[write.slot.channel_index as usize], write.raw,
                "{owner:?}: the fitted write is the rendered native output"
            );
        }
        let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
        let mode = profile
            .mode(fixture.definition.mode_id.unwrap())
            .expect("patched mode");
        let address = fixture.address.expect("patched address");
        let mut expected = [0u8; 512];
        let values: Vec<_> = (0u32..).zip(native.iter().copied()).collect();
        mode.compile_encoding_plan()
            .unwrap()
            .encode_split_by_index(&mut expected, address, 1, &values)
            .unwrap();
        let bytes = self
            .universe(fixture.universe.expect("patched universe"))
            .expect("the patched universe is output");
        let footprint = usize::from(address) - 1
            ..usize::from(address) - 1 + usize::from(mode.splits[0].footprint);
        assert_eq!(
            &bytes[footprint.clone()],
            &expected[footprint],
            "{}: rendered bytes are the encoded native output",
            fixture.name
        );
        native
    }
}

impl Desk {
    /// Open a fresh desk on one reopened, compiled show snapshot.
    pub fn open(snapshot: EngineSnapshot) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let (state, data_dir) = test_state_with_family_adapters(
            programmers.clone(),
            Some(clock.clone()),
            PROGRAMMING_CONTRACT_VERSION,
        );
        state.output.replace_snapshot(snapshot).unwrap();
        Self {
            state,
            clock,
            programmers,
            data_dir,
        }
    }

    pub fn engine(&self) -> &Engine {
        self.state.output.engine()
    }

    /// Install a re-compiled patch generation in place (a patch edit, not a show activation).
    pub fn replace(&self, snapshot: EngineSnapshot) {
        self.state.output.replace_snapshot(snapshot).unwrap();
    }

    /// GoTo `cue` on the harness Playback and settle well past any default fade.
    pub fn go(&self, cue: f64) {
        self.engine()
            .execute_playback(EnginePlaybackCommand::Pool {
                number: PLAYBACK,
                action: PoolPlaybackAction::GoTo(CueNumber::try_from_legacy_f64(cue).unwrap()),
            })
            .unwrap();
        self.clock.advance_millis(600_000);
    }

    /// Render one frame through the production Live boundary.
    pub fn frame(&self) -> DeskFrame {
        self.clock.advance_millis(25);
        let rendered = self
            .state
            .output
            .render_with_playback_events(
                &self.state.active_show.output_projection(),
                &self.state.playback.render_capability(),
                self.state.output.render_options(),
            )
            .unwrap()
            .rendered;
        let writes = self
            .state
            .output
            .live_family_adapters()
            .take_published()
            .map(|published| {
                assert_eq!(
                    published.token.sampled_at(),
                    rendered.sampled_at,
                    "one frame identity"
                );
                published.writes
            })
            .unwrap_or_default();
        DeskFrame { rendered, writes }
    }

    /// The composed value of `owner` on `target` in the engine (what the desk plays).
    pub fn played(&self, target: FixtureId, owner: ProgrammingOwner) -> Option<AttributeValue> {
        self.engine()
            .resolved_values()
            .get(&(target, owner.key()))
            .cloned()
    }
}

/// The Programmer's current values recorded as Cue `number` on `show`, keeping the Programmer.
pub(super) fn cue_changes(body: &Value, cue: usize, path: &str) -> Vec<Value> {
    body.pointer(&format!("/cues/{cue}/{path}"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The stored typed value of `attribute` for `fixture` in Cue `cue`.
pub(super) fn stored_fixture_value(
    body: &Value,
    cue: usize,
    fixture: FixtureId,
    attribute: &str,
) -> AttributeValue {
    let change = cue_changes(body, cue, "changes")
        .into_iter()
        .find(|change| {
            change["fixture_id"] == Value::String(fixture.0.to_string())
                && change["attribute"] == attribute
        })
        .unwrap_or_else(|| panic!("Cue {cue} stores {attribute} for {fixture:?}: {body}"));
    serde_json::from_value(change["value"].clone()).unwrap()
}

/// The stored typed value of `attribute` for the live Group in Cue `cue`.
pub(super) fn stored_group_value(body: &Value, cue: usize, attribute: &str) -> AttributeValue {
    let change = cue_changes(body, cue, "group_changes")
        .into_iter()
        .find(|change| change["group_id"] == GROUP && change["attribute"] == attribute)
        .unwrap_or_else(|| panic!("Cue {cue} stores Group {attribute}: {body}"));
    serde_json::from_value(change["value"].clone()).unwrap()
}

/// Record Cue `number` from `session` of an arbitrary Programmer through the real Cue writer of
/// `show` with an explicit capture policy. Returns the captured source.
pub(super) fn record_from(
    show: &Show,
    programming: &ProgrammingService,
    session: SessionId,
    number: f64,
    capture_policy: ProgrammingCueCapturePolicy,
) -> light_programmer::CueRecordingCapturedSource {
    let context = ActionContext::operator(Uuid::from_u128(1), session.0, ActionSource::Http)
        .with_request_id(format!("tl560-{number}-{}", Uuid::new_v4()));
    let result = programming
        .handle_cue_recording(
            ActionEnvelope {
                context,
                command: ProgrammingCueRecordRequest {
                    show_id: show.ports.show_id,
                    target: ProgrammingCueRecordTarget::Pool {
                        playback_number: PLAYBACK,
                    },
                    operation: ProgrammingCueRecordOperation::Overwrite,
                    cue_number: Some(CueNumber::try_from_legacy_f64(number).unwrap()),
                    timing: ProgrammingCueRecordTiming::default(),
                    cue_only: false,
                    name: None,
                    capture_policy,
                    activation_policy: ProgrammingCueActivationPolicy::Hold,
                    expected_show_revision: ProgrammingCueShowRevisionExpectation::Current,
                },
            },
            &show.ports,
        )
        .unwrap();
    assert!(
        matches!(
            result.outcome,
            light_application::ProgrammingCueRecordOutcome::Changed { .. }
        ),
        "{result:?}"
    );
    result.captured_source
}

/// The real Preset writer: `ActiveShowService::commit_programming_preset` over the SQLite show.
impl light_application::ProgrammingPresetRecordingPorts for Ports {
    fn authorize_preset_recording(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn commit_preset(
        &self,
        context: &ActionContext,
        commit: &light_application::ProgrammingPresetCommit,
    ) -> Result<light_application::ProgrammingPresetCommitResult, ActionError> {
        self.service
            .commit_programming_preset(context, commit, self)
    }
}

impl light_application::ProgrammingPresetActiveShowPorts for Ports {}

/// The real Update writer: `ActiveShowService::commit_programming_update` over the SQLite show.
impl light_application::programming_update::ProgrammingUpdatePorts for Ports {
    fn authorize_programming_update(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn active_update_cue_contexts(
        &self,
        _context: &ActionContext,
    ) -> Result<Vec<light_application::programming_update::ActiveCueContext>, ActionError> {
        Ok(Vec::new())
    }
}

fn operator(session: SessionId, request: &str) -> ActionContext {
    ActionContext::operator(Uuid::from_u128(1), session.0, ActionSource::Http)
        .with_request_id(format!("tl560-{request}-{}", Uuid::new_v4()))
}

/// Record the Programmer into Preset `family` `number` (Overwrite) through the real Preset writer.
/// Returns the stored object id.
pub(super) fn record_preset(
    show: &Show,
    programming: &ProgrammingService,
    session: SessionId,
    family: light_programmer::PresetFamily,
    number: u32,
) -> String {
    let result = programming
        .handle_preset_recording(
            ActionEnvelope {
                context: operator(session, "preset"),
                command: light_application::ProgrammingPresetRecordRequest {
                    show_id: show.ports.show_id,
                    address: light_programmer::PresetAddress::new(family, number).unwrap(),
                    name: format!("TL-560 {family:?} {number}"),
                    mode: light_programmer::PresetStoreMode::Overwrite,
                    expected_object_revision:
                        light_application::ProgrammingPresetRevisionExpectation::Current,
                    expected_show_revision: None,
                },
            },
            &show.ports,
        )
        .unwrap();
    let light_application::ProgrammingPresetRecordOutcome::Changed { projection, .. } =
        result.outcome
    else {
        panic!("a changed Preset recording: {:?}", result.outcome)
    };
    projection.object_id.clone()
}

/// Preview, then apply exactly that preview (the desk's Update dialog), Update Existing on
/// Preset `object_id` through the real Update writer.
pub(super) fn update_preset(
    show: &Show,
    programming: &ProgrammingService,
    session: SessionId,
    object_id: &str,
) {
    use light_application::programming_update::{
        ExistingContentMode, ProgrammingUpdateCommand, ProgrammingUpdatePreviewRequest,
        ProgrammingUpdateTargetRequest, UpdateMode,
    };
    let target = ProgrammingUpdateTargetRequest::Preset {
        object_id: object_id.to_owned(),
    };
    let preview = programming
        .preview_update(
            ActionEnvelope {
                context: operator(session, "update-preview"),
                command: ProgrammingUpdatePreviewRequest {
                    show_id: show.ports.show_id,
                    target: target.clone(),
                    mode: UpdateMode::ExistingContent(ExistingContentMode::UpdateExisting),
                },
            },
            &show.ports.service,
            &show.ports,
        )
        .unwrap();
    programming
        .handle_update(
            ActionEnvelope {
                context: operator(session, "update-apply"),
                command: ProgrammingUpdateCommand {
                    show_id: show.ports.show_id,
                    target,
                    mode: preview.preview.mode,
                    expected_object_revision: Some(preview.object_revision),
                    expected_programmer_revision: Some(preview.programmer_revision),
                    expected_show_revision: Some(preview.show_revision),
                },
            },
            &show.ports.service,
            &show.ports,
        )
        .unwrap();
}
