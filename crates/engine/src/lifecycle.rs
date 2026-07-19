use crate::{
    Engine, EngineError, EngineSnapshot, ProfileEncodingIndex, ProfileProjectionIndex,
    RuntimeGeneration, value_for_ordered_position,
};
use light_playback::{Cue, CueChange, CueList, GroupCueChange, PlaybackEngine};
use light_programmer::{GroupDefinition, resolve_group};
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering},
};

/// A snapshot whose validation and playback compilation have already succeeded.
///
/// Preparing a snapshot is side-effect free. Installing it consumes this value and cannot fail,
/// which lets callers complete fallible work before committing an authoritative show mutation.
#[derive(Debug)]
#[must_use = "a prepared snapshot must be installed to affect the live engine"]
pub struct PreparedEngineSnapshot {
    snapshot: EngineSnapshot,
    runtime: PreparedRuntime,
}

#[derive(Debug)]
struct PreparedRuntime {
    playback: PlaybackEngine,
    groups: HashMap<String, GroupDefinition>,
    profile_encodings: ProfileEncodingIndex,
    profile_projections: ProfileProjectionIndex,
}

impl PreparedEngineSnapshot {
    /// Returns the validated snapshot that will become live when this value is installed.
    pub fn snapshot(&self) -> &EngineSnapshot {
        &self.snapshot
    }
}

impl Engine {
    pub fn replace_snapshot(&self, snapshot: EngineSnapshot) -> Result<(), EngineError> {
        let prepared = self.prepare_snapshot(snapshot)?;
        self.install_prepared_snapshot(prepared);
        Ok(())
    }

