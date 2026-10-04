//! TL-548 C3: the gated all-family Live path through the real `dynamic_output_frame`, the
//! boundary both Live call sites share. A mixed show (a calibrated mover, an RGB wash, a Media
//! layer, a Focus/Zoom wash and a plain dimmer running an Intensity Dynamic) is rendered with the
//! family adapters opted in and, for parity, through the legacy path. `desk.rs` covers the
//! AppState paths (`render_test_tick`, show activation, the default test state).
use super::*;
use crate::runtime::DynamicSnapshotPublication;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::{
    color::profiles::{patched, rgb},
    color::tests::{intent, magenta, program},
    media_color::tests::{media_fixture, shipped_media_server},
    optics::profiles::{OpticsBuilder, wash_a},
    optics::tests::{field, focus},
    position::tests::{angles, moving_head},
};
use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::HybridFrameResolver;
use light_core::programming::{PROGRAMMING_CONTRACT_VERSION, ProgrammingOwner};
use light_core::{ManualClock, SessionId};
use light_engine::RenderResult;
use light_fixture::PatchedFixture;
use light_programmer::DynamicProgrammerValueMutation;

mod color_report;
mod derived_coverage;
mod desk;
mod fixat_without_underlay;
mod rejection;
mod static_values;

/// Every fixture sits alone on its own universe (1..=4 and 6).
#[derive(Clone, Copy)]
pub(super) struct Show {
    pub mover: FixtureId,
    pub wash: FixtureId,
    pub media: FixtureId,
    pub layer: FixtureId,
    pub optics: FixtureId,
    pub dimmer: FixtureId,
}

impl Show {
    pub fn new() -> Self {
        Self {
            mover: FixtureId::new(),
            wash: FixtureId::new(),
            media: FixtureId::new(),
            layer: FixtureId::new(),
            optics: FixtureId::new(),
            dimmer: FixtureId::new(),
        }
    }

    pub fn fixtures(&self) -> Vec<PatchedFixture> {
        let on = |mut fixture: PatchedFixture, universe| {
            fixture.universe = Some(universe);
            fixture
        };
        let media = media_fixture(
            &shipped_media_server(),
            self.media,
            &[self.layer, FixtureId::new()],
        );
        let dimmer = OpticsBuilder::new("TL-548 C3 dimmer").build();
        let mut fixtures = vec![
            on(patched(&moving_head(), self.mover, 1), 1),
            on(patched(&rgb(), self.wash, 1), 2),
            on(media, 3),
            on(patched(&wash_a(), self.optics, 1), 4),
            on(patched(&dimmer, self.dimmer, 1), 6),
        ];
        for (number, fixture) in (1..).zip(&mut fixtures) {
            fixture.fixture_number = Some(number);
        }
        fixtures
    }

    /// Every family owner the show programs.
    pub fn owners(&self) -> Vec<(ProgrammingOwner, FixtureId)> {
        let mut owners = vec![
            (ProgrammingOwner::Position, self.mover),
            (ProgrammingOwner::Color, self.wash),
            (ProgrammingOwner::Color, self.layer),
            (ProgrammingOwner::Focus, self.optics),
            (ProgrammingOwner::Zoom, self.optics),
        ];
        owners.sort_by_key(|(owner, target)| (owner.key().0.to_string(), target.0));
        owners
    }
}

/// A whole-family FixAT over a static underlay of the same owner.
pub(super) fn fix(
    programmers: &ProgrammerRegistry,
    clock: &ManualClock,
    session: SessionId,
    target: FixtureId,
    owner: ProgrammingOwner,
    underlay: AttributeValue,
    value: AttributeValue,
) {
    // A frozen ManualClock would otherwise make successive edits rank ties.
    clock.advance_millis(10);
    programmers.set(session, target, owner.key(), underlay);
    assert!(
        programmers.apply_dynamic_values(
            session,
            &[DynamicProgrammerValueMutation::Set {
                fixture_id: target,
                attribute: owner.key(),
                value: DynamicSemanticValue::ProgrammingFixAt {
                    mask: light_dynamics::ProgrammingFamilyFixAt::from_family(owner, None, value)
                        .unwrap(),
                    timing: Default::default(),
                },
            }],
            None,
        )
    );
}

