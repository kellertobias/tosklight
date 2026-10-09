use crate::{
    Engine, EngineError, EngineSnapshot, GroupMasterIndex, GroupMasterLevels, ProfileEncodingIndex,
    ProfileProjectionIndex, RuntimeGeneration, group_stage_positions,
};
use chrono::{DateTime, Utc};
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
    snapshot: Arc<EngineSnapshot>,
    runtime: PreparedRuntime,
}

#[derive(Debug)]
struct PreparedRuntime {
    playback: Arc<RwLock<PlaybackEngine>>,
    playback_reused: bool,
    groups: Arc<HashMap<String, GroupDefinition>>,
    profile_encodings: Arc<ProfileEncodingIndex>,
    profile_projections: Arc<ProfileProjectionIndex>,
}

/// A prepared snapshot whose Playback state and Dynamic inputs have been finalized together.
///
/// Finalization is side-effect free and never ticks Playback. Preserving Live requires the
/// ordered mutation lease from finalization through persistence and installation. A destination
/// restored with Release may wait, then reserve current/checkpoint provenance watermarks under
/// its ordered commit gate. Installation never rebases owners onto later controls or wall time.
#[derive(Debug)]
#[must_use = "a finalized snapshot must be installed to affect the live engine"]
pub struct FinalizedEngineSnapshot {
    prepared: PreparedEngineSnapshot,
    preserve_playback: bool,
    sampled_at: DateTime<Utc>,
    cue_dynamic_values: Vec<light_playback::ActiveCueDynamicValue>,
    dynamic_playbacks: Vec<light_playback::ActiveDynamicPlayback>,
    playback_dynamics_paused: bool,
    /// Release-policy Group Master levels prepared against this token's own snapshot and Groups.
    /// Installation adopts this index verbatim instead of recompiling portable seeds.
    group_masters: Option<Arc<GroupMasterIndex>>,
}

/// One persisted Group Master level applied to a detached destination.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedGroupMaster {
    pub group_id: String,
    pub level: f32,
    /// Whether the level differs from the destination's portable seed.
    pub changed: bool,
}

/// Structured result of [`FinalizedEngineSnapshot::prepare_group_masters`].
///
/// Both lists are sorted by Group ID. `missing` lists persisted levels whose Group has no Group
/// Master binding in the destination (deleted, never defined, unassigned to any Playback, or
/// unresolvable). They are skipped passively, as the ordinary per-Group replay skipped them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupMasterPreparation {
    restored: Vec<PreparedGroupMaster>,
    missing: Vec<String>,
}

impl GroupMasterPreparation {
    pub fn restored(&self) -> &[PreparedGroupMaster] {
        &self.restored
    }

    pub fn missing(&self) -> &[String] {
        &self.missing
    }
}

