//! TL-548 C1: one mixed fixture rig (a calibrated mover, an RGB wash, a shipped Media Server
//! layer and a Focus/Zoom wash) through ONE `FamilyLanes` set, the real hybrid composer and the
//! real engine finalizers. Live cases here; retained Preload cases in `tests/preload.rs`.
//! Values are whole-family FixATs over static underlays so every family reaches its observer
//! every frame; the profiles are the synthetic family test profiles, not physical measurements.
use super::super::color::profiles::{patched, rgb};
use super::super::color::tests::{intent, magenta, program};
use super::super::media_color::tests::{media_fixture, shipped_media_server};
use super::super::optics::profiles::wash_a;
use super::super::optics::tests::{field, focus, zoom};
use super::super::position::tests::{angles, moving_head};
use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use crate::runtime::output_scheduler::dynamic_projection::CapturedDynamicInputs;
use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::{
    HybridFamilyRequirementReason, HybridFrameScratch, prepare_captured_hybrid_frame,
    prepare_captured_hybrid_frame_with_observer,
};
use light_core::programming::{ColorIntent, PROGRAMMING_CONTRACT_VERSION};
use light_core::{ManualClock, OpeningConvention, SessionId};
use light_dynamics::{
    DynamicFamilyRepresentation, DynamicOutputFrameScratch, DynamicRuntime, DynamicSemanticValue,
    DynamicSpeedTransport, ProgrammingFamilyFixAt,
};
use light_fixture::{OpticsFamily, PatchedFixture, PositionFitStatus};
use light_programmer::{DynamicProgrammerValueMutation, ProgrammerRegistry};

mod multipatch;
mod preload;

fn tinted(white_blend: f32) -> ColorIntent {
    ColorIntent {
        white_blend,
        ..intent([1., 0.735, 0.], 0.)
    }
}

fn cyan() -> AttributeValue {
    program(&intent([0., 1., 1.], 0.))
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

/// Target ids of the mixed show. Every fixture sits alone on its own universe.
#[derive(Clone, Copy)]
pub(in crate::runtime) struct Show {
    pub mover: FixtureId,
    pub wash: FixtureId,
    pub media: FixtureId,
    pub layer: FixtureId,
    pub optics: FixtureId,
}

impl Show {
    fn new() -> Self {
        Self {
            mover: FixtureId::new(),
            wash: FixtureId::new(),
            media: FixtureId::new(),
            layer: FixtureId::new(),
            optics: FixtureId::new(),
        }
    }

    fn fixtures(&self) -> Vec<PatchedFixture> {
        let mut media = media_fixture(
            &shipped_media_server(),
            self.media,
            &[self.layer, FixtureId::new()],
        );
        media.universe = Some(3);
        let mut wash = patched(&rgb(), self.wash, 1);
        wash.universe = Some(2);
        let mut optics = patched(&wash_a(), self.optics, 1);
        optics.universe = Some(4);
        let mut fixtures = vec![patched(&moving_head(), self.mover, 1), wash, media, optics];
        for (number, fixture) in (1..).zip(&mut fixtures) {
            fixture.fixture_number = Some(number);
        }
        fixtures
    }

    /// Every programmed (target, owner): one per family plus the Media layer's Color.
    pub fn owners(&self) -> [(FixtureId, ProgrammingOwner); 5] {
        [
            (self.mover, ProgrammingOwner::Position),
            (self.wash, ProgrammingOwner::Color),
            (self.layer, ProgrammingOwner::Color),
            (self.optics, ProgrammingOwner::Focus),
            (self.optics, ProgrammingOwner::Zoom),
        ]
    }
}

pub(in crate::runtime) struct Rig {
    /// Shared so a Pending worker thread can evaluate against the same engine (TL-548 C4).
    pub engine: Arc<Engine>,
    pub programmers: ProgrammerRegistry,
    pub session: SessionId,
    pub clock: Arc<ManualClock>,
    pub show: Show,
}

impl Rig {
    pub fn new() -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        let show = Show::new();
        let engine = Arc::new(Engine::with_programming_contract_support(
            programmers.clone(),
            PROGRAMMING_CONTRACT_VERSION,
        ));
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: show.fixtures().into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        Self {
            engine,
            programmers,
            session,
            clock,
            show,
        }
    }

    /// Program every family: a whole FixAT over a static underlay of the same owner.
    pub fn program_all(&self) {
        let show = self.show;
        let mover = ProgrammingOwner::Position;
        self.fix(show.mover, mover, angles(10., 20.), angles(450., 30.));
        self.fix(
            show.wash,
            ProgrammingOwner::Color,
            cyan(),
            program(&magenta()),
        );
        self.fix(
            show.layer,
            ProgrammingOwner::Color,
            cyan(),
            program(&tinted(0.5)),
        );
        self.fix(
            show.optics,
            ProgrammingOwner::Focus,
            focus(0.2),
            focus(0.75),
        );
        self.fix(show.optics, ProgrammingOwner::Zoom, field(30.), field(20.));
    }

    pub fn fix(
        &self,
        target: FixtureId,
        owner: ProgrammingOwner,
        underlay: AttributeValue,
        value: AttributeValue,
    ) {
        // A frozen ManualClock would otherwise make successive edits rank ties.
        self.clock.advance_millis(10);
        self.programmers
            .set(self.session, target, owner.key(), underlay);
        assert!(self.programmers.apply_dynamic_values(
            self.session,
            &[DynamicProgrammerValueMutation::Set {
                fixture_id: target,
                attribute: owner.key(),
                value: DynamicSemanticValue::ProgrammingFixAt {
                    mask: ProgrammingFamilyFixAt::from_family(owner, None, value).unwrap(),
                    timing: Default::default(),
                },
            }],
            None,
        ));
    }

    pub fn release(&self, target: FixtureId, owner: ProgrammingOwner) {
        assert!(self.programmers.apply_dynamic_values(
            self.session,
            &[DynamicProgrammerValueMutation::Set {
                fixture_id: target,
                attribute: owner.key(),
                value: DynamicSemanticValue::Release,
            }],
            None,
        ));
    }

    fn capture(&self) -> PreparedOutputFrame {
        self.clock.advance_millis(25);
        self.engine.prepare_output_frame(Default::default())
    }
}

