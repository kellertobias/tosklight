use crate::*;

impl PlaybackEngine {
    pub fn on(&mut self, number: u16) -> Result<(), String> {
        self.on_mutation(number).map(|_| ())
    }

    pub fn on_mutation(&mut self, number: u16) -> Result<PlaybackMutation<()>, String> {
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        let mut changed = false;
        if !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now())?;
            changed = true;
        }
        changed |= self.restart_first_cue_if_needed(key, id);
        changed |= activate_normal(self.active.get_mut(&key).unwrap(), number);
        let addressed_effect = durable_effect(changed);
        let related_effect = durable_effect(self.auto_off_overwritten());
        Ok(PlaybackMutation::with_related_effect(
            (),
            addressed_effect,
            related_effect,
        ))
    }

    fn restart_first_cue_if_needed(&mut self, key: PlaybackKey, id: CueListId) -> bool {
        if !self.should_restart_first(key, id) {
            return false;
        }
        let first = &self.cue_lists[&id].cues[0];
        let (cue_id, cue_number, now) = (first.id, first.number, self.clock.now());
        let active = self.active.get_mut(&key).unwrap();
        active.previous_index = None;
        active.cue_index = 0;
        active.current_cue_id = Some(cue_id);
        active.current_cue_number = Some(cue_number);
        active.deleted_cue_hold = None;
        active.deleted_cue_transition_source = None;
        active.activated_at = now;
        true
    }

    fn should_restart_first(&self, key: PlaybackKey, id: CueListId) -> bool {
        let active = &self.active[&key];
        if active.enabled {
            return false;
        }
        if self.cue_lists[&id].restart_mode == RestartMode::FirstCue {
            return true;
        }
        match active.current_cue_id {
            Some(current_id) => !self.cue_lists[&id]
                .cues
                .iter()
                .any(|cue| cue.id == current_id),
            None => !active.current_cue_number.is_some_and(|current_number| {
                self.cue_lists[&id]
                    .cues
                    .iter()
                    .any(|cue| cue.number == current_number)
            }),
        }
    }

    pub fn off(&mut self, number: u16) -> Result<bool, String> {
        self.off_mutation(number).map(|mutation| mutation.value)
    }

    pub fn off_mutation(&mut self, number: u16) -> Result<PlaybackMutation<bool>, String> {
        self.cue_list_for(number)?;
        let Some(playback) = self.active.get_mut(&PlaybackKey::Number(number)) else {
            return Ok(PlaybackMutation::new(false, PlaybackRuntimeEffect::None));
        };
        let was_enabled = playback.enabled;
        let changed = deactivate(playback);
        Ok(PlaybackMutation::new(was_enabled, durable_effect(changed)))
    }

    pub fn toggle(&mut self, number: u16) -> Result<bool, String> {
        self.toggle_mutation(number).map(|mutation| mutation.value)
    }

    pub fn toggle_mutation(&mut self, number: u16) -> Result<PlaybackMutation<bool>, String> {
        self.cue_list_for(number)?;
        if self
            .playback_runtime(number)
            .is_some_and(|playback| playback.enabled)
        {
            return self
                .off_mutation(number)
                .map(|mutation| mutation.map(|_| false));
        }
        self.on_mutation(number)
            .map(|mutation| mutation.map(|_| true))
    }

    pub fn set_master(&mut self, number: u16, value: f32) -> Result<(), String> {
        self.set_master_mutation(number, value).map(|_| ())
    }

    pub fn set_master_mutation(
        &mut self,
        number: u16,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_master_inner_mutation(number, value, false)
    }

    /// Set the authoritative level through a virtual fader supplied by a remote control
    /// protocol. Faderless/button-only layouts intentionally have no local fader, but their
    /// playback master remains a valid runtime control and feedback source.
    pub fn set_virtual_master(&mut self, number: u16, value: f32) -> Result<(), String> {
        self.set_virtual_master_mutation(number, value).map(|_| ())
    }

    pub fn set_virtual_master_mutation(
        &mut self,
        number: u16,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_master_inner_mutation(number, value, true)
    }

    fn set_master_inner_mutation(
        &mut self,
        number: u16,
        value: f32,
        allow_faderless: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        let mode = self.validate_master(number, value, allow_faderless)?;
        match mode {
            PlaybackFaderMode::Temp => return self.set_temp_fader_mutation(number, value),
            PlaybackFaderMode::XFade => {
                return self.set_manual_xfade_inner_mutation(number, value, allow_faderless);
            }
            PlaybackFaderMode::Master => {}
            _ => return Err("fader mode is not handled by the Cuelist engine".into()),
        }
        self.set_cuelist_master_mutation(number, value)
    }

    fn validate_master(
        &self,
        number: u16,
        value: f32,
        allow_faderless: bool,
    ) -> Result<PlaybackFaderMode, String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("playback master must be within 0-1".into());
        }
        let definition = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?;
        if !definition.has_fader && !allow_faderless {
            return Err("playback does not have a fader".into());
        }
        Ok(definition.fader)
    }

    fn set_cuelist_master_mutation(
        &mut self,
        number: u16,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        if let Some(effect) = self.update_fader_pickup(key, value) {
            return Ok(PlaybackMutation::new((), effect));
        }
        let mut changed = self
            .active
            .get(&key)
            .is_some_and(|active| active.fader_position != value);
        if value > 0.0 && !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now())?;
            changed = true;
        }
        if let Some(active) = self.active.get_mut(&key) {
            changed |= set_master_state(active, number, value);
        }
        let addressed_effect = durable_effect(changed);
        let related_effect = durable_effect(self.auto_off_overwritten());
        Ok(PlaybackMutation::with_related_effect(
            (),
            addressed_effect,
            related_effect,
        ))
    }

    fn update_fader_pickup(
        &mut self,
        key: PlaybackKey,
        value: f32,
    ) -> Option<PlaybackRuntimeEffect> {
        let active = self.active.get_mut(&key)?;
        if !active.fader_pickup_required {
            return None;
        }
        let position_changed = active.fader_position != value;
        active.fader_position = value;
        let released = value == 0.0;
        let changed = position_changed || released;
        if released {
            active.fader_pickup_required = false;
            active.master = 0.0;
        }
        Some(durable_effect(changed))
    }

    pub fn set_flash(&mut self, number: u16, pressed: bool) -> Result<(), String> {
        self.set_flash_mutation(number, pressed).map(|_| ())
    }

    pub fn set_flash_mutation(
        &mut self,
        number: u16,
        pressed: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        self.cue_list_for(number)?;
        let key = (number, TemporaryPlaybackKind::Flash);
        if pressed {
            if self.temporary.contains_key(&key) {
                return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
            }
            let playback = self.temporary_playback(number, 1.0, true)?;
            self.temporary.insert(key, playback);
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Transient));
        }
        let Some(released) = self.temporary.remove(&key) else {
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
        };
        let promoted = self.definitions[&number].flash_release
            == FlashReleaseMode::ReleaseIntensityOnly
            && self.promote_intensity_release(number, released, true);
        let effect = PlaybackRuntimeEffect::Transient.combine(durable_effect(promoted));
        Ok(PlaybackMutation::new((), effect))
    }
}