impl FinalizedEngineSnapshot {
    /// Applies persisted Group Master levels to this detached Release-policy destination.
    ///
    /// This is the activation replacement for replaying `Engine::set_group_master` per Group after
    /// installation. It compiles the destination Group Master index exactly as Release
    /// installation would (portable seeds), applies each persisted level, and retargets the
    /// destination's physical Group fader pickup exactly as `set_group_master` does. Installation
    /// then adopts that index and Playback verbatim, so no fallible replay remains afterwards.
    ///
    /// Every level is validated (finite, within 0-1) before anything changes; one invalid level
    /// rejects the whole preparation and leaves this token untouched. Groups without a destination
    /// binding are skipped and reported in [`GroupMasterPreparation::missing`].
    ///
    /// The token owns its detached Playback, so this never touches Live generation storage, Live
    /// Playback, ticks, events, persistence, or source-occurrence watermarks, and it does not
    /// change the captured owners or snapshot identity. It rejects a preserving token (Live
    /// levels carry over there) and a second preparation of the same token.
    ///
    /// Integration (TL-584): call after `finalize_snapshot_playback_restoring_dynamics` and before
    /// strict destination Dynamic reconciliation and final owner capture, inside the same
    /// fallible preparation phase; then install with `install_finalized_snapshot` and drop the
    /// post-install Group Master replay.
    pub fn prepare_group_masters(
        &mut self,
        levels: &HashMap<String, f32>,
    ) -> Result<GroupMasterPreparation, EngineError> {
        if self.preserve_playback {
            return Err(EngineError::Invalid(
                "persisted Group Master levels apply only to a Release-policy destination".into(),
            ));
        }
        if self.group_masters.is_some() {
            return Err(EngineError::Invalid(
                "destination Group Master levels are already prepared".into(),
            ));
        }
        let mut entries = levels
            .iter()
            .map(|(group_id, level)| (group_id.as_str(), *level))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.0.cmp(right.0));
        if let Some((group_id, level)) = entries
            .iter()
            .find(|(_, level)| !level.is_finite() || !(0.0..=1.0).contains(level))
        {
            return Err(EngineError::Invalid(format!(
                "persisted Group Master {group_id} level {level} must be within 0-1"
            )));
        }
        let runtime = &self.prepared.runtime;
        let mut index =
            GroupMasterIndex::compile_released(&runtime.groups, &self.prepared.snapshot);
        let mut preparation = GroupMasterPreparation::default();
        for &(group_id, level) in &entries {
            match index.set_existing_master(group_id, level) {
                Some(changed) => preparation.restored.push(PreparedGroupMaster {
                    group_id: group_id.to_owned(),
                    level,
                    changed,
                }),
                None => preparation.missing.push(group_id.to_owned()),
            }
        }
        // Finalization gave this token a Playback Arc of its own, never shared with Live.
        let mut playback = runtime.playback.write();
        for restored in &preparation.restored {
            playback.retarget_group_physical_controls(&restored.group_id, restored.level, None);
        }
        drop(playback);
        self.group_masters = Some(Arc::new(index));
        Ok(preparation)
    }

    /// The prepared destination Group Master level, or `None` before preparation or when the
    /// Group has no destination binding.
    pub fn group_master(&self, group_id: &str) -> Option<f32> {
        self.group_masters.as_ref()?.master(group_id)
    }

    /// The detached destination's physical control state for one Playback address.
    pub fn playback_control_state_at(
        &self,
        identity: light_playback::PlaybackIdentity,
    ) -> light_playback::PlaybackControlState {
        self.prepared
            .runtime
            .playback
            .read()
            .control_state_at(identity)
    }

    #[cfg(test)]
    pub(crate) fn playback_arc_for_test(&self) -> Arc<RwLock<PlaybackEngine>> {
        Arc::clone(&self.prepared.runtime.playback)
    }

    /// Reserves provenance IDs on this detached destination without changing captured owners.
    ///
    /// Under the ordered commit gate, activation must reserve the maximum of the then-current
    /// Live watermark and the restored checkpoint watermark before installing this token.
    /// Reservation is monotonic, allocates no occurrence, and does not mutate Live.
    pub fn reserve_playback_source_occurrence_watermark(&mut self, watermark: u64) {
        self.prepared
            .runtime
            .playback
            .write()
            .reserve_source_occurrence_watermark(watermark);
    }

    pub fn snapshot(&self) -> &EngineSnapshot {
        self.prepared.snapshot()
    }

    pub fn snapshot_arc(&self) -> Arc<EngineSnapshot> {
        self.prepared.snapshot_arc()
    }

    pub fn sampled_at(&self) -> DateTime<Utc> {
        self.sampled_at
    }

    pub fn cue_dynamic_values(&self) -> &[light_playback::ActiveCueDynamicValue] {
        &self.cue_dynamic_values
    }

    pub fn dynamic_playbacks(&self) -> &[light_playback::ActiveDynamicPlayback] {
        &self.dynamic_playbacks
    }

    pub fn playback_dynamics_paused(&self) -> bool {
        self.playback_dynamics_paused
    }
}

impl PreparedEngineSnapshot {
    /// Returns the validated snapshot that will become live when this value is installed.
    pub fn snapshot(&self) -> &EngineSnapshot {
        &self.snapshot
    }

    /// The exact immutable snapshot identity retained by installation and frame capture.
    /// Callers can pair other prepared dependencies with this identity before publication.
    pub fn snapshot_arc(&self) -> Arc<EngineSnapshot> {
        Arc::clone(&self.snapshot)
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
        Ok(PreparedEngineSnapshot {
            snapshot: Arc::new(snapshot),
            runtime,
        })
    }

    /// Installs a previously prepared snapshot while preserving compatible playback state.
    pub fn install_prepared_snapshot(&self, prepared: PreparedEngineSnapshot) {
        self.install_prepared_snapshot_with_playback_policy(prepared, true, false, None);
    }

    /// Installs a prepared snapshot while dropping runtime playback state from the previous show.
    ///
    /// Show activation prepares before committing any persisted migration, then uses this
    /// infallible boundary so a successful commit cannot leave persistence ahead of the engine.
    pub fn install_prepared_snapshot_releasing_playback(&self, prepared: PreparedEngineSnapshot) {
        self.install_prepared_snapshot_with_playback_policy(prepared, false, false, None);
    }