#[derive(Clone, Copy)]
enum Finish {
    Accept,
    /// The finalizer is handed another capture and must fail.
    Foreign,
    /// Prepared, never finalized.
    Drop,
    /// One family's lane loses its staged attempt before the finalizer (a family-local failure).
    Tamper(fn(&FamilyLanes)),
}

#[derive(Default)]
struct Live {
    runtime: Option<DynamicRuntime>,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
}

struct Attempt {
    token: CapturedFrameToken,
    output: Result<Option<PublishedFamilyFrame>, DynamicRuntimeError>,
}

/// The Live sampler inputs of one capture, bound to `$inputs` for `$body`.
macro_rules! with_inputs {
    ($capture:expr, |$inputs:ident| $body:expr) => {{
        let snapshot = $capture.snapshot();
        let addresser = $capture.frame_addresser();
        let speeds = transports();
        let $inputs = CapturedDynamicInputs {
            now: $capture.sampled_at(),
            speed_transports: &speeds,
            rate: 40,
            snapshot: &snapshot,
            programmer_values: $capture.dynamic_programmer_values(),
            programmer_rows: Some($capture.dynamic_programmer_rows()),
            cue_values: $capture.cue_dynamic_values(),
            dynamic_playbacks: $capture.dynamic_playbacks(),
            playback_paused: $capture.playback_dynamics_paused(),
            addresser: &addresser,
            extra_programmer_values: &[],
            programmer_reconciliation_cache: None,
            force_source_reconciliation: false,
        };
        $body
    }};
}

