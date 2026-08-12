use crate::{
    Engine, EngineError, EngineSnapshot, ProfileEncodingIndex, ProfileProjectionIndex,
    RuntimeGeneration, group_stage_positions, value_for_ordered_position,
};
use light_playback::{Cue, CueChange, CueList, GroupCueChange, PlaybackEngine};
use light_programmer::{GroupDefinition, resolve_group_spatial};
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering},
};

// @tour rust-by-example:20 Encode preparation as typestate
// Construction is fallible and side-effect free; installation consumes this value and cannot
// fail. The type prevents an unprepared or twice-installed engine snapshot.

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
    playback: Arc<RwLock<PlaybackEngine>>,
    groups: Arc<HashMap<String, GroupDefinition>>,
    profile_encodings: Arc<ProfileEncodingIndex>,
    profile_projections: Arc<ProfileProjectionIndex>,
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
        let current = self.generation.load();
        let previous = current.snapshot();
        snapshot.validate_changed(Some(previous))?;
        let fixtures_changed = !Arc::ptr_eq(&snapshot.fixtures, &previous.fixtures);
        let playback_changed = !Arc::ptr_eq(&snapshot.cue_lists, &previous.cue_lists)
            || !Arc::ptr_eq(&snapshot.playbacks, &previous.playbacks)
            || !Arc::ptr_eq(&snapshot.playback_pages, &previous.playback_pages)
            || !Arc::ptr_eq(&snapshot.groups, &previous.groups)
            || !Arc::ptr_eq(
                &snapshot.dynamic_stage_positions,
                &previous.dynamic_stage_positions,
            );
        let (profile_encodings, profile_projections) = if fixtures_changed {
            (
                Arc::new(ProfileEncodingIndex::compile(snapshot)?),
                Arc::new(ProfileProjectionIndex::compile(snapshot)?),
            )
        } else {
            (
                current.profile_encodings_arc(),
                current.profile_projections_arc(),
            )
        };
        let (playback, groups) = if playback_changed {
            let (playback, groups) = self.compile_playback(snapshot)?;
            (Arc::new(RwLock::new(playback)), Arc::new(groups))
        } else {
            (current.playback_arc(), current.groups_arc())
        };
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
        let PreparedEngineSnapshot { snapshot, runtime } = prepared;
        let current = self.generation.load_full();
        let detached_group_masters = if preserve_playback {
            detached_group_targets(current.snapshot(), &snapshot)
        } else {
            assigned_group_targets(current.snapshot())
        };
        if !Arc::ptr_eq(&snapshot.groups, &current.snapshot().groups) {
            self.programmers.refresh_live_selections(&runtime.groups);
        }
        let current_playback = current.playback_arc();
        if !Arc::ptr_eq(&runtime.playback, &current_playback) {
            self.preserve_playback_state(
                &current,
                &snapshot,
                &mut runtime.playback.write(),
                preserve_playback,
            );
        }
        if !detached_group_masters.is_empty() {
            self.group_master_flashes
                .write()
                .retain(|group_id, _| !detached_group_masters.contains(group_id));
            self.group_master_transitions
                .lock()
                .retain(|group_id, _| !detached_group_masters.contains(group_id));
        }
        if preserve_playback {
            self.group_colors
                .write()
                .retain(|group_id, _| runtime.groups.contains_key(group_id));
        } else {
            self.group_colors.write().clear();
        }
        self.generation.store(Arc::new(RuntimeGeneration::replacing(
            &current,
            snapshot,
            runtime.playback,
            runtime.groups,
            runtime.profile_encodings,
            runtime.profile_projections,
            preserve_playback,
        )));
    }

    fn preserve_playback_state(
        &self,
        generation: &Arc<RuntimeGeneration>,
        snapshot: &EngineSnapshot,
        playback: &mut PlaybackEngine,
        preserve_playback: bool,
    ) {
        if !preserve_playback {
            return;
        }
        let (mut active, active_dynamics, dynamics_paused_at) = {
            let current = generation.playback().read();
            (
                current.active_for_snapshot(&snapshot.cue_lists, self.clock.now()),
                current.active_dynamics_for_snapshot(playback),
                current.dynamics_paused_since(),
            )
        };
        let detached_cue_lists = detached_cue_list_targets(generation.snapshot(), snapshot);
        active.retain(|runtime| !detached_cue_lists.contains(&runtime.cue_list_id));
        playback.restore_active(active);
        playback.restore_active_dynamics(active_dynamics);
        playback.restore_dynamics_paused_since(dynamics_paused_at);
    }

    fn compile_playback(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Result<(PlaybackEngine, HashMap<String, GroupDefinition>), EngineError> {
        let groups = snapshot_groups(snapshot);
        let stage_positions = group_stage_positions(snapshot);
        let mut playback = self.playback_for_current_controls();
        for source in snapshot.cue_lists.iter() {
            let cue_list = expand_group_references(source, &groups, &stage_positions);
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

    pub fn output_routes(&self) -> Arc<[light_output::OutputRoute]> {
        self.generation.load().routes()
    }
    pub fn set_timecode_frame(&self, frame: Option<u64>) {
        self.timecode_frame
            .store(frame.unwrap_or(u64::MAX), Ordering::Relaxed);
    }
}

fn detached_cue_list_targets(
    current: &EngineSnapshot,
    replacement: &EngineSnapshot,
) -> HashSet<light_core::CueListId> {
    let replacement = assigned_cue_list_targets(replacement);
    assigned_cue_list_targets(current)
        .difference(&replacement)
        .copied()
        .collect()
}

fn assigned_cue_list_targets(snapshot: &EngineSnapshot) -> HashSet<light_core::CueListId> {
    snapshot
        .playbacks
        .iter()
        .chain(
            snapshot
                .playback_pages
                .iter()
                .flat_map(|page| page.virtual_playbacks.values()),
        )
        .filter_map(|definition| match definition.target {
            light_playback::PlaybackTarget::CueList { cue_list_id } => Some(cue_list_id),
            _ => None,
        })
        .collect()
}

fn detached_group_targets(
    current: &EngineSnapshot,
    replacement: &EngineSnapshot,
) -> HashSet<String> {
    let replacement = assigned_group_targets(replacement);
    assigned_group_targets(current)
        .difference(&replacement)
        .cloned()
        .collect()
}

fn assigned_group_targets(snapshot: &EngineSnapshot) -> HashSet<String> {
    snapshot
        .playbacks
        .iter()
        .chain(
            snapshot
                .playback_pages
                .iter()
                .flat_map(|page| page.virtual_playbacks.values()),
        )
        .filter_map(|definition| match &definition.target {
            light_playback::PlaybackTarget::Group { group_id, .. } => Some(group_id.clone()),
            _ => None,
        })
        .collect()
}

fn snapshot_groups(snapshot: &EngineSnapshot) -> HashMap<String, GroupDefinition> {
    snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect()
}

fn expand_group_references(
    source: &CueList,
    groups: &HashMap<String, GroupDefinition>,
    stage_positions: &HashMap<light_core::FixtureId, light_dynamics::Position3d>,
) -> CueList {
    let mut cue_list = source.clone();
    for cue in &mut cue_list.cues {
        expand_group_changes(cue, groups, stage_positions);
    }
    cue_list
}

fn expand_group_changes(
    cue: &mut Cue,
    groups: &HashMap<String, GroupDefinition>,
    stage_positions: &HashMap<light_core::FixtureId, light_dynamics::Position3d>,
) {
    let mut addresses = cue
        .changes
        .iter()
        .map(|change| (change.fixture_id, change.attribute.clone()))
        .collect::<HashSet<_>>();
    for change in &cue.group_changes {
        for expanded in resolved_group_changes(change, groups, stage_positions) {
            let address = (expanded.fixture_id, expanded.attribute.clone());
            if addresses.insert(address) {
                cue.changes.push(expanded);
            }
        }
    }
}

fn resolved_group_changes(
    change: &GroupCueChange,
    groups: &HashMap<String, GroupDefinition>,
    stage_positions: &HashMap<light_core::FixtureId, light_dynamics::Position3d>,
) -> Vec<CueChange> {
    let Ok(resolved) = resolve_group_spatial(&change.group_id, groups, stage_positions) else {
        return Vec::new();
    };
    let ranking = resolved.ranked_selection;
    let count = ranking.rank_count;
    let rank_by_fixture = ranking.rank_by_fixture;
    ranking
        .ordered_fixture_ids
        .into_iter()
        .map(|fixture_id| CueChange {
            fixture_id,
            attribute: change.attribute.clone(),
            value: spread_group_value(change, rank_by_fixture[&fixture_id], count),
            automatic_restore: false,
            fade_millis: change.fade_millis,
            delay_millis: change.delay_millis,
        })
        .collect()
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

fn register_playback_definitions(
    playback: &mut PlaybackEngine,
    snapshot: &EngineSnapshot,
) -> Result<(), EngineError> {
    for definition in snapshot.playbacks.iter() {
        playback
            .register_definition(definition.clone())
            .map_err(EngineError::Invalid)?;
    }
    for page in snapshot.playback_pages.iter() {
        for (&number, definition) in &page.virtual_playbacks {
            let address = light_playback::VirtualPlaybackAddress::new(page.number, number)
                .map_err(EngineError::Invalid)?;
            playback
                .register_virtual_definition(address, definition.clone())
                .map_err(EngineError::Invalid)?;
        }
    }
    Ok(())
}
