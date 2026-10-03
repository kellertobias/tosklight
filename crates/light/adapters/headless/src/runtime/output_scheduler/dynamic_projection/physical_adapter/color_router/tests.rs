//! TL-612 references. Every case resolves against real captured engine frames:
//! - delegation equivalence: the router and the original lamp/Media adapter resolve the same
//!   captured frame and must publish identical writes, request, achieved, quality and continuity;
//! - classification: reserved Media targets never reach lamp compilation or fitting;
//! - lanes: ONE outer Live `PhysicalAdapterLane` through the real hybrid composer and engine
//!   finalizer, and ONE ordinary `PhysicalPreloadLanes` pair through the real retained paired
//!   evaluator and Preload finalizer.
use super::super::color::profiles::{cmy_wheel, patched, rgb};
use super::super::color::tests::direct::{catalogue, direct};
use super::super::color::tests::{intent, magenta, program};
use super::super::media_color::tests::{media_fixture, shipped_media_server};
use super::*;
use crate::runtime::dynamic_snapshot_publication::{
    DynamicSnapshotPublication, RetainedFrameCapture, RetainedInputCapture,
};
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use crate::runtime::output_scheduler::dynamic_projection::CapturedDynamicInputs;
use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::{
    HybridFrameScratch, prepare_captured_hybrid_frame,
};
use crate::runtime::output_scheduler::dynamic_projection::retained_preload_hybrid::{
    PendingHybridBranch, PendingHybridResult, RetainedPreloadHybridEvaluator,
};
use crate::runtime::preload::retained_history::paired::{
    PairedPendingHistory, PendingPairEvaluator, PendingPairWindowOutcome,
};
use crate::runtime::preload::retained_history::{
    PendingEpisodeKey, PendingHistoryLimits, PendingHistoryPosition, PendingHistorySeed,
};
use light_core::programming::{PROGRAMMING_CONTRACT_VERSION, TransitionRequirement};
use light_core::{AttributeKey, ManualClock, SessionId};
use light_dynamics::{
    DynamicOutputFrameScratch, DynamicRuntime, DynamicRuntimeError, DynamicSemanticValue,
    DynamicSpeedTransport, ProgrammingFamilyFixAt,
};
use light_engine::{Engine, PreloadBranch, PreparedOutputFrame};
use light_fixture::{FixtureProfile, MultiPatchInstance, PatchedFixture};
use light_programmer::{DynamicProgrammerValueMutation, ProgrammerRegistry};
use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use uuid::Uuid;

type Sidecar = PhysicalHeadResult<RoutingColorAdapter>;
type Pair = PairedPendingHistory<PendingHybridResult<Sidecar>>;

const LAMP_ADDRESS: u16 = 300;
const MIXED_ADDRESS: u16 = 400;

fn tinted(white_blend: f32) -> ColorIntent {
    ColorIntent {
        white_blend,
        ..intent([1., 0.735, 0.], 0.)
    }
}

/// The shipped Media personality with layer Cyan inverted: Media identities stay (Color is
/// reserved for Media), but the wire contract is unsupported, so both adapters decline it.
fn unsupported_media_server() -> FixtureProfile {
    let mut profile = shipped_media_server();
    let mut inverted = 0;
    for channel in &mut profile.modes[0].channels {
        if &*channel.fixture_attribute.0 == "media.layer.cyan" {
            channel.invert = true;
            inverted += 1;
        }
    }
    assert!(inverted > 0);
    profile
}

/// An RGB lamp head (master-shared) plus a second head carrying a Media identity. Patched without
/// a logical head, the root target owns both: a mixed lamp+Media target. Patch validation rejects
/// that topology, so the case is compiled against an unvalidated captured snapshot (defensive).
fn mixed_lamp_and_media() -> FixtureProfile {
    let mut profile = rgb();
    let mode = &mut profile.modes[0];
    let media_head = Uuid::new_v4();
    mode.heads.push(light_fixture::FixtureHead {
        id: media_head,
        name: "Media layer".into(),
        master_shared: false,
    });
    let mut channel = mode.channels[0].clone();
    channel.id = Uuid::new_v4();
    channel.head_id = media_head;
    channel.fixture_attribute = AttributeKey("media.layer.cyan".into());
    for function in &mut channel.functions {
        function.id = Uuid::new_v4();
    }
    if !channel.secondary_slots.is_empty() {
        channel.secondary_slots = vec![mode.splits[0].footprint + 2];
    }
    mode.splits[0].footprint += channel.resolution.bytes() as u16;
    mode.channels.push(channel);
    profile
}

/// Mode channel index of layer 1's dimmer (Intensity).
fn layer_dimmer(profile: &FixtureProfile) -> usize {
    let mode = &profile.modes[0];
    let layer = mode.heads.iter().find(|h| !h.master_shared).unwrap().id;
    mode.channels
        .iter()
        .position(|c| c.head_id == layer && &*c.fixture_attribute.0 == "media.layer.dimmer")
        .unwrap()
}

fn with_copy(mut fixture: PatchedFixture, address: u16) -> PatchedFixture {
    fixture.multipatch = vec![MultiPatchInstance {
        id: Uuid::new_v4(),
        universe: Some(2),
        address: Some(address),
        ..Default::default()
    }];
    fixture
}

fn transports() -> [DynamicSpeedTransport; 5] {
    [DynamicSpeedTransport {
        effective_bpm: 120.,
        phase_origin_millis: 0,
        phase_reference_millis: 0,
        beat_phase: 0.,
        phase_advancing: true,
    }; 5]
}