impl Live {
    fn run(
        &mut self,
        rig: &Rig,
        lanes: &FamilyLanes,
        finish: Finish,
        incidental: &[HybridFamilyRequirement],
    ) -> Attempt {
        let capture = rig.capture();
        let foreign = rig.capture();
        let finalize = match finish {
            Finish::Foreign => &foreign,
            _ => &capture,
        };
        let engine = &rig.engine;
        let mut candidate = self.origins.clone();
        let scratch = &mut self.scratch;
        let runtime = self.runtime.get_or_insert_with(|| {
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION)
        });
        let output = with_inputs!(capture, |inputs| runtime.with_output_frame_transaction(
            &mut DynamicOutputFrameScratch::default(),
            |runtime| {
                let prepared = prepare_captured_hybrid_frame_with_observer(
                    engine,
                    &capture,
                    &[],
                    runtime,
                    &mut candidate,
                    &inputs,
                    scratch,
                    lanes,
                    None,
                    &mut FamilyFrameObserver::new(lanes),
                )?;
                if !incidental.is_empty() {
                    lanes
                        .hold_frame(&prepared.frame_token, incidental)
                        .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
                }
                match finish {
                    Finish::Drop => return Ok(None),
                    Finish::Tamper(tamper) => tamper(lanes),
                    Finish::Accept | Finish::Foreign => {}
                }
                finalize_live_family_frame(engine, finalize, lanes, prepared).map(Some)
            },
        ));
        if matches!(output, Ok(Some(_))) {
            self.origins = candidate;
        }
        Attempt {
            token: capture.frame_token(),
            output,
        }
    }

    fn accept(&mut self, rig: &Rig, lanes: &FamilyLanes) -> PublishedFamilyFrame {
        self.run(rig, lanes, Finish::Accept, &[])
            .output
            .unwrap()
            .unwrap()
    }
}

pub(super) fn row(
    rows: &[FamilySidecar],
    target: FixtureId,
    owner: ProgrammingOwner,
) -> Option<&FamilySidecar> {
    let mut found = rows
        .iter()
        .filter(|row| row.target() == target && row.owner() == owner);
    let row = found.next();
    assert!(found.next().is_none(), "one sidecar per head owner");
    row
}

/// Committed continuity of every programmed owner of the show (`Show::owners` order).
pub(super) fn continuity(lanes: &FamilyLanes, show: &Show) -> Vec<String> {
    let optics = |family| lanes.optics().lane(family);
    vec![
        format!(
            "{:?}",
            lanes
                .position()
                .continuity(show.mover, ProgrammingOwner::Position)
        ),
        format!(
            "{:?}",
            lanes.color().continuity(show.wash, ProgrammingOwner::Color)
        ),
        format!(
            "{:?}",
            lanes
                .color()
                .continuity(show.layer, ProgrammingOwner::Color)
        ),
        format!(
            "{:?}",
            optics(OpticsFamily::Focus).continuity(show.optics, ProgrammingOwner::Focus)
        ),
        format!(
            "{:?}",
            optics(OpticsFamily::Zoom).continuity(show.optics, ProgrammingOwner::Zoom)
        ),
    ]
}

/// No single lane may still stage `token`: a lane skipped by `abandon` would commit it here.
pub(super) fn assert_every_lane_abandoned(
    lanes: &FamilyLanes,
    token: &CapturedFrameToken,
    case: &str,
) {
    assert!(
        !lanes.position().accept_frame(token),
        "{case}: Position lane still staged"
    );
    assert!(
        !lanes.color().accept_frame(token),
        "{case}: Color lane still staged"
    );
    for family in [OpticsFamily::Focus, OpticsFamily::Zoom] {
        assert!(
            !lanes.optics().lane(family).accept_frame(token),
            "{case}: {family:?} lane still staged"
        );
    }
}

pub(super) fn released(lanes: &FamilyLanes) -> Vec<(FixtureId, ProgrammingOwner)> {
    lanes
        .released()
        .iter()
        .map(|row| (row.target, row.owner))
        .collect()
}

pub(super) fn every(token: &CapturedFrameToken) -> [Option<CapturedFrameToken>; 4] {
    [0; 4].map(|_| Some(token.clone()))
}

