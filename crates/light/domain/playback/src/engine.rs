use crate::*;

/// Cold, runtime-only clock authority captured with the active owners it belongs to.
/// It is never part of a saved checkpoint or a portable show.
#[derive(Clone, Debug)]
pub struct PlaybackTimelineClockState(Vec<TimelineClockOwner>);

#[derive(Clone, Debug)]
struct TimelineClockOwner {
    id: CueListId,
    sampled_at: Option<DateTime<Utc>>,
    activated_at: DateTime<Utc>,
    transition_ordinal: u64,
    cue_id: Uuid,
    source: SequenceMasterSource,
    paused: bool,
    paused_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct PlaybackEngine {
    pub(crate) cue_lists: HashMap<CueListId, CueList>,
    pub(crate) compiled_cue_lists: HashMap<CueListId, Arc<CompiledCueList>>,
    pub(crate) active: HashMap<PlaybackKey, ActivePlayback>,
    /// Runtime-only arrival counts for configured Cue jump points. A Cuelist has one shared
    /// authority regardless of how many physical, virtual, keyboard, OSC, or hardware controls
    /// address it.
    pub(crate) jump_counts: HashMap<(CueListId, Uuid), u32>,
    pub(crate) jump_bypass_once: HashSet<CueListId>,
    pub(crate) control_states: HashMap<PlaybackControlKey, PlaybackControlState>,
    pub(crate) active_dynamics: HashMap<uuid::Uuid, ActiveDynamicPlayback>,
    pub(crate) cuelist_flash_states: HashMap<PlaybackControlKey, CuelistFlashState>,
    pub(crate) cuelist_swap_states: HashMap<PlaybackControlKey, CuelistFlashState>,
    pub(crate) dynamic_flash_states: HashMap<PlaybackControlKey, DynamicFlashState>,
    pub(crate) temporary: HashMap<(PlaybackControlKey, TemporaryPlaybackKind), ActivePlayback>,
    pub(crate) swap_held: HashSet<PlaybackControlKey>,
    /// Timeline ownership with an optional external sample clock; not persisted or operator Pause.
    pub(crate) timeline_controlled: HashMap<CueListId, Option<DateTime<Utc>>>,
    /// Final live Timecode crossings awaiting one authoritative output capture; never persisted.
    pub(crate) timeline_transitions: HashMap<CueListId, AutomaticPlaybackTransition>,
    pub(crate) dynamics_paused_at: Option<DateTime<Utc>>,
    pub(crate) speed_groups_bpm: [f64; 5],
    pub(crate) speed_groups_paused: [bool; 5],
    pub(crate) sequence_master_fade_millis: u64,
    pub(crate) release_fade_millis: u64,
    pub(crate) definitions: HashMap<PlaybackAddress, PlaybackDefinition>,
    pub(crate) clock: SharedClock,
    pub(crate) next_activation_ordinal: u64,
    pub(crate) next_transition_ordinal: u64,
    pub(crate) next_source_occurrence_ordinal: u64,
    /// Where a Position fading in from no previous Cue value starts (TL-552).
    pub(crate) family_start: crate::contribution::FamilyStartSlot,
    /// TL-659: Cue transitions waiting for the first output frame that carries them.
    pub(crate) change_lead: light_core::ChangeLeadLedger,
}

impl Default for PlaybackEngine {
    fn default() -> Self {
        Self::with_clock(Arc::new(SystemClock))
    }
}

impl PlaybackEngine {
    pub fn with_clock(clock: SharedClock) -> Self {
        Self {
            cue_lists: HashMap::new(),
            compiled_cue_lists: HashMap::new(),
            active: HashMap::new(),
            jump_counts: HashMap::new(),
            jump_bypass_once: HashSet::new(),
            control_states: HashMap::new(),
            active_dynamics: HashMap::new(),
            cuelist_flash_states: HashMap::new(),
            cuelist_swap_states: HashMap::new(),
            dynamic_flash_states: HashMap::new(),
            temporary: HashMap::new(),
            swap_held: HashSet::new(),
            timeline_controlled: HashMap::new(),
            timeline_transitions: HashMap::new(),
            dynamics_paused_at: None,
            speed_groups_bpm: [120.0, 90.0, 60.0, 30.0, 15.0],
            speed_groups_paused: [false; 5],
            sequence_master_fade_millis: 0,
            release_fade_millis: 0,
            definitions: HashMap::new(),
            clock,
            next_activation_ordinal: 1,
            next_transition_ordinal: 1,
            next_source_occurrence_ordinal: 1,
            family_start: Default::default(),
            change_lead: Default::default(),
        }
    }