struct Rig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    clock: Arc<ManualClock>,
}

impl Rig {
    fn new(fixtures: Vec<PatchedFixture>) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        let rig = Self {
            engine: Engine::with_programming_contract_support(
                programmers.clone(),
                PROGRAMMING_CONTRACT_VERSION,
            ),
            programmers,
            session,
            clock,
        };
        rig.install(fixtures);
        rig
    }

    /// Install (or replace) the patch; every call is a new runtime generation.
    fn install(&self, mut fixtures: Vec<PatchedFixture>) {
        for (number, fixture) in (1..).zip(&mut fixtures) {
            fixture.fixture_number = Some(number);
        }
        self.engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: fixtures.into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
    }

    /// A whole Color FixAT over an explicit static underlay: the composed owner reaches the
    /// physical observer every frame.
    fn fix(&self, target: FixtureId, request: &ColorIntent) {
        let value = program(request);
        self.programmers.set(
            self.session,
            target,
            ProgrammingOwner::Color.key(),
            value.clone(),
        );
        assert!(
            self.programmers.apply_dynamic_values(
                self.session,
                &[DynamicProgrammerValueMutation::Set {
                    fixture_id: target,
                    attribute: ProgrammingOwner::Color.key(),
                    value: DynamicSemanticValue::ProgrammingFixAt {
                        mask: ProgrammingFamilyFixAt::from_family(
                            ProgrammingOwner::Color,
                            None,
                            value
                        )
                        .unwrap(),
                        timing: Default::default(),
                    },
                }],
                None,
            )
        );
    }

    fn release(&self, target: FixtureId) {
        assert!(self.programmers.apply_dynamic_values(
            self.session,
            &[DynamicProgrammerValueMutation::Set {
                fixture_id: target,
                attribute: ProgrammingOwner::Color.key(),
                value: DynamicSemanticValue::Release,
            }],
            None,
        ));
    }

    fn capture(&self) -> PreparedOutputFrame {
        self.clock.advance_millis(25);
        self.engine.prepare_output_frame(Default::default())
    }

    /// The same captured frame context an adapter receives inside the hybrid seam.
    fn with_frame<R>(
        &self,
        capture: &PreparedOutputFrame,
        f: impl FnOnce(HybridFrameContext<'_>) -> R,
    ) -> R {
        let token = capture.frame_token();
        let mut scalar = self.engine.prepare_static_family_frame(capture, &[]);
        let geometry = self
            .engine
            .observe_static_family_geometry(capture, &mut scalar)
            .unwrap();
        let models = DynamicRuntime::default().captured_native_color_models();
        f(HybridFrameContext {
            capture,
            geometry: &geometry,
            native_models: models.as_ref(),
            token: &token,
            scalar: &scalar,
        })
    }
}

fn resolve_with<A: PhysicalFamilyAdapter>(
    adapter: &A,
    frame: HybridFrameContext<'_>,
    descriptor: &A::Descriptor,
    target: FixtureId,
    value: &AttributeValue,
    previous: Option<&A::Continuity>,
) -> Result<PhysicalResolution<A>, TransitionError> {
    let resolved = adapter.resolve(PhysicalRequest {
        frame,
        target,
        owner: ProgrammingOwner::Color,
        descriptor,
        value,
        previous,
    })?;
    validate_complete_writes(adapter.footprint(descriptor), &resolved.writes)?;
    Ok(resolved)
}

/// The router's resolution is exactly the lamp adapter's, tagged Lamp.
fn assert_lamp_identical(
    routed: &PhysicalResolution<RoutingColorAdapter>,
    original: &PhysicalResolution<ColorAdapter>,
    case: &str,
) {
    assert_eq!(routed.writes, original.writes, "{case}: native writes");
    assert_eq!(
        routed.requested,
        RoutedColorRequest::Lamp(original.requested.clone()),
        "{case}"
    );
    assert_eq!(
        routed.achieved,
        RoutedAchievedColor::Lamp(original.achieved),
        "{case}"
    );
    assert_eq!(
        routed.quality,
        RoutedColorQuality::Lamp(original.quality.clone()),
        "{case}"
    );
    assert_eq!(
        routed.continuity,
        RoutedColorContinuity::Lamp(original.continuity.clone()),
        "{case}"
    );
}

/// The router's resolution is exactly the Media adapter's, tagged Media.
fn assert_media_identical(
    routed: &PhysicalResolution<RoutingColorAdapter>,
    original: &PhysicalResolution<MediaColorAdapter>,
    case: &str,
) {
    assert_eq!(routed.writes, original.writes, "{case}: native writes");
    assert_eq!(
        routed.requested,
        RoutedColorRequest::Media(original.requested.clone()),
        "{case}"
    );
    assert_eq!(
        routed.achieved,
        RoutedAchievedColor::Media(original.achieved),
        "{case}"
    );
    assert_eq!(
        routed.quality,
        RoutedColorQuality::Media(original.quality),
        "{case}"
    );
    assert_eq!(
        routed.continuity,
        RoutedColorContinuity::Media(()),
        "{case}"
    );
}

fn raws(writes: &[NativeControlWrite]) -> Vec<u32> {
    writes.iter().map(|w| w.raw).collect()
}