#[test]
fn one_live_frame_produces_every_family_under_one_token_and_installs_position_natively() {
    let rig = Rig::new();
    rig.program_all();
    let show = rig.show;
    let lanes = FamilyLanes::live();
    let published = Live::default().accept(&rig, &lanes);
    assert!(published.requirements.is_empty(), "every family is modeled");
    assert_eq!(published.results.len(), 5);
    // The same show through each family's own lane and finalizer: identical writes.
    let references = references(&rig);
    for (target, owner) in show.owners() {
        let row =
            row(&published.results, target, owner).unwrap_or_else(|| panic!("{owner:?} sidecar"));
        assert_eq!(row.token(), &published.token, "{owner:?}: one token");
        assert!(
            !row.writes().is_empty(),
            "{owner:?}: complete native writes"
        );
        assert_eq!(
            published
                .rendered
                .resolved_values
                .value(target, &owner.key()),
            Some(row.value()),
            "{owner:?}: the engine received the composed value the sidecar describes",
        );
        let (_, writes) = references
            .iter()
            .find(|(key, _)| *key == (target, owner))
            .unwrap_or_else(|| panic!("{owner:?} reference"));
        assert_eq!(
            row.writes(),
            writes.as_slice(),
            "{owner:?}: own-lane parity"
        );
    }
    let family = |target, owner| row(&published.results, target, owner).unwrap();
    assert!(
        family(show.mover, ProgrammingOwner::Position)
            .position()
            .is_some()
    );
    assert!(
        family(show.layer, ProgrammingOwner::Color)
            .color()
            .is_some()
    );
    assert!(family(show.wash, ProgrammingOwner::Color).color().is_some());
    for owner in [ProgrammingOwner::Focus, ProgrammingOwner::Zoom] {
        assert!(family(show.optics, owner).optics().is_some(), "{owner:?}");
    }
    let position = family(show.mover, ProgrammingOwner::Position)
        .position()
        .unwrap();
    assert_eq!(
        position.achieved.outcomes[0].result.status,
        PositionFitStatus::Fitted
    );
    // Position native writes reach the bytes once, through the family observer's projection.
    let bytes = &published.rendered.universes[&1];
    for write in &position.writes {
        let start = 2 * write.slot.channel_index as usize;
        let raw = u16::from_be_bytes([bytes[start], bytes[start + 1]]);
        assert_eq!(u32::from(raw), write.raw, "Position native bytes");
    }
    assert_eq!(lanes.last_accepted(), every(&published.token));
    assert!(
        !lanes.accept_frame(&published.token),
        "accepted exactly once"
    );
    assert!(published.released.is_empty());
    assert!(continuity(&lanes, &show).iter().all(|row| row != "None"));
}

type Reference = ((FixtureId, ProgrammingOwner), Vec<NativeControlWrite>);

/// The programmed show through the existing single-family lanes and finalizers, each on its own
/// capture of the same static programmer state.
fn references(rig: &Rig) -> Vec<Reference> {
    let mut rows: Vec<Reference> = Vec::new();
    let position = PhysicalAdapterLane::live(PositionAdapter::default());
    let color = PhysicalAdapterLane::live(RoutingColorAdapter::default());
    let optics = OpticsLanes::live();
    for family in 0..3 {
        let capture = rig.capture();
        let mut runtime =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        let mut origins = DynamicSourceOrigins::default();
        let mut scratch = HybridFrameScratch::default();
        let (engine, rows) = (&rig.engine, &mut rows);
        with_inputs!(capture, |inputs| runtime.with_output_frame_transaction(
            &mut DynamicOutputFrameScratch::default(),
            |runtime| -> Result<(), DynamicRuntimeError> {
                let mut collect = |target, owner, writes: &[NativeControlWrite]| {
                    rows.push(((target, owner), writes.to_vec()));
                };
                macro_rules! prepare {
                    ($prepare:ident, $lane:expr, $observer:expr) => {
                        $prepare(
                            engine,
                            &capture,
                            &[],
                            runtime,
                            &mut origins,
                            &inputs,
                            &mut scratch,
                            $lane,
                            None,
                            $observer,
                        )?
                    };
                }
                match family {
                    0 => {
                        let mut observer =
                            super::super::position::PositionFrameObserver::new(&position);
                        let prepared = prepare!(
                            prepare_captured_hybrid_frame_with_observer,
                            &position,
                            &mut observer
                        );
                        let published =
                            finalize_live_physical_frame(engine, &capture, &position, prepared)?;
                        for r in published.results {
                            collect(r.target, r.owner, &r.writes);
                        }
                    }
                    1 => {
                        let prepared =
                            prepare!(prepare_captured_hybrid_frame, &color, |o| color.observe(o));
                        let published =
                            finalize_live_physical_frame(engine, &capture, &color, prepared)?;
                        for r in published.results {
                            collect(r.target, r.owner, &r.writes);
                        }
                    }
                    _ => {
                        let prepared = prepare!(prepare_captured_hybrid_frame, &optics, |o| optics
                            .observe(o));
                        let published = super::super::optics::finalize_live_optics_frame(
                            engine, &capture, &optics, prepared,
                        )?;
                        for r in published.results {
                            collect(r.target, r.owner, &r.writes);
                        }
                    }
                }
                Ok(())
            },
        ))
        .unwrap();
    }
    rows
}