/// Program every family plus non-family Intensity: a scalar on the Focus/Zoom wash and the
/// Intensity Dynamic `definition` on the dimmer.
pub(super) fn program_show(
    programmers: &ProgrammerRegistry,
    clock: &ManualClock,
    session: SessionId,
    show: &Show,
    definition: Option<&DynamicDefinition>,
) {
    let cyan = || program(&intent([0., 1., 1.], 0.));
    let fix = |target, owner, underlay, value| {
        fix(programmers, clock, session, target, owner, underlay, value)
    };
    fix(
        show.mover,
        ProgrammingOwner::Position,
        angles(10., 20.),
        angles(450., 30.),
    );
    fix(
        show.wash,
        ProgrammingOwner::Color,
        cyan(),
        program(&magenta()),
    );
    fix(
        show.layer,
        ProgrammingOwner::Color,
        cyan(),
        program(&magenta()),
    );
    fix(
        show.optics,
        ProgrammingOwner::Focus,
        focus(0.2),
        focus(0.75),
    );
    fix(show.optics, ProgrammingOwner::Zoom, field(30.), field(20.));
    programmers.set(
        session,
        show.optics,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    );
    if let Some(definition) = definition {
        start_dynamic(programmers, session, show.dimmer, definition);
    }
}

pub(super) fn start_dynamic(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    fixture: FixtureId,
    definition: &DynamicDefinition,
) {
    assert!(programmers.apply_dynamic_values(
        session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::from_u128(548),
                lane_id: definition.lanes[0].id,
                dynamic: DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: 1,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(definition.clone()),
                    },
                },
                overrides: DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: Default::default(),
            },
        }],
        None
    ));
}

/// One authoritative Live output boundary: Engine, Dynamics, publication and family lanes.
struct Bench {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    clock: Arc<ManualClock>,
    dynamics: Mutex<DynamicRuntime>,
    publication: DynamicSnapshotPublication,
    origins: SharedDynamicSourceOrigins,
    groups: Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: AtomicU16,
    cache: ProgrammerReconciliationCache,
    family: LiveFamilyAdapters,
}

/// One committed frame and whether the hybrid source produced it.
struct Rendered {
    rendered: RenderResult,
    hybrid: bool,
    committed: CommittedDynamicOutput<()>,
}

impl Bench {
    fn new(show: &Show, definition: &DynamicDefinition, contract: u16, opted_in: bool) -> Self {
        Self::with_fixtures(show.fixtures(), definition, contract, opted_in)
    }

    fn with_fixtures(
        fixtures: Vec<PatchedFixture>,
        definition: &DynamicDefinition,
        contract: u16,
        opted_in: bool,
    ) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        let engine = Engine::with_programming_contract_support(programmers.clone(), contract);
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: fixtures.into(),
                dynamics: vec![definition.clone()].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        let mut runtime = DynamicRuntime::with_programming_contract_support(contract);
        runtime.install_definitions([definition.clone()]).unwrap();
        Self {
            publication: DynamicSnapshotPublication::new(engine.snapshot()),
            engine,
            programmers,
            session,
            clock,
            dynamics: Mutex::new(runtime),
            origins: SharedDynamicSourceOrigins::default(),
            groups: Mutex::new(std::array::from_fn(|_| {
                light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
            })),
            rate: AtomicU16::new(40),
            cache: ProgrammerReconciliationCache::default(),
            family: LiveFamilyAdapters::new(opted_in),
        }
    }

    fn capture(&self) -> light_engine::PreparedOutputFrame {
        self.clock.advance_millis(25);
        self.engine.prepare_output_frame(Default::default())
    }

    fn render(&self, frame: &light_engine::PreparedOutputFrame) -> Result<Rendered, EngineError> {
        let mut rendered = None;
        let committed = dynamic_output_frame(
            &self.engine,
            frame,
            None,
            &[],
            &self.dynamics,
            &self.publication,
            &self.origins,
            &self.groups,
            &self.rate,
            &self.cache,
            &self.family,
            |source| {
                let (result, hybrid) = match source {
                    OutputRenderSource::Legacy(batches) => {
                        (self.engine.render_prepared(frame, batches)?, false)
                    }
                    OutputRenderSource::Hybrid(result) => (result, true),
                };
                rendered = Some((result, hybrid));
                Ok(())
            },
        )?;
        let (rendered, hybrid) = rendered.expect("a committed frame was rendered");
        Ok(Rendered {
            rendered,
            hybrid,
            committed,
        })
    }

    fn frame(&self) -> Rendered {
        self.render(&self.capture()).unwrap()
    }

    fn last_accepted(&self) -> [Option<light_engine::CapturedFrameToken>; 4] {
        self.family.with_lanes(|lanes| lanes.last_accepted())
    }
}

fn instance_raw(rendered: &RenderResult, instance: FixtureId, channel: u32) -> u32 {
    rendered
        .physical
        .instances
        .iter()
        .find(|output| output.instance_id == instance.0)
        .unwrap_or_else(|| panic!("physical instance {instance:?}"))
        .native_raw[channel as usize]
}

