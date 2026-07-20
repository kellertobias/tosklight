use crate::*;

#[derive(Clone, Debug)]
pub struct PlaybackEngine {
    pub(crate) cue_lists: HashMap<CueListId, CueList>,
    pub(crate) compiled_cue_lists: HashMap<CueListId, Arc<CompiledCueList>>,
    pub(crate) active: HashMap<PlaybackKey, ActivePlayback>,
    pub(crate) temporary: HashMap<(u16, TemporaryPlaybackKind), ActivePlayback>,
    pub(crate) swap_held: HashSet<u16>,
    pub(crate) dynamics_paused_at: Option<DateTime<Utc>>,
    pub(crate) speed_groups_bpm: [f64; 5],
    pub(crate) speed_groups_paused: [bool; 5],
    pub(crate) sequence_master_fade_millis: u64,
    pub(crate) definitions: HashMap<u16, PlaybackDefinition>,
    pub(crate) clock: SharedClock,
    pub(crate) next_activation_ordinal: u64,
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
            temporary: HashMap::new(),
            swap_held: HashSet::new(),
            dynamics_paused_at: None,
            speed_groups_bpm: [120.0, 90.0, 60.0, 30.0, 15.0],
            speed_groups_paused: [false; 5],
            sequence_master_fade_millis: 0,
            definitions: HashMap::new(),
            clock,
            next_activation_ordinal: 1,
        }
    }

    pub fn clock(&self) -> SharedClock {
        Arc::clone(&self.clock)
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
            (false, Some(paused_at)) => {
                let shift_timestamp = |timestamp: &mut DateTime<Utc>| {
                    if *timestamp <= paused_at {
                        *timestamp += now - paused_at;
                    } else {
                        *timestamp = now;
                    }
                };
                for playback in self.active.values_mut().chain(self.temporary.values_mut()) {
                    shift_timestamp(&mut playback.activated_at);
                    if let Some(paused) = &mut playback.paused_at {
                        shift_timestamp(paused);
                    }
                    if let Some(transition) = &mut playback.master_transition {
                        shift_timestamp(&mut transition.started_at);
                    }
                }
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
        definition.validate()?;
        if self.definitions.contains_key(&definition.number) {
            return Err("duplicate playback number".into());
        }
        let cue_list_id = match &definition.target {
            PlaybackTarget::CueList { cue_list_id } => Some(*cue_list_id),
            _ => None,
        };
        if let Some(cue_list_id) = cue_list_id {
            if !self.cue_lists.contains_key(&cue_list_id) {
                return Err("playback cue list does not exist".into());
            }
            let first_assignment = !self.definitions.values().any(|existing| matches!(existing.target, PlaybackTarget::CueList { cue_list_id: existing_id } if existing_id == cue_list_id));
            if first_assignment
                && let Some(mut playback) = self.active.remove(&PlaybackKey::CueList(cue_list_id))
            {
                playback.playback_number = Some(definition.number);
                self.active
                    .insert(PlaybackKey::Number(definition.number), playback);
            }
        }
        self.definitions.insert(definition.number, definition);
        Ok(())
    }

    pub fn definition(&self, number: u16) -> Option<&PlaybackDefinition> {
        self.definitions.get(&number)
    }
}