#[test]
fn a_failed_finalizer_in_any_family_advances_no_lane_and_abandons_every_lane() {
    let rig = Rig::new();
    rig.program_all();
    let show = rig.show;
    let lanes = FamilyLanes::live();
    let mut live = Live::default();
    let accepted = live.accept(&rig, &lanes);
    let committed = continuity(&lanes, &show);
    // New requests in every family, so any lane that advanced would change its continuity.
    let (position, color) = (ProgrammingOwner::Position, ProgrammingOwner::Color);
    rig.fix(show.mover, position, angles(10., 20.), angles(-200., 60.));
    rig.fix(show.wash, color, program(&magenta()), cyan());
    rig.fix(
        show.optics,
        ProgrammingOwner::Focus,
        focus(0.2),
        focus(0.25),
    );
    rig.fix(show.optics, ProgrammingOwner::Zoom, field(30.), field(40.));
    let cases: [(&str, Finish); 6] = [
        ("foreign capture", Finish::Foreign),
        (
            "Position",
            Finish::Tamper(|lanes| lanes.position().abandon()),
        ),
        ("Color", Finish::Tamper(|lanes| lanes.color().abandon())),
        (
            "Focus",
            Finish::Tamper(|lanes| lanes.optics().lane(OpticsFamily::Focus).abandon()),
        ),
        (
            "Zoom",
            Finish::Tamper(|lanes| lanes.optics().lane(OpticsFamily::Zoom).abandon()),
        ),
        ("dropped attempt", Finish::Drop),
    ];
    for (case, finish) in cases {
        let attempt = live.run(&rig, &lanes, finish, &[]);
        if matches!(finish, Finish::Drop) {
            assert!(attempt.output.unwrap().is_none());
            // Every lane staged it; once one family loses its attempt, accept commits none.
            lanes.optics().lane(OpticsFamily::Zoom).abandon();
            assert!(
                !lanes.accept_frame(&attempt.token),
                "{case}: accept is all or nothing"
            );
        } else {
            assert!(attempt.output.is_err(), "{case}: the finalizer rejects it");
            assert_every_lane_abandoned(&lanes, &attempt.token, case);
        }
        assert_eq!(lanes.last_accepted(), every(&accepted.token), "{case}");
        assert_eq!(
            continuity(&lanes, &show),
            committed,
            "{case}: no lane advanced"
        );
        assert!(lanes.released().is_empty(), "{case}: nothing released");
    }
    // The retry commits every family's new request together.
    let retried = live.accept(&rig, &lanes);
    assert_eq!(lanes.last_accepted(), every(&retried.token));
    let advanced = continuity(&lanes, &show);
    for index in [0, 1, 3, 4] {
        assert_ne!(advanced[index], committed[index], "owner {index} advanced");
    }
    assert_eq!(advanced[2], committed[2], "the unchanged Media layer");
}

pub(super) fn requirement(target: FixtureId, owner: ProgrammingOwner) -> HybridFamilyRequirement {
    HybridFamilyRequirement {
        target,
        owner,
        reason: HybridFamilyRequirementReason::Composition(owner_requirement(owner)),
    }
}