    /// Finalizes the candidate Playback state for cold dependency reconciliation before commit.
    ///
    /// Even an unchanged compiled Playback is detached from Live. Reused preparation from an
    /// older generation is rebuilt cleanly before preserving the latest active state; otherwise
    /// removed or temporary state from that older generation could leak into the candidate.
    pub fn finalize_snapshot_playback(
        &self,
        prepared: PreparedEngineSnapshot,
        preserve_playback: bool,
    ) -> Result<FinalizedEngineSnapshot, EngineError> {
        self.finalize_snapshot_playback_with_restored_dynamics(prepared, preserve_playback, None)
    }

    /// Finalizes a destination show with Release policy and its saved Dynamic Playback owners.
    ///
    /// Restores only the supplied rows and global pause into detached Playback before collecting
    /// cold dependency inputs. Normal and temporary Cue state from Live is released. Restore
    /// keeps Playback's existing target remapping, passive missing-target, and flash normalization
    /// rules; it never ticks a Cue or advances a master transition.
    ///
    /// A destination may await commit while the previous show remains Live. The ordered commit
    /// must reserve then-current Live and checkpoint provenance watermarks on this token before
    /// installation; owner arrays are not recaptured. Ordinary preserved finalization still
    /// requires its mutation lease throughout.
    pub fn finalize_snapshot_playback_restoring_dynamics(
        &self,
        prepared: PreparedEngineSnapshot,
        saved: &[light_playback::ActiveDynamicPlayback],
        paused_since: Option<DateTime<Utc>>,
    ) -> Result<FinalizedEngineSnapshot, EngineError> {
        for (index, row) in saved.iter().enumerate() {
            // Identity wrappers can arrive through Serde without their constructors.
            let identity = match row.playback_identity {
                None => light_playback::PlaybackIdentity::physical(row.playback_number),
                Some(light_playback::PlaybackIdentity::Physical(number)) => {
                    light_playback::PlaybackIdentity::physical(number.get())
                }
                Some(light_playback::PlaybackIdentity::Virtual(address)) => {
                    light_playback::PlaybackIdentity::virtual_playback(
                        address.page(),
                        address.number().get(),
                    )
                }
            };
            identity.map_err(|error| {
                EngineError::Invalid(format!("saved Dynamic Playback row {index}: {error}"))
            })?;
            if !row.fader_value.is_finite()
                || !row.size.is_finite()
                || !row.master.is_finite()
                || row.local_speed_multiplier.denominator == 0
                || row.master_transition.as_ref().is_some_and(|transition| {
                    !transition.from.is_finite() || !transition.to.is_finite()
                })
            {
                return Err(EngineError::Invalid(format!(
                    "saved Dynamic Playback row {index} contains invalid controls"
                )));
            }
        }
        self.finalize_snapshot_playback_with_restored_dynamics(
            prepared,
            false,
            Some((saved, paused_since)),
        )
    }

    fn finalize_snapshot_playback_with_restored_dynamics(
        &self,
        mut prepared: PreparedEngineSnapshot,
        preserve_playback: bool,
        restored: Option<(
            &[light_playback::ActiveDynamicPlayback],
            Option<DateTime<Utc>>,
        )>,
    ) -> Result<FinalizedEngineSnapshot, EngineError> {
        let current = self.generation.load_full();
        let current_playback = current.playback_arc();
        let sampled_at = self.clock.now();
        let reuse_current = prepared.runtime.playback_reused
            && Arc::ptr_eq(&prepared.runtime.playback, &current_playback);
        let mut playback = if preserve_playback && reuse_current {
            // A plain clone retains the live clock. A preview fork would permanently freeze it.
            current_playback.read().clone()
        } else {
            let mut playback = if prepared.runtime.playback_reused {
                self.compile_playback(&prepared.snapshot)?.0
            } else {
                prepared.runtime.playback.read().clone()
            };
            self.preserve_playback_state(
                &current,
                &prepared.snapshot,
                &mut playback,
                preserve_playback,
                sampled_at,
            );
            playback
        };
        if let Some((saved, paused_since)) = restored {
            playback.restore_active_dynamics(saved.iter().cloned());
            playback.restore_dynamics_paused_since(paused_since);
        }
        let cue_dynamic_values = playback.active_cue_dynamic_values();
        let dynamic_playbacks = playback.active_dynamic_playbacks();
        let playback_dynamics_paused = playback.dynamics_paused();
        prepared.runtime.playback = Arc::new(RwLock::new(playback));
        prepared.runtime.playback_reused = false;
        Ok(FinalizedEngineSnapshot {
            prepared,
            preserve_playback,
            sampled_at,
            cue_dynamic_values,
            dynamic_playbacks,
            playback_dynamics_paused,
            group_masters: None,
        })
    }