    /// Install where a Position that fades in from no previous Cue value starts (TL-552).
    pub fn set_family_start(&mut self, source: Option<Arc<dyn crate::FamilyStartSource>>) {
        self.family_start = crate::contribution::FamilyStartSlot(source);
    }

    pub(crate) fn timeline_time(
        &self,
        playback: &ActivePlayback,
        now: DateTime<Utc>,
    ) -> DateTime<Utc> {
        playback
            .paused_at
            .or_else(|| {
                if playback.temporary {
                    None
                } else {
                    self.timeline_controlled
                        .get(&playback.cue_list_id)
                        .copied()
                        .flatten()
                }
            })
            .unwrap_or(now)
    }

    /// Capture while the caller reads the same active rows for a preserving generation.
    pub fn capture_timeline_clock_state(&self) -> PlaybackTimelineClockState {
        PlaybackTimelineClockState(
            self.timeline_controlled
                .iter()
                .filter_map(|(&id, &sampled_at)| {
                    let active = self.active.get(&PlaybackKey::CueList(id))?;
                    (active.enabled && !active.temporary).then_some(TimelineClockOwner {
                        id,
                        sampled_at,
                        activated_at: active.activated_at,
                        transition_ordinal: active.transition_ordinal,
                        cue_id: active.current_cue_id?,
                        source: active.sequence_master_source(),
                        paused: active.paused,
                        paused_at: active.paused_at,
                    })
                })
                .collect(),
        )
    }

    /// A same-show generation may retain the exact old owner. New/deleted/remapped owners
    /// receive no clock authority, and show activation never calls this preserving operation.
    pub fn restore_timeline_clock_state(&mut self, state: PlaybackTimelineClockState) {
        self.timeline_transitions.clear();
        self.timeline_controlled.clear();
        for owner in state.0 {
            let matches =
                self.active
                    .get(&PlaybackKey::CueList(owner.id))
                    .is_some_and(|active| {
                        active.enabled
                            && !active.temporary
                            && active.activated_at == owner.activated_at
                            && active.transition_ordinal == owner.transition_ordinal
                            && active.current_cue_id == Some(owner.cue_id)
                            && active.sequence_master_source() == owner.source
                            && active.paused == owner.paused
                            && active.paused_at == owner.paused_at
                            && self.cue_lists.get(&owner.id).is_some_and(|list| {
                                list.cues.iter().any(|cue| cue.id == owner.cue_id)
                            })
                    });
            if matches {
                self.timeline_controlled.insert(owner.id, owner.sampled_at);
            }
        }
    }

    pub fn clock(&self) -> SharedClock {
        Arc::clone(&self.clock)
    }

    /// Copy the retained Playback state for isolated preview operations at one captured instant.
    /// Immutable compiled Cue data remains shared, while runtime mutations and time belong to
    /// the fork. Creating the fork does not sample the Live clock or advance any Playback.
    pub fn fork_for_preview(&self, sampled_at: DateTime<Utc>) -> Self {
        let mut preview = self.clone();
        preview.clock = Arc::new(light_core::ManualClock::new(sampled_at));
        preview
    }