#[test]
fn lamp_rgb_and_cmy_roots_and_copies_delegate_unchanged() {
    for (name, profile) in [("RGB", rgb()), ("CMY wheel", cmy_wheel())] {
        for copies in [false, true] {
            let case = format!("{name} copies={copies}");
            let target = FixtureId::new();
            let mut fixture = patched(&profile, target, 1);
            if copies {
                fixture = with_copy(fixture, 100);
            }
            let rig = Rig::new(vec![fixture]);
            let router = RoutingColorAdapter::default();
            let lamp = ColorAdapter::default();
            let snapshot = rig.engine.snapshot();
            let routed = router.compile(&snapshot, target).unwrap().unwrap();
            let original = lamp.compile(&snapshot, target).unwrap().unwrap();
            assert_eq!(routed.route(), ColorRoute::Lamp, "{case}");
            assert_eq!(
                router.footprint(&routed),
                lamp.footprint(&original),
                "{case}"
            );
            assert_eq!(
                routed.lamp().unwrap().heads.len(),
                if copies { 2 } else { 1 },
                "{case}: copies stay the lamp adapter's destinations"
            );
            let mut previous: Option<(RoutedColorContinuity, ColorContinuity)> = None;
            for request in [magenta(), intent([1., 0., 0.], 0.), tinted(0.5)] {
                let value = program(&request);
                let capture = rig.capture();
                let (r, o) = rig.with_frame(&capture, |frame| {
                    let r = resolve_with(
                        &router,
                        frame,
                        &routed,
                        target,
                        &value,
                        previous.as_ref().map(|p| &p.0),
                    )
                    .unwrap();
                    let o = resolve_with(
                        &lamp,
                        frame,
                        &original,
                        target,
                        &value,
                        previous.as_ref().map(|p| &p.1),
                    )
                    .unwrap();
                    (r, o)
                });
                assert_lamp_identical(&r, &o, &case);
                assert_eq!(r.requested.semantic(), Some(&request), "{case}: unchanged");
                previous = Some((r.continuity, o.continuity));
            }
            let (routed_work, original_work) = (router.lamp().counters(), lamp.counters());
            assert_eq!(
                (routed_work.resolves, routed_work.fits, routed_work.refits),
                (
                    original_work.resolves,
                    original_work.fits,
                    original_work.refits
                ),
                "{case}: fitted once per head, never twice"
            );
            assert_eq!(
                router.media().counters(),
                Default::default(),
                "{case}: lamps never reach Media"
            );
            assert_eq!(router.counters().lamp_routes, 1);
            assert_eq!(router.counters().discarded_continuity, 0);
        }
    }
}

#[test]
fn shipped_media_layers_and_master_delegate_white_blend_tint_and_leave_intensity_alone() {
    let (root, layers) = (FixtureId::new(), [FixtureId::new(), FixtureId::new()]);
    let profile = shipped_media_server();
    let rig = Rig::new(vec![media_fixture(&profile, root, &layers)]);
    rig.programmers.set(
        rig.session,
        layers[0],
        AttributeKey("intensity".into()),
        AttributeValue::Normalized(0.6),
    );
    let router = RoutingColorAdapter::default();
    let media = MediaColorAdapter::default();
    let snapshot = rig.engine.snapshot();
    let dimmer_index = layer_dimmer(&profile);
    for target in [layers[0], layers[1], root] {
        let routed = router.compile(&snapshot, target).unwrap().unwrap();
        let original = media.compile(&snapshot, target).unwrap().unwrap();
        assert_eq!(routed.route(), ColorRoute::Media);
        assert_eq!(router.footprint(&routed), media.footprint(&original));
        let layer = target != root;
        for white_blend in [0., 0.5, 1.] {
            let request = tinted(white_blend);
            let value = program(&request);
            let capture = rig.capture();
            let (r, o, dimmer) = rig.with_frame(&capture, |frame| {
                let r = resolve_with(&router, frame, &routed, target, &value, None).unwrap();
                let o = resolve_with(&media, frame, &original, target, &value, None).unwrap();
                let native = frame
                    .scalar
                    .native_raw(&capture, frame.token, target)
                    .unwrap();
                (r, o, native.raw()[dimmer_index])
            });
            let case = format!("layer={layer} White Blend {white_blend}");
            assert_media_identical(&r, &o, &case);
            // Persistent tint: cyan/magenta/yellow do not move with White Blend.
            assert_eq!(raws(&r.writes)[..3], [0, 128, 255], "{case}");
            let RoutedAchievedColor::Media(achieved) = r.achieved else {
                unreachable!()
            };
            if layer {
                let grayscale = [0, 128, 255][(white_blend * 2.) as usize];
                assert_eq!(
                    raws(&r.writes)[3],
                    grayscale,
                    "{case}: White Blend drives Grayscale"
                );
                assert_eq!(
                    achieved.white_blend,
                    Some(f32::from(grayscale as u8) / 255.)
                );
            } else {
                assert_eq!(r.writes.len(), 3, "the master has no White Blend control");
                assert_eq!(achieved.white_blend, None, "{case}");
            }
            assert!(
                r.writes.iter().all(|w| {
                    profile.modes[0].channels[w.slot.channel_index as usize]
                        .attribute
                        .0
                        != "intensity".into()
                }),
                "{case}: the layer dimmer is Intensity, never a Color write"
            );
            assert_eq!(dimmer, 153, "{case}: pre-master layer intensity untouched");
        }
    }
    let lamp = router.lamp().counters();
    assert_eq!(
        (lamp.descriptor_compiles, lamp.resolves, lamp.fits),
        (0, 0, 0),
        "Media is never lamp compiled or fitted"
    );
    assert_eq!(router.counters().media_routes, 3);
    assert_eq!(router.media().counters().resolves, 9);
}