#[test]
fn a_passive_zoom_hold_never_holds_color_and_released_owners_are_reported_per_family() {
    let rig = Rig::new();
    rig.program_all();
    let show = rig.show;
    let lanes = FamilyLanes::live();
    let mut live = Live::default();
    let first = live.accept(&rig, &lanes);
    let zoom_lane = lanes.optics().lane(OpticsFamily::Zoom);
    let zoom_continuity = zoom_lane.continuity(show.optics, ProgrammingOwner::Zoom);

    // Color on the wash and Zoom are both removed; Zoom is passive (on its own head and, as an
    // incidental row, on the Color target). Only Color is a genuine removal.
    rig.release(show.wash, ProgrammingOwner::Color);
    // TL-554: a static Color program is a Color adapter owner too; remove the underlay as well.
    rig.programmers.release_fixture_attribute(
        rig.session,
        show.wash,
        &ProgrammingOwner::Color.key(),
    );
    rig.release(show.optics, ProgrammingOwner::Zoom);
    // TL-560: a static typed Zoom is a Zoom adapter owner too (as Color since TL-554); remove the
    // underlay as well so Zoom is genuinely removed.
    assert!(rig.programmers.release_fixture_attribute(
        rig.session,
        show.optics,
        &ProgrammingOwner::Zoom.key(),
    ));
    let held = [
        requirement(show.optics, ProgrammingOwner::Zoom),
        requirement(show.wash, ProgrammingOwner::Zoom),
    ];
    let second = live.run(&rig, &lanes, Finish::Accept, &held);
    let second = second.output.unwrap().unwrap();
    let produced = |target, owner| row(&second.results, target, owner).is_some();
    assert!(
        !produced(show.optics, ProgrammingOwner::Zoom),
        "held: no sidecar"
    );
    assert!(!produced(show.wash, ProgrammingOwner::Color));
    assert_eq!(
        released(&lanes),
        [(show.wash, ProgrammingOwner::Color)],
        "a Zoom hold holds Zoom only, never the Color owner of the same target",
    );
    let [color] = <[_; 1]>::try_from(lanes.color().released()).unwrap();
    assert_eq!(color.last_token, first.token, "its last produced token");
    assert!(
        lanes
            .color()
            .continuity(show.wash, ProgrammingOwner::Color)
            .is_none()
    );
    assert_eq!(
        zoom_lane.continuity(show.optics, ProgrammingOwner::Zoom),
        zoom_continuity
    );
    assert_eq!(zoom_lane.last_accepted(), Some(second.token.clone()));
    for (target, owner) in [
        (show.mover, ProgrammingOwner::Position),
        (show.layer, ProgrammingOwner::Color),
        (show.optics, ProgrammingOwner::Focus),
    ] {
        assert!(produced(target, owner), "{owner:?} keeps producing");
    }

    // Without the hold, Zoom is a genuine removal too; Position and Focus are released with it.
    // A static Position is still fitted (static peers), so its underlay is released as well.
    rig.release(show.mover, ProgrammingOwner::Position);
    let position = ProgrammingOwner::Position.key();
    assert!(
        rig.programmers
            .release_fixture_attribute(rig.session, show.mover, &position)
    );
    rig.release(show.optics, ProgrammingOwner::Focus);
    let third = live.accept(&rig, &lanes);
    assert_eq!(
        released(&lanes),
        [
            (show.mover, ProgrammingOwner::Position),
            (show.optics, ProgrammingOwner::Focus),
            (show.optics, ProgrammingOwner::Zoom),
        ],
        "released owners per family, in lane order",
    );
    let zoom = zoom_lane.released();
    assert_eq!(
        zoom[0].last_token, first.token,
        "the held Zoom's solve token"
    );
    assert!(
        zoom_lane
            .continuity(show.optics, ProgrammingOwner::Zoom)
            .is_none()
    );
    assert_eq!(third.results.len(), 1, "only the Media layer remains");
}

#[test]
fn transition_routing_needs_the_owning_requirement_and_matching_endpoints() {
    use TransitionRequirement as R;
    let (position, color, beam) = (
        angles(10., 20.),
        program(&magenta()),
        zoom(10., OpeningConvention::Beam),
    );
    let (focus_a, focus_b) = (focus(0.2), focus(0.8));
    let table = [
        (
            R::LiveJointAngles,
            &position,
            &position,
            Some(PhysicalFamily::Position),
        ),
        (
            R::LiveTargetPoints,
            &position,
            &position,
            Some(PhysicalFamily::Position),
        ),
        (
            R::ColorAppearance,
            &color,
            &color,
            Some(PhysicalFamily::Color),
        ),
        (
            R::ZoomConvention,
            &beam,
            &beam,
            Some(PhysicalFamily::Optics),
        ),
        (R::ZoomConvention, &focus_a, &focus_b, None),
        (R::CompatibleOwners, &focus_a, &focus_b, None),
        (R::ColorAppearance, &beam, &beam, None),
        (R::ZoomConvention, &color, &color, None),
        (R::LiveJointAngles, &beam, &position, None),
        (R::MaterializedEndpoints, &position, &position, None),
    ];
    for (requirement, from, to, expected) in table {
        assert_eq!(
            transition_family(requirement, from, to),
            expected,
            "{requirement:?}"
        );
    }
}