    /// Run mutations on an isolated candidate at the transaction's captured instant. Restore
    /// its original clock on both Result success and error before the candidate can install.
    pub fn with_captured_time<T>(
        &mut self,
        at: DateTime<Utc>,
        operation: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let clock = std::mem::replace(&mut self.clock, Arc::new(light_core::ManualClock::new(at)));
        let result = operation(self);
        self.clock = clock;
        result
    }

    pub fn set_external_completion_millis(
        &mut self,
        cue_list_id: CueListId,
        duration_millis: u64,
    ) -> bool {
        let Some(playback) = self.active.get_mut(&PlaybackKey::CueList(cue_list_id)) else {
            return false;
        };
        let changed = playback.external_completion_millis != duration_millis;
        playback.external_completion_millis = duration_millis;
        changed
    }
    pub fn set_control_timing(
        &mut self,
        speed_groups_bpm: [f64; 5],
        sequence_master_fade_millis: u64,
        release_fade_millis: u64,
    ) {
        let next_speed_groups_bpm = speed_groups_bpm.map(|bpm| {
            if bpm.is_finite() {
                bpm.clamp(0.1, 999.0)
            } else {
                120.0
            }
        });
        let now = self.clock.now();
        for playback in self.active.values_mut() {
            let Some(cue_list) = self.cue_lists.get(&playback.cue_list_id) else {
                continue;
            };
            if cue_list.mode != CueListMode::Chaser || cue_list.speed_group.is_none() {
                continue;
            }
            let old_step = effective_chaser_step_millis(cue_list, &self.speed_groups_bpm);
            let next_step = effective_chaser_step_millis(cue_list, &next_speed_groups_bpm);
            if old_step == next_step {
                continue;
            }
            let sampled = self
                .timeline_controlled
                .get(&playback.cue_list_id)
                .copied()
                .flatten();
            let phase_at = playback.paused_at.or(sampled).unwrap_or(now);
            let elapsed = (phase_at - playback.activated_at).num_milliseconds().max(0) as u64;
            let completed_steps = elapsed / old_step;
            advance_chaser_steps(playback, cue_list, completed_steps);
            if completed_steps > 0 {
                // Retiming can replace the previous-value route even at the final Cue.
                playback.source_history = None;
            }
            let old_phase = elapsed % old_step;
            let next_phase =
                ((old_phase as f64 / old_step as f64) * next_step as f64).round() as i64;
            playback.activated_at = phase_at - ChronoDuration::milliseconds(next_phase);
        }
        self.speed_groups_bpm = next_speed_groups_bpm;
        self.sequence_master_fade_millis = sequence_master_fade_millis.min(60_000);
        self.release_fade_millis = release_fade_millis.min(60_000);
    }
    pub fn set_speed_groups_paused(&mut self, paused: [bool; 5]) {
        self.speed_groups_paused = paused;
    }
    pub fn dynamics_paused(&self) -> bool {
        self.dynamics_paused_at.is_some()
    }
    pub fn dynamics_paused_since(&self) -> Option<DateTime<Utc>> {
        self.dynamics_paused_at
    }
    pub fn restore_dynamics_paused_since(&mut self, paused_at: Option<DateTime<Utc>>) {
        self.dynamics_paused_at = paused_at;
    }
    pub fn set_dynamics_paused(&mut self, paused: bool) {
        let now = self.clock.now();
        match (paused, self.dynamics_paused_at) {
            (true, None) => self.dynamics_paused_at = Some(now),
            (false, Some(_)) => {
                self.dynamics_paused_at = None;
            }
            _ => {}
        }
    }
    pub fn toggle_dynamics_paused(&mut self) -> bool {
        let paused = !self.dynamics_paused();
        self.set_dynamics_paused(paused);
        paused
    }
    /// Tell every compiled cue list where the engine's frame keeps its pairs.
    ///
    /// Called when this engine joins a patch generation. A cue list registered afterwards is
    /// offered by name until the next generation resolves it, which is slower and still right.
    pub fn resolve_frame_addresses(&mut self, resolver: &dyn light_core::FrameAddressResolver) {
        for compiled in self.compiled_cue_lists.values_mut() {
            Arc::make_mut(compiled).resolve_frame_addresses(resolver);
        }
    }