fn durable_effect(changed: bool) -> PlaybackRuntimeEffect {
    if changed {
        PlaybackRuntimeEffect::Durable
    } else {
        PlaybackRuntimeEffect::None
    }
}

fn activate_normal(playback: &mut ActivePlayback, number: u16) -> bool {
    let changed = playback.playback_number != Some(number)
        || playback.master != 1.0
        || !playback.enabled
        || playback.temporary
        || playback.fader_pickup_required
        || playback.master_transition.is_some()
        || playback.deleted_cue_transition_source.is_some()
        || playback.transition_timing_bypassed
        || playback.transition_fade_fallback_millis.is_some()
        || playback.manual_xfade_from_index.is_some()
        || playback.manual_xfade_to_index.is_some()
        || playback.manual_xfade_progress != 0.0;
    playback.playback_number = Some(number);
    playback.master = 1.0;
    playback.enabled = true;
    playback.temporary = false;
    playback.fader_pickup_required = false;
    playback.master_transition = None;
    playback.deleted_cue_transition_source = None;
    reset_manual_transition(playback);
    changed
}

fn deactivate(playback: &mut ActivePlayback) -> bool {
    let pickup_required = playback.fader_position > 0.0;
    let changed = playback.enabled
        || playback.flash
        || playback.fader_pickup_required != pickup_required
        || playback.master_transition.is_some()
        || playback.deleted_cue_hold.is_some()
        || playback.deleted_cue_transition_source.is_some()
        || playback.loaded_cue_id.is_some()
        || playback.loaded_cue_number.is_some();
    playback.enabled = false;
    playback.activation = None;
    playback.flash = false;
    playback.fader_pickup_required = pickup_required;
    playback.master_transition = None;
    playback.deleted_cue_hold = None;
    playback.deleted_cue_transition_source = None;
    playback.loaded_cue_id = None;
    playback.loaded_cue_number = None;
    changed
}

fn set_master_state(playback: &mut ActivePlayback, number: u16, value: f32) -> bool {
    let enables = value > 0.0;
    let changed = playback.playback_number != Some(number)
        || playback.master != value
        || playback.fader_position != value
        || playback.master_transition.is_some()
        || playback.temporary
        || (enables && !playback.enabled);
    playback.playback_number = Some(number);
    playback.master = value;
    playback.fader_position = value;
    playback.master_transition = None;
    playback.temporary = false;
    if enables {
        playback.enabled = true;
    }
    changed
}
