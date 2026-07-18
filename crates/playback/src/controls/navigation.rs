use crate::*;

impl PlaybackEngine {
    pub fn go_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        let definition = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .clone();
        let PlaybackTarget::CueList { cue_list_id } = definition.target else {
            return Err("group playback does not have cues".into());
        };
        let key = PlaybackKey::Number(number);
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
        self.back_at_key(PlaybackKey::Number(number), id, self.clock.now())
    }

    pub fn fast_forward_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        self.go_playback(number)?;
        let playback = self
            .active
            .get_mut(&PlaybackKey::Number(number))
            .ok_or("playback is not active")?;
        playback.transition_timing_bypassed = true;
        Ok(playback)
    }

    pub fn fast_rewind_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        self.back_playback(number)?;
        let playback = self
            .active
            .get_mut(&PlaybackKey::Number(number))
            .ok_or("playback is not active")?;
        playback.transition_timing_bypassed = true;
        Ok(playback)
    }

    pub fn goto_playback(
        &mut self,
        number: u16,
        cue_number: f64,
    ) -> Result<&ActivePlayback, String> {
        let id = self.cue_list_for(number)?;
        if !self.cue_lists[&id]
            .cues
            .iter()
            .any(|cue| cue.number == cue_number)
        {
            return Err("cue does not exist".into());
        }
        let key = PlaybackKey::Number(number);
        self.jump_at_key(key, id, cue_number, self.clock.now())?;
        let playback = self.active.get_mut(&key).unwrap();
        playback.playback_number = Some(number);
        playback.master = 1.0;
        playback.enabled = true;
        playback.loaded_cue_id = None;
        playback.loaded_cue_number = None;
        self.auto_off_overwritten();
        self.active
            .get(&key)
            .ok_or_else(|| "playback was automatically switched off".into())
    }

    pub fn load_playback(
        &mut self,
        number: u16,
        cue_number: f64,
    ) -> Result<&ActivePlayback, String> {
        let id = self.cue_list_for(number)?;
        let cue = self.cue_lists[&id]
            .cues
            .iter()
            .find(|cue| cue.number == cue_number)
            .ok_or("cue does not exist")?;
        let key = PlaybackKey::Number(number);
        let playback = self.active.entry(key).or_insert(ActivePlayback {
            playback_number: Some(number),
            cue_list_id: id,
            cue_index: 0,
            previous_index: None,
            paused: false,
            activated_at: self.clock.now(),
            paused_at: None,
            master: 0.0,
            fader_position: 0.0,
            fader_pickup_required: false,
            flash: false,
            master_transition: None,
            temporary: false,
            enabled: false,
            flash_restore_off: false,
            transition_timing_bypassed: false,
            transition_fade_fallback_millis: None,
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
        });
        playback.playback_number = Some(number);
        playback.loaded_cue_id = Some(cue.id);
        playback.loaded_cue_number = Some(cue.number);
        Ok(playback)
    }
}