#[test]
fn reserved_media_targets_stay_passive_and_never_reach_lamp_fitting() {
    let (root, layers) = (FixtureId::new(), [FixtureId::new(), FixtureId::new()]);
    let mixed = FixtureId::new();
    let unsplit = FixtureId::new();
    let copied_root = FixtureId::new();
    let copied = [FixtureId::new(), FixtureId::new()];
    let shipped = shipped_media_server();
    let unsupported = unsupported_media_server();
    let mixed_profile = mixed_lamp_and_media();
    // `validated`: installed through the engine's patch validation. The unsplit and mixed
    // topologies are rejected there (one shared head; every other head needs a logical head),
    // so the router's guard for them is checked on an unvalidated captured snapshot.
    let cases: [(&str, Vec<PatchedFixture>, FixtureId, bool); 4] = [
        (
            "unsupported Media personality",
            vec![media_fixture(&unsupported, root, &layers)],
            layers[0],
            true,
        ),
        (
            "unsplit multi-head Media root",
            vec![patched(&shipped, unsplit, 1)],
            unsplit,
            false,
        ),
        (
            "mixed lamp and Media heads",
            vec![patched(&mixed_profile, mixed, MIXED_ADDRESS)],
            mixed,
            false,
        ),
        (
            "Media head with a multipatch copy",
            vec![with_copy(media_fixture(&shipped, copied_root, &copied), 1)],
            copied[0],
            true,
        ),
    ];
    for (case, fixtures, target, validated) in cases {
        let snapshot = if validated {
            Rig::new(fixtures).engine.snapshot()
        } else {
            Arc::new(light_engine::EngineSnapshot {
                fixtures: fixtures.into(),
                revision: 1,
                ..Default::default()
            })
        };
        assert!(
            !light_engine::profile_head_destinations(&snapshot, target).is_empty(),
            "{case}"
        );
        let router = RoutingColorAdapter::default();
        assert!(
            router.compile(&snapshot, target).unwrap().is_none(),
            "{case}"
        );
        assert_eq!(
            router.lamp().counters(),
            Default::default(),
            "{case}: no lamp compile, fit or fallback"
        );
        if case.starts_with("mixed") {
            // Load-bearing: the lamp adapter alone would accept the lamp head and drop the
            // reserved Media head.
            let partial = ColorAdapter::default()
                .compile(&snapshot, target)
                .unwrap()
                .expect("the lamp adapter alone compiles the lamp subset");
            assert_eq!(partial.heads.len(), 1);
        }
    }

    // Direct and scalar values on a supported Media head are passive, with no lamp fallback.
    let rig = Rig::new(vec![media_fixture(&shipped, root, &layers)]);
    let snapshot = rig.engine.snapshot();
    let router = RoutingColorAdapter::default();
    let descriptor = router.compile(&snapshot, layers[0]).unwrap().unwrap();
    let lamp_profile = rgb();
    let direct = direct(&catalogue(&[&lamp_profile]), &lamp_profile, &[0, 0, 255]);
    for value in [direct, AttributeValue::Normalized(0.5)] {
        let capture = rig.capture();
        let outcome = rig.with_frame(&capture, |frame| {
            resolve_with(&router, frame, &descriptor, layers[0], &value, None).map(|_| ())
        });
        assert!(matches!(
            outcome,
            Err(TransitionError::Requires(
                TransitionRequirement::ColorAppearance
            ))
        ));
    }
    assert_eq!(router.lamp().counters(), Default::default());
}

/// One outer Live lane through the existing hybrid composer and engine finalizer.
#[derive(Default)]
struct LiveRun {
    runtime: Option<DynamicRuntime>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
}

#[derive(Clone, Copy, PartialEq)]
enum Finish {
    /// Engine finalizer with this frame's own capture.
    Accept,
    /// Finalizer handed a foreign capture: must fail and abandon the staged lane frame.
    Foreign,
    /// Prepared but never finalized (a dropped attempt).
    Drop,
}

impl LiveRun {
    fn run(
        &mut self,
        rig: &Rig,
        lane: &PhysicalAdapterLane<RoutingColorAdapter>,
        finish: Finish,
    ) -> Result<Option<PublishedPhysicalFrame<RoutingColorAdapter>>, DynamicRuntimeError> {
        let capture = rig.capture();
        let foreign = rig.capture();
        let finalize = if finish == Finish::Foreign {
            &foreign
        } else {
            &capture
        };
        let runtime = self.runtime.get_or_insert_with(|| {
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION)
        });
        let snapshot = capture.snapshot();
        let addresser = capture.frame_addresser();
        let speeds = transports();
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
        let mut candidate = self.origins.clone();
        let scratch = &mut self.scratch;
        let engine = &rig.engine;
        let output = runtime.with_output_frame_transaction(
            &mut DynamicOutputFrameScratch::default(),
            |runtime| {
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
                if finish == Finish::Drop {
                    return Ok(None);
                }
                finalize_live_physical_frame(engine, finalize, lane, prepared).map(Some)
            },
        );
        if matches!(output, Ok(Some(_))) {
            self.origins = candidate;
        }
        output
    }
}

