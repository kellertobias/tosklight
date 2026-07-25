use crate::*;

impl PlaybackEngine {
    pub(crate) fn cue_list_for(&self, number: u16) -> Result<CueListId, String> {
        match &self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .target
        {
            PlaybackTarget::CueList { cue_list_id } => Ok(*cue_list_id),
            PlaybackTarget::Group { .. } => {
                Err("operation is not available for a group playback".into())
            }
            _ => Err("operation is not available for this playback function".into()),
        }
    }

    fn key_for_cue_list(&self, id: CueListId) -> Result<PlaybackKey, String> {
        let assigned = self
            .definitions
            .values()
            .filter_map(|definition| match definition.target {
                PlaybackTarget::CueList { cue_list_id } if cue_list_id == id => {
                    Some(definition.number)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        match assigned.as_slice() {
            [] => Ok(PlaybackKey::CueList(id)),
            [number] => Ok(PlaybackKey::Number(*number)),
            _ => Err(
                "cue list is assigned to multiple playbacks; address a concrete playback".into(),
            ),
        }
    }

    pub fn go(&mut self, id: CueListId) -> Result<&ActivePlayback, String> {
        self.go_at(id, self.clock.now())
    }

    pub fn go_at(&mut self, id: CueListId, now: DateTime<Utc>) -> Result<&ActivePlayback, String> {
        let key = self.key_for_cue_list(id)?;
        self.go_at_key(key, id, now)
    }

    pub(crate) fn go_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let cue_list = self.cue_lists.get(&id).ok_or("cue list does not exist")?;
        let playback = match self.active.entry(key) {
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(ActivePlayback {
                playback_number: None,
                activation: None,
                cue_list_id: id,
                cue_index: 0,
                previous_index: None,
                paused: false,
                activated_at: now,
                paused_at: None,
                master: 1.0,
                fader_position: 1.0,
                fader_pickup_required: false,
                flash: false,
                master_transition: None,
                temporary: false,
                enabled: true,
                flash_restore_off: false,
                transition_timing_bypassed: false,
                transition_fade_fallback_millis: None,
                manual_xfade_position: 0.0,
                manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
                manual_xfade_from_index: None,
                manual_xfade_to_index: None,
                manual_xfade_progress: 0.0,
                tracking_wrap: false,
                current_cue_id: Some(cue_list.cues[0].id),
                current_cue_number: Some(cue_list.cues[0].number),
                deleted_cue_hold: None,
                deleted_cue_transition_source: None,
                loaded_cue_id: None,
                loaded_cue_number: None,
            }),
            std::collections::hash_map::Entry::Occupied(entry) => {
                let playback = entry.into_mut();
                if let Some(loaded) = playback.loaded_cue_id.take() {
                    let index = cue_list
                        .cues
                        .iter()
                        .position(|cue| cue.id == loaded)
                        .ok_or("loaded cue no longer exists")?;
                    if playback.enabled && playback.current_cue_number.is_some() {
                        playback.previous_index = Some(playback.cue_index);
                    } else {
                        playback.previous_index = None;
                    }
                    playback.cue_index = index;
                    playback.current_cue_id = Some(cue_list.cues[index].id);
                    playback.current_cue_number = Some(cue_list.cues[index].number);
                    playback.loaded_cue_number = None;
                    playback.deleted_cue_transition_source = None;
                    playback.tracking_wrap = false;
                    playback.paused = false;
                    playback.paused_at = None;
                    playback.activated_at = now;
                    reset_manual_transition(playback);
                    return Ok(playback);
                }
                if let Some(hold) = playback.deleted_cue_hold.take() {
                    if let Some(next) = hold.next_number
                        && let Some(index) = cue_list.cues.iter().position(|cue| cue.number == next)
                    {
                        playback.deleted_cue_transition_source = Some(hold.contributions.clone());
                        playback.previous_index = None;
                        playback.cue_index = index;
                        playback.current_cue_id = Some(cue_list.cues[index].id);
                        playback.current_cue_number = Some(next);
                        playback.tracking_wrap = false;
                        playback.activated_at = now;
                    } else {
                        playback.deleted_cue_hold = Some(hold);
                    }
                    reset_manual_transition(playback);
                    return Ok(playback);
                }
                playback.deleted_cue_transition_source = None;
                let resumed = playback.paused;
                if playback.paused {
                    if let Some(paused_at) = playback.paused_at.take() {
                        playback.activated_at += now - paused_at;
                    }
                    playback.paused = false;
                } else if playback.cue_index + 1 < cue_list.cues.len() {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index += 1;
                } else if cue_list.effective_wrap_mode() != WrapMode::Off {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index = 0;
                    playback.tracking_wrap = cue_list.effective_wrap_mode() == WrapMode::Tracking;
                }
                if !resumed {
                    playback.activated_at = now;
                }
                playback.current_cue_number = Some(cue_list.cues[playback.cue_index].number);
                playback.current_cue_id = Some(cue_list.cues[playback.cue_index].id);
                playback
            }
        };
        reset_manual_transition(playback);
        Ok(playback)
    }

    pub fn jump(&mut self, id: CueListId, cue_number: f64) -> Result<&ActivePlayback, String> {
        self.jump_at(id, cue_number, self.clock.now())
    }

    pub fn jump_at(
        &mut self,
        id: CueListId,
        cue_number: f64,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let key = self.key_for_cue_list(id)?;
        self.jump_at_key(key, id, cue_number, now)
    }

    pub(crate) fn jump_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        cue_number: f64,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let cue_list = self.cue_lists.get(&id).ok_or("cue list does not exist")?;
        let index = cue_list
            .cues
            .iter()
            .position(|cue| cue.number == cue_number)
            .ok_or("cue does not exist")?;
        let playback = self.active.entry(key).or_insert(ActivePlayback {
            playback_number: None,
            activation: None,
            cue_list_id: id,
            cue_index: index,
            previous_index: None,
            paused: false,
            activated_at: now,
            paused_at: None,
            master: 1.0,
            fader_position: 1.0,
            fader_pickup_required: false,
            flash: false,
            master_transition: None,
            temporary: false,
            enabled: true,
            flash_restore_off: false,
            transition_timing_bypassed: false,
            transition_fade_fallback_millis: None,
            manual_xfade_position: 0.0,
            manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
            manual_xfade_from_index: None,
            manual_xfade_to_index: None,
            manual_xfade_progress: 0.0,
            tracking_wrap: false,
            current_cue_id: Some(cue_list.cues[index].id),
            current_cue_number: Some(cue_list.cues[index].number),
            deleted_cue_hold: None,
            deleted_cue_transition_source: None,
            loaded_cue_id: None,
            loaded_cue_number: None,
        });
        if playback.cue_index != index {
            playback.previous_index = Some(playback.cue_index);
        }
        playback.cue_index = index;
        playback.current_cue_id = Some(cue_list.cues[index].id);
        playback.current_cue_number = Some(cue_number);
        playback.deleted_cue_hold = None;
        playback.deleted_cue_transition_source = None;
        playback.loaded_cue_id = None;
        playback.loaded_cue_number = None;
        playback.tracking_wrap = false;
        playback.paused = false;
        playback.paused_at = None;
        playback.activated_at = now;
        reset_manual_transition(playback);
        Ok(playback)
    }

    pub fn back(&mut self, id: CueListId) -> Result<&ActivePlayback, String> {
        self.back_at(id, self.clock.now())
    }
    pub fn back_at(
        &mut self,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let key = self.key_for_cue_list(id)?;
        self.back_at_key(key, id, now)
    }
    pub(crate) fn back_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let playback = self.active.get_mut(&key).ok_or("cue list is not active")?;
        reset_manual_transition(playback);
        if let Some(hold) = playback.deleted_cue_hold.take() {
            if let Some(previous) = hold.previous_number
                && let Some(index) = self.cue_lists[&id]
                    .cues
                    .iter()
                    .position(|cue| cue.number == previous)
            {
                playback.deleted_cue_transition_source = Some(hold.contributions.clone());
                playback.previous_index = None;
                playback.cue_index = index;
                playback.current_cue_id = Some(self.cue_lists[&id].cues[index].id);
                playback.current_cue_number = Some(previous);
                playback.tracking_wrap = false;
                playback.activated_at = now;
                playback.paused = false;
                playback.paused_at = None;
            } else {
                playback.deleted_cue_hold = Some(hold);
            }
            return Ok(playback);
        }
        playback.deleted_cue_transition_source = None;
        playback.previous_index = Some(playback.cue_index);
        playback.cue_index = playback.cue_index.saturating_sub(1);
        playback.current_cue_id = Some(self.cue_lists[&id].cues[playback.cue_index].id);
        playback.current_cue_number = Some(self.cue_lists[&id].cues[playback.cue_index].number);
        playback.tracking_wrap = false;
        playback.activated_at = now;
        playback.paused = false;
        playback.paused_at = None;
        Ok(playback)
    }
    pub fn pause(&mut self, id: CueListId) -> Result<(), String> {
        self.pause_mutation(id).map(|_| ())
    }
    pub fn pause_mutation(&mut self, id: CueListId) -> Result<PlaybackMutation<()>, String> {
        self.pause_at_mutation(id, self.clock.now())
    }
    pub fn pause_playback(&mut self, number: u16) -> Result<(), String> {
        self.pause_playback_mutation(number).map(|_| ())
    }
    pub fn pause_playback_mutation(&mut self, number: u16) -> Result<PlaybackMutation<()>, String> {
        let now = self.clock.now();
        let key = PlaybackKey::Number(number);
        self.pause_key_at_mutation(key, now, "playback is not active")
    }
    pub fn pause_at(&mut self, id: CueListId, now: DateTime<Utc>) -> Result<(), String> {
        self.pause_at_mutation(id, now).map(|_| ())
    }
    pub fn pause_at_mutation(
        &mut self,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<PlaybackMutation<()>, String> {
        let key = self.key_for_cue_list(id)?;
        self.pause_key_at_mutation(key, now, "cue list is not active")
    }
    fn pause_key_at_mutation(
        &mut self,
        key: PlaybackKey,
        now: DateTime<Utc>,
        inactive_error: &'static str,
    ) -> Result<PlaybackMutation<()>, String> {
        let playback = self.active.get_mut(&key).ok_or(inactive_error)?;
        if playback.paused {
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
        }
        playback.paused = true;
        playback.paused_at = Some(now);
        Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Durable))
    }
    pub fn release(&mut self, id: CueListId) -> bool {
        self.key_for_cue_list(id)
            .ok()
            .is_some_and(|key| self.active.remove(&key).is_some())
    }
    pub fn active(&self) -> Vec<ActivePlayback> {
        self.active
            .values()
            .filter(|playback| playback.enabled)
            .chain(self.temporary.values())
            .cloned()
            .collect()
    }
    pub fn runtime(&self) -> Vec<ActivePlayback> {
        let mut runtime = self.active.values().cloned().collect::<Vec<_>>();
        runtime.sort_by_key(|playback| playback.playback_number.unwrap_or(u16::MAX));
        runtime
    }
    pub fn playback_runtime(&self, number: u16) -> Option<&ActivePlayback> {
        self.active.get(&PlaybackKey::Number(number))
    }
}
