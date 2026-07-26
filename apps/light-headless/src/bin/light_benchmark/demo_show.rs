use crate::light_benchmark::{
    arguments::{ProfileConfig, ProtocolSelection},
    scenario::{
        BenchmarkScenario, FixtureInventoryEntry, GROUP_ID, SLOTS_PER_UNIVERSE,
        ScenarioFixtureInventory,
    },
};
use chrono::{TimeZone, Utc};
use light_core::{
    AttributeKey, AttributeValue, CueListId, FixtureId, ManualClock, SessionId, UserId,
};
use light_engine::{Engine, EnginePlaybackCommand, EngineSnapshot, PoolPlaybackAction};
use light_fixture::{
    FixtureDefinition, PatchedFixture, PatchedHead, SplitPatch, read_fixture_package,
};
use light_output::{DeliveryMode, OutputRoute};
use light_playback::{
    AttributePhaser, Cue, CueList, CueListMode, CueTrigger, FlashReleaseMode, GroupCueChange,
    IntensityPriorityMode, Phaser, PhaserCurve, PhaserMode, PhaserStep, PlaybackButtonAction,
    PlaybackDefinition, PlaybackFaderMode, PlaybackTarget, RestartMode, WrapMode,
};
use light_programmer::{GroupDefinition, ProgrammerRegistry};
use std::{fs, net::SocketAddr, path::Path, sync::Arc};
use uuid::Uuid;

const UNIVERSES: usize = 32;
const SUNSTRIP_QUANTITY: usize = 20;
const LEDWASH_QUANTITY: usize = 40;
const DLS_QUANTITY: usize = 32;
const LEDBEAM_QUANTITY: usize = 32;

struct FixtureTemplate {
    manufacturer: &'static str,
    name: &'static str,
    mode: &'static str,
    definition: Arc<FixtureDefinition>,
}

struct DemoTemplates {
    sunstrip: Arc<FixtureTemplate>,
    ledwash: Arc<FixtureTemplate>,
    dls: Arc<FixtureTemplate>,
    ledbeam: Arc<FixtureTemplate>,
    rgb_three: Arc<FixtureTemplate>,
    rgb_four: Arc<FixtureTemplate>,
}

impl FixtureTemplate {
    fn load(
        package_dir: &Path,
        manufacturer: &'static str,
        name: &'static str,
        mode: &'static str,
        package: &'static str,
    ) -> Result<Self, String> {
        let path = package_dir.join(package);
        let bytes = fs::read(&path)
            .map_err(|error| format!("read fixture package {}: {error}", path.display()))?;
        let mut profile = read_fixture_package(&bytes)
            .map_err(|error| format!("load fixture package {}: {error}", path.display()))?;
        if profile.manufacturer != manufacturer || profile.name != name {
            return Err(format!(
                "{} contains {} {} instead of {manufacturer} {name}",
                path.display(),
                profile.manufacturer,
                profile.name
            ));
        }
        profile.photograph_asset = None;
        profile.stage_icon_asset = None;
        profile.model_asset = None;
        let mode_id = profile
            .modes
            .iter()
            .find(|candidate| candidate.name == mode)
            .map(|candidate| candidate.id)
            .ok_or_else(|| format!("{} has no mode named {mode}", path.display()))?;
        let definition = profile
            .resolved_definition(mode_id)
            .map_err(|error| format!("resolve {manufacturer} {name} {mode}: {error}"))?;
        Ok(Self {
            manufacturer,
            name,
            mode,
            definition: Arc::new(definition),
        })
    }

    fn footprint(&self) -> u16 {
        self.definition.footprint
    }

    fn inventory(&self, quantity: usize) -> FixtureInventoryEntry {
        FixtureInventoryEntry {
            manufacturer: self.manufacturer.into(),
            name: self.name.into(),
            mode: self.mode.into(),
            quantity,
            footprint: self.footprint(),
            dmx_slots: quantity * usize::from(self.footprint()),
        }
    }
}

