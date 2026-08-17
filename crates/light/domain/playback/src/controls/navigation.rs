use crate::*;

impl PlaybackEngine {
    pub fn go_playback_at(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<&ActivePlayback, String> {
        match identity {
            PlaybackIdentity::Physical(number) => self.go_playback(number.get()),
            PlaybackIdentity::Virtual(address) => {
                let definition = self
                    .definition_at(identity)
                    .ok_or("virtual playback does not exist")?
                    .clone();
                let PlaybackTarget::CueList { cue_list_id } = definition.target else {
                    return Err("virtual playback does not have cues".into());
                };
                let key = PlaybackKey::CueList(cue_list_id);
                self.disarm_cuelist_flash(cue_list_id);
                let was_active = self
                    .active
                    .get(&key)
                    .is_some_and(|playback| playback.enabled);
                let has_loaded_cue = self
                    .active
                    .get(&key)
                    .is_some_and(|playback| playback.loaded_cue_id.is_some());
                if definition.go_activates && !was_active && !has_loaded_cue {
                    self.on_at(identity)?;
                    return self
                        .active
                        .get(&key)
                        .ok_or_else(|| "virtual playback was automatically switched off".into());
                }
                self.go_at_key(key, cue_list_id, self.clock.now())?;
                let result = self
                    .active
                    .get_mut(&key)
                    .expect("Virtual GO inserted active playback");
                result.playback_number = Some(address.number().get());
                result.playback_identity = Some(identity);
                result.fader_zero_auto_off_armed = false;
                if definition.go_activates && !was_active {
                    result.master = 1.0;
                    result.enabled = true;
                }
                self.auto_off_overwritten();
                self.active
                    .get(&key)
                    .ok_or_else(|| "virtual playback was automatically switched off".into())
            }
        }
    }

    // @tour playback-runtime:20 Advance a Playback
    // GO resolves the assigned Cuelist, honors activation policy and loaded Cues, advances runtime,
    // restores the Playback master when required, and applies automatic exclusion behavior.
    pub fn go_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        let definition = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .clone();
        let PlaybackTarget::CueList { cue_list_id } = definition.target else {
            return Err("group playback does not have cues".into());
        };
        let key = PlaybackKey::CueList(cue_list_id);
        self.disarm_cuelist_flash(cue_list_id);
        let was_active = self
            .active
            .get(&key)
            .is_some_and(|playback| playback.enabled);
        let has_loaded_cue = self
            .active
            .get(&key)
            .is_some_and(|playback| playback.loaded_cue_id.is_some());
        if definition.go_activates && !was_active && !has_loaded_cue {
            self.on(number)?;
            return self
                .active
                .get(&key)
                .ok_or_else(|| "playback was automatically switched off".into());
        }
        self.go_at_key(key, cue_list_id, self.clock.now())?;
        let result = self
            .active
            .get_mut(&key)
            .expect("go inserted active playback");
        result.playback_number = Some(number);
        result.fader_zero_auto_off_armed = false;
        if definition.go_activates && !was_active {
            result.master = 1.0;
            result.enabled = true;
        }
        self.auto_off_overwritten();
        self.active
            .get(&key)
            .ok_or_else(|| "playback was automatically switched off".into())
    }

