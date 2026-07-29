use crate::light_benchmark::{
    arguments::{ProfileConfig, ProtocolSelection},
    scenario::{BenchmarkDynamic, BenchmarkScenario, ScenarioFixtureInventory},
    sustained_show::{
        FixtureTemplate, benchmark_start, demo_group, demo_playback, fixed_uuid, load_templates,
        patched_fixture, routes,
    },
};
use light_core::{AttributeKey, AttributeValue, FixtureId, ManualClock, SessionId, UserId};
use light_engine::{Engine, EnginePlaybackCommand, EngineSnapshot, PoolPlaybackAction};
use light_programmer::ProgrammerRegistry;
use std::{net::SocketAddr, path::Path, sync::Arc};

pub(super) const SUPPORTED_FIXTURE_COUNTS: [usize; 2] = [2_000, 4_000];
const BASE_MANIFEST: [StressTemplate; 5] = [
    StressTemplate::Dls,
    StressTemplate::LedWash,
    StressTemplate::Sunstrip,
    StressTemplate::LedBeam,
    StressTemplate::Dimmer,
];

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
    fn load(package_dir: &Path) -> Result<Self, String> {
        let shipped = load_templates(package_dir)?;
        Ok(Self {
            dls: shipped.dls,
            ledwash: shipped.ledwash,
            sunstrip: shipped.sunstrip,
            ledbeam: shipped.ledbeam,
            dimmer: Arc::new(FixtureTemplate::load(
                package_dir,
                "Generic",
                "Dimmer",
                "8-bit",
                "generic--dimmer.toskfixture",
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
) -> Result<BenchmarkScenario, String> {
    let layout = prepare_layout(fixture_count, package_dir)?;
    config.universes = u16::try_from(layout.expected_patched_slots.len())
        .map_err(|_| "headless stress universe count exceeds u16".to_owned())?;
    let logical_start = benchmark_start();
    let clock = Arc::new(ManualClock::new(logical_start));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId(fixed_uuid(0x90, 1));
    programmers.start(session, UserId(fixed_uuid(0x91, 1)));
    let group_ids = layout
        .fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let group = demo_group(&group_ids);
    let (cue_list, playback) = demo_playback();
    let output_routes = routes(config.universes, protocol, loopback_destination);
    let packet_count = output_routes.len();
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: layout.fixtures.into(),
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
    let dynamic = BenchmarkDynamic::production(&layout.dynamic_targets, logical_start, 20)?;
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
        dynamic_lane_attributes: &[
            "intensity",
            "color.red",
            "color.green",
            "color.blue",
            "pan",
            "tilt",
        ],
        dynamic_excluded_fixture_count: layout.static_fixture_ids.len(),
        active_ui_surfaces: &[],
        visualization_enabled: false,
        release_blocking: false,
        programmers,
        dynamic_attribute: AttributeKey::intensity(),
        dynamic_overlaps_static_or_programmer: false,
        programmer_assignment_fraction: "fixed-dimmer control population only",
        dynamic: Some(dynamic),
    })
}

fn prepare_layout(fixture_count: usize, package_dir: &Path) -> Result<StressLayout, String> {
    if !SUPPORTED_FIXTURE_COUNTS.contains(&fixture_count) {
        return Err("headless stress fixtures must be exactly 2000 or 4000".into());
    }
    let scale = fixture_count / 2_000;
    let templates = Templates::load(package_dir)?;
    let mut placements = Vec::with_capacity(fixture_count);
    let mut universe_slots = Vec::<u16>::new();
    for kind in BASE_MANIFEST {
        let template = templates.get(kind);
        for _ in 0..kind.base_quantity() * scale {
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
        .map(|kind| templates.get(*kind).inventory(kind.base_quantity() * scale))
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
    fn shipped_profiles_build_the_exact_headless_capacity_tiers() {
        let package_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixture-library");
        for (fixture_count, universes, final_slot, slots) in
            [(2_000, 74, 344, 37_720), (4_000, 148, 176, 75_440)]
        {
            let scenario = build(
                fixture_count,
                BenchmarkProfile::HeadlessStress.config(),
                ProtocolSelection::ArtNet,
                None,
                &package_dir,
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