fn result(published: &PublishedPhysicalFrame<RoutingColorAdapter>, target: FixtureId) -> &Sidecar {
    let mut rows = published.results.iter().filter(|r| r.target == target);
    let row = rows.next().expect("a Color result for the target");
    assert!(rows.next().is_none());
    row
}

fn held(published: &PublishedPhysicalFrame<RoutingColorAdapter>, target: FixtureId) -> bool {
    published.results.iter().all(|r| r.target != target)
        && published
            .requirements
            .iter()
            .any(|row| row.target == target && row.owner == ProgrammingOwner::Color)
}

#[test]
fn one_live_lane_routes_lamps_and_media_with_acceptance_rollback_and_passive_reserved_targets() {
    let (lamp, root, layers) = (
        FixtureId::new(),
        FixtureId::new(),
        [FixtureId::new(), FixtureId::new()],
    );
    // A second Media Server with an unsupported personality: its layer is reserved for Media,
    // so it stays passive (no lamp fallback, no partial writes) beside the routed targets.
    let (unsupported_root, unsupported_layers) =
        (FixtureId::new(), [FixtureId::new(), FixtureId::new()]);
    let mut unsupported = media_fixture(
        &unsupported_media_server(),
        unsupported_root,
        &unsupported_layers,
    );
    unsupported.universe = Some(3);
    let mixed = unsupported_layers[0];
    let rig = Rig::new(vec![
        media_fixture(&shipped_media_server(), root, &layers),
        patched(&rgb(), lamp, LAMP_ADDRESS),
        unsupported,
    ]);
    rig.fix(lamp, &magenta());
    rig.fix(layers[0], &tinted(0.5));
    rig.fix(mixed, &intent([1., 0., 0.], 0.));
    let lane = PhysicalAdapterLane::live(RoutingColorAdapter::default());
    let mut live = LiveRun::default();

    // Failed finalizer: nothing accepted, nothing committed, the staged frame is abandoned.
    assert!(live.run(&rig, &lane, Finish::Foreign).is_err());
    // A prepared attempt that is never finalized commits nothing either.
    assert!(live.run(&rig, &lane, Finish::Drop).unwrap().is_none());
    assert_eq!(lane.last_accepted(), None);
    for target in [lamp, layers[0], mixed] {
        assert_eq!(lane.continuity(target, ProgrammingOwner::Color), None);
    }

    let mut previous_lamp = None;
    for (frame, request) in [magenta(), intent([0., 1., 1.], 0.)].iter().enumerate() {
        if frame > 0 {
            rig.fix(lamp, request);
        }
        let before = lane.adapter().lamp().counters();
        let published = live.run(&rig, &lane, Finish::Accept).unwrap().unwrap();
        assert_eq!(lane.last_accepted(), Some(published.token.clone()));
        assert!(
            !lane.accept_frame(&published.token),
            "frame {frame}: accepted exactly once"
        );
        assert_eq!(published.results.len(), 2, "frame {frame}: lamp + Media");
        assert!(
            held(&published, mixed),
            "unsupported reserved Media target stays passive, no writes"
        );
        let (l, m) = (result(&published, lamp), result(&published, layers[0]));
        for row in [l, m] {
            assert_eq!(row.token, published.token);
            assert_eq!(
                published
                    .rendered
                    .resolved_values
                    .value(row.target, &ProgrammingOwner::Color.key()),
                Some(&row.value)
            );
        }
        assert_eq!(l.requested.semantic(), Some(request), "requested unchanged");
        assert_eq!(m.requested.semantic(), Some(&tinted(0.5)));
        // The same composed values through the ORIGINAL adapters give identical output.
        let reference_lamp = ColorAdapter::default();
        let reference_media = MediaColorAdapter::default();
        let lamp_continuity: Option<ColorContinuity> = previous_lamp.take();
        let (o_lamp, o_media) = live_reference(
            &rig,
            &reference_lamp,
            &reference_media,
            [(lamp, &l.value), (layers[0], &m.value)],
            lamp_continuity.as_ref(),
        );
        assert_eq!(l.writes, o_lamp.writes, "frame {frame}: lamp writes");
        assert_eq!(l.achieved, RoutedAchievedColor::Lamp(o_lamp.achieved));
        assert_eq!(l.quality, RoutedColorQuality::Lamp(o_lamp.quality.clone()));
        assert_eq!(m.writes, o_media.writes, "frame {frame}: Media writes");
        assert_eq!(m.achieved, RoutedAchievedColor::Media(o_media.achieved));
        assert_eq!(m.quality, RoutedColorQuality::Media(o_media.quality));
        assert_eq!(raws(&m.writes), [0, 128, 255, 128]);
        let after = lane.adapter().lamp().counters();
        assert_eq!(
            after.resolves - before.resolves,
            1,
            "one lamp resolve per frame"
        );
        assert_eq!(after.fits - before.fits, u64::from(o_lamp_fits(&o_lamp)));
        let RoutedColorContinuity::Lamp(committed) =
            lane.continuity(lamp, ProgrammingOwner::Color).unwrap()
        else {
            panic!("lamp continuity")
        };
        assert_eq!(committed, o_lamp.continuity);
        previous_lamp = Some(committed);
        assert_eq!(
            lane.continuity(layers[0], ProgrammingOwner::Color),
            Some(RoutedColorContinuity::Media(()))
        );
        assert_eq!(lane.continuity(mixed, ProgrammingOwner::Color), None);
    }
    let counters = lane.adapter().counters();
    assert_eq!(
        (
            counters.lamp_routes,
            counters.media_routes,
            counters.reserved_passive
        ),
        (1, 1, 1),
        "one descriptor per target and generation, owned by the outer lane"
    );
    assert_eq!(counters.discarded_continuity, 0);
    // The failed and the dropped attempt resolved too (staged only, never committed); each
    // accepted frame resolved once.
    assert_eq!(lane.adapter().media().counters().resolves, 4);
}

