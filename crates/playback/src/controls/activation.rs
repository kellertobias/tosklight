use crate::*;

impl PlaybackEngine {
    pub fn on(&mut self, number: u16) -> Result<(), String> {
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        if !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now())?;
        }
        let should_restart_first = self.active.get(&key).is_some_and(|active| {
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
        });
        if should_restart_first {
            let active = self.active.get_mut(&key).unwrap();
            active.previous_index = None;
            active.cue_index = 0;
            active.current_cue_id = Some(self.cue_lists[&id].cues[0].id);
            active.current_cue_number = Some(self.cue_lists[&id].cues[0].number);
            active.deleted_cue_hold = None;
            active.deleted_cue_transition_source = None;
            active.activated_at = self.clock.now();
        }
        let active = self.active.get_mut(&key).unwrap();
        active.playback_number = Some(number);
        active.master = 1.0;
        active.enabled = true;
        active.temporary = false;
        active.fader_pickup_required = false;
        active.master_transition = None;
        active.deleted_cue_transition_source = None;
        reset_manual_transition(active);
        self.auto_off_overwritten();
        Ok(())
    }

    pub fn off(&mut self, number: u16) -> Result<bool, String> {
        self.cue_list_for(number)?;
        Ok(self
            .active
            .get_mut(&PlaybackKey::Number(number))
            .map(|playback| {
                let was = playback.enabled;
                playback.enabled = false;
                playback.flash = false;
                playback.fader_pickup_required = playback.fader_position > 0.0;
                playback.master_transition = None;
                playback.deleted_cue_hold = None;
                playback.deleted_cue_transition_source = None;
                playback.loaded_cue_id = None;
                playback.loaded_cue_number = None;
                was
            })
            .unwrap_or(false))
    }
    pub fn toggle(&mut self, number: u16) -> Result<bool, String> {
        self.cue_list_for(number)?;
        if self
            .active
            .get(&PlaybackKey::Number(number))
            .is_some_and(|playback| playback.enabled)
        {
            self.off(number)?;
            Ok(false)
        } else {
            self.on(number)?;
            Ok(true)
        }
    }
    pub fn set_master(&mut self, number: u16, value: f32) -> Result<(), String> {
        self.set_master_inner(number, value, false)
    }

    /// Set the authoritative level through a virtual fader supplied by a remote control
    /// protocol. Faderless/button-only layouts intentionally have no local fader, but their
    /// playback master remains a valid runtime control and feedback source.
    pub fn set_virtual_master(&mut self, number: u16, value: f32) -> Result<(), String> {
        self.set_master_inner(number, value, true)
    }

    fn set_master_inner(
        &mut self,
        number: u16,
        value: f32,
        allow_faderless: bool,
    ) -> Result<(), String> {
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
        match definition.fader {
            PlaybackFaderMode::Temp => return self.set_temp_fader(number, value),
            PlaybackFaderMode::XFade => {
                return self.set_manual_xfade_inner(number, value, allow_faderless);
            }
            PlaybackFaderMode::Master => {}
            _ => return Err("fader mode is not handled by the Cuelist engine".into()),
        }
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        if let Some(active) = self.active.get_mut(&key) {
            active.fader_position = value;
            if active.fader_pickup_required {
                if value == 0.0 {
                    active.fader_pickup_required = false;
                    active.master = 0.0;
                }
                return Ok(());
            }
        }
        if value > 0.0 && !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now())?;
        }
        if let Some(active) = self.active.get_mut(&key) {
            active.playback_number = Some(number);
            active.master = value;
            active.fader_position = value;
            active.master_transition = None;
            active.temporary = false;
            if value > 0.0 {
                active.enabled = true;
            }
        }
        self.auto_off_overwritten();
        Ok(())
    }
    pub fn set_flash(&mut self, number: u16, pressed: bool) -> Result<(), String> {
        self.cue_list_for(number)?;
        if pressed {
            let playback = self.temporary_playback(number, 1.0, true)?;
            self.temporary
                .insert((number, TemporaryPlaybackKind::Flash), playback);
        } else {
            let released = self
                .temporary
                .remove(&(number, TemporaryPlaybackKind::Flash));
            if self.definitions[&number].flash_release == FlashReleaseMode::ReleaseIntensityOnly
                && let Some(mut released) = released
            {
                let key = PlaybackKey::Number(number);
                let active = self.active.entry(key).or_insert_with(|| {
                    released.temporary = false;
                    released.flash = false;
                    released
                });
                active.enabled = true;
                active.master = 0.0;
                active.flash = false;
                active.temporary = false;
                active.flash_restore_off = false;
            }
        }
        Ok(())
    }
}