fn u16_at(bytes: &[u8], start: usize) -> u32 {
    u32::from(u16::from_be_bytes([bytes[start], bytes[start + 1]]))
}

#[test]
fn an_opted_in_mixed_frame_installs_every_family_in_the_bytes_under_one_frame_identity() {
    let (show, definition) = (Show::new(), definition());
    let bench = Bench::new(&show, &definition, PROGRAMMING_CONTRACT_VERSION, true);
    program_show(
        &bench.programmers,
        &bench.clock,
        bench.session,
        &show,
        Some(&definition),
    );
    let first = bench.frame();
    assert!(
        first.hybrid,
        "contract 1 plus the opt-in takes the family path"
    );
    let published = bench
        .family
        .take_published()
        .expect("a published family frame");
    let token = published.token.clone();
    // Frame identity: every lane accepted exactly this capture, which produced these bytes.
    assert_eq!(bench.last_accepted(), [0; 4].map(|_| Some(token.clone())));
    assert_eq!(token.generation(), first.rendered.generation);
    assert_eq!(token.sampled_at(), first.rendered.sampled_at);
    assert_eq!(
        token.show_revision(),
        first.rendered.source_snapshot.revision
    );
    let mut owners: Vec<_> = published
        .writes
        .iter()
        .map(|(owner, target, _)| (*owner, *target))
        .collect();
    owners.sort_by_key(|(owner, target)| (owner.key().0.to_string(), target.0));
    owners.dedup();
    assert_eq!(owners, show.owners(), "one frame carries every family");
    // Every fitted write is the native output of its own physical instance.
    for (owner, target, write) in &published.writes {
        assert_eq!(
            instance_raw(
                &first.rendered,
                write.slot.destination,
                write.slot.channel_index
            ),
            write.raw,
            "{owner:?} on {target:?}"
        );
    }
    let raw = |owner, channel| {
        published
            .writes
            .iter()
            .find(|(o, _, w)| *o == owner && w.slot.channel_index == channel)
            .map(|(_, _, w)| w.raw)
            .unwrap_or_else(|| panic!("{owner:?} write on channel {channel}"))
    };
    // Mover: U16 Pan then U16 Tilt. Wash A: Intensity, U16 Zoom, U8 Focus.
    let (mover, optics) = (&first.rendered.universes[&1], &first.rendered.universes[&4]);
    assert_eq!(u16_at(mover, 0), raw(ProgrammingOwner::Position, 0));
    assert_eq!(u16_at(mover, 2), raw(ProgrammingOwner::Position, 1));
    assert_eq!(u16_at(optics, 1), raw(ProgrammingOwner::Zoom, 1));
    assert_eq!(u32::from(optics[3]), raw(ProgrammingOwner::Focus, 2));
    assert_eq!(
        optics[0], 102,
        "the scalar Intensity 0.4 stays on the legacy scalar path"
    );
    // The next frame is a new identity and every lane advances to it.
    let second = bench.frame();
    assert!(second.hybrid);
    let next = bench.family.take_published().unwrap().token;
    assert!(next.sampled_at() > token.sampled_at());
    assert_eq!(bench.last_accepted(), [0; 4].map(|_| Some(next.clone())));
    assert_eq!(
        second.committed.samples.len(),
        first.committed.samples.len()
    );
}