#[test]
fn resolver_reaches_each_lane_by_owner_and_only_focus_zoom_reach_optics() {
    use TransitionRequirement as R;
    let rig = Rig::new();
    let show = rig.show;
    let lanes = FamilyLanes::live();
    let capture = rig.capture();
    let token = capture.frame_token();
    lanes.begin_frame(&token).unwrap();
    let mut scalar = rig.engine.prepare_static_family_frame(&capture, &[]);
    let geometry = rig
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
    let compiles = || {
        (
            lanes.position().adapter().counters().compiles,
            lanes.color().adapter().counters().lamp_routes,
            lanes.optics().counters().descriptor_compiles,
        )
    };
    let transition = FamilyExpressionOperation::Transition { progress: 0.5 };
    let resolve = |target, requirement, from: &AttributeValue, to: &AttributeValue| {
        lanes.resolve(frame, target, requirement, from, to, transition)
    };
    let (color, beam) = (program(&magenta()), zoom(10., OpeningConvention::Beam));
    // Passive rows touch no lane and keep their own requirement.
    for (target, requirement, from, to) in [
        (show.optics, R::ZoomConvention, focus(0.2), focus(0.8)),
        (show.optics, R::CompatibleOwners, focus(0.2), focus(0.8)),
        (show.optics, R::ColorAppearance, field(20.), beam.clone()),
        (show.wash, R::ZoomConvention, color.clone(), color.clone()),
    ] {
        let resolved = resolve(target, requirement, &from, &to);
        assert_eq!(resolved.err(), Some(TransitionError::Requires(requirement)));
    }
    assert_eq!(
        compiles(),
        (0, 0, 0),
        "a Normalized pair never reaches optics"
    );
    // Zoom reaches optics, whose fitter keeps a convention change passive.
    let resolved = resolve(show.optics, R::ZoomConvention, &field(20.), &beam);
    assert_eq!(
        resolved.err(),
        Some(TransitionError::Requires(R::ZoomConvention))
    );
    assert_eq!(
        compiles(),
        (0, 0, 1),
        "the Zoom lane compiled its descriptor"
    );
    let _ = resolve(show.wash, R::ColorAppearance, &color, &cyan());
    assert_eq!(
        compiles(),
        (0, 1, 1),
        "Color reached the routed Color lane only"
    );
    let _ = resolve(
        show.mover,
        R::LiveJointAngles,
        &angles(1., 2.),
        &angles(40., 10.),
    );
    assert_eq!(
        compiles(),
        (1, 1, 1),
        "Position reached the Position lane only"
    );
    // Adoption routes by the address owner.
    let address = |representation| DynamicValueAddress {
        representation,
        component: None,
    };
    let focus_address = address(DynamicFamilyRepresentation::Focus);
    let adopted = lanes.adopt(frame, show.optics, &focus(0.5), &focus_address);
    assert!(matches!(adopted, Err(TransitionError::Requires(_))));
    assert_eq!(
        compiles(),
        (1, 1, 2),
        "Focus adoption compiled the Focus lane"
    );
    let zoom_address = address(DynamicFamilyRepresentation::Zoom {
        convention: OpeningConvention::Field,
    });
    let adopted = lanes.adopt(frame, show.optics, &focus(0.5), &zoom_address);
    assert!(matches!(adopted, Err(TransitionError::Requires(_))));
    assert_eq!(
        compiles(),
        (1, 1, 2),
        "the Zoom lane reuses its own descriptor"
    );
    let color_address = address(DynamicFamilyRepresentation::SemanticColor {
        basis: light_dynamics::DynamicSemanticColorBasis::Whole,
    });
    let _ = lanes.adopt(frame, show.layer, &cyan(), &color_address);
    assert_eq!(compiles().1, 1, "Media routes do not count as lamp routes");
    assert_eq!(lanes.color().adapter().counters().media_routes, 1);
    lanes.abandon();
}
