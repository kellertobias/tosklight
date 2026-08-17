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
        if self.cue_lists.contains_key(&id) {
            Ok(PlaybackKey::CueList(id))
        } else {
            Err("cue list does not exist".into())
        }
    }

    pub fn go(&mut self, id: CueListId) -> Result<&ActivePlayback, String> {
        self.go_at(id, self.clock.now())
    }

    pub fn go_at(&mut self, id: CueListId, now: DateTime<Utc>) -> Result<&ActivePlayback, String> {
        self.timeline_controlled.remove(&id);
        let key = self.key_for_cue_list(id)?;
        self.go_at_key(key, id, now)
    }

    pub(crate) fn go_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let interrupted_source = self.transition_source_at(key, now);
        let transition_ordinal = self.take_transition_ordinal();
        let jump_index = self
            .active
            .get(&key)
            .filter(|playback| {
                !playback.paused
                    && playback.loaded_cue_id.is_none()
                    && playback.deleted_cue_hold.is_none()
                    && !self.jump_bypass_once.contains(&id)
            })
            .map(|playback| playback.cue_index)
            .and_then(|index| self.jump_destination(id, index));
        let cue_list = self.cue_lists.get(&id).ok_or("cue list does not exist")?;
        let playback = match self.active.entry(key) {
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(ActivePlayback {
                playback_number: None,
                playback_identity: None,
                activation: None,
                transition_ordinal,
                cue_list_id: id,
                cue_index: 0,
                previous_index: None,
                paused: false,
                activated_at: now,
                paused_at: None,
                completed_trigger_cue_id: None,
                master: 1.0,
                fader_position: 1.0,
                fader_pickup_required: false,
                fader_pickup_target: None,
                flash: false,
                master_transition: None,
                temporary: false,
                enabled: true,
                fader_zero_auto_off_armed: false,
                flash_restore_off: false,
                transition_timing_bypassed: false,
                discrete_cue_actions_suppressed: false,
                transition_fade_fallback_millis: None,
                external_completion_millis: 0,
                manual_xfade_position: 0.0,
                manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
                manual_xfade_from_index: None,
                manual_xfade_to_index: None,
                manual_xfade_progress: 0.0,
                tracking_wrap: false,
                current_cue_id: Some(cue_list.cues[0].id),
                current_cue_number: Some(cue_list.cues[0].number.clone()),
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
                        playback.deleted_cue_transition_source = interrupted_source;
                        playback.previous_index = Some(playback.cue_index);
                    } else {
                        playback.previous_index = None;
                    }
                    playback.cue_index = index;
                    playback.current_cue_id = Some(cue_list.cues[index].id);
                    playback.current_cue_number = Some(cue_list.cues[index].number.clone());
                    playback.loaded_cue_number = None;
                    playback.tracking_wrap = false;
                    playback.paused = false;
                    playback.paused_at = None;
                    playback.activated_at = now;
                    playback.completed_trigger_cue_id = None;
                    playback.transition_ordinal = transition_ordinal;
                    reset_manual_transition(playback);
                    return Ok(playback);
                }
                if let Some(hold) = playback.deleted_cue_hold.take() {
                    if let Some(next) = hold.next_number.as_ref()
                        && let Some(index) =
                            cue_list.cues.iter().position(|cue| cue.number == *next)
                    {
                        playback.deleted_cue_transition_source = Some(hold.contributions.clone());
                        playback.previous_index = None;
                        playback.cue_index = index;
                        playback.current_cue_id = Some(cue_list.cues[index].id);
                        playback.current_cue_number = Some(next.clone());
                        playback.tracking_wrap = false;
                        playback.activated_at = now;
                        playback.completed_trigger_cue_id = None;
                        playback.transition_ordinal = transition_ordinal;
                    } else {
                        playback.deleted_cue_hold = Some(hold);
                    }
                    reset_manual_transition(playback);
                    return Ok(playback);
                }
                let resumed = playback.paused;
                if playback.paused {
                    if let Some(paused_at) = playback.paused_at.take() {
                        playback.activated_at += now - paused_at;
                    }
                    playback.paused = false;
                } else if let Some(index) = jump_index {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index = index;
                    playback.tracking_wrap = false;
                } else if playback.cue_index + 1 < cue_list.cues.len() {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index += 1;
                } else if cue_list.effective_wrap_mode() != WrapMode::Off {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index = 0;
                    playback.tracking_wrap = cue_list.effective_wrap_mode() == WrapMode::Tracking;
                }
                if !resumed {
                    if interrupted_source.is_some() {
                        playback.deleted_cue_transition_source = interrupted_source;
                    }
                    playback.activated_at = now;
                    playback.completed_trigger_cue_id = None;
                }
                playback.transition_ordinal = transition_ordinal;
                playback.current_cue_number =
                    Some(cue_list.cues[playback.cue_index].number.clone());
                playback.current_cue_id = Some(cue_list.cues[playback.cue_index].id);
                playback
            }
        };
        reset_manual_transition(playback);
        Ok(playback)
    }

    fn jump_destination(&mut self, id: CueListId, cue_index: usize) -> Option<usize> {
        let cue_list = self.cue_lists.get(&id)?;
        let cue = cue_list.cues.get(cue_index)?;
        let (destination_id, limit) = cue.actions.iter().find_map(|action| match action {
            CueAction::Jump { cue_id, count } => Some((*cue_id, *count)),
            _ => None,
        })?;
        let arrivals = self.jump_counts.entry((id, cue.id)).or_default();
        *arrivals = arrivals.saturating_add(1);
        (*arrivals <= limit).then(|| {
            cue_list
                .cues
                .iter()
                .position(|candidate| candidate.id == destination_id)
                .expect("validated jump destination remains in the Cuelist")
        })
    }

    pub fn jump(
        &mut self,
        id: CueListId,
        cue_number: CueNumber,
    ) -> Result<&ActivePlayback, String> {
        self.jump_at(id, cue_number, self.clock.now())
    }

    pub fn jump_to_cue_id(
        &mut self,
        id: CueListId,
        cue_id: Uuid,
    ) -> Result<&ActivePlayback, String> {
        let number = self
            .cue_lists
            .get(&id)
            .ok_or("cue list does not exist")?
            .cues
            .iter()
            .find(|cue| cue.id == cue_id)
            .map(|cue| cue.number.clone())
            .ok_or("cue does not exist")?;
        self.jump(id, number)
    }

    pub fn execute_timeline_cue_id(
        &mut self,
        id: CueListId,
        cue_id: Uuid,
    ) -> Result<&ActivePlayback, String> {
        self.jump_to_cue_id(id, cue_id)?;
        self.timeline_controlled.insert(id);
        let key = self.key_for_cue_list(id)?;
        let playback = self.active.get_mut(&key).expect("jump installed playback");
        playback.discrete_cue_actions_suppressed = false;
        Ok(playback)
    }

    pub fn reconstruct_to_cue_id(
        &mut self,
        id: CueListId,
        cue_id: Uuid,
        elapsed_millis: u64,
    ) -> Result<&ActivePlayback, String> {
        let number = self
            .cue_lists
            .get(&id)
            .ok_or("cue list does not exist")?
            .cues
            .iter()
            .find(|cue| cue.id == cue_id)
            .map(|cue| cue.number.clone())
            .ok_or("cue does not exist")?;
        let elapsed =
            chrono::Duration::milliseconds(i64::try_from(elapsed_millis).unwrap_or(i64::MAX));
        self.jump_at(id, number, self.clock.now() - elapsed)?;
        self.timeline_controlled.insert(id);
        let key = self.key_for_cue_list(id)?;
        let playback = self.active.get_mut(&key).expect("jump installed playback");
        playback.discrete_cue_actions_suppressed = true;
        playback.previous_index = None;
        playback.deleted_cue_transition_source = None;
        Ok(playback)
    }

    pub fn jump_at(
        &mut self,
        id: CueListId,
        cue_number: CueNumber,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let key = self.key_for_cue_list(id)?;
        self.jump_at_key(key, id, cue_number, now)
    }

    pub(crate) fn jump_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        cue_number: CueNumber,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let interrupted_source = self.transition_source_at(key, now);
        let transition_ordinal = self.take_transition_ordinal();
        let cue_list = self.cue_lists.get(&id).ok_or("cue list does not exist")?;
        let index = cue_list
            .cues
            .iter()
            .position(|cue| cue.number == cue_number)
            .ok_or("cue does not exist")?;
        let playback = self.active.entry(key).or_insert(ActivePlayback {
            playback_number: None,
            playback_identity: None,
            activation: None,
            transition_ordinal,
            cue_list_id: id,
            cue_index: index,
            previous_index: None,
            paused: false,
            activated_at: now,
            paused_at: None,
            completed_trigger_cue_id: None,
            master: 1.0,
            fader_position: 1.0,
            fader_pickup_required: false,
            fader_pickup_target: None,
            flash: false,
            master_transition: None,
            temporary: false,
            enabled: true,
            fader_zero_auto_off_armed: false,
            flash_restore_off: false,
            transition_timing_bypassed: false,
            discrete_cue_actions_suppressed: false,
            transition_fade_fallback_millis: None,
            external_completion_millis: 0,
            manual_xfade_position: 0.0,
            manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
            manual_xfade_from_index: None,
            manual_xfade_to_index: None,
            manual_xfade_progress: 0.0,
            tracking_wrap: false,
            current_cue_id: Some(cue_list.cues[index].id),
            current_cue_number: Some(cue_list.cues[index].number.clone()),
            deleted_cue_hold: None,
            deleted_cue_transition_source: None,
            loaded_cue_id: None,
            loaded_cue_number: None,
        });
        if interrupted_source.is_some() {
            playback.deleted_cue_transition_source = interrupted_source;
            playback.previous_index = Some(playback.cue_index);
        } else if playback.cue_index != index {
            playback.previous_index = Some(playback.cue_index);
        }
        playback.cue_index = index;
        playback.current_cue_id = Some(cue_list.cues[index].id);
        playback.current_cue_number = Some(cue_number);
        playback.deleted_cue_hold = None;
        playback.loaded_cue_id = None;
        playback.loaded_cue_number = None;
        playback.tracking_wrap = false;
        playback.paused = false;
        playback.paused_at = None;
        playback.activated_at = now;
        playback.completed_trigger_cue_id = None;
        playback.transition_timing_bypassed = false;
        playback.discrete_cue_actions_suppressed = false;
        playback.transition_ordinal = transition_ordinal;
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
        self.timeline_controlled.remove(&id);
        let key = self.key_for_cue_list(id)?;
        self.back_at_key(key, id, now)
    }
    pub(crate) fn back_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let interrupted_source = self.transition_source_at(key, now);
        let transition_ordinal = self.take_transition_ordinal();
        let playback = self.active.get_mut(&key).ok_or("cue list is not active")?;
        reset_manual_transition(playback);
        if let Some(hold) = playback.deleted_cue_hold.take() {
            if let Some(previous) = hold.previous_number.as_ref()
                && let Some(index) = self.cue_lists[&id]
                    .cues
                    .iter()
                    .position(|cue| cue.number == *previous)
            {
                playback.deleted_cue_transition_source = Some(hold.contributions.clone());
                playback.previous_index = None;
                playback.cue_index = index;
                playback.current_cue_id = Some(self.cue_lists[&id].cues[index].id);
                playback.current_cue_number = Some(previous.clone());
                playback.tracking_wrap = false;
                playback.activated_at = now;
                playback.completed_trigger_cue_id = None;
                playback.transition_ordinal = transition_ordinal;
                playback.paused = false;
                playback.paused_at = None;
            } else {
                playback.deleted_cue_hold = Some(hold);
            }
            return Ok(playback);
        }
        playback.deleted_cue_transition_source = interrupted_source;
        playback.previous_index = Some(playback.cue_index);
        playback.cue_index = playback.cue_index.saturating_sub(1);
        playback.current_cue_id = Some(self.cue_lists[&id].cues[playback.cue_index].id);
        playback.current_cue_number =
            Some(self.cue_lists[&id].cues[playback.cue_index].number.clone());
        playback.tracking_wrap = false;
        playback.activated_at = now;
        playback.completed_trigger_cue_id = None;
        playback.transition_ordinal = transition_ordinal;
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
        let key = self.runtime_key(number)?;
        self.pause_key_at_mutation(key, now, "playback is not active")
    }
    pub fn pause_playback_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<()>, String> {
        match identity {
            PlaybackIdentity::Physical(number) => self.pause_playback_mutation(number.get()),
            PlaybackIdentity::Virtual(_) => {
                let key = self.runtime_key_at(identity)?;
                self.pause_key_at_mutation(key, self.clock.now(), "virtual playback is not active")
            }
        }
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
    pub fn resume(&mut self, id: CueListId) -> Result<(), String> {
        let key = self.key_for_cue_list(id)?;
        let now = self.clock.now();
        let playback = self.active.get_mut(&key).ok_or("cue list is not active")?;
        if let Some(paused_at) = playback.paused_at.take() {
            playback.activated_at += now - paused_at;
        }
        playback.paused = false;
        Ok(())
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
        self.timeline_controlled.remove(&id);
        self.jump_counts
            .retain(|(cue_list_id, _), _| *cue_list_id != id);
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
        let key = self.runtime_key(number).ok()?;
        self.active.get(&key)
    }

    pub fn playback_runtime_at(&self, identity: PlaybackIdentity) -> Option<&ActivePlayback> {
        let key = self.runtime_key_at(identity).ok()?;
        self.active.get(&key)
    }

    pub fn is_active_at(&self, identity: PlaybackIdentity) -> bool {
        self.playback_runtime_at(identity)
            .is_some_and(|runtime| runtime.enabled)
            || self
                .active_dynamic_playback_at(identity)
                .is_some_and(|runtime| runtime.enabled)
            || self
                .temporary
                .keys()
                .any(|(candidate, _)| *candidate == identity)
    }

    pub fn release_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<()>, String> {
        self.definition_at(identity)
            .ok_or("playback does not exist")?;
        let dynamic_flash_effect = if self.dynamic_flash_states.contains_key(&identity) {
            self.set_dynamic_flash_at_mutation(identity, false)?.effect
        } else {
            PlaybackRuntimeEffect::None
        };
        let durable = if self.dynamic_assignment_at(identity).is_some() {
            self.off_dynamic_at_mutation(identity)?.value
        } else {
            self.off_at(identity)?
        };
        let before = self.temporary.len();
        self.temporary
            .retain(|(candidate, _), _| *candidate != identity);
        let transient = before != self.temporary.len()
            || self.swap_held.remove(&identity)
            || self.cuelist_flash_states.remove(&identity).is_some()
            || self.cuelist_swap_states.remove(&identity).is_some()
            || dynamic_flash_effect.changed();
        Ok(PlaybackMutation::new(
            (),
            (if durable {
                PlaybackRuntimeEffect::Durable
            } else {
                PlaybackRuntimeEffect::None
            })
            .combine(if transient {
                PlaybackRuntimeEffect::Transient
            } else {
                PlaybackRuntimeEffect::None
            }),
        ))
    }
}
