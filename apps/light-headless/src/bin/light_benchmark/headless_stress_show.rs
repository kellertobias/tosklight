use crate::light_benchmark::{
    arguments::{ProfileConfig, ProtocolSelection},
    scenario::{BenchmarkDynamic, BenchmarkScenario, ScenarioFixtureInventory},
    sustained_show::{
        FixtureTemplate, benchmark_start, demo_group, demo_playback, fixed_uuid,
        load_templates_with, patched_fixture, routes,
    },
};
use crate::light_benchmark::{
    semantic_programming::{
        DynamicStart, SEMANTIC_LANE_ATTRIBUTES, SemanticBuild, live_bench, set_semantic_bases,
        start_dynamics, stress_starts,
    },
    semantic_runner::{LiveScenario, LiveWorkloadDescription, PendingStart},
    semantic_workload::OMITTED,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, ManualClock, SessionId};
use light_engine::{Engine, EnginePlaybackCommand, EngineSnapshot, PoolPlaybackAction};
use light_programmer::ProgrammerRegistry;
use std::{net::SocketAddr, path::Path, sync::Arc};

/// 650 is the TL-639 round 8 cap: 24 universes (12,259 of 12,288 parameters).
pub(super) const SUPPORTED_FIXTURE_COUNTS: [usize; 4] = [650, 1_000, 2_000, 4_000];
const BASE_MANIFEST: [StressTemplate; 5] = [
    StressTemplate::Dls,
    StressTemplate::LedWash,
    StressTemplate::Sunstrip,
    StressTemplate::LedBeam,
    StressTemplate::Dimmer,
];

/// The Keyframes variant, whose default activation is Start Now (the PWM variant joins its
/// Speed Group). Its phase spread moves most targets off their base on the first sample.
const PROBE_VARIANT: usize = 0;

#[derive(Clone, Copy)]
enum StressTemplate {
    Dls,
    LedWash,
    Sunstrip,
    LedBeam,
    Dimmer,
}

impl StressTemplate {
    const fn base_quantity(self) -> usize {
        match self {
            Self::Dls => 280,
            Self::LedWash => 360,
            Self::Sunstrip => 80,
            Self::LedBeam => 360,
            Self::Dimmer => 920,
        }
    }

    const fn dynamic(self) -> bool {
        !matches!(self, Self::Dimmer)
    }
}

struct Templates {
    dls: Arc<FixtureTemplate>,
    ledwash: Arc<FixtureTemplate>,
    sunstrip: Arc<FixtureTemplate>,
    ledbeam: Arc<FixtureTemplate>,
    dimmer: Arc<FixtureTemplate>,
}

impl Templates {
    fn load(package_dir: &Path, semantic: bool) -> Result<Self, String> {
        let shipped = load_templates_with(package_dir, semantic)?;
        Ok(Self {
            dls: shipped.dls,
            ledwash: shipped.ledwash,
            sunstrip: shipped.sunstrip,
            ledbeam: shipped.ledbeam,
            dimmer: Arc::new(FixtureTemplate::load_with(
                package_dir,
                "Generic",
                "Dimmer",
                "8-bit",
                "generic--dimmer.toskfixture",
                semantic,
            )?),
        })
    }

    fn get(&self, kind: StressTemplate) -> &Arc<FixtureTemplate> {
        match kind {
            StressTemplate::Dls => &self.dls,
            StressTemplate::LedWash => &self.ledwash,
            StressTemplate::Sunstrip => &self.sunstrip,
            StressTemplate::LedBeam => &self.ledbeam,
            StressTemplate::Dimmer => &self.dimmer,
        }
    }
}

struct StressLayout {
    fixtures: Vec<light_fixture::PatchedFixture>,
    dynamic_targets: Vec<FixtureId>,
    static_fixture_ids: Vec<FixtureId>,
    expected_patched_slots: std::collections::HashMap<u16, u16>,
    inventory: ScenarioFixtureInventory,
}