fn load_templates(package_dir: &Path) -> Result<DemoTemplates, String> {
    Ok(DemoTemplates {
        sunstrip: Arc::new(FixtureTemplate::load(
            package_dir,
            "Showtec",
            "Sunstrip LED RGB 42206",
            "30 Channel",
            "showtec--sunstrip-led-rgb-42206.toskfixture",
        )?),
        ledwash: Arc::new(FixtureTemplate::load(
            package_dir,
            "ROBE",
            "Robin 600X LEDWash",
            "Mode 1",
            "robe--robin-600x-ledwash.toskfixture",
        )?),
        dls: Arc::new(FixtureTemplate::load(
            package_dir,
            "ROBE",
            "Robin DLS Profile",
            "Mode 1",
            "robe--robin-dls-profile.toskfixture",
        )?),
        ledbeam: Arc::new(FixtureTemplate::load(
            package_dir,
            "ROBE",
            "Robin LEDBeam 150",
            "Mode 1 – Standard 16-bit",
            "robe--robin-ledbeam-150.toskfixture",
        )?),
        rgb_three: Arc::new(FixtureTemplate::load(
            package_dir,
            "Generic",
            "RGB LED",
            "RGB virtual dimmer",
            "generic--rgb-led.toskfixture",
        )?),
        rgb_four: Arc::new(FixtureTemplate::load(
            package_dir,
            "Generic",
            "RGB LED",
            "RGBD 8-bit dimmer last",
            "generic--rgb-led.toskfixture",
        )?),
    })
}

pub fn build(
    config: ProfileConfig,
    protocol: ProtocolSelection,
    loopback_destination: Option<SocketAddr>,
    package_dir: &Path,
) -> Result<BenchmarkScenario, String> {
    if config.universes != UNIVERSES as u16 {
        return Err("the sustained demo show requires the 32-universe hard-floor profile".into());
    }
    let templates = load_templates(package_dir)?;
    let DemoTemplates {
        sunstrip,
        ledwash,
        dls,
        ledbeam,
        rgb_three,
        rgb_four,
    } = templates;

    let mut universes = (0..UNIVERSES)
        .map(|_| Vec::<Arc<FixtureTemplate>>::new())
        .collect::<Vec<_>>();
    distribute(&mut universes, Arc::clone(&sunstrip), SUNSTRIP_QUANTITY);
    distribute(&mut universes, Arc::clone(&ledwash), LEDWASH_QUANTITY);
    distribute(&mut universes, Arc::clone(&dls), DLS_QUANTITY);
    distribute(&mut universes, Arc::clone(&ledbeam), LEDBEAM_QUANTITY);

    let manufacturer_fixture_slots = universes
        .iter()
        .flatten()
        .map(|fixture| usize::from(fixture.footprint()))
        .sum::<usize>();
    let mut rgb_three_count = 0_usize;
    let mut rgb_four_count = 0_usize;
    for universe in &mut universes {
        let used = used_slots(universe);
        let remaining = usize::from(SLOTS_PER_UNIVERSE)
            .checked_sub(used)
            .ok_or_else(|| format!("demo show overfilled a universe with {used} slots"))?;
        let (three_count, four_count) = rgb_fill_counts(remaining)?;
        rgb_three_count += three_count;
        rgb_four_count += four_count;
        universe.extend(std::iter::repeat_n(Arc::clone(&rgb_three), three_count));
        universe.extend(std::iter::repeat_n(Arc::clone(&rgb_four), four_count));
        if used_slots(universe) != usize::from(SLOTS_PER_UNIVERSE) {
            return Err("RGB PAR fill did not complete a universe".into());
        }
    }

    let logical_start = benchmark_start();
    let clock = Arc::new(ManualClock::new(logical_start));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId(fixed_uuid(0x80, 1));
    programmers.start(session, UserId(fixed_uuid(0x81, 1)));
    let mut fixture_number = 0_u32;
    let mut fixtures = Vec::new();
    for (universe_index, templates) in universes.iter().enumerate() {
        let universe = universe_index as u16 + 1;
        let mut address = 1_u16;
        for template in templates {
            fixture_number += 1;
            let fixture_id = FixtureId(fixed_uuid(0x82, u64::from(fixture_number)));
            fixtures.push(patched_fixture(
                fixture_id,
                fixture_number,
                universe,
                address,
                template,
            ));
            address += template.footprint();
        }
        if address != SLOTS_PER_UNIVERSE + 1 {
            return Err(format!(
                "demo show universe {universe} ended at address {address}"
            ));
        }
    }
    let fixture_ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let group = demo_group(&fixture_ids);
    let (cue_list, playback) = demo_playback();
    let output_routes = routes(config.universes, protocol, loopback_destination);
    let packet_count = output_routes.len();
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
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
        .map_err(|error| format!("activate demo-show playback: {error}"))?;
    programmers.set_many(
        session,
        fixture_ids
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 4 == 0)
            .map(|(_, fixture_id)| {
                (
                    *fixture_id,
                    AttributeKey::intensity(),
                    AttributeValue::Normalized(0.9),
                )
            }),
    );

    let rgb_par_fill_slots = rgb_three_count * usize::from(rgb_three.footprint())
        + rgb_four_count * usize::from(rgb_four.footprint());
    let total_slots = manufacturer_fixture_slots + rgb_par_fill_slots;
    let entries = vec![
        sunstrip.inventory(SUNSTRIP_QUANTITY),
        ledwash.inventory(LEDWASH_QUANTITY),
        dls.inventory(DLS_QUANTITY),
        ledbeam.inventory(LEDBEAM_QUANTITY),
        rgb_three.inventory(rgb_three_count),
        rgb_four.inventory(rgb_four_count),
    ];
    Ok(BenchmarkScenario {
        engine,
        clock,
        logical_start,
        universes: config.universes,
        fixture_count: fixture_ids.len(),
        fixture_footprint: None,
        packet_count,
        fixture_inventory: ScenarioFixtureInventory {
            scenario: "mixed_manufacturer_demo_show",
            entries,
            manufacturer_fixture_slots,
            rgb_par_fill_slots,
            total_slots,
        },
        programmers,
        phaser_attribute: AttributeKey::intensity(),
        phaser_overlaps_static_or_programmer: true,
        programmer_assignment_fraction: "1/4 of physical fixtures",
    })
}

