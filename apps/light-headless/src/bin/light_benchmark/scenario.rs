use crate::light_benchmark::arguments::{ProfileConfig, ProtocolSelection};
use chrono::{TimeZone, Utc};
use light_core::{
    AttributeKey, AttributeValue, CueListId, FixtureId, ManualClock, SessionId, UserId,
};
use light_engine::{
    ContributionBatch, ContributionSample, ContributionSourceId, Engine, EnginePlaybackCommand,
    EngineSnapshot, PoolPlaybackAction,
};
use light_fixture::{
    ChannelBehavior, ChannelResolution, FixtureChannel, FixtureProfile, PatchedFixture, SplitPatch,
};
use light_output::{DeliveryMode, OutputRoute};
use light_playback::{
    AttributePhaser, Cue, CueList, CueListMode, CueTrigger, FlashReleaseMode, GroupCueChange,
    IntensityPriorityMode, Phaser, PhaserCurve, PhaserMode, PhaserStep, PlaybackButtonAction,
    PlaybackDefinition, PlaybackFaderMode, PlaybackTarget, RestartMode, WrapMode,
};
use light_programmer::{GroupDefinition, ProgrammerRegistry};
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc};
use uuid::Uuid;

pub const SLOTS_PER_UNIVERSE: u16 = 512;
pub const PROGRAMMER_ASSIGNMENT_DIVISOR: usize = 4;
pub const SAMPLED_ASSIGNMENT_DIVISOR: usize = 8;
pub const SAMPLED_BATCH_COUNT: usize = 4;
pub const GROUP_ID: &str = "benchmark.static-group";

#[derive(Clone, Debug, Serialize)]
pub struct FixtureInventoryEntry {
    pub manufacturer: String,
    pub name: String,
    pub mode: String,
    pub quantity: usize,
    pub footprint: u16,
    pub dmx_slots: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioFixtureInventory {
    pub scenario: &'static str,
    pub entries: Vec<FixtureInventoryEntry>,
    pub manufacturer_fixture_slots: usize,
    pub rgb_par_fill_slots: usize,
    pub total_slots: usize,
}

pub struct BenchmarkScenario {
    pub engine: Engine,
    pub clock: Arc<ManualClock>,
    pub logical_start: chrono::DateTime<Utc>,
    pub universes: u16,
    pub fixture_count: usize,
    pub fixture_footprint: Option<u16>,
    pub packet_count: usize,
    pub fixture_inventory: ScenarioFixtureInventory,
    pub(super) programmers: ProgrammerRegistry,
    pub(super) phaser_attribute: AttributeKey,
    pub(super) phaser_overlaps_static_or_programmer: bool,
    pub(super) programmer_assignment_fraction: &'static str,
}

impl BenchmarkScenario {
    pub fn build(
        config: ProfileConfig,
        protocol: ProtocolSelection,
        loopback_destination: Option<SocketAddr>,
    ) -> Result<Self, String> {
        let logical_start = Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .single()
            .expect("benchmark timestamp is valid");
        let clock = Arc::new(ManualClock::new(logical_start));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId(fixed_uuid(0x20, 1));
        programmers.start(session, UserId(fixed_uuid(0x21, 1)));

        let fixture_footprint = SLOTS_PER_UNIVERSE / config.fixtures_per_universe;
        let fixture_count =
            usize::from(config.universes) * usize::from(config.fixtures_per_universe);
        let fixture_ids = (1..=fixture_count)
            .map(|number| FixtureId(fixed_uuid(0x30, number as u64)))
            .collect::<Vec<_>>();
        let definition = packed_definition(fixture_footprint)?;
        let fixtures = fixture_ids
            .iter()
            .enumerate()
            .map(|(index, fixture_id)| {
                let fixture_index = index as u16;
                let universe = fixture_index / config.fixtures_per_universe + 1;
                let index_in_universe = fixture_index % config.fixtures_per_universe;
                packed_fixture(
                    *fixture_id,
                    index as u32 + 1,
                    universe,
                    index_in_universe * fixture_footprint + 1,
                    &definition,
                )
            })
            .collect::<Vec<_>>();
        let group = static_group(&fixture_ids, fixture_footprint);
        let (cue_list, playback) = playback(fixture_footprint);
        let routes = routes(config.universes, protocol, loopback_destination);
        let packet_count = routes.len();
        let engine = Engine::new(programmers.clone());
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: fixtures.into(),
                cue_lists: vec![cue_list].into(),
                playbacks: vec![playback].into(),
                routes: routes.into(),
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
            .map_err(|error| format!("activate benchmark playback: {error}"))?;
        programmers.set_many(
            session,
            programmer_assignments(&fixture_ids, fixture_footprint),
        );
        Ok(Self {
            engine,
            clock,
            logical_start,
            universes: config.universes,
            fixture_count,
            fixture_footprint: Some(fixture_footprint),
            packet_count,
            fixture_inventory: ScenarioFixtureInventory {
                scenario: "synthetic_equal_footprint",
                entries: vec![FixtureInventoryEntry {
                    manufacturer: "ToskLight Benchmark".into(),
                    name: format!("Fully packed {fixture_footprint}-slot fixture"),
                    mode: "Synthetic".into(),
                    quantity: fixture_count,
                    footprint: fixture_footprint,
                    dmx_slots: fixture_count * usize::from(fixture_footprint),
                }],
                manufacturer_fixture_slots: fixture_count * usize::from(fixture_footprint),
                rgb_par_fill_slots: 0,
                total_slots: fixture_count * usize::from(fixture_footprint),
            },
            programmers,
            phaser_attribute: slot_attribute(animated_slot(fixture_footprint)),
            phaser_overlaps_static_or_programmer: false,
            programmer_assignment_fraction: "1/4 of mapped slots",
        })
    }

