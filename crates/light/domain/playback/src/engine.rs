use crate::*;

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
    pub(crate) control_states: HashMap<PlaybackIdentity, PlaybackControlState>,
    pub(crate) active_dynamics: HashMap<uuid::Uuid, ActiveDynamicPlayback>,
    pub(crate) cuelist_flash_states: HashMap<PlaybackIdentity, CuelistFlashState>,
    pub(crate) cuelist_swap_states: HashMap<PlaybackIdentity, CuelistFlashState>,
    pub(crate) dynamic_flash_states: HashMap<PlaybackIdentity, DynamicFlashState>,
    pub(crate) temporary: HashMap<(PlaybackIdentity, TemporaryPlaybackKind), ActivePlayback>,
    pub(crate) swap_held: HashSet<PlaybackIdentity>,
    pub(crate) dynamics_paused_at: Option<DateTime<Utc>>,
    pub(crate) speed_groups_bpm: [f64; 5],
    pub(crate) speed_groups_paused: [bool; 5],
    pub(crate) sequence_master_fade_millis: u64,
    pub(crate) definitions: HashMap<u16, PlaybackDefinition>,
    pub(crate) virtual_definitions: HashMap<VirtualPlaybackAddress, PlaybackDefinition>,
    pub(crate) clock: SharedClock,
    pub(crate) next_activation_ordinal: u64,
    pub(crate) next_transition_ordinal: u64,
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
            dynamics_paused_at: None,
            speed_groups_bpm: [120.0, 90.0, 60.0, 30.0, 15.0],
            speed_groups_paused: [false; 5],
            sequence_master_fade_millis: 0,
            definitions: HashMap::new(),
            virtual_definitions: HashMap::new(),
            clock,
            next_activation_ordinal: 1,
            next_transition_ordinal: 1,
        }
    }

    pub fn clock(&self) -> SharedClock {
        Arc::clone(&self.clock)
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
            let phase_at = playback.paused_at.unwrap_or(now);
            let elapsed = (phase_at - playback.activated_at).num_milliseconds().max(0) as u64;
            let completed_steps = elapsed / old_step;
            advance_chaser_steps(playback, cue_list, completed_steps);
            let old_phase = elapsed % old_step;
            let next_phase =
                ((old_phase as f64 / old_step as f64) * next_step as f64).round() as i64;
            playback.activated_at = phase_at - ChronoDuration::milliseconds(next_phase);
        }
        self.speed_groups_bpm = next_speed_groups_bpm;
        self.sequence_master_fade_millis = sequence_master_fade_millis.min(60_000);
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
    pub fn register(&mut self, mut cue_list: CueList) -> Result<(), String> {
        cue_list.validate()?;
        cue_list.migrate_legacy_chaser_xfade(&self.speed_groups_bpm);
        let compiled = CompiledCueList::new(&cue_list);
        self.compiled_cue_lists
            .insert(cue_list.id, Arc::new(compiled));
        self.cue_lists.insert(cue_list.id, cue_list);
        Ok(())
    }
    pub fn register_definition(&mut self, definition: PlaybackDefinition) -> Result<(), String> {
        PhysicalPlaybackNumber::new(definition.number)?;
        definition.validate()?;
        if self.definitions.contains_key(&definition.number) {
            return Err("duplicate playback number".into());
        }
        let cue_list_id = match &definition.target {
            PlaybackTarget::CueList { cue_list_id } => Some(*cue_list_id),
            _ => None,
        };
        if let Some(cue_list_id) = cue_list_id
            && !self.cue_lists.contains_key(&cue_list_id)
        {
            return Err("playback cue list does not exist".into());
        }
        self.definitions.insert(definition.number, definition);
        Ok(())
    }

    pub fn definition(&self, number: u16) -> Option<&PlaybackDefinition> {
        self.definitions.get(&number)
    }

    pub fn register_virtual_definition(
        &mut self,
        address: VirtualPlaybackAddress,
        definition: PlaybackDefinition,
    ) -> Result<(), String> {
        definition.validate()?;
        if definition.number != address.number().get() {
            return Err("virtual playback address and definition number must match".into());
        }
        if self.virtual_definitions.contains_key(&address) {
            return Err("duplicate virtual playback address".into());
        }
        if let PlaybackTarget::CueList { cue_list_id } = definition.target
            && !self.cue_lists.contains_key(&cue_list_id)
        {
            return Err("playback cue list does not exist".into());
        }
        self.virtual_definitions.insert(address, definition);
        Ok(())
    }

    pub fn definition_at(&self, identity: PlaybackIdentity) -> Option<&PlaybackDefinition> {
        match identity {
            PlaybackIdentity::Physical(number) => self.definitions.get(&number.get()),
            PlaybackIdentity::Virtual(address) => self.virtual_definitions.get(&address),
        }
    }

    pub(crate) fn runtime_key(&self, number: u16) -> Result<PlaybackKey, String> {
        self.runtime_key_at(PlaybackIdentity::physical(number)?)
    }

    pub(crate) fn runtime_key_at(&self, identity: PlaybackIdentity) -> Result<PlaybackKey, String> {
        match self
            .definition_at(identity)
            .ok_or("playback does not exist")?
            .target
        {
            PlaybackTarget::CueList { cue_list_id } => Ok(PlaybackKey::CueList(cue_list_id)),
            _ => Err("playback does not target a Cuelist".into()),
        }
    }

    pub fn control_state_at(&self, identity: PlaybackIdentity) -> PlaybackControlState {
        self.control_states
            .get(&identity)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn retarget_physical_controls(
        &mut self,
        cue_list_id: CueListId,
        target: f32,
        controlling: Option<PlaybackIdentity>,
    ) -> bool {
        let identities = self
            .definitions
            .values()
            .filter_map(|definition| match definition.target {
                PlaybackTarget::CueList {
                    cue_list_id: candidate,
                } if candidate == cue_list_id
                    && definition.has_fader
                    && definition.fader == PlaybackFaderMode::Master =>
                {
                    PlaybackIdentity::physical(definition.number).ok()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut changed = false;
        for identity in identities {
            let state = self.control_states.entry(identity).or_default();
            let satisfied =
                controlling == Some(identity) || (state.observed && state.fader_position == target);
            let required = !satisfied;
            let pickup_target = required.then_some(target);
            changed |= state.fader_pickup_required != required
                || state.fader_pickup_target != pickup_target;
            state.fader_pickup_required = required;
            state.fader_pickup_target = pickup_target;
        }
        changed
    }

    pub(crate) fn retarget_dynamic_physical_controls(
        &mut self,
        target_id: Uuid,
        target: f32,
        controlling: Option<PlaybackIdentity>,
    ) -> bool {
        let identities = self
            .definitions
            .values()
            .filter_map(|definition| match &definition.target {
                PlaybackTarget::Dynamic { assignment }
                    if assignment.target_id() == target_id
                        && definition.has_fader
                        && assignment.fader_mode != DynamicPlaybackFaderMode::None =>
                {
                    PlaybackIdentity::physical(definition.number).ok()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut changed = false;
        for identity in identities {
            let state = self.control_states.entry(identity).or_default();
            let satisfied =
                controlling == Some(identity) || (state.observed && state.fader_position == target);
            let required = !satisfied;
            let pickup_target = required.then_some(target);
            changed |= state.fader_pickup_required != required
                || state.fader_pickup_target != pickup_target;
            state.fader_pickup_required = required;
            state.fader_pickup_target = pickup_target;
        }
        changed
    }

    pub fn retarget_group_physical_controls(
        &mut self,
        group_id: &str,
        target: f32,
        controlling: Option<PlaybackIdentity>,
    ) -> bool {
        let identities = self
            .definitions
            .values()
            .filter_map(|definition| match &definition.target {
                PlaybackTarget::Group {
                    group_id: candidate,
                    ..
                } if candidate == group_id && definition.has_fader => {
                    PlaybackIdentity::physical(definition.number).ok()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut changed = false;
        for identity in identities {
            let state = self.control_states.entry(identity).or_default();
            let satisfied =
                controlling == Some(identity) || (state.observed && state.fader_position == target);
            let required = !satisfied;
            let pickup_target = required.then_some(target);
            changed |= state.fader_pickup_required != required
                || state.fader_pickup_target != pickup_target;
            state.fader_pickup_required = required;
            state.fader_pickup_target = pickup_target;
        }
        changed
    }

    pub fn set_group_master_fader_mutation(
        &mut self,
        number: u16,
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
        let identity = PlaybackIdentity::physical(number)?;
        let mut control_changed = false;
        let state = match self.control_states.entry(identity) {
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
        control_changed |= self.retarget_group_physical_controls(&group_id, value, Some(identity));
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
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CuelistFlashState {
    pub(crate) restore_off: bool,
}