fn benchmark_start() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
        .single()
        .expect("benchmark timestamp is valid")
}

fn distribute(
    universes: &mut [Vec<Arc<FixtureTemplate>>],
    fixture: Arc<FixtureTemplate>,
    quantity: usize,
) {
    for index in 0..quantity {
        universes[index % universes.len()].push(Arc::clone(&fixture));
    }
}

fn used_slots(fixtures: &[Arc<FixtureTemplate>]) -> usize {
    fixtures
        .iter()
        .map(|fixture| usize::from(fixture.footprint()))
        .sum()
}

fn rgb_fill_counts(remaining: usize) -> Result<(usize, usize), String> {
    for four_count in 0..=remaining / 4 {
        let after_four = remaining - four_count * 4;
        if after_four.is_multiple_of(3) {
            return Ok((after_four / 3, four_count));
        }
    }
    Err(format!(
        "{remaining} remaining slots cannot be filled with 3- and 4-channel RGB PARs"
    ))
}

fn patched_fixture(
    fixture_id: FixtureId,
    fixture_number: u32,
    universe: u16,
    address: u16,
    template: &FixtureTemplate,
) -> PatchedFixture {
    let logical_heads = template
        .definition
        .heads
        .iter()
        .filter(|head| !head.shared)
        .map(|head| PatchedHead {
            profile_head_id: None,
            head_index: head.index,
            fixture_id: FixtureId(fixed_uuid(
                0x85 + u64::from(head.index),
                u64::from(fixture_number),
            )),
        })
        .collect();
    PatchedFixture {
        fixture_id,
        fixture_number: Some(fixture_number),
        virtual_fixture_number: None,
        name: format!("{} {}", template.name, fixture_number),
        definition: template.definition.as_ref().clone(),
        universe: Some(universe),
        address: Some(address),
        split_patches: vec![SplitPatch {
            split: 1,
            universe: Some(universe),
            address: Some(address),
        }],
        layer_id: "default".into(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads,
        multipatch: vec![],
        move_in_black_enabled: false,
        move_in_black_delay_millis: 0,
        highlight_overrides: Default::default(),
    }
}

fn demo_group(fixtures: &[FixtureId]) -> GroupDefinition {
    GroupDefinition {
        id: GROUP_ID.into(),
        name: "Sustained demo-show fixtures".into(),
        fixtures: fixtures.to_vec(),
        programming: [(AttributeKey::intensity(), AttributeValue::Normalized(0.2))].into(),
        master: 0.9,
        ..Default::default()
    }
}

fn demo_playback() -> (CueList, PlaybackDefinition) {
    let cue_list_id = CueListId(fixed_uuid(0x83, 1));
    let cue = Cue {
        id: fixed_uuid(0x84, 1),
        number: 1.0,
        name: "Overlapping demo-show intensity and phaser".into(),
        changes: vec![],
        fade_millis: 0,
        delay_millis: 0,
        trigger: CueTrigger::Manual,
        cue_only: false,
        group_changes: vec![GroupCueChange {
            group_id: GROUP_ID.into(),
            attribute: AttributeKey::intensity(),
            value: Some(AttributeValue::Normalized(0.6)),
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        }],
        phasers: vec![AttributePhaser {
            fixture_ids: vec![],
            group_ids: vec![GROUP_ID.into()],
            attribute: AttributeKey::intensity(),
            phaser: Phaser {
                mode: PhaserMode::Absolute,
                steps: vec![
                    PhaserStep {
                        position: 0.0,
                        value: 0.1,
                        curve_to_next: PhaserCurve::Linear,
                    },
                    PhaserStep {
                        position: 0.5,
                        value: 0.9,
                        curve_to_next: PhaserCurve::Linear,
                    },
                ],
                cycles_per_minute: 600.0,
                phase_start_degrees: 0.0,
                phase_end_degrees: 360.0,
                width: 1.0,
            },
        }],
    };
    let cue_list = CueList {
        id: cue_list_id,
        name: "Sustained demo-show playback".into(),
        priority: 10,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Off),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![cue],
    };
    let playback = PlaybackDefinition {
        number: 1,
        name: "Sustained demo-show playback".into(),
        target: PlaybackTarget::CueList { cue_list_id },
        buttons: [
            PlaybackButtonAction::GoMinus,
            PlaybackButtonAction::Go,
            PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    (cue_list, playback)
}

fn routes(
    universes: u16,
    selection: ProtocolSelection,
    loopback_destination: Option<SocketAddr>,
) -> Vec<OutputRoute> {
    (1..=universes)
        .flat_map(|universe| {
            selection
                .protocols()
                .iter()
                .map(move |protocol| OutputRoute {
                    protocol: *protocol,
                    logical_universe: universe,
                    destination_universe: universe,
                    delivery_mode: loopback_destination.map(|_| DeliveryMode::Unicast),
                    destination: loopback_destination,
                    enabled: true,
                    minimum_slots: SLOTS_PER_UNIVERSE,
                })
        })
        .collect()
}

fn fixed_uuid(namespace: u64, value: u64) -> Uuid {
    Uuid::from_u128((u128::from(namespace) << 64) | u128::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_par_modes_fill_the_evenly_distributed_demo_show() {
        assert_eq!(rgb_fill_counts(339).unwrap(), (113, 0));
        assert_eq!(rgb_fill_counts(376).unwrap(), (124, 1));
        assert_eq!(rgb_fill_counts(406).unwrap(), (134, 1));
        assert!(rgb_fill_counts(5).is_err());
    }

    #[test]
    fn shipped_profiles_build_the_exact_fully_packed_demo_show() {
        let package_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixture-library");
        let scenario = build(
            crate::light_benchmark::arguments::BenchmarkProfile::HardFloor.config(),
            ProtocolSelection::ArtNet,
            None,
            &package_dir,
        )
        .unwrap();

        assert_eq!(scenario.fixture_count, 4_148);
        assert_eq!(scenario.fixture_inventory.manufacturer_fixture_slots, 4_288);
        assert_eq!(scenario.fixture_inventory.rgb_par_fill_slots, 12_096);
        assert_eq!(scenario.fixture_inventory.total_slots, 32 * 512);
        let quantities = scenario
            .fixture_inventory
            .entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.mode.as_str(), entry.quantity))
            .collect::<Vec<_>>();
        assert_eq!(
            quantities,
            vec![
                ("Sunstrip LED RGB 42206", "30 Channel", 20),
                ("Robin 600X LEDWash", "Mode 1", 40),
                ("Robin DLS Profile", "Mode 1", 32),
                ("Robin LEDBeam 150", "Mode 1 – Standard 16-bit", 32),
                ("RGB LED", "RGB virtual dimmer", 4_000),
                ("RGB LED", "RGBD 8-bit dimmer last", 24),
            ]
        );
    }
}