    pub fn back_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        let id = self.cue_list_for(number)?;
        self.disarm_cuelist_flash(id);
        if let Some(playback) = self.active.get_mut(&PlaybackKey::CueList(id)) {
            playback.fader_zero_auto_off_armed = false;
        }
        self.back_at_key(PlaybackKey::CueList(id), id, self.clock.now())
    }

    pub fn back_playback_at(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<&ActivePlayback, String> {
        match identity {
            PlaybackIdentity::Physical(number) => self.back_playback(number.get()),
            PlaybackIdentity::Virtual(_address) => {
                let definition = self
                    .definition_at(identity)
                    .ok_or("virtual playback does not exist")?;
                let PlaybackTarget::CueList { cue_list_id } = definition.target else {
                    return Err("virtual playback does not have cues".into());
                };
                self.disarm_cuelist_flash(cue_list_id);
                if let Some(playback) = self.active.get_mut(&PlaybackKey::CueList(cue_list_id)) {
                    playback.fader_zero_auto_off_armed = false;
                }
                self.back_at_key(
                    PlaybackKey::CueList(cue_list_id),
                    cue_list_id,
                    self.clock.now(),
                )
            }
        }
    }

    pub fn fast_forward_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        let id = self.cue_list_for(number)?;
        self.reset_current_jump_count(id);
        self.jump_bypass_once.insert(id);
        let advanced = self.go_playback(number).map(|_| ());
        self.jump_bypass_once.remove(&id);
        advanced?;
        let key = self.runtime_key(number)?;
        let playback = self.active.get_mut(&key).ok_or("playback is not active")?;
        playback.transition_timing_bypassed = true;
        Ok(playback)
    }

    pub fn fast_rewind_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        let id = self.cue_list_for(number)?;
        self.reset_current_jump_count(id);
        self.back_playback(number)?;
        let key = self.runtime_key(number)?;
        let playback = self.active.get_mut(&key).ok_or("playback is not active")?;
        playback.transition_timing_bypassed = true;
        Ok(playback)
    }

    pub fn fast_forward_playback_at(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<&ActivePlayback, String> {
        if let PlaybackIdentity::Physical(number) = identity {
            return self.fast_forward_playback(number.get());
        }
        let definition = self
            .definition_at(identity)
            .ok_or("virtual playback does not exist")?;
        let PlaybackTarget::CueList { cue_list_id: id } = definition.target else {
            return Err("virtual playback does not have cues".into());
        };
        self.reset_current_jump_count(id);
        self.jump_bypass_once.insert(id);
        let advanced = self.go_playback_at(identity).map(|_| ());
        self.jump_bypass_once.remove(&id);
        advanced?;
        let key = self.runtime_key_at(identity)?;
        let playback = self
            .active
            .get_mut(&key)
            .ok_or("virtual playback is not active")?;
        playback.transition_timing_bypassed = true;
        Ok(playback)
    }

    pub fn fast_rewind_playback_at(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<&ActivePlayback, String> {
        if let PlaybackIdentity::Physical(number) = identity {
            return self.fast_rewind_playback(number.get());
        }
        let definition = self
            .definition_at(identity)
            .ok_or("virtual playback does not exist")?;
        let PlaybackTarget::CueList { cue_list_id: id } = definition.target else {
            return Err("virtual playback does not have cues".into());
        };
        self.reset_current_jump_count(id);
        self.back_playback_at(identity)?;
        let key = self.runtime_key_at(identity)?;
        let playback = self
            .active
            .get_mut(&key)
            .ok_or("virtual playback is not active")?;
        playback.transition_timing_bypassed = true;
        Ok(playback)
    }

    fn reset_current_jump_count(&mut self, id: CueListId) {
        let Some(cue_id) = self
            .active
            .get(&PlaybackKey::CueList(id))
            .and_then(|playback| playback.current_cue_id)
        else {
            return;
        };
        self.jump_counts.remove(&(id, cue_id));
    }

    fn reset_crossed_jump_counts(&mut self, key: PlaybackKey, id: CueListId, destination_id: Uuid) {
        let Some(from) = self.active.get(&key).map(|playback| playback.cue_index) else {
            return;
        };
        let Some(to) = self.cue_lists[&id]
            .cues
            .iter()
            .position(|cue| cue.id == destination_id)
        else {
            return;
        };
        let crossed = if from < to {
            from..to
        } else {
            to.saturating_add(1)..from.saturating_add(1)
        };
        let cue_ids = crossed
            .filter_map(|index| self.cue_lists[&id].cues.get(index).map(|cue| cue.id))
            .collect::<Vec<_>>();
        for cue_id in cue_ids {
            self.jump_counts.remove(&(id, cue_id));
        }
    }

    pub fn goto_playback(
        &mut self,
        number: u16,
        cue_number: CueNumber,
    ) -> Result<&ActivePlayback, String> {
        self.goto_playback_mutation(number, cue_number)
            .map(|mutation| mutation.value)
    }

    // @tour cue-tracking-and-goto:30 GOTO selects a target, not a history
    // Direct navigation resolves the addressed Cue and installs its runtime identity without
    // replaying intervening operator actions.
    pub fn goto_playback_mutation(
        &mut self,
        number: u16,
        cue_number: CueNumber,
    ) -> Result<PlaybackMutation<&ActivePlayback>, String> {
        let id = self.cue_list_for(number)?;
        let cue = self.cue_lists[&id]
            .cues
            .iter()
            .find(|cue| cue.number == cue_number)
            .ok_or("cue does not exist")?;
        let (cue_id, cue_number) = (cue.id, cue.number.clone());
        let key = PlaybackKey::CueList(id);
        self.reset_crossed_jump_counts(key, id, cue_id);
        self.disarm_cuelist_flash(id);
        let now = self.clock.now();
        let changed = self.active.get(&key).is_none_or(|playback| {
            goto_changes_runtime(playback, number, cue_id, &cue_number, now)
        });
        self.jump_at_key(key, id, cue_number, now)?;
        let playback = self.active.get_mut(&key).unwrap();
        playback.playback_number = Some(number);
        playback.master = 1.0;
        playback.enabled = true;
        playback.fader_zero_auto_off_armed = false;
        playback.loaded_cue_id = None;
        playback.loaded_cue_number = None;
        let addressed_effect = runtime_effect(changed);
        let related_effect = runtime_effect(self.auto_off_overwritten());
        let playback = self
            .active
            .get(&key)
            .ok_or_else(|| "playback was automatically switched off".to_owned())?;
        Ok(PlaybackMutation::with_related_effect(
            playback,
            addressed_effect,
            related_effect,
        ))
    }

    pub fn goto_playback_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        cue_number: CueNumber,
    ) -> Result<PlaybackMutation<&ActivePlayback>, String> {
        if let PlaybackIdentity::Physical(number) = identity {
            return self.goto_playback_mutation(number.get(), cue_number);
        }
        let address = identity
            .virtual_address()
            .expect("physical identity returned above");
        let definition = self
            .definition_at(identity)
            .ok_or("virtual playback does not exist")?;
        let PlaybackTarget::CueList { cue_list_id } = definition.target else {
            return Err("virtual playback does not have cues".into());
        };
        let cue = self.cue_lists[&cue_list_id]
            .cues
            .iter()
            .find(|cue| cue.number == cue_number)
            .ok_or("cue does not exist")?;
        let (cue_id, cue_number) = (cue.id, cue.number.clone());
        let key = PlaybackKey::CueList(cue_list_id);
        self.reset_crossed_jump_counts(key, cue_list_id, cue_id);
        self.disarm_cuelist_flash(cue_list_id);
        let now = self.clock.now();
        let changed = self.active.get(&key).is_none_or(|playback| {
            playback.playback_identity != Some(identity)
                || goto_changes_runtime(playback, address.number().get(), cue_id, &cue_number, now)
        });
        self.jump_at_key(key, cue_list_id, cue_number, now)?;
        let playback = self.active.get_mut(&key).unwrap();
        playback.playback_number = Some(address.number().get());
        playback.playback_identity = Some(identity);
        playback.master = 1.0;
        playback.enabled = true;
        playback.fader_zero_auto_off_armed = false;
        playback.loaded_cue_id = None;
        playback.loaded_cue_number = None;
        let addressed_effect = runtime_effect(changed);
        let related_effect = runtime_effect(self.auto_off_overwritten());
        let playback = self
            .active
            .get(&key)
            .ok_or_else(|| "virtual playback was automatically switched off".to_owned())?;
        Ok(PlaybackMutation::with_related_effect(
            playback,
            addressed_effect,
            related_effect,
        ))
    }

    pub fn load_playback(
        &mut self,
        number: u16,
        cue_number: CueNumber,
    ) -> Result<&ActivePlayback, String> {
        self.load_playback_mutation(number, cue_number)
            .map(|mutation| mutation.value)
    }

    pub fn load_playback_mutation(
        &mut self,
        number: u16,
        cue_number: CueNumber,
    ) -> Result<PlaybackMutation<&ActivePlayback>, String> {
        let id = self.cue_list_for(number)?;
        let cue = self.cue_lists[&id]
            .cues
            .iter()
            .find(|cue| cue.number == cue_number)
            .ok_or("cue does not exist")?;
        let (cue_id, cue_number) = (cue.id, cue.number.clone());
        let key = PlaybackKey::CueList(id);
        let inserted = !self.active.contains_key(&key);
        let now = self.clock.now();
        let playback = self
            .active
            .entry(key)
            .or_insert_with(|| inactive_playback(number, id, now));
        let changed = inserted
            || playback.playback_number != Some(number)
            || playback.loaded_cue_id != Some(cue_id)
            || playback.loaded_cue_number.as_ref() != Some(&cue_number);
        playback.playback_number = Some(number);
        playback.loaded_cue_id = Some(cue_id);
        playback.loaded_cue_number = Some(cue_number);
        let effect = if changed {
            PlaybackRuntimeEffect::Durable
        } else {
            PlaybackRuntimeEffect::None
        };
        Ok(PlaybackMutation::new(playback, effect))
    }

    pub fn load_playback_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        cue_number: CueNumber,
    ) -> Result<PlaybackMutation<&ActivePlayback>, String> {
        if let PlaybackIdentity::Physical(number) = identity {
            return self.load_playback_mutation(number.get(), cue_number);
        }
        let address = identity
            .virtual_address()
            .expect("physical identity returned above");
        let definition = self
            .definition_at(identity)
            .ok_or("virtual playback does not exist")?;
        let PlaybackTarget::CueList { cue_list_id } = definition.target else {
            return Err("virtual playback does not have cues".into());
        };
        let cue = self.cue_lists[&cue_list_id]
            .cues
            .iter()
            .find(|cue| cue.number == cue_number)
            .ok_or("cue does not exist")?;
        let (cue_id, cue_number) = (cue.id, cue.number.clone());
        let key = PlaybackKey::CueList(cue_list_id);
        let inserted = !self.active.contains_key(&key);
        let now = self.clock.now();
        let playback = self.active.entry(key).or_insert_with(|| {
            let mut playback = inactive_playback(address.number().get(), cue_list_id, now);
            playback.playback_identity = Some(identity);
            playback
        });
        let changed = inserted
            || playback.playback_identity != Some(identity)
            || playback.loaded_cue_id != Some(cue_id)
            || playback.loaded_cue_number.as_ref() != Some(&cue_number);
        playback.playback_number = Some(address.number().get());
        playback.playback_identity = Some(identity);
        playback.loaded_cue_id = Some(cue_id);
        playback.loaded_cue_number = Some(cue_number);
        Ok(PlaybackMutation::new(playback, runtime_effect(changed)))
    }
}