fn o_lamp_fits(resolution: &PhysicalResolution<ColorAdapter>) -> u32 {
    resolution.quality.work.fits
}

/// Re-resolve a published Live frame's composed values through fresh ORIGINAL adapters on a
/// newly captured frame of the same installed snapshot (the Color result depends only on the
/// composed value, the descriptor, the pre-master native baseline and the passed continuity).
fn live_reference(
    rig: &Rig,
    lamp: &ColorAdapter,
    media: &MediaColorAdapter,
    [(lamp_target, lamp_value), (media_target, media_value)]: [(FixtureId, &AttributeValue); 2],
    previous: Option<&ColorContinuity>,
) -> (
    PhysicalResolution<ColorAdapter>,
    PhysicalResolution<MediaColorAdapter>,
) {
    let capture = rig.capture();
    let snapshot = capture.snapshot();
    let lamp_descriptor = lamp.compile(&snapshot, lamp_target).unwrap().unwrap();
    let media_descriptor = media.compile(&snapshot, media_target).unwrap().unwrap();
    rig.with_frame(&capture, |frame| {
        (
            resolve_with(
                lamp,
                frame,
                &lamp_descriptor,
                lamp_target,
                lamp_value,
                previous,
            )
            .unwrap(),
            resolve_with(
                media,
                frame,
                &media_descriptor,
                media_target,
                media_value,
                None,
            )
            .unwrap(),
        )
    })
}

#[test]
fn live_replacement_reroutes_discards_foreign_continuity_and_recovers_from_captured_current() {
    let (target, root, other) = (FixtureId::new(), FixtureId::new(), FixtureId::new());
    let rgb_profile = rgb();
    let lamp_fixture = || vec![patched(&rgb_profile, target, LAMP_ADDRESS)];
    let rig = Rig::new(lamp_fixture());
    let request = magenta();
    rig.fix(target, &request);
    let lane = PhysicalAdapterLane::live(RoutingColorAdapter::default());
    let mut live = LiveRun::default();

    // 1. Lamp.
    let published = live.run(&rig, &lane, Finish::Accept).unwrap().unwrap();
    let lamp_writes = result(&published, target).writes.clone();
    assert_eq!(
        lane.continuity(target, ProgrammingOwner::Color)
            .map(|c| c.route()),
        Some(ColorRoute::Lamp)
    );
    let generation = lane.descriptor_generation();

    // 2. The same target becomes a supported Media layer: re-routed from the new captured
    //    profile; the lamp continuity is discarded, the request is untouched.
    rig.install(vec![media_fixture(
        &shipped_media_server(),
        root,
        &[target, other],
    )]);
    let published = live.run(&rig, &lane, Finish::Accept).unwrap().unwrap();
    assert_ne!(
        lane.descriptor_generation(),
        generation,
        "no stale descriptor"
    );
    let row = result(&published, target);
    assert_eq!(row.requested, RoutedColorRequest::Media(request.clone()));
    assert_eq!(row.writes.len(), 4);
    assert!(row.writes.iter().all(|w| w.slot.destination == root));
    assert_eq!(lane.adapter().counters().discarded_continuity, 1);
    assert_eq!(
        lane.continuity(target, ProgrammingOwner::Color),
        Some(RoutedColorContinuity::Media(()))
    );

    // 3. An unsupported Media personality: passive hold, no writes, no lamp fallback; the last
    //    accepted continuity stays and nothing is released.
    rig.install(vec![media_fixture(
        &unsupported_media_server(),
        root,
        &[target, other],
    )]);
    let lamp_compiles = lane.adapter().lamp().counters().descriptor_compiles;
    let published = live.run(&rig, &lane, Finish::Accept).unwrap().unwrap();
    assert!(held(&published, target));
    assert!(published.released.is_empty(), "a hold is not a removal");
    assert_eq!(
        lane.continuity(target, ProgrammingOwner::Color),
        Some(RoutedColorContinuity::Media(()))
    );
    assert_eq!(
        lane.adapter().lamp().counters().descriptor_compiles,
        lamp_compiles
    );

    // 4. Back to the lamp: not a permanent hold. The Media continuity is discarded and the lamp
    //    fits from the actual captured scalar Current, exactly like a fresh lamp adapter.
    rig.install(lamp_fixture());
    let published = live.run(&rig, &lane, Finish::Accept).unwrap().unwrap();
    let row = result(&published, target);
    assert_eq!(lane.adapter().counters().discarded_continuity, 2);
    assert_eq!(
        row.requested,
        RoutedColorRequest::Lamp(ColorRequest::Semantic(request.clone()))
    );
    let reference = ColorAdapter::default();
    let capture = rig.capture();
    let descriptor = reference
        .compile(&capture.snapshot(), target)
        .unwrap()
        .unwrap();
    let fresh = rig.with_frame(&capture, |frame| {
        resolve_with(&reference, frame, &descriptor, target, &row.value, None).unwrap()
    });
    assert_eq!(row.writes, fresh.writes);
    assert_eq!(row.writes, lamp_writes);
    assert_eq!(row.quality, RoutedColorQuality::Lamp(fresh.quality));
    assert_eq!(
        lane.continuity(target, ProgrammingOwner::Color),
        Some(RoutedColorContinuity::Lamp(fresh.continuity))
    );
    let state = rig.programmers.get(rig.session).unwrap();
    assert!(
        state.values.iter().any(|v| v.fixture_id == target
            && v.attribute == ProgrammingOwner::Color.key()
            && v.value == program(&request)),
        "the programmed intent is never rewritten"
    );

    // 5. Removal: Release retires the owner with its last provenance.
    rig.release(target);
    let published = live.run(&rig, &lane, Finish::Accept).unwrap().unwrap();
    assert!(published.results.iter().all(|r| r.target != target));
    let [released] = published.released.as_slice() else {
        panic!("one released Color owner")
    };
    assert_eq!(
        (released.target, released.owner),
        (target, ProgrammingOwner::Color)
    );
    assert_eq!(lane.continuity(target, ProgrammingOwner::Color), None);
}