    pub fn sampled_batches(&self, at: chrono::DateTime<Utc>) -> Vec<ContributionBatch> {
        sampled_batches(&self.engine, &self.programmers, at, &self.phaser_attribute)
    }
}

fn sampled_batches(
    engine: &Engine,
    programmers: &ProgrammerRegistry,
    at: chrono::DateTime<Utc>,
    phaser_attribute: &AttributeKey,
) -> Vec<ContributionBatch> {
    let mut buckets = (0..SAMPLED_BATCH_COUNT)
        .map(|_| Vec::new())
        .collect::<Vec<_>>();
    let mut index = 0_usize;
    for programmer in programmers.active() {
        let source = ContributionSourceId::programmer(programmer.id);
        for value in programmer
            .values
            .into_iter()
            .step_by(SAMPLED_ASSIGNMENT_DIVISOR)
        {
            buckets[index % SAMPLED_BATCH_COUNT]
                .push(ContributionSample::replacing(value, source.clone()));
            index += 1;
        }
    }
    for contribution in engine
        .playback_contributions_at(at)
        .into_iter()
        .filter(|contribution| contribution.value.attribute != *phaser_attribute)
        .step_by(SAMPLED_ASSIGNMENT_DIVISOR)
    {
        buckets[index % SAMPLED_BATCH_COUNT].push(ContributionSample::replacing_playback(
            contribution.value,
            contribution.source,
            contribution.sequence_master,
        ));
        index += 1;
    }
    buckets.into_iter().map(ContributionBatch::new).collect()
}

fn packed_definition(footprint: u16) -> Result<light_fixture::FixtureDefinition, String> {
    let mut profile = FixtureProfile::blank();
    profile.id = FixtureId(fixed_uuid(0x40, 1));
    profile.revision = 1;
    profile.manufacturer = "ToskLight Benchmark".into();
    profile.name = format!("Fully packed {footprint}-slot fixture");
    profile.short_name = format!("Packed{footprint}");
    let mode_id = {
        let mode = &mut profile.modes[0];
        mode.id = fixed_uuid(0x41, 1);
        mode.splits[0].footprint = footprint;
        mode.heads[0].id = fixed_uuid(0x42, 1);
        let head_id = mode.heads[0].id;
        mode.channels = (0..footprint)
            .map(|slot| FixtureChannel {
                id: fixed_uuid(0x43, u64::from(slot) + 1),
                head_id,
                split: 1,
                attribute: slot_attribute(slot),
                resolution: ChannelResolution::U8,
                secondary_slots: vec![],
                default_raw: 1,
                highlight_raw: u8::MAX.into(),
                physical_min: None,
                physical_max: None,
                unit: None,
                invert: false,
                snap: false,
                reacts_to_virtual_intensity: false,
                reacts_to_sequence_master: true,
                reacts_to_group_master: true,
                reacts_to_grand_master: true,
                behavior: ChannelBehavior::Controlled,
                functions: vec![],
            })
            .collect();
        mode.id
    };
    profile
        .resolved_definition(mode_id)
        .map_err(|error| error.to_string())
}

fn packed_fixture(
    fixture_id: FixtureId,
    fixture_number: u32,
    universe: u16,
    address: u16,
    definition: &light_fixture::FixtureDefinition,
) -> PatchedFixture {
    PatchedFixture {
        fixture_id,
        fixture_number: Some(fixture_number),
        virtual_fixture_number: None,
        name: format!("Packed fixture {fixture_number}"),
        definition: definition.clone(),
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
        logical_heads: vec![],
        multipatch: vec![],
        move_in_black_enabled: false,
        move_in_black_delay_millis: 0,
        highlight_overrides: Default::default(),
    }
}

fn static_group(fixtures: &[FixtureId], fixture_footprint: u16) -> GroupDefinition {
    GroupDefinition {
        id: GROUP_ID.into(),
        name: "Benchmark static Group".into(),
        fixtures: fixtures.to_vec(),
        programming: static_slots(fixture_footprint)
            .map(|slot| {
                (
                    slot_attribute(slot),
                    AttributeValue::Normalized(0.20 + f32::from(slot % 7) * 0.01),
                )
            })
            .collect(),
        master: 0.9,
        ..Default::default()
    }
}

fn playback(fixture_footprint: u16) -> (CueList, PlaybackDefinition) {
    let cue_list_id = CueListId(fixed_uuid(0x50, 1));
    let cue = Cue {
        id: fixed_uuid(0x51, 1),
        number: 1.0,
        name: "Overlapping static and animated values".into(),
        changes: vec![],
        fade_millis: 0,
        delay_millis: 0,
        trigger: CueTrigger::Manual,
        cue_only: false,
        group_changes: static_slots(fixture_footprint)
            .map(|slot| GroupCueChange {
                group_id: GROUP_ID.into(),
                attribute: slot_attribute(slot),
                value: Some(AttributeValue::Normalized(
                    0.50 + f32::from(slot % 11) * 0.01,
                )),
                automatic_restore: false,
                fade_millis: None,
                delay_millis: None,
            })
            .collect(),
        phasers: vec![AttributePhaser {
            fixture_ids: vec![],
            group_ids: vec![GROUP_ID.into()],
            attribute: slot_attribute(animated_slot(fixture_footprint)),
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
        name: "Benchmark playback".into(),
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
        name: "Benchmark playback".into(),
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

fn programmer_assignments(
    fixtures: &[FixtureId],
    fixture_footprint: u16,
) -> impl Iterator<Item = (FixtureId, AttributeKey, AttributeValue)> + '_ {
    let animated_slot = animated_slot(fixture_footprint);
    fixtures
        .iter()
        .enumerate()
        .flat_map(move |(fixture_index, fixture_id)| {
            (0..fixture_footprint)
                .filter(move |slot| {
                    *slot != animated_slot
                        && (fixture_index + usize::from(*slot)) % PROGRAMMER_ASSIGNMENT_DIVISOR == 0
                })
                .map(move |slot| {
                    (
                        *fixture_id,
                        slot_attribute(slot),
                        AttributeValue::Normalized(0.9),
                    )
                })
        })
}

fn static_slots(fixture_footprint: u16) -> impl Iterator<Item = u16> {
    let animated_slot = animated_slot(fixture_footprint);
    (0..fixture_footprint).filter(move |slot| *slot != animated_slot)
}

pub(super) const fn animated_slot(fixture_footprint: u16) -> u16 {
    fixture_footprint - 1
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

pub fn slot_attribute(slot: u16) -> AttributeKey {
    AttributeKey(format!("benchmark.slot.{slot:03}"))
}

fn fixed_uuid(namespace: u64, value: u64) -> Uuid {
    Uuid::from_u128((u128::from(namespace) << 64) | u128::from(value))
}

#[cfg(test)]
#[path = "scenario_tests.rs"]
mod tests;