    pub fn register(&mut self, mut cue_list: CueList) -> Result<(), String> {
        cue_list.validate()?;
        cue_list.migrate_legacy_chaser_xfade(&self.speed_groups_bpm);
        let compiled = CompiledCueList::new(&cue_list);
        self.compiled_cue_lists
            .insert(cue_list.id, Arc::new(compiled));
        self.cue_lists.insert(cue_list.id, cue_list);
        Ok(())
    }
    pub fn register_definition(
        &mut self,
        address: PlaybackAddress,
        definition: PlaybackDefinition,
    ) -> Result<(), String> {
        definition.validate()?;
        if self.definitions.contains_key(&address) {
            return Err("duplicate Playback address".into());
        }
        if let PlaybackTarget::CueList { cue_list_id } = definition.target {
            if !self.cue_lists.contains_key(&cue_list_id) {
                return Err("playback cue list does not exist".into());
            }
        }
        self.definitions.insert(address, definition);
        Ok(())
    }

    pub fn definition(&self, address: PlaybackAddress) -> Option<&PlaybackDefinition> {
        self.definitions.get(&address)
    }

    pub(crate) fn runtime_key(&self, identity: PlaybackAddress) -> Result<PlaybackKey, String> {
        match self
            .definition(identity)
            .ok_or("playback does not exist")?
            .target
        {
            PlaybackTarget::CueList { cue_list_id } => Ok(PlaybackKey::CueList(cue_list_id)),
            _ => Err("playback does not target a Cuelist".into()),
        }
    }

    pub fn control_state(
        &self,
        address: PlaybackAddress,
        source: &PlaybackControlSource,
    ) -> PlaybackControlState {
        self.control_states
            .get(&PlaybackControlKey {
                address,
                source: source.clone(),
            })
            .copied()
            .unwrap_or_default()
    }
    /// Assignment aggregates expose pickup conservatively; per-surface readers use control_state.
    pub fn control_state_at(&self, address: PlaybackAddress) -> PlaybackControlState {
        self.control_states
            .iter()
            .filter(|(held, _)| held.address == address)
            .find(|(_, state)| state.fader_pickup_required)
            .or_else(|| {
                self.control_states
                    .iter()
                    .find(|(held, _)| held.address == address)
            })
            .map(|(_, state)| *state)
            .unwrap_or_default()
    }
    fn retarget_controls(
        &mut self,
        addresses: HashSet<PlaybackAddress>,
        target: f32,
        controlling: Option<&PlaybackControlKey>,
    ) -> bool {
        let mut changed = false;
        for (held, state) in &mut self.control_states {
            if !addresses.contains(&held.address) {
                continue;
            }
            let required =
                controlling != Some(held) && !(state.observed && state.fader_position == target);
            let pickup_target = required.then_some(target);
            changed |= state.fader_pickup_required != required
                || state.fader_pickup_target != pickup_target;
            state.fader_pickup_required = required;
            state.fader_pickup_target = pickup_target;
        }
        changed
    }
    pub(crate) fn retarget_physical_controls(
        &mut self,
        id: CueListId,
        target: f32,
        controlling: Option<&PlaybackControlKey>,
    ) -> bool {
        let addresses = self
            .definitions
            .iter()
            .filter_map(|(address, definition)| {
                matches!(definition.target,PlaybackTarget::CueList{cue_list_id} if cue_list_id==id)
                    .then_some(*address)
            })
            .collect();
        self.retarget_controls(addresses, target, controlling)
    }
    pub(crate) fn retarget_dynamic_physical_controls(
        &mut self,
        id: Uuid,
        target: f32,
        controlling: Option<&PlaybackControlKey>,
    ) -> bool {
        let addresses = self.dynamic_identities(id).into_iter().collect();
        self.retarget_controls(addresses, target, controlling)
    }
    pub fn retarget_group_physical_controls(
        &mut self,
        id: &str,
        target: f32,
        controlling: Option<&PlaybackControlKey>,
    ) -> bool {
        let addresses = self
            .definitions
            .iter()
            .filter_map(|(address, definition)| {
                matches!(&definition.target,PlaybackTarget::Group{group_id,..} if group_id==id)
                    .then_some(*address)
            })
            .collect();
        self.retarget_controls(addresses, target, controlling)
    }