fn capacity(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

fn limits() -> PendingHistoryLimits {
    PendingHistoryLimits {
        attempts: capacity(16),
        cold_changes: capacity(16),
        controls: capacity(64),
    }
}

/// The retained Preload rig of the existing paired evaluator tests, for several targets.
struct PreloadRig {
    rig: Rig,
    key: PendingEpisodeKey,
    publication: DynamicSnapshotPublication,
    live: RefCell<DynamicRuntime>,
    started: Instant,
    selected: Cell<u64>,
}

impl PreloadRig {
    fn new(fixtures: Vec<PatchedFixture>, programmed: &[(FixtureId, ColorIntent)]) -> Self {
        let rig = Rig::new(fixtures);
        let programmer = rig.programmers.get(rig.session).unwrap().id;
        let publication = DynamicSnapshotPublication::new(rig.engine.snapshot());
        let mut live =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        publication
            .begin_retained_history(&mut live, &rig.engine.snapshot(), capacity(64))
            .unwrap();
        rig.programmers.arm_preload(rig.session, true);
        rig.clock.advance_millis(10);
        for (target, request) in programmed {
            rig.fix(*target, request);
        }
        Self {
            rig,
            key: PendingEpisodeKey {
                activation: Uuid::new_v4(),
                programmer,
                branch: PreloadBranch::BeforeRelease,
            },
            publication,
            live: RefCell::new(live),
            started: Instant::now(),
            selected: Cell::new(0),
        }
    }

    fn pair(&self) -> Pair {
        let mut live = self.live.borrow_mut();
        let (cold, controls) = self
            .publication
            .begin_retained_history(&mut live, &self.rig.engine.snapshot(), capacity(64))
            .unwrap();
        let seed = |branch| PendingHistorySeed {
            key: PendingEpisodeKey { branch, ..self.key },
            runtime: live.fork_for_pending_preview(),
            origins: Default::default(),
            snapshot: self.rig.engine.snapshot(),
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

    fn capture(&self) -> Arc<RetainedInputCapture> {
        self.rig.clock.advance_millis(40);
        let cursor = self.publication.input_capture_cursor().unwrap();
        let selected = self.selected.get();
        self.selected.set(selected + 1);
        let frame = RetainedFrameCapture::select(
            self.rig.engine.prepare_output_frame(Default::default()),
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
        pair: &mut Pair,
        evaluator: &mut impl PendingPairEvaluator<PendingHybridResult<Sidecar>>,
    ) -> PendingPairWindowOutcome {
        let input = self.capture();
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
        pair.consume_window(window, evaluator)
    }
}

fn branch_row(branch: &PendingHybridBranch<Sidecar>, target: FixtureId) -> Option<&Sidecar> {
    let mut rows = branch.sidecars.iter().filter(|row| row.target == target);
    let row = rows.next();
    assert!(rows.next().is_none(), "at most one Color owner per target");
    row
}

#[test]
fn one_paired_preload_lane_set_routes_independently_with_removal_and_pair_rollback() {
    let (lamp, root, layers) = (
        FixtureId::new(),
        FixtureId::new(),
        [FixtureId::new(), FixtureId::new()],
    );
    let rig = PreloadRig::new(
        vec![
            media_fixture(&shipped_media_server(), root, &layers),
            patched(&rgb(), lamp, LAMP_ADDRESS),
        ],
        &[(lamp, magenta()), (layers[0], tinted(0.5))],
    );
    let mut pair = rig.pair();
    let lanes = PhysicalPreloadLanes::new(
        RoutingColorAdapter::default(),
        RoutingColorAdapter::default(),
    );
    let mut evaluator = RetainedPreloadHybridEvaluator::new(
        &rig.rig.engine,
        rig.key.programmer,
        &lanes,
        |branch, observation: HybridFamilyObservation<'_>| lanes.observe(branch, observation),
    );
    let branches = [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease];
    let outcome = rig.consume(&mut pair, &mut evaluator);
    assert_eq!(
        outcome.successful_attempts, 1,
        "{:?}",
        outcome.failed_attempts
    );
    let first = &pair.last_success().unwrap().value;
    assert_ne!(first.before.frame_token, first.after.frame_token);
    assert!(
        first
            .before
            .frame_token
            .same_capture(&first.after.frame_token)
    );
    for (branch, value) in [(branches[0], &first.before), (branches[1], &first.after)] {
        let (l, m) = (
            branch_row(value, lamp).expect("lamp row"),
            branch_row(value, layers[0]).expect("Media row"),
        );
        for row in [l, m] {
            assert_eq!(row.token, value.frame_token, "{branch:?}: its own frame");
            assert_eq!(row.token.lane().preload_branch(), Some(branch));
        }
        assert!(matches!(l.quality, RoutedColorQuality::Lamp(_)));
        assert_eq!(raws(&m.writes), [0, 128, 255, 128], "{branch:?}");
        assert_eq!(m.requested, RoutedColorRequest::Media(tinted(0.5)));
        assert_eq!(l.requested.semantic(), Some(&magenta()));
        let lane = lanes.lane(branch);
        assert_eq!(lane.last_accepted(), Some(value.frame_token.clone()));
        assert!(
            !lanes.accept_frame(&value.frame_token),
            "accepted exactly once"
        );
        assert_eq!(
            lane.continuity(lamp, ProgrammingOwner::Color)
                .map(|c| c.route()),
            Some(ColorRoute::Lamp)
        );
        assert_eq!(
            lane.continuity(layers[0], ProgrammingOwner::Color),
            Some(RoutedColorContinuity::Media(()))
        );
        let counters = lane.adapter().counters();
        assert_eq!(
            (counters.lamp_routes, counters.media_routes),
            (1, 1),
            "{branch:?}: each lane compiles its own descriptors"
        );
        assert_eq!(lane.adapter().lamp().counters().resolves, 1);
        assert_eq!(lane.adapter().media().counters().resolves, 1);
    }
    assert_eq!(
        branch_row(&first.before, lamp).unwrap().writes,
        branch_row(&first.after, lamp).unwrap().writes,
        "identical inputs fit identically in both branches"
    );

    // A pending Release of the Media layer changes After only.
    rig.rig.release(layers[0]);
    let outcome = rig.consume(&mut pair, &mut evaluator);
    assert_eq!(
        outcome.successful_attempts, 1,
        "{:?}",
        outcome.failed_attempts
    );
    let second = &pair.last_success().unwrap().value;
    assert!(branch_row(&second.before, layers[0]).is_some());
    assert!(branch_row(&second.after, layers[0]).is_none());
    let after_lane = lanes.lane(PreloadBranch::AfterRelease);
    let [released] = after_lane
        .released()
        .try_into()
        .unwrap_or_else(|rows: Vec<_>| panic!("one released owner, got {}", rows.len()));
    assert_eq!(
        (released.target, released.owner),
        (layers[0], ProgrammingOwner::Color)
    );
    assert_eq!(
        after_lane.continuity(layers[0], ProgrammingOwner::Color),
        None
    );
    let before_lane = lanes.lane(PreloadBranch::BeforeRelease);
    assert!(before_lane.released().is_empty());
    assert_eq!(
        before_lane.continuity(layers[0], ProgrammingOwner::Color),
        Some(RoutedColorContinuity::Media(()))
    );

    // A rejected pair (paired engine finalizer token mismatch) advances neither branch.
    let accepted = branches.map(|branch| lanes.lane(branch).last_accepted());
    let continuity =
        branches.map(|branch| lanes.lane(branch).continuity(lamp, ProgrammingOwner::Color));
    let resolves = branches.map(|branch| lanes.lane(branch).adapter().lamp().counters().resolves);
    rig.rig.fix(lamp, &intent([0., 1., 1.], 0.));
    evaluator.swap_finalization_tokens = true;
    assert_eq!(
        rig.consume(&mut pair, &mut evaluator).failed_attempts.len(),
        1
    );
    for (index, branch) in branches.into_iter().enumerate() {
        let lane = lanes.lane(branch);
        assert_eq!(lane.last_accepted(), accepted[index], "{branch:?}");
        assert_eq!(
            lane.continuity(lamp, ProgrammingOwner::Color),
            continuity[index],
            "{branch:?}: rolled back"
        );
    }
    evaluator.swap_finalization_tokens = false;
    let outcome = rig.consume(&mut pair, &mut evaluator);
    assert_eq!(
        outcome.successful_attempts, 1,
        "{:?}",
        outcome.failed_attempts
    );
    let third = &pair.last_success().unwrap().value;
    for (index, (branch, value)) in [(branches[0], &third.before), (branches[1], &third.after)]
        .into_iter()
        .enumerate()
    {
        let lane = lanes.lane(branch);
        let row = branch_row(value, lamp).unwrap();
        assert_eq!(row.requested.semantic(), Some(&intent([0., 1., 1.], 0.)));
        assert_eq!(lane.last_accepted(), Some(value.frame_token.clone()));
        assert!(!lanes.accept_frame(&value.frame_token));
        let RoutedColorContinuity::Lamp(committed) =
            lane.continuity(lamp, ProgrammingOwner::Color).unwrap()
        else {
            panic!("lamp continuity")
        };
        assert_eq!(
            raws(&row.writes),
            committed.heads[0]
                .controls
                .iter()
                .map(|c| c.2)
                .collect::<Vec<_>>(),
            "{branch:?}: the accepted writes became continuity"
        );
        // The failed attempt fitted, but each later frame is still one resolve per head.
        assert!(lane.adapter().lamp().counters().resolves > resolves[index]);
        assert_eq!(lane.adapter().counters().discarded_continuity, 0);
    }
    assert!(branch_row(&third.after, layers[0]).is_none());
    assert!(branch_row(&third.before, layers[0]).is_some());
}
