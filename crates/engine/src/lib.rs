#![forbid(unsafe_code)]
//! Deterministic bridge from fixture attributes and playbacks to immutable DMX universe frames.

use arc_swap::ArcSwap;
use light_core::{
    AttributeKey, AttributeValue, FixtureId, MergeMode, SharedClock, TimedValue, Universe,
};
use light_fixture::{
    PatchedFixture, SignalLossPolicy, encode_parameter, mix_color, validate_patch,
};
use light_output::{DmxFrame, OutputRoute};
use light_playback::{
    CueList, PlaybackDefinition, PlaybackEngine, PlaybackPage, PlaybackTarget, resolve,
};
use light_programmer::ProgrammerRegistry;
use light_programmer::{GroupDefinition, resolve_group};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU16, AtomicU64, Ordering},
    },
};
use thiserror::Error;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub fixtures: Vec<PatchedFixture>,
    pub cue_lists: Vec<CueList>,
    #[serde(default)]
    pub playbacks: Vec<PlaybackDefinition>,
    #[serde(default)]
    pub playback_pages: Vec<PlaybackPage>,
    pub routes: Vec<OutputRoute>,
    pub control_mappings: Vec<light_control::ControlMapping>,
    #[serde(default)]
    pub groups: Vec<GroupDefinition>,
    pub revision: u64,
}