#[test]
fn a_failed_family_frame_rolls_back_dynamics_catalogue_and_reconciliation_and_advances_no_lane() {
    let (show, definition) = (Show::new(), definition());
    let bench = Bench::new(&show, &definition, PROGRAMMING_CONTRACT_VERSION, true);
    program_show(
        &bench.programmers,
        &bench.clock,
        bench.session,
        &show,
        Some(&definition),
    );
    let accepted = bench.frame();
    assert!(accepted.hybrid);
    let accepted = bench.family.take_published().unwrap().token;
    // A later edit, then a frame whose final render rejects its stale continuity token.
    let (programmers, clock, session) = (&bench.programmers, &bench.clock, bench.session);
    let zoom = ProgrammingOwner::Zoom;
    fix(
        programmers,
        clock,
        session,
        show.optics,
        zoom,
        field(30.),
        field(25.),
    );
    let position = ProgrammingOwner::Position;
    fix(
        programmers,
        clock,
        session,
        show.mover,
        position,
        angles(10., 20.),
        angles(90., 45.),
    );
    let runtime = bench.dynamics.lock().snapshot();
    let origins = bench.origins.load_full();
    let boundary = bench.dynamics.lock().committed_sample_boundary();
    let frame = bench.capture();
    bench.engine.clear_programmer_transitions();
    let failed = bench.render(&frame);
    assert!(
        matches!(failed, Err(EngineError::StalePreparedFrame)),
        "the stale family frame keeps the legacy error contract"
    );
    assert!(
        bench.family.take_published().is_none(),
        "nothing was published"
    );
    assert_eq!(
        bench.dynamics.lock().snapshot(),
        runtime,
        "Dynamics rolled back"
    );
    assert_eq!(bench.dynamics.lock().committed_sample_boundary(), boundary);
    assert!(
        Arc::ptr_eq(&origins, &bench.origins.load_full()),
        "catalogue kept"
    );
    assert!(
        bench
            .cache
            .changed(frame.dynamic_programmer_values(), &frame.snapshot()),
        "reconciliation is not acknowledged"
    );
    assert_eq!(
        bench.last_accepted(),
        [0; 4].map(|_| Some(accepted.clone()))
    );
    let failed_token = frame.frame_token();
    bench.family.with_lanes(|lanes| {
        assert!(
            !lanes.accept_frame(&failed_token),
            "no lane still stages the failed frame"
        );
        for family in [
            light_fixture::OpticsFamily::Focus,
            light_fixture::OpticsFamily::Zoom,
        ] {
            assert!(!lanes.optics().lane(family).accept_frame(&failed_token));
        }
        assert!(!lanes.position().accept_frame(&failed_token));
        assert!(!lanes.color().accept_frame(&failed_token));
    });
    // The retry commits every lane together.
    let retry = bench.frame();
    assert!(retry.hybrid);
    let retried = bench.family.take_published().unwrap().token;
    assert_eq!(bench.last_accepted(), [0; 4].map(|_| Some(retried.clone())));
}

#[test]
fn non_family_attributes_are_byte_identical_to_the_legacy_path_on_the_same_show() {
    let (show, definition) = (Show::new(), definition());
    let hybrid = Bench::new(&show, &definition, PROGRAMMING_CONTRACT_VERSION, true);
    let legacy = Bench::new(&show, &definition, PROGRAMMING_CONTRACT_VERSION, false);
    for bench in [&hybrid, &legacy] {
        program_show(
            &bench.programmers,
            &bench.clock,
            bench.session,
            &show,
            Some(&definition),
        );
    }
    let mut dimmer_levels = Vec::new();
    for _ in 0..6 {
        let (a, b) = (hybrid.frame(), legacy.frame());
        assert!(a.hybrid && !b.hybrid);
        assert_eq!(a.rendered.sampled_at, b.rendered.sampled_at);
        // The Dynamic-driven dimmer universe and the scalar Intensity byte of Wash A.
        assert_eq!(
            a.rendered.universes[&6], b.rendered.universes[&6],
            "dimmer bytes"
        );
        assert_eq!(
            a.rendered.universes[&4][0], b.rendered.universes[&4][0],
            "Intensity"
        );
        let intensity = AttributeKey::intensity();
        for fixture in [show.dimmer, show.optics] {
            assert_eq!(
                a.rendered.resolved_values.value(fixture, &intensity),
                b.rendered.resolved_values.value(fixture, &intensity),
            );
        }
        let dimmer = |r: &Rendered| {
            let samples = r.committed.samples.iter();
            samples
                .filter(|sample| sample.target == show.dimmer)
                .count()
        };
        assert_eq!(
            (dimmer(&a), dimmer(&b)),
            (1, 1),
            "one scalar Dynamic sample each"
        );
        dimmer_levels.push(a.rendered.universes[&6][0]);
    }
    dimmer_levels.dedup();
    assert!(
        dimmer_levels.len() > 1,
        "the Dynamic moved: {dimmer_levels:?}"
    );
    // The family channels themselves are fitted natively on the hybrid path only.
    assert!(hybrid.family.take_published().is_some());
    assert!(legacy.family.take_published().is_none());
}

#[test]
fn contract_zero_with_the_opt_in_set_keeps_the_legacy_path() {
    let (show, definition) = (Show::new(), definition());
    let bench = Bench::new(&show, &definition, 0, true);
    start_dynamic(&bench.programmers, bench.session, show.dimmer, &definition);
    for _ in 0..3 {
        let frame = bench.capture();
        dynamic_output_frame(
            &bench.engine,
            &frame,
            None,
            &[],
            &bench.dynamics,
            &bench.publication,
            &bench.origins,
            &bench.groups,
            &bench.rate,
            &bench.cache,
            &bench.family,
            // Panics if the family path were taken.
            legacy(|batches| bench.engine.render_prepared(&frame, batches).map(|_| ())),
        )
        .unwrap();
    }
    assert!(!bench.family.engaged(&bench.engine));
    assert!(bench.family.take_published().is_none());
    assert_eq!(bench.last_accepted(), [None, None, None, None]);
}