    /// Publishes exactly the Playback state used to validate the finalized candidate's inputs.
    /// No runtime recapture or fallible compilation remains after persistence succeeds.
    pub fn install_finalized_snapshot(&self, finalized: FinalizedEngineSnapshot) {
        self.install_prepared_snapshot_with_playback_policy(
            finalized.prepared,
            finalized.preserve_playback,
            true,
            finalized.group_masters,
        );
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
        self.install_prepared_snapshot_with_playback_policy(prepared, false, false, None);
        Ok(())
    }

    fn prepare_runtime(&self, snapshot: &EngineSnapshot) -> Result<PreparedRuntime, EngineError> {
        let current = self.generation.load();
        let previous = current.snapshot();
        // Existing projections were accepted under this immutable engine capability. Point or
        // patch edits must not rescan an unrelated cue history just to repeat that check.
        let required = snapshot.changed_programming_contract(Some(previous));
        if required > self.supported_programming_contract() {
            return Err(EngineError::Invalid(format!(
                "this show requires programming contract {required}; this runtime supports {}",
                self.supported_programming_contract()
            )));
        }
        snapshot.validate_changed(Some(previous))?;
        let fixtures_changed = !Arc::ptr_eq(&snapshot.fixtures, &previous.fixtures);
        let playback_changed = fixtures_changed
            || !Arc::ptr_eq(&snapshot.cue_lists, &previous.cue_lists)
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
            playback_reused: !playback_changed,
            groups,
            profile_encodings,
            profile_projections,
        })
    }

    fn install_prepared_snapshot_with_playback_policy(
        &self,
        prepared: PreparedEngineSnapshot,
        preserve_playback: bool,
        playback_finalized: bool,
        prepared_group_masters: Option<Arc<GroupMasterIndex>>,
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
        if !playback_finalized && !Arc::ptr_eq(&runtime.playback, &current_playback) {
            self.preserve_playback_state(
                &current,
                &snapshot,
                &mut runtime.playback.write(),
                preserve_playback,
                self.clock.now(),
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
        let group_master_levels = match prepared_group_masters {
            Some(prepared) => GroupMasterLevels::Prepared(prepared),
            None if preserve_playback => GroupMasterLevels::Preserved,
            None => GroupMasterLevels::Released,
        };
        self.generation.store(Arc::new(RuntimeGeneration::replacing(
            &current,
            snapshot,
            runtime.playback,
            runtime.groups,
            runtime.profile_encodings,
            runtime.profile_projections,
            group_master_levels,
        )));
    }

    fn preserve_playback_state(
        &self,
        generation: &Arc<RuntimeGeneration>,
        snapshot: &EngineSnapshot,
        playback: &mut PlaybackEngine,
        preserve_playback: bool,
        sampled_at: DateTime<Utc>,
    ) {
        let (mut active, active_dynamics, dynamics_paused_at) = {
            let current = generation.playback().read();
            // Source history can survive outside active Playback rows (for example in a
            // held Dynamic). Generation replacement must never reopen those identities.
            playback.reserve_source_occurrence_watermark(current.source_occurrence_watermark());
            if !preserve_playback {
                return;
            }
            (
                current.active_for_snapshot(&snapshot.cue_lists, sampled_at),
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
            self.release_fade_millis.load(Ordering::Relaxed),
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

/// The Cuelist as playback registers it, for a Cue preview that must spread Group values the same
/// way playback does.
pub(crate) fn expand_group_references_for_preview(
    source: &CueList,
    groups: &HashMap<String, GroupDefinition>,
    stage_positions: &HashMap<light_core::FixtureId, light_dynamics::Position3d>,
) -> CueList {
    expand_group_references(source, groups, stage_positions)
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
    let values = match &change.value {
        Some(value) => match crate::group_programming::compile_group_values(value, &ranking) {
            Ok(values) => values
                .into_iter()
                .map(|(id, value)| (id, Some(value)))
                .collect::<Vec<_>>(),
            Err(_) => return Vec::new(),
        },
        None => ranking
            .ordered_fixture_ids
            .iter()
            .map(|id| (*id, None))
            .collect(),
    };
    values
        .into_iter()
        .map(|(fixture_id, value)| CueChange {
            preset_reference: change.preset_reference.clone(),
            fixture_id,
            attribute: change.attribute.clone(),
            value,
            automatic_restore: false,
            fade_millis: change.fade_millis,
            delay_millis: change.delay_millis,
        })
        .collect()
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