    pub fn set_group_master_fader_mutation(
        &mut self,
        number: PlaybackAddress,
        source: &PlaybackControlSource,
        value: f32,
        authoritative: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("Group Master fader must be within 0-1".into());
        }
        if !authoritative.is_finite() || !(0.0..=1.0).contains(&authoritative) {
            return Err("authoritative Group Master must be within 0-1".into());
        }
        let definition = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?;
        if !definition.has_fader {
            return Err("playback does not have a fader".into());
        }
        let PlaybackTarget::Group { group_id, .. } = &definition.target else {
            return Err("Playback is not assigned to a Group Master".into());
        };
        let group_id = group_id.clone();
        let identity = PlaybackControlKey::new(number, source)?;
        let mut control_changed = false;
        let state = match self.control_states.entry(identity.clone()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                control_changed = true;
                entry.insert(PlaybackControlState {
                    fader_pickup_required: true,
                    fader_pickup_target: Some(authoritative),
                    ..PlaybackControlState::default()
                })
            }
        };
        let previous = state.fader_position;
        let was_observed = state.observed;
        control_changed |= !was_observed || previous != value;
        state.fader_position = value;
        state.observed = true;
        if state.fader_pickup_required {
            let target = state.fader_pickup_target.unwrap_or(authoritative);
            let crossed = value == target
                || (was_observed && previous == target)
                || (was_observed
                    && ((previous < target && value > target)
                        || (previous > target && value < target)));
            if !crossed {
                return Ok(PlaybackMutation::new(
                    (),
                    if control_changed {
                        PlaybackRuntimeEffect::Transient
                    } else {
                        PlaybackRuntimeEffect::None
                    },
                ));
            }
            state.fader_pickup_required = false;
            state.fader_pickup_target = None;
            control_changed = true;
        }
        control_changed |= self.retarget_group_physical_controls(&group_id, value, Some(&identity));
        Ok(PlaybackMutation::new(
            (),
            if control_changed {
                PlaybackRuntimeEffect::Transient
            } else {
                PlaybackRuntimeEffect::None
            },
        ))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct DynamicFlashState {
    pub(crate) restore_off: bool,
    pub(crate) release_activation_inhibited: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CuelistFlashState {
    pub(crate) restore_off: bool,
    pub(crate) release_activation_inhibited: bool,
}

#[cfg(test)]
mod captured_transaction_clock_tests {
    use super::*;
    use light_core::ApplicationClock;
    #[test]
    fn captured_time_restores_advancing_source_on_success_and_failure() {
        let at = DateTime::from_timestamp(1_000, 0).unwrap();
        let clock = Arc::new(light_core::ManualClock::new(at));
        let mut playback = PlaybackEngine::with_clock(clock.clone());
        for fails in [false, true] {
            clock.advance_millis(1_000);
            let result = playback.with_captured_time(at, |candidate| {
                assert_eq!(candidate.clock().now(), at);
                clock.advance_millis(2_000);
                assert_eq!(candidate.clock().now(), at);
                if fails {
                    Err("later preparation rejected".into())
                } else {
                    Ok(())
                }
            });
            assert_eq!(result.is_err(), fails);
            assert_eq!(playback.clock().now(), clock.now());
        }
        clock.advance_millis(7_000);
        assert_eq!(playback.clock().now(), at + chrono::Duration::seconds(13));
    }
}
