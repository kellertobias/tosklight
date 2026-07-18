use crate::*;

impl PlaybackEngine {
    pub(crate) fn temporary_playback(
        &self,
        number: u16,
        master: f32,
        flash: bool,
    ) -> Result<ActivePlayback, String> {
        let cue_list_id = self.cue_list_for(number)?;
        let cue_list = self
            .cue_lists
            .get(&cue_list_id)
            .ok_or("playback cue list does not exist")?;
        let now = self.clock.now();
        let mut playback = self
            .active
            .get(&PlaybackKey::Number(number))
            .cloned()
            .unwrap_or_else(|| new_active_playback(Some(number), cue_list, now, master, true));
        playback.playback_number = Some(number);
        playback.enabled = true;
        playback.temporary = true;
        playback.flash = flash;
        playback.master = master;
        playback.fader_position = master;
        playback.fader_pickup_required = false;
        playback.master_transition = None;
        playback.activated_at = now + ChronoDuration::microseconds(1);
        playback.paused = false;
        playback.paused_at = None;
        playback.previous_index = None;
        playback.transition_timing_bypassed = true;
        playback.transition_fade_fallback_millis = None;
        playback.manual_xfade_from_index = None;
        playback.manual_xfade_to_index = None;
        playback.manual_xfade_progress = 0.0;
        Ok(playback)
    }

    pub fn toggle_temp(&mut self, number: u16) -> Result<bool, String> {
        let key = (number, TemporaryPlaybackKind::TempButton);
        if self.temporary.remove(&key).is_some() {
            return Ok(false);
        }
        let playback = self.temporary_playback(number, 1.0, false)?;
        self.temporary.insert(key, playback);
        Ok(true)
    }

    pub fn set_temp_button(&mut self, number: u16, active: bool) -> Result<(), String> {
        let key = (number, TemporaryPlaybackKind::TempButton);
        if active {
            if !self.temporary.contains_key(&key) {
                let playback = self.temporary_playback(number, 1.0, false)?;
                self.temporary.insert(key, playback);
            }
        } else {
            self.cue_list_for(number)?;
            self.temporary.remove(&key);
        }
        Ok(())
    }

    pub fn set_temp_fader(&mut self, number: u16, value: f32) -> Result<(), String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("playback Temp fader must be within 0-1".into());
        }
        let key = (number, TemporaryPlaybackKind::TempFader);
        if value == 0.0 {
            self.temporary.remove(&key);
            return Ok(());
        }
        if let Some(playback) = self.temporary.get_mut(&key) {
            playback.master = value;
            playback.fader_position = value;
        } else {
            let playback = self.temporary_playback(number, value, false)?;
            self.temporary.insert(key, playback);
        }
        Ok(())
    }

    pub fn set_swap(&mut self, number: u16, pressed: bool) -> Result<(), String> {
        self.cue_list_for(number)?;
        let key = (number, TemporaryPlaybackKind::Swap);
        if pressed {
            let playback = self.temporary_playback(number, 1.0, true)?;
            self.temporary.insert(key, playback);
            self.swap_held.insert(number);
        } else {
            let released = self.temporary.remove(&key);
            self.swap_held.remove(&number);
            if self.definitions[&number].flash_release == FlashReleaseMode::ReleaseIntensityOnly
                && let Some(mut released) = released
            {
                let active = self
                    .active
                    .entry(PlaybackKey::Number(number))
                    .or_insert_with(|| {
                        released.temporary = false;
                        released.flash = false;
                        released
                    });
                active.enabled = true;
                active.master = 0.0;
                active.temporary = false;
                active.flash = false;
            }
        }
        Ok(())
    }
    pub fn button(&mut self, number: u16, button: u8, pressed: bool) -> Result<(), String> {
        let action = *self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .buttons
            .get(button.checked_sub(1).ok_or("button must be within 1-3")? as usize)
            .ok_or("button must be within 1-3")?;
        if button > self.definitions[&number].button_count {
            return Err("button is not present on this playback".into());
        }
        if !pressed
            && !matches!(
                action,
                PlaybackButtonAction::Flash | PlaybackButtonAction::Swap
            )
        {
            return Ok(());
        }
        match action {
            PlaybackButtonAction::On => self.on(number),
            PlaybackButtonAction::Off => self.off(number).map(|_| ()),
            PlaybackButtonAction::Toggle => self.toggle(number).map(|_| ()),
            PlaybackButtonAction::Go => self.go_playback(number).map(|_| ()),
            PlaybackButtonAction::GoMinus => self.back_playback(number).map(|_| ()),
            PlaybackButtonAction::FastForward => self.fast_forward_playback(number).map(|_| ()),
            PlaybackButtonAction::FastRewind => self.fast_rewind_playback(number).map(|_| ()),
            PlaybackButtonAction::Flash => self.set_flash(number, pressed),
            PlaybackButtonAction::Temp => {
                if pressed {
                    self.toggle_temp(number).map(|_| ())
                } else {
                    Ok(())
                }
            }
            PlaybackButtonAction::Swap => self.set_swap(number, pressed),
            PlaybackButtonAction::Select
            | PlaybackButtonAction::SelectContents
            | PlaybackButtonAction::SelectDereferenced
            | PlaybackButtonAction::Learn
            | PlaybackButtonAction::Double
            | PlaybackButtonAction::Half
            | PlaybackButtonAction::Pause
            | PlaybackButtonAction::Blackout
            | PlaybackButtonAction::PauseDynamics => Ok(()),
            PlaybackButtonAction::None => Ok(()),
        }
    }
}