    /// Validates and compiles a candidate without changing live engine state.
    pub fn prepare_snapshot(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<PreparedEngineSnapshot, EngineError> {
        let runtime = self.prepare_runtime(&snapshot)?;
        Ok(PreparedEngineSnapshot { snapshot, runtime })
    }

    /// Installs a previously prepared snapshot while preserving compatible playback state.
    pub fn install_prepared_snapshot(&self, prepared: PreparedEngineSnapshot) {
        self.install_prepared_snapshot_with_playback_policy(prepared, true);
    }

    /// Installs a prepared snapshot while dropping runtime playback state from the previous show.
    ///
    /// Show activation prepares before committing any persisted migration, then uses this
    /// infallible boundary so a successful commit cannot leave persistence ahead of the engine.
    pub fn install_prepared_snapshot_releasing_playback(&self, prepared: PreparedEngineSnapshot) {
        self.install_prepared_snapshot_with_playback_policy(prepared, false);
    }

    /// Validates every runtime-dependent part of a candidate snapshot without mutating the live
    /// engine. Server persistence uses this preflight so an invalid Chaser or playback assignment
    /// cannot be written first and rejected only during the subsequent live-engine refresh.
    pub fn validate_snapshot_for_runtime(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Result<(), EngineError> {
        self.prepare_runtime(snapshot).map(|_| ())
    }

    pub fn replace_snapshot_releasing_playback(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<(), EngineError> {
        let prepared = self.prepare_snapshot(snapshot)?;
        self.install_prepared_snapshot_with_playback_policy(prepared, false);
        Ok(())
    }

    fn prepare_runtime(&self, snapshot: &EngineSnapshot) -> Result<PreparedRuntime, EngineError> {
        snapshot.validate()?;
        let profile_encodings = ProfileEncodingIndex::compile(snapshot)?;
        let profile_projections = ProfileProjectionIndex::compile(snapshot)?;
        let (playback, groups) = self.compile_playback(snapshot)?;
        Ok(PreparedRuntime {
            playback,
            groups,
            profile_encodings,
            profile_projections,
        })
    }

    fn install_prepared_snapshot_with_playback_policy(
        &self,
        prepared: PreparedEngineSnapshot,
        preserve_playback: bool,
    ) {
        let PreparedEngineSnapshot {
            snapshot,
            mut runtime,
        } = prepared;
        self.preserve_playback_state(&snapshot, &mut runtime.playback, preserve_playback);
        self.programmers.refresh_live_selections(&runtime.groups);
        self.generation.store(Arc::new(RuntimeGeneration::new(
            snapshot,
            runtime.playback,
            runtime.groups,
            runtime.profile_encodings,
            runtime.profile_projections,
        )));
    }

    fn preserve_playback_state(
        &self,
        snapshot: &EngineSnapshot,
        playback: &mut PlaybackEngine,
        preserve_playback: bool,
    ) {
        if !preserve_playback {
            return;
        }
        let (active, dynamics_paused_at) = {
            let generation = self.generation.load();
            let current = generation.playback().read();
            (
                current.active_for_snapshot(&snapshot.cue_lists, self.clock.now()),
                current.dynamics_paused_since(),
            )
        };
        playback.restore_active(active);
        playback.restore_dynamics_paused_since(dynamics_paused_at);
    }

    fn compile_playback(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Result<(PlaybackEngine, HashMap<String, GroupDefinition>), EngineError> {
        let groups = snapshot_groups(snapshot);
        let mut playback = self.playback_for_current_controls();
        for source in &snapshot.cue_lists {
            let cue_list = expand_group_references(source, &groups);
            playback.register(cue_list).map_err(EngineError::Invalid)?;
        }
        register_playback_definitions(&mut playback, snapshot)?;
        Ok((playback, groups))
    }

    fn playback_for_current_controls(&self) -> PlaybackEngine {
        let mut playback = PlaybackEngine::with_clock(Arc::clone(&self.clock));
        playback.set_control_timing(
            self.current_speed_groups_bpm(),
            self.sequence_master_fade_millis.load(Ordering::Relaxed),
        );
        playback.set_speed_groups_paused(self.current_speed_groups_paused());
        playback
    }

    fn current_speed_groups_bpm(&self) -> [f64; 5] {
        self.speed_groups_bpm
            .each_ref()
            .map(|bpm| f64::from_bits(bpm.load(Ordering::Relaxed)))
    }

    fn current_speed_groups_paused(&self) -> [bool; 5] {
        self.speed_groups_paused
            .each_ref()
            .map(|paused| paused.load(Ordering::Relaxed))
    }

    pub fn snapshot(&self) -> Arc<EngineSnapshot> {
        self.generation.load().snapshot_arc()
    }

    /// Temporary compatibility accessor while callers migrate to typed Playback commands and
    /// projections. The returned lock belongs to one immutable engine generation.
    pub fn playback(&self) -> Arc<RwLock<PlaybackEngine>> {
        self.generation.load().playback_arc()
    }

    pub fn output_routes(&self) -> Arc<[light_output::OutputRoute]> {
        self.generation.load().routes()
    }
    pub fn set_timecode_frame(&self, frame: Option<u64>) {
        self.timecode_frame
            .store(frame.unwrap_or(u64::MAX), Ordering::Relaxed);
    }
}

fn snapshot_groups(snapshot: &EngineSnapshot) -> HashMap<String, GroupDefinition> {
    snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect()
}

fn expand_group_references(source: &CueList, groups: &HashMap<String, GroupDefinition>) -> CueList {
    let mut cue_list = source.clone();
    for cue in &mut cue_list.cues {
        expand_group_changes(cue, groups);
        expand_group_phasers(cue, groups);
    }
    cue_list
}

fn expand_group_changes(cue: &mut Cue, groups: &HashMap<String, GroupDefinition>) {
    let mut addresses = cue
        .changes
        .iter()
        .map(|change| (change.fixture_id, change.attribute.clone()))
        .collect::<HashSet<_>>();
    for change in &cue.group_changes {
        for expanded in resolved_group_changes(change, groups) {
            let address = (expanded.fixture_id, expanded.attribute.clone());
            if addresses.insert(address) {
                cue.changes.push(expanded);
            }
        }
    }
}

fn resolved_group_changes<'a>(
    change: &'a GroupCueChange,
    groups: &'a HashMap<String, GroupDefinition>,
) -> impl Iterator<Item = CueChange> + 'a {
    let fixtures = resolve_group(&change.group_id, groups).unwrap_or_default();
    let count = fixtures.len();
    fixtures
        .into_iter()
        .enumerate()
        .map(move |(index, fixture_id)| CueChange {
            fixture_id,
            attribute: change.attribute.clone(),
            value: spread_group_value(change, index, count),
            automatic_restore: false,
            fade_millis: change.fade_millis,
            delay_millis: change.delay_millis,
        })
}

fn spread_group_value(
    change: &GroupCueChange,
    index: usize,
    count: usize,
) -> Option<light_core::AttributeValue> {
    change
        .value
        .as_ref()
        .map(|value| value_for_ordered_position(value, index, count))
}

fn expand_group_phasers(cue: &mut Cue, groups: &HashMap<String, GroupDefinition>) {
    for phaser in &mut cue.phasers {
        let mut fixture_ids = phaser.fixture_ids.iter().copied().collect::<HashSet<_>>();
        for group_id in &phaser.group_ids {
            let Ok(fixtures) = resolve_group(group_id, groups) else {
                continue;
            };
            for fixture_id in fixtures {
                if fixture_ids.insert(fixture_id) {
                    phaser.fixture_ids.push(fixture_id);
                }
            }
        }
    }
}

fn register_playback_definitions(
    playback: &mut PlaybackEngine,
    snapshot: &EngineSnapshot,
) -> Result<(), EngineError> {
    for definition in &snapshot.playbacks {
        playback
            .register_definition(definition.clone())
            .map_err(EngineError::Invalid)?;
    }
    Ok(())
}