impl EngineSnapshot {
    pub fn validate(&self) -> Result<(), EngineError> {
        validate_patch(&self.fixtures)?;
        let groups = self
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for group in &self.groups {
            if let Some(derived) = &group.derived_from {
                derived.rule.validate().map_err(EngineError::Invalid)?;
            }
            if !group.master.is_finite() || !(0.0..=1.0).contains(&group.master) {
                return Err(EngineError::Invalid(format!(
                    "group {} master must be within 0-1",
                    group.id
                )));
            }
            resolve_group(&group.id, &groups).map_err(EngineError::Invalid)?;
        }
        for cue_list in &self.cue_lists {
            cue_list.validate().map_err(EngineError::Invalid)?;
        }
        let mut playback_numbers = std::collections::HashSet::new();
        let mut playback_targets = std::collections::HashSet::new();
        for playback in &self.playbacks {
            playback.validate().map_err(EngineError::Invalid)?;
            if !playback_numbers.insert(playback.number) {
                return Err(EngineError::Invalid("duplicate playback number".into()));
            }
            if !playback_targets.insert(playback.target.clone()) {
                return Err(EngineError::Invalid(
                    "a target may only belong to one playback".into(),
                ));
            }
            match &playback.target {
                PlaybackTarget::CueList { cue_list_id }
                    if !self.cue_lists.iter().any(|cue| cue.id == *cue_list_id) =>
                {
                    return Err(EngineError::Invalid(
                        "playback references a missing cue list".into(),
                    ));
                }
                PlaybackTarget::Group { group_id }
                    if !self.groups.iter().any(|group| group.id == *group_id) =>
                {
                    return Err(EngineError::Invalid(
                        "playback references a missing group".into(),
                    ));
                }
                _ => {}
            }
        }
        for page in &self.playback_pages {
            page.validate().map_err(EngineError::Invalid)?;
            if page
                .slots
                .values()
                .any(|number| !playback_numbers.contains(number))
            {
                return Err(EngineError::Invalid(
                    "page references a missing playback".into(),
                ));
            }
        }
        for route in &self.routes {
            if route.destination_universe == 0 || route.logical_universe == 0 {
                return Err(EngineError::Invalid(
                    "universe zero is not valid for show routes".into(),
                ));
            }
            if route.protocol == light_output::Protocol::ArtNet && route.destination.is_none() {
                return Err(EngineError::Invalid(
                    "Art-Net routes require a destination".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RenderOptions {
    pub grand_master: f32,
    pub blackout: bool,
    pub control_loss_progress: Option<f32>,
}
impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            grand_master: 1.0,
            blackout: false,
            control_loss_progress: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderResult {
    pub universes: HashMap<Universe, DmxFrame>,
    pub revision: u64,
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("snapshot validation failed: {0}")]
    Invalid(String),
    #[error(transparent)]
    Fixture(#[from] light_fixture::FixtureError),
}

pub struct Engine {
    snapshot: ArcSwap<EngineSnapshot>,
    playback: RwLock<PlaybackEngine>,
    programmers: ProgrammerRegistry,
    timecode_frame: AtomicU64,
    programmer_fade_millis: AtomicU64,
    speed_groups_bpm: [AtomicU16; 5],
    sequence_master_fade_millis: AtomicU64,
    programmer_transitions: Mutex<HashMap<(FixtureId, AttributeKey), ProgrammerTransition>>,
    group_master_flashes: RwLock<HashMap<String, f32>>,
    clock: SharedClock,
}

#[derive(Clone)]
struct ProgrammerTransition {
    changed_at: chrono::DateTime<chrono::Utc>,
    from: AttributeValue,
    target: AttributeValue,
}

impl Engine {
    pub fn new(programmers: ProgrammerRegistry) -> Self {
        let clock = programmers.clock();
        Self {
            snapshot: ArcSwap::from_pointee(EngineSnapshot::default()),
            playback: RwLock::new(PlaybackEngine::with_clock(Arc::clone(&clock))),
            programmers,
            timecode_frame: AtomicU64::new(u64::MAX),
            programmer_fade_millis: AtomicU64::new(0),
            speed_groups_bpm: [
                AtomicU16::new(120),
                AtomicU16::new(90),
                AtomicU16::new(60),
                AtomicU16::new(30),
                AtomicU16::new(15),
            ],
            sequence_master_fade_millis: AtomicU64::new(0),
            programmer_transitions: Mutex::new(HashMap::new()),
            group_master_flashes: RwLock::new(HashMap::new()),
            clock,
        }
    }

    pub fn set_control_timing(
        &self,
        speed_groups_bpm: [u16; 5],
        programmer_fade_millis: u64,
        sequence_master_fade_millis: u64,
    ) {
        self.programmer_fade_millis
            .store(programmer_fade_millis.min(60_000), Ordering::Relaxed);
        for (target, bpm) in self.speed_groups_bpm.iter().zip(speed_groups_bpm) {
            target.store(bpm.clamp(1, 999), Ordering::Relaxed);
        }
        self.sequence_master_fade_millis
            .store(sequence_master_fade_millis.min(60_000), Ordering::Relaxed);
        self.playback
            .write()
            .set_control_timing(speed_groups_bpm, sequence_master_fade_millis);
    }

    pub fn clear_programmer_transitions(&self) {
        self.programmer_transitions.lock().clear();
    }

    fn faded_programmer_value(
        &self,
        mut value: TimedValue,
        now: chrono::DateTime<chrono::Utc>,
    ) -> TimedValue {
        let duration = value
            .fade_millis
            .unwrap_or_else(|| self.programmer_fade_millis.load(Ordering::Relaxed));
        if duration == 0 || value.value.normalized().is_none() {
            return value;
        }
        let key = (value.fixture_id, value.attribute.clone());
        let mut transitions = self.programmer_transitions.lock();
        let transition = transitions
            .entry(key)
            .or_insert_with(|| ProgrammerTransition {
                changed_at: value.changed_at,
                from: AttributeValue::Normalized(0.0),
                target: value.value.clone(),
            });
        let interpolate = |transition: &ProgrammerTransition| {
            let elapsed = (now - transition.changed_at).num_milliseconds().max(0) as u64;
            let elapsed = elapsed.saturating_sub(value.delay_millis.unwrap_or(0));
            let progress = (elapsed as f32 / duration as f32).clamp(0.0, 1.0);
            match (transition.from.normalized(), transition.target.normalized()) {
                (Some(from), Some(target)) => {
                    AttributeValue::Normalized(from + (target - from) * progress)
                }
                _ => transition.target.clone(),
            }
        };
        if transition.changed_at != value.changed_at || transition.target != value.value {
            let from = interpolate(transition);
            *transition = ProgrammerTransition {
                changed_at: value.changed_at,
                from,
                target: value.value.clone(),
            };
        }
        value.value = interpolate(transition);
        value
    }

    pub fn replace_snapshot(&self, snapshot: EngineSnapshot) -> Result<(), EngineError> {
        self.replace_snapshot_with_playback_policy(snapshot, true)
    }
    pub fn replace_snapshot_releasing_playback(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<(), EngineError> {
        self.replace_snapshot_with_playback_policy(snapshot, false)
    }
    fn replace_snapshot_with_playback_policy(
        &self,
        snapshot: EngineSnapshot,
        preserve_playback: bool,
    ) -> Result<(), EngineError> {
        snapshot.validate()?;
        let active_playbacks = if preserve_playback {
            self.playback.read().active()
        } else {
            Vec::new()
        };
        let mut playback = PlaybackEngine::with_clock(Arc::clone(&self.clock));
        playback.set_control_timing(
            self.speed_groups_bpm
                .each_ref()
                .map(|bpm| bpm.load(Ordering::Relaxed)),
            self.sequence_master_fade_millis.load(Ordering::Relaxed),
        );
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for source in &snapshot.cue_lists {
            let mut cue_list = source.clone();
            for cue in &mut cue_list.cues {
                let mut expanded_addresses = cue
                    .changes
                    .iter()
                    .map(|change| (change.fixture_id, change.attribute.clone()))
                    .collect::<std::collections::HashSet<_>>();
                for change in cue.group_changes.clone() {
                    if let Ok(fixtures) = resolve_group(&change.group_id, &groups) {
                        for fixture_id in fixtures {
                            if expanded_addresses.insert((fixture_id, change.attribute.clone())) {
                                cue.changes.push(light_playback::CueChange {
                                    fixture_id,
                                    attribute: change.attribute.clone(),
                                    value: change.value.clone(),
                                    automatic_restore: false,
                                    fade_millis: change.fade_millis,
                                    delay_millis: change.delay_millis,
                                });
                            }
                        }
                    }
                }
                for phaser in &mut cue.phasers {
                    for group_id in &phaser.group_ids {
                        if let Ok(fixtures) = resolve_group(group_id, &groups) {
                            for fixture in fixtures {
                                if !phaser.fixture_ids.contains(&fixture) {
                                    phaser.fixture_ids.push(fixture);
                                }
                            }
                        }
                    }
                }
            }
            playback.register(cue_list).map_err(EngineError::Invalid)?;
        }
        for definition in snapshot.playbacks.clone() {
            playback
                .register_definition(definition)
                .map_err(EngineError::Invalid)?;
        }
        self.programmers.refresh_live_selections(&groups);
        playback.restore_active(active_playbacks);
        *self.playback.write() = playback;
        self.snapshot.store(Arc::new(snapshot));
        Ok(())
    }

    pub fn snapshot(&self) -> Arc<EngineSnapshot> {
        self.snapshot.load_full()
    }
    pub fn playback(&self) -> &RwLock<PlaybackEngine> {
        &self.playback
    }
    pub fn set_timecode_frame(&self, frame: Option<u64>) {
        self.timecode_frame
            .store(frame.unwrap_or(u64::MAX), Ordering::Relaxed);
    }

    /// Sets a transient group flash level without changing the group's fader value.
    pub fn set_group_master_flash(&self, group_id: String, value: f32) {
        let mut flashes = self.group_master_flashes.write();
        if value <= 0.0 {
            flashes.remove(&group_id);
        } else {
            flashes.insert(group_id, value.clamp(0.0, 1.0));
        }
    }

    pub fn render(&self, options: RenderOptions) -> Result<RenderResult, EngineError> {
        let snapshot = self.snapshot.load_full();
        let now = self.clock.now();
        let resolved = self.resolved_values_at(&snapshot, now);
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        let group_master_flashes = self.group_master_flashes.read();
        let mut universes = HashMap::new();
        for fixture in &snapshot.fixtures {
            let mut patches = vec![(fixture.universe, fixture.address)];
            patches.extend(
                fixture
                    .multipatch
                    .iter()
                    .map(|instance| (instance.universe, instance.address)),
            );
            for (universe, address) in patches {
                let (Some(universe), Some(address)) = (universe, address) else {
                    continue;
                };
                let frame = universes.entry(universe).or_insert([0; 512]);
                let mut instance = fixture.clone();
                instance.universe = Some(universe);
                instance.address = Some(address);
                render_fixture(
                    frame,
                    &instance,
                    &resolved,
                    options,
                    &groups,
                    &group_master_flashes,
                )?;
            }
        }
        Ok(RenderResult {
            universes,
            revision: snapshot.revision,
        })
    }

    /// Returns the same merged abstract attributes that feed DMX rendering. Consumers such as
    /// visualizers can use this without attempting to reverse fixture-specific DMX encoding.
    pub fn resolved_values(&self) -> HashMap<(FixtureId, AttributeKey), AttributeValue> {
        let snapshot = self.snapshot.load_full();
        self.resolved_values_at(&snapshot, self.clock.now())
    }

    fn resolved_values_at(
        &self,
        snapshot: &EngineSnapshot,
        now: chrono::DateTime<chrono::Utc>,
    ) -> HashMap<(FixtureId, AttributeKey), AttributeValue> {
        let mut playback = self.playback.write();
        let timecode = self.timecode_frame.load(Ordering::Relaxed);
        playback.tick(now, (timecode != u64::MAX).then_some(timecode));
        let mut contributions = playback.contributions_at(now);
        contributions.extend(
            self.programmers
                .active()
                .into_iter()
                .flat_map(|programmer| {
                    programmer
                        .values
                        .into_iter()
                        .chain(programmer.preload_active)
                })
                .map(|value| {
                    if value.fade {
                        self.faded_programmer_value(value, now)
                    } else {
                        value
                    }
                }),
        );
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for programmer in self.programmers.active() {
            for (group_id, attributes) in programmer
                .group_values
                .into_iter()
                .chain(programmer.preload_group_active)
            {
                let Ok(fixtures) = resolve_group(&group_id, &groups) else {
                    continue;
                };
                for fixture_id in fixtures {
                    for (attribute, scoped) in &attributes {
                        let value = TimedValue {
                            fixture_id,
                            attribute: attribute.clone(),
                            value: scoped.value.clone(),
                            priority: programmer.priority,
                            changed_at: scoped.changed_at,
                            merge_mode: MergeMode::Ltp,
                            fade: scoped.fade,
                            fade_millis: scoped.fade_millis,
                            delay_millis: scoped.delay_millis,
                        };
                        contributions.push(if value.fade {
                            self.faded_programmer_value(value, now)
                        } else {
                            value
                        });
                    }
                }
            }
        }
        for group in &snapshot.groups {
            let Ok(fixtures) = resolve_group(&group.id, &groups) else {
                continue;
            };
            for fixture_id in fixtures {
                for (attribute, value) in &group.programming {
                    contributions.push(TimedValue {
                        fixture_id,
                        attribute: attribute.clone(),
                        value: value.clone(),
                        priority: 0,
                        changed_at: now,
                        merge_mode: if attribute.is_intensity() {
                            MergeMode::Htp
                        } else {
                            MergeMode::Ltp
                        },
                        fade: false,
                        fade_millis: None,
                        delay_millis: None,
                    });
                }
            }
        }
        resolve(contributions)
    }
}
fn render_fixture(
    frame: &mut DmxFrame,
    fixture: &PatchedFixture,
    resolved: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
    options: RenderOptions,
    groups: &HashMap<String, GroupDefinition>,
    group_master_flashes: &HashMap<String, f32>,
) -> Result<(), EngineError> {
    let Some(address) = fixture.address else {
        return Ok(());
    };
    for head in &fixture.definition.heads {
        let owner = if head.shared {
            fixture.fixture_id
        } else {
            fixture
                .logical_heads
                .iter()
                .find(|patched| patched.head_index == head.index)
                .map(|patched| patched.fixture_id)
                .unwrap_or(fixture.fixture_id)
        };
        let group_scale = groups
            .values()
            .filter(|group| group.playback_fader.is_some())
            .filter_map(|group| {
                resolve_group(&group.id, groups)
                    .ok()
                    .filter(|members| members.contains(&owner))
                    .map(|_| {
                        group
                            .master
                            .max(group_master_flashes.get(&group.id).copied().unwrap_or(0.0))
                            .clamp(0.0, 1.0)
                    })
            })
            .reduce(f32::max)
            .unwrap_or(1.0);
        let mut abstract_values: HashMap<AttributeKey, AttributeValue> = resolved
            .iter()
            .filter(|((fixture_id, _), _)| *fixture_id == owner)
            .map(|((_, attribute), value)| (attribute.clone(), value.clone()))
            .collect();
        if let Some(progress) = options.control_loss_progress {
            match fixture.definition.effective_signal_loss_policy() {
                SignalLossPolicy::HoldLast => {}
                SignalLossPolicy::ImmediateSafe => {
                    apply_safe_values(&mut abstract_values, &fixture.definition.safe_values, 1.0)
                }
                SignalLossPolicy::FadeToSafe { .. } => apply_safe_values(
                    &mut abstract_values,
                    &fixture.definition.safe_values,
                    progress.clamp(0.0, 1.0),
                ),
            }
        }
        if fixture.definition.hazardous && options.blackout {
            for (attribute, value) in &fixture.definition.safe_values {
                abstract_values.insert(attribute.clone(), value.clone());
            }
        }
        let intensity_key = AttributeKey::intensity();
        let intensity = if options.blackout {
            0.0
        } else {
            abstract_values
                .get(&intensity_key)
                .and_then(AttributeValue::normalized)
                .unwrap_or(1.0)
                * group_scale
                * options.grand_master.clamp(0.0, 1.0)
        };
        let has_physical_dimmer = head
            .parameters
            .iter()
            .any(|parameter| parameter.attribute.is_intensity() && !parameter.virtual_dimmer);
        if let (Some(AttributeValue::ColorXyz(color)), Some(calibration)) = (
            abstract_values.get(&AttributeKey("color".into())),
            &fixture.definition.color_calibration,
        ) {
            let mut levels = mix_color(*color, calibration)?;
            if !has_physical_dimmer {
                for level in &mut levels {
                    *level *= intensity;
                }
            }
            for (emitter, level) in calibration.emitters.iter().zip(levels) {
                abstract_values
                    .entry(AttributeKey(format!(
                        "color.emitter.{}",
                        emitter.name.to_lowercase()
                    )))
                    .or_insert(AttributeValue::Normalized(level));
            }
        }
        for parameter in &head.parameters {
            let mut level = abstract_values
                .get(&parameter.attribute)
                .and_then(AttributeValue::normalized)
                .unwrap_or(parameter.default);
            if parameter.attribute.is_intensity() {
                level *= group_scale;
                level *= options.grand_master.clamp(0.0, 1.0);
                if options.blackout {
                    level = 0.0;
                }
            }
            if parameter.virtual_dimmer {
                level *= intensity;
            }
            if parameter.components.is_empty() {
                continue;
            }
            encode_parameter(frame, address, parameter, level)?;
        }
        for (attribute, value) in &abstract_values {
            if let (Some(offset), AttributeValue::RawDmx(raw)) = (
                attribute
                    .0
                    .strip_prefix("dmx.")
                    .and_then(|offset| offset.parse::<u16>().ok()),
                value,
            ) && offset < fixture.definition.footprint
            {
                frame[usize::from(address - 1 + offset)] = *raw;
            }
        }
    }
    Ok(())
}

fn apply_safe_values(
    values: &mut HashMap<AttributeKey, AttributeValue>,
    safe: &std::collections::BTreeMap<AttributeKey, AttributeValue>,
    progress: f32,
) {
    for (attribute, target) in safe {
        let value = match (values.get(attribute), target) {
            (Some(AttributeValue::Normalized(current)), AttributeValue::Normalized(target)) => {
                AttributeValue::Normalized(current + (target - current) * progress)
            }
            _ if progress >= 1.0 => target.clone(),
            (Some(current), _) => current.clone(),
            (None, AttributeValue::Normalized(target)) => {
                AttributeValue::Normalized(target * progress)
            }
            _ => continue,
        };
        values.insert(attribute.clone(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_fixture::{
        ByteOrder, ChannelComponent, FixtureDefinition, LogicalHead, MultiPatchInstance, Parameter,
        PatchedHead,
    };
    use std::collections::BTreeMap;

    fn fixture() -> (PatchedFixture, FixtureId) {
        let physical = FixtureId::new();
        let logical = FixtureId::new();
        let parameter = Parameter {
            attribute: AttributeKey::intensity(),
            components: vec![ChannelComponent {
                offset: 0,
                byte_order: ByteOrder::MsbFirst,
            }],
            default: 0.0,
            virtual_dimmer: false,
            metadata: light_fixture::ParameterMetadata::default(),
            capabilities: vec![],
        };
        (
            PatchedFixture {
                fixture_id: physical,
                fixture_number: None,
                name: "Cell".into(),
                layer_id: "default".into(),
                definition: FixtureDefinition {
                    schema_version: 1,
                    id: FixtureId::new(),
                    revision: 1,
                    manufacturer: "Test".into(),
                    device_type: "other".into(),
                    name: "Cell".into(),
                    model: "Cell".into(),
                    mode: "1ch".into(),
                    footprint: 1,
                    heads: vec![LogicalHead {
                        index: 1,
                        name: "Cell".into(),
                        shared: false,
                        parameters: vec![parameter],
                    }],
                    color_calibration: None,
                    physical: Default::default(),
                    model_asset: None,
                    icon_asset: None,
                    hazardous: false,
                    direct_control_protocols: Vec::new(),
                    signal_loss_policy: SignalLossPolicy::HoldLast,
                    safe_values: BTreeMap::new(),
                },
                universe: Some(1),
                address: Some(1),
                direct_control: None,
                location: Default::default(),
                rotation: Default::default(),
                logical_heads: vec![PatchedHead {
                    head_index: 1,
                    fixture_id: logical,
                }],
                multipatch: Vec::new(),
            },
            logical,
        )
    }

    #[test]
    fn patched_multipatch_instances_duplicate_output_while_visual_only_instances_do_not() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (mut fixture, logical) = fixture();
        fixture.multipatch = vec![
            MultiPatchInstance {
                id: FixtureId::new().0,
                name: "Patched clone".into(),
                universe: Some(1),
                address: Some(8),
                location: Default::default(),
                rotation: Default::default(),
            },
            MultiPatchInstance {
                id: FixtureId::new().0,
                name: "Visualizer clone".into(),
                universe: None,
                address: None,
                location: Default::default(),
                rotation: Default::default(),
            },
        ];
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![],
                playbacks: vec![],
                playback_pages: vec![],
                routes: vec![],
                control_mappings: vec![],
                groups: vec![],
                revision: 1,
            })
            .unwrap();
        let result = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(result.universes[&1][0], 128);
        assert_eq!(result.universes[&1][7], 128);
        assert_eq!(
            result.universes[&1]
                .iter()
                .filter(|value| **value != 0)
                .count(),
            2
        );
    }

    #[test]
    fn logical_head_programmer_value_renders_to_physical_patch() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (fixture, logical) = fixture();
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![],
                playbacks: vec![],
                playback_pages: vec![],
                routes: vec![],
                control_mappings: vec![],
                groups: vec![],
                revision: 7,
            })
            .unwrap();
        let result = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(result.universes[&1][0], 128);
        assert_eq!(result.revision, 7);
        assert_eq!(
            engine
                .resolved_values()
                .get(&(logical, AttributeKey::intensity())),
            Some(&AttributeValue::Normalized(0.5))
        );
    }

    #[test]
    fn parent_programmer_value_does_not_fan_out_to_child_heads() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (fixture, _) = fixture();
        programmers.set(
            session,
            fixture.fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            0
        );
    }

    #[test]
    fn master_only_group_fader_does_not_scale_child_heads() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (mut fixture, child) = fixture();
        fixture.definition.footprint = 2;
        let mut master_parameter = fixture.definition.heads[0].parameters[0].clone();
        master_parameter.components[0].offset = 1;
        fixture.definition.heads.insert(
            0,
            LogicalHead {
                index: 0,
                name: "Master".into(),
                shared: true,
                parameters: vec![master_parameter],
            },
        );
        let master = fixture.fixture_id;
        for fixture_id in [master, child] {
            programmers.set(
                session,
                fixture_id,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.8),
            );
        }
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    id: "master".into(),
                    name: "Master only".into(),
                    fixtures: vec![master],
                    master: 0.5,
                    playback_fader: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        let rendered = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(rendered.universes[&1][0], 204);
        assert_eq!(rendered.universes[&1][1], 102);
    }

    #[test]
    fn grand_master_and_blackout_affect_intensity() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (fixture, logical) = fixture();
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![],
                playbacks: vec![],
                playback_pages: vec![],
                routes: vec![],
                control_mappings: vec![],
                groups: vec![],
                revision: 1,
            })
            .unwrap();
        assert_eq!(
            engine
                .render(RenderOptions {
                    grand_master: 0.5,
                    blackout: false,
                    control_loss_progress: None,
                })
                .unwrap()
                .universes[&1][0],
            128
        );
        assert_eq!(
            engine
                .render(RenderOptions {
                    grand_master: 1.0,
                    blackout: true,
                    control_loss_progress: None,
                })
                .unwrap()
                .universes[&1][0],
            0
        );
    }

    #[test]
    fn group_masters_scale_before_encoding_and_use_highest_master() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (fixture, logical) = fixture();
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.8),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![],
                playbacks: vec![],
                playback_pages: vec![],
                routes: vec![],
                control_mappings: vec![],
                groups: vec![
                    GroupDefinition {
                        id: "a".into(),
                        name: "A".into(),
                        fixtures: vec![logical],
                        master: 0.5,
                        playback_fader: Some(1),
                        ..Default::default()
                    },
                    GroupDefinition {
                        id: "b".into(),
                        name: "B".into(),
                        fixtures: vec![logical],
                        master: 0.75,
                        playback_fader: Some(2),
                        ..Default::default()
                    },
                    GroupDefinition {
                        id: "unassigned".into(),
                        name: "Unassigned".into(),
                        fixtures: vec![logical],
                        master: 1.0,
                        playback_fader: None,
                        ..Default::default()
                    },
                ],
                revision: 1,
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            153
        );
    }

    #[test]
    fn group_master_flash_is_temporary_and_does_not_move_the_fader() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (fixture, logical) = fixture();
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.8),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    id: "front".into(),
                    name: "Front".into(),
                    fixtures: vec![logical],
                    master: 0.25,
                    playback_fader: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();

        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            51
        );
        engine.set_group_master_flash("front".into(), 1.0);
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            204
        );
        assert_eq!(engine.snapshot().groups[0].master, 0.25);
        engine.set_group_master_flash("front".into(), 0.0);
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            51
        );
    }
    #[test]
    fn logical_head_master_does_not_limit_sibling_heads() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let physical = FixtureId::new();
        let first = FixtureId::new();
        let second = FixtureId::new();
        let parameter = |offset| Parameter {
            attribute: AttributeKey::intensity(),
            components: vec![ChannelComponent {
                offset,
                byte_order: light_fixture::ByteOrder::MsbFirst,
            }],
            default: 0.0,
            virtual_dimmer: false,
            metadata: light_fixture::ParameterMetadata::default(),
            capabilities: vec![],
        };
        let fixture = PatchedFixture {
            fixture_id: physical,
            fixture_number: None,
            name: "Two cell".into(),
            layer_id: "default".into(),
            definition: FixtureDefinition {
                schema_version: 1,
                id: FixtureId::new(),
                revision: 1,
                manufacturer: "Test".into(),
                device_type: "other".into(),
                name: "Two cell".into(),
                model: "Two cell".into(),
                mode: "2ch".into(),
                footprint: 2,
                heads: vec![
                    LogicalHead {
                        index: 1,
                        name: "One".into(),
                        shared: false,
                        parameters: vec![parameter(0)],
                    },
                    LogicalHead {
                        index: 2,
                        name: "Two".into(),
                        shared: false,
                        parameters: vec![parameter(1)],
                    },
                ],
                color_calibration: None,
                physical: Default::default(),
                model_asset: None,
                icon_asset: None,
                hazardous: false,
                direct_control_protocols: vec![],
                signal_loss_policy: SignalLossPolicy::HoldLast,
                safe_values: BTreeMap::new(),
            },
            universe: Some(1),
            address: Some(1),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![
                PatchedHead {
                    head_index: 1,
                    fixture_id: first,
                },
                PatchedHead {
                    head_index: 2,
                    fixture_id: second,
                },
            ],
            multipatch: vec![],
        };
        for fixture_id in [first, second] {
            programmers.set(
                session,
                fixture_id,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.8),
            );
        }
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    id: "first".into(),
                    name: "First".into(),
                    fixtures: vec![first],
                    master: 0.5,
                    playback_fader: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        let rendered = engine.render(RenderOptions::default()).unwrap();
        let frame = &rendered.universes[&1];
        assert_eq!(frame[0], 102);
        assert_eq!(frame[1], 204);
    }
    #[test]
    fn group_ltp_uses_operator_edit_time_not_render_time() {
        let programmers = ProgrammerRegistry::default();
        let group_session = light_core::SessionId::new();
        let direct_session = light_core::SessionId::new();
        programmers.start(group_session, light_core::UserId::new());
        programmers.start(direct_session, light_core::UserId::new());
        let (mut fixture, logical) = fixture();
        fixture.definition.heads[0].parameters[0].attribute = AttributeKey("pan".into());
        programmers.set_group(
            group_session,
            "position".into(),
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.2),
        );
        programmers.set(
            direct_session,
            logical,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.8),
        );
        let engine = Engine::new(programmers.clone());
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    id: "position".into(),
                    name: "Position".into(),
                    fixtures: vec![logical],
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            204
        );
        programmers.set_group(
            group_session,
            "position".into(),
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.1),
        );
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            26
        );
    }

    #[test]
    fn empty_group_programming_becomes_effective_when_members_are_added() {
        let programmers = ProgrammerRegistry::default();
        let (fixture, logical) = fixture();
        let engine = Engine::new(programmers);
        let group = GroupDefinition {
            id: "template".into(),
            name: "Template".into(),
            programming: HashMap::from([(
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.6),
            )]),
            fixtures: vec![],
            ..Default::default()
        };
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture.clone()],
                groups: vec![group.clone()],
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            0
        );
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    fixtures: vec![logical],
                    ..group
                }],
                revision: 2,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            153
        );
    }
    #[test]
    fn session_group_programmer_remains_live_across_membership_changes() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        let frozen_session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        programmers.start(frozen_session, light_core::UserId::new());
        programmers.select_expression(
            session,
            vec![],
            light_programmer::SelectionExpression::LiveGroup {
                group_id: "template".into(),
                rule: light_programmer::SelectionRule::All,
            },
        );
        programmers.select_expression(
            frozen_session,
            vec![],
            light_programmer::SelectionExpression::FrozenGroup {
                group_id: "template".into(),
                source_revision: 0,
            },
        );
        programmers.set_group(
            session,
            "template".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.6),
        );
        let (fixture, logical) = fixture();
        let observed = programmers.clone();
        let engine = Engine::new(programmers);
        let group = GroupDefinition {
            id: "template".into(),
            name: "Template".into(),
            fixtures: vec![],
            ..Default::default()
        };
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture.clone()],
                groups: vec![group.clone()],
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            0
        );
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![GroupDefinition {
                    fixtures: vec![logical],
                    ..group
                }],
                revision: 2,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            engine.render(RenderOptions::default()).unwrap().universes[&1][0],
            153
        );
        assert_eq!(observed.get(session).unwrap().selected, vec![logical]);
        assert!(observed.get(frozen_session).unwrap().selected.is_empty());
    }
    #[test]
    fn explicit_cue_change_wins_when_group_expansion_targets_same_attribute() {
        let programmers = ProgrammerRegistry::default();
        let (fixture, logical) = fixture();
        let mut cue = light_playback::Cue::new(1.0);
        cue.changes.push(light_playback::CueChange::set(
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        ));
        cue.group_changes.push(light_playback::GroupCueChange {
            group_id: "group".into(),
            attribute: AttributeKey::intensity(),
            value: Some(AttributeValue::Normalized(0.5)),
            fade_millis: None,
            delay_millis: None,
        });
        let cue_list = light_playback::CueList {
            id: light_core::CueListId::new(),
            name: "Deduplicated".into(),
            priority: 10,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            cues: vec![cue],
        };
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![cue_list],
                groups: vec![GroupDefinition {
                    id: "group".into(),
                    name: "Group".into(),
                    fixtures: vec![logical],
                    master: 1.0,
                    playback_fader: None,
                    programming: Default::default(),
                    derived_from: None,
                    frozen_from: None,
                }],
                revision: 1,
                ..Default::default()
            })
            .expect("overlapping group and fixture cue values must compile");
    }

    #[test]
    fn active_group_cue_survives_snapshot_swap_and_gains_new_members() {
        let programmers = ProgrammerRegistry::default();
        let (first, first_logical) = fixture();
        let (mut second, second_logical) = fixture();
        second.address = Some(2);
        let list_id = light_core::CueListId::new();
        let mut cue = light_playback::Cue::new(1.0);
        cue.group_changes.push(light_playback::GroupCueChange {
            group_id: "live".into(),
            attribute: AttributeKey::intensity(),
            value: Some(AttributeValue::Normalized(0.6)),
            fade_millis: None,
            delay_millis: None,
        });
        let list = light_playback::CueList {
            id: list_id,
            name: "Live group".into(),
            priority: 10,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            cues: vec![cue],
        };
        let engine = Engine::new(programmers);
        let snapshot = |members| EngineSnapshot {
            fixtures: vec![first.clone(), second.clone()],
            cue_lists: vec![list.clone()],
            groups: vec![GroupDefinition {
                id: "live".into(),
                name: "Live".into(),
                fixtures: members,
                master: 0.5,
                playback_fader: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        };
        engine
            .replace_snapshot(snapshot(vec![first_logical]))
            .unwrap();
        engine
            .playback()
            .write()
            .go_at(
                list_id,
                chrono::Utc::now() - chrono::Duration::milliseconds(1),
            )
            .unwrap();
        let playback_values = engine
            .playback()
            .write()
            .contributions_at(chrono::Utc::now());
        assert!(
            playback_values
                .iter()
                .any(|value| value.fixture_id == first_logical
                    && value.attribute.is_intensity()
                    && value.value.normalized().is_some_and(|level| level > 0.59))
        );
        let before = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(before.universes[&1][0], 77);
        assert_eq!(before.universes[&1][1], 0);
        engine
            .replace_snapshot(snapshot(vec![first_logical, second_logical]))
            .unwrap();
        assert_eq!(engine.playback().read().active().len(), 1);
        let after = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(after.universes[&1][0], 77);
        assert_eq!(after.universes[&1][1], 77);
        engine
            .replace_snapshot_releasing_playback(snapshot(vec![first_logical, second_logical]))
            .unwrap();
        assert!(engine.playback().read().active().is_empty());
    }

    #[test]
    fn unpatched_group_member_keeps_programming_but_outputs_no_dmx() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        programmers.set_group(
            session,
            "look".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        let (patched, patched_logical) = fixture();
        let (mut unpatched, unpatched_logical) = fixture();
        unpatched.universe = None;
        unpatched.address = None;
        let group = GroupDefinition {
            id: "look".into(),
            name: "Look".into(),
            fixtures: vec![patched_logical, unpatched_logical],
            master: 1.0,
            playback_fader: None,
            ..Default::default()
        };
        let snapshot = |unpatched_fixture: PatchedFixture| EngineSnapshot {
            fixtures: vec![patched.clone(), unpatched_fixture],
            cue_lists: vec![],
            playbacks: vec![],
            playback_pages: vec![],
            routes: vec![],
            control_mappings: vec![],
            groups: vec![group.clone()],
            revision: 1,
        };
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(snapshot(unpatched.clone()))
            .unwrap();
        let resolved = engine.resolved_values();
        assert_eq!(
            resolved
                .get(&(unpatched_logical, AttributeKey::intensity()))
                .and_then(AttributeValue::normalized),
            Some(0.5),
        );
        assert_eq!(group.fixtures, vec![patched_logical, unpatched_logical]);
        let rendered = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(rendered.universes[&1][0], 128);
        assert_eq!(rendered.universes[&1][1], 0);

        unpatched.universe = Some(1);
        unpatched.address = Some(2);
        engine.replace_snapshot(snapshot(unpatched)).unwrap();
        let repatched = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(repatched.universes[&1][0], 128);
        assert_eq!(repatched.universes[&1][1], 128);
    }

    #[test]
    fn hazardous_fixture_defaults_to_immediate_safe_on_control_loss() {
        let programmers = ProgrammerRegistry::default();
        let session = light_core::SessionId::new();
        programmers.start(session, light_core::UserId::new());
        let (mut fixture, logical) = fixture();
        fixture.definition.hazardous = true;
        fixture
            .definition
            .safe_values
            .insert(AttributeKey::intensity(), AttributeValue::Normalized(0.0));
        programmers.set(
            session,
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
        let engine = Engine::new(programmers);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                cue_lists: vec![],
                playbacks: vec![],
                playback_pages: vec![],
                routes: vec![],
                control_mappings: vec![],
                groups: vec![],
                revision: 1,
            })
            .unwrap();
        let rendered = engine
            .render(RenderOptions {
                grand_master: 1.0,
                blackout: false,
                control_loss_progress: Some(0.0),
            })
            .unwrap();
        assert_eq!(rendered.universes[&1][0], 0);
    }

    #[test]
    fn programmer_master_fade_interpolates_live_values() {
        let engine = Engine::new(ProgrammerRegistry::default());
        engine.set_control_timing([120, 90, 60, 30, 15], 1_000, 0);
        let now = chrono::Utc::now();
        let value = TimedValue {
            fixture_id: FixtureId::new(),
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(1.0),
            priority: 100,
            changed_at: now - chrono::Duration::milliseconds(500),
            merge_mode: MergeMode::Htp,
            fade: true,
            fade_millis: None,
            delay_millis: None,
        };
        let faded = engine.faded_programmer_value(value, now);
        assert!(
            faded
                .value
                .normalized()
                .is_some_and(|level| (level - 0.5).abs() < 0.02)
        );
    }
}