fn goto_changes_runtime(
    playback: &ActivePlayback,
    number: u16,
    cue_id: Uuid,
    cue_number: &CueNumber,
    now: DateTime<Utc>,
) -> bool {
    playback.playback_number != Some(number)
        || playback.current_cue_id != Some(cue_id)
        || playback.current_cue_number.as_ref() != Some(cue_number)
        || playback.deleted_cue_hold.is_some()
        || playback.deleted_cue_transition_source.is_some()
        || playback.loaded_cue_id.is_some()
        || playback.loaded_cue_number.is_some()
        || playback.tracking_wrap
        || playback.paused
        || playback.paused_at.is_some()
        || playback.activated_at != now
        || playback.transition_timing_bypassed
        || playback.transition_fade_fallback_millis.is_some()
        || playback.manual_xfade_from_index.is_some()
        || playback.manual_xfade_to_index.is_some()
        || playback.manual_xfade_progress != 0.0
        || playback.master != 1.0
        || !playback.enabled
}

const fn runtime_effect(changed: bool) -> PlaybackRuntimeEffect {
    if changed {
        PlaybackRuntimeEffect::Durable
    } else {
        PlaybackRuntimeEffect::None
    }
}

fn inactive_playback(number: u16, cue_list_id: CueListId, now: DateTime<Utc>) -> ActivePlayback {
    ActivePlayback {
        playback_number: Some(number),
        playback_identity: None,
        activation: None,
        transition_ordinal: 0,
        cue_list_id,
        cue_index: 0,
        previous_index: None,
        paused: false,
        activated_at: now,
        paused_at: None,
        completed_trigger_cue_id: None,
        master: 0.0,
        fader_position: 0.0,
        fader_pickup_required: false,
        fader_pickup_target: None,
        flash: false,
        master_transition: None,
        temporary: false,
        enabled: false,
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
        current_cue_id: None,
        current_cue_number: None,
        deleted_cue_hold: None,
        deleted_cue_transition_source: None,
        loaded_cue_id: None,
        loaded_cue_number: None,
    }
}