pub(super) fn build(
    fixture_count: usize,
    mut config: ProfileConfig,
    protocol: ProtocolSelection,
    loopback_destination: Option<SocketAddr>,
    package_dir: &Path,
    semantic: Option<SemanticBuild>,
) -> Result<BenchmarkScenario, String> {
    let layout = prepare_layout(fixture_count, package_dir, semantic.is_some())?;
    let animated_attribute_count = layout.dynamic_targets.len() * 6;
    config.universes = u16::try_from(layout.expected_patched_slots.len())
        .map_err(|_| "headless stress universe count exceeds u16".to_owned())?;
    let logical_start = benchmark_start();
    let clock = Arc::new(ManualClock::new(logical_start));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId(fixed_uuid(0x90, 1));
    programmers.start(session);
    let group_ids = layout
        .fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let group = demo_group(&group_ids);
    let (cue_list, playback) = demo_playback();
    let output_routes = routes(config.universes, protocol, loopback_destination);
    let packet_count = output_routes.len();
    let starts = semantic
        .map(|_| stress_starts(&layout.dynamic_targets, 20))
        .transpose()?;
    let engine = Arc::new(Engine::new(programmers.clone()));
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: layout.fixtures.into(),
            dynamics: starts
                .iter()
                .flatten()
                .map(|start| start.definition.clone())
                .collect::<Vec<_>>()
                .into(),
            cue_lists: vec![cue_list].into(),
            playbacks: vec![playback].into(),
            routes: output_routes.into(),
            groups: vec![group].into(),
            revision: 1,
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
    engine
        .execute_playback(EnginePlaybackCommand::Pool {
            number: 1,
            action: PoolPlaybackAction::Go,
        })
        .map_err(|error| format!("activate headless stress playback: {error}"))?;
    programmers.set_many(
        session,
        layout.static_fixture_ids.iter().map(|fixture_id| {
            (
                *fixture_id,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.55),
            )
        }),
    );
    let (dynamic, live) = match (semantic, starts) {
        (Some(options), Some(starts)) => (
            None,
            Some(semantic_live(
                &engine,
                &programmers,
                session,
                options,
                starts,
                (&layout.dynamic_targets, &layout.static_fixture_ids),
            )?),
        ),
        _ => (
            Some(BenchmarkDynamic::production(
                &layout.dynamic_targets,
                logical_start,
                20,
            )?),
            None,
        ),
    };
    Ok(BenchmarkScenario {
        engine,
        clock,
        logical_start,
        universes: config.universes,
        fixture_count,
        fixture_footprint: None,
        packet_count,
        fixture_inventory: layout.inventory,
        expected_patched_slots: layout.expected_patched_slots,
        workload_tier: "headless_stress",
        physical_instance_count: fixture_count,
        dynamic_definition_count: 20,
        animated_attribute_count,
        dynamic_lane_attributes: if live.is_some() {
            SEMANTIC_LANE_ATTRIBUTES
        } else {
            &[
                "intensity",
                "color.red",
                "color.green",
                "color.blue",
                "pan",
                "tilt",
            ]
        },
        dynamic_excluded_fixture_count: layout.static_fixture_ids.len(),
        active_ui_surfaces: &[],
        visualization_enabled: false,
        release_blocking: false,
        programmers,
        dynamic_attribute: AttributeKey::intensity(),
        dynamic_overlaps_static_or_programmer: false,
        programmer_assignment_fraction: "fixed-dimmer control population only",
        dynamic,
        live,
    })
}

/// The typed-lane Live lane: semantic bases, then the Dynamics started (or left to the probe).
fn semantic_live(
    engine: &Arc<Engine>,
    programmers: &ProgrammerRegistry,
    session: SessionId,
    options: SemanticBuild,
    starts: Vec<DynamicStart>,
    (dynamic_targets, static_fixture_ids): (&[FixtureId], &[FixtureId]),
) -> Result<LiveScenario, String> {
    set_semantic_bases(programmers, session, dynamic_targets)?;
    let pending_start = if options.defer_starts {
        Some(probe_start(
            session,
            &starts,
            dynamic_targets,
            static_fixture_ids,
        ))
    } else {
        start_dynamics(programmers, session, &starts)?;
        None
    };
    let description = LiveWorkloadDescription {
        kind: "headless_stress_typed_lanes",
        typed_lane_attributes: SEMANTIC_LANE_ATTRIBUTES
            .iter()
            .map(|lane| (*lane).to_owned())
            .collect(),
        semantic_base_targets: dynamic_targets.len(),
        started_dynamics: starts.len(),
        animated_targets: dynamic_targets.len(),
        manifest_sha256: None,
        workload_id: None,
        expected_dirty_targets: None,
        expected_moving_points: None,
        harness_rig_height_mm: None,
        omitted_from_live_transaction: OMITTED,
    };
    let bench = live_bench(
        Arc::clone(engine),
        starts.into_iter().map(|start| start.definition),
        options.rate_hz,
        options.publish,
    )?;
    Ok(LiveScenario {
        bench,
        tracking: None,
        description,
        pending_start,
    })
}

/// TL-641: the start-latency probe starts one Dynamic itself, on every fixture and head.
fn probe_start(
    session: SessionId,
    starts: &[DynamicStart],
    dynamic_targets: &[FixtureId],
    static_fixture_ids: &[FixtureId],
) -> PendingStart {
    PendingStart {
        session,
        definition: starts[PROBE_VARIANT].definition.clone(),
        targets: dynamic_targets
            .iter()
            .chain(static_fixture_ids)
            .copied()
            .collect(),
    }
}

fn prepare_layout(
    fixture_count: usize,
    package_dir: &Path,
    semantic: bool,
) -> Result<StressLayout, String> {
    if !SUPPORTED_FIXTURE_COUNTS.contains(&fixture_count) {
        return Err("headless stress fixtures must be exactly 650, 1000, 2000 or 4000".into());
    }
    // The 2,000-fixture manifest, halved or doubled; every base quantity is even.
    let quantity = |kind: StressTemplate| kind.base_quantity() * fixture_count / 2_000;
    let templates = Templates::load(package_dir, semantic)?;
    let mut placements = Vec::with_capacity(fixture_count);
    let mut universe_slots = Vec::<u16>::new();
    for kind in BASE_MANIFEST {
        let template = templates.get(kind);
        for _ in 0..quantity(kind) {
            let footprint = template.footprint();
            let universe_index = universe_slots
                .iter()
                .position(|used| 512 - *used >= footprint)
                .unwrap_or_else(|| {
                    universe_slots.push(0);
                    universe_slots.len() - 1
                });
            let address = universe_slots[universe_index] + 1;
            universe_slots[universe_index] += footprint;
            placements.push((
                kind,
                Arc::clone(template),
                universe_index as u16 + 1,
                address,
            ));
        }
    }
    if placements.len() != fixture_count {
        return Err(format!(
            "headless stress manifest produced {} fixtures instead of {fixture_count}",
            placements.len()
        ));
    }
    let mut fixtures = Vec::with_capacity(fixture_count);
    let mut dynamic_targets = Vec::new();
    let mut static_fixture_ids = Vec::new();
    for (index, (kind, template, universe, address)) in placements.into_iter().enumerate() {
        let fixture_number = index as u32 + 1;
        let fixture = patched_fixture(
            FixtureId(fixed_uuid(0x92, fixture_number.into())),
            fixture_number,
            universe,
            address,
            &template,
        );
        let targets = std::iter::once(fixture.fixture_id)
            .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
            .collect::<Vec<_>>();
        if kind.dynamic() {
            dynamic_targets.extend(targets);
        } else {
            static_fixture_ids.push(fixture.fixture_id);
        }
        fixtures.push(fixture);
    }
    let expected_patched_slots = universe_slots
        .iter()
        .enumerate()
        .map(|(index, slots)| (index as u16 + 1, *slots))
        .collect();
    let entries = BASE_MANIFEST
        .iter()
        .map(|kind| templates.get(*kind).inventory(quantity(*kind)))
        .collect::<Vec<_>>();
    let total_slots = entries.iter().map(|entry| entry.dmx_slots).sum();
    Ok(StressLayout {
        fixtures,
        dynamic_targets,
        static_fixture_ids,
        expected_patched_slots,
        inventory: ScenarioFixtureInventory {
            scenario: "mixed_shipped_mode_headless_stress",
            entries,
            manufacturer_fixture_slots: total_slots,
            rgb_par_fill_slots: 0,
            total_slots,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::light_benchmark::arguments::BenchmarkProfile;

    #[test]
    fn the_semantic_variant_keeps_the_tier_and_renders_through_the_live_transaction() {
        let package_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixture-library");
        let scenario = build(
            2_000,
            BenchmarkProfile::HeadlessStress.config(),
            ProtocolSelection::ArtNet,
            None,
            &package_dir,
            Some(SemanticBuild {
                rate_hz: 60,
                publish: true,
                defer_starts: false,
            }),
        )
        .unwrap();
        assert_eq!(scenario.fixture_count, 2_000);
        assert_eq!(scenario.universes, 74);
        assert_eq!(scenario.dynamic_definition_count, 20);
        assert!(scenario.dynamic.is_none());
        let live = scenario.live.as_ref().expect("semantic scenario");
        assert!(live.bench.family_engaged());
        assert_eq!(live.description.started_dynamics, 20);
        assert_eq!(scenario.dynamic_lane_attributes, SEMANTIC_LANE_ATTRIBUTES);
    }

    #[test]
    fn shipped_profiles_build_the_exact_headless_capacity_tiers() {
        let package_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixture-library");
        for (fixture_count, universes, final_slot, slots) in [
            (650, 24, 483, 12_259),
            (2_000, 74, 344, 37_720),
            (4_000, 148, 176, 75_440),
        ] {
            let scenario = build(
                fixture_count,
                BenchmarkProfile::HeadlessStress.config(),
                ProtocolSelection::ArtNet,
                None,
                &package_dir,
                None,
            )
            .unwrap();
            assert_eq!(scenario.fixture_count, fixture_count);
            assert_eq!(scenario.universes, universes);
            assert_eq!(scenario.fixture_inventory.total_slots, slots);
            assert_eq!(scenario.expected_patched_slots[&universes], final_slot);
            assert!(
                (1..universes).all(|universe| scenario.expected_patched_slots[&universe] == 512)
            );
            assert_eq!(scenario.dynamic_definition_count, 20);
            assert_eq!(
                scenario.dynamic_excluded_fixture_count,
                fixture_count * 920 / 2_000
            );
            assert_eq!(
                scenario.dynamic_lane_attributes,
                [
                    "intensity",
                    "color.red",
                    "color.green",
                    "color.blue",
                    "pan",
                    "tilt"
                ]
            );
            assert!(scenario.active_ui_surfaces.is_empty());
            assert!(!scenario.visualization_enabled);
            assert!(!scenario.release_blocking);
        }
    }
}
