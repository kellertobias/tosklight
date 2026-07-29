use crate::*;

impl PlaybackEngine {
    pub(crate) fn temporary_playback_at(
        &self,
        identity: PlaybackIdentity,
        master: f32,
        flash: bool,
    ) -> Result<ActivePlayback, String> {
        let number = identity.number();
        let definition = self
            .definition_at(identity)
            .ok_or("playback does not exist")?;
        let PlaybackTarget::CueList { cue_list_id } = definition.target else {
            return Err("temporary control is available only for Cuelist playbacks".into());
        };
        let cue_list = self
            .cue_lists
            .get(&cue_list_id)
            .ok_or("playback cue list does not exist")?;
        let now = self.clock.now();
        let active_key = match identity {
            PlaybackIdentity::Physical(_) => PlaybackKey::Number(number),
            PlaybackIdentity::Virtual(address) => PlaybackKey::Virtual(address),
        };
        let mut playback = self
            .active
            .get(&active_key)
            .cloned()
            .unwrap_or_else(|| new_active_playback(Some(number), cue_list, now, master, true));
        playback.playback_number = Some(number);
        playback.playback_identity = identity.virtual_address().map(PlaybackIdentity::Virtual);
        playback.enabled = true;
        playback.temporary = true;
        playback.flash = flash;
        playback.master = master;
        playback.fader_position = master;
        playback.fader_pickup_required = false;
        playback.fader_pickup_target = None;
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
        self.toggle_temp_mutation(number)
            .map(|mutation| mutation.value)
    }

    pub fn toggle_temp_mutation(&mut self, number: u16) -> Result<PlaybackMutation<bool>, String> {
        self.toggle_temp_at_mutation(PlaybackIdentity::physical(number)?)
    }

    pub fn toggle_temp_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<bool>, String> {
        let key = (identity, TemporaryPlaybackKind::TempButton);
        if self.temporary.remove(&key).is_some() {
            return Ok(PlaybackMutation::new(
                false,
                PlaybackRuntimeEffect::Transient,
            ));
        }
        let playback = self.temporary_playback_at(identity, 1.0, false)?;
        self.temporary.insert(key, playback);
        Ok(PlaybackMutation::new(
            true,
            PlaybackRuntimeEffect::Transient,
        ))
    }

    pub fn set_temp_button(&mut self, number: u16, active: bool) -> Result<(), String> {
        self.set_temp_button_mutation(number, active).map(|_| ())
    }

    pub fn set_temp_button_mutation(
        &mut self,
        number: u16,
        active: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_temp_button_at_mutation(PlaybackIdentity::physical(number)?, active)
    }

    pub fn set_temp_button_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        active: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        let key = (identity, TemporaryPlaybackKind::TempButton);
        if active {
            if self.temporary.contains_key(&key) {
                return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
            }
            let playback = self.temporary_playback_at(identity, 1.0, false)?;
            self.temporary.insert(key, playback);
        } else {
            self.definition_at(identity)
                .ok_or("playback does not exist")?;
            if self.temporary.remove(&key).is_none() {
                return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
            }
        }
        Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Transient))
    }

    pub fn set_temp_fader(&mut self, number: u16, value: f32) -> Result<(), String> {
        self.set_temp_fader_mutation(number, value).map(|_| ())
    }

    pub fn set_temp_fader_mutation(
        &mut self,
        number: u16,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_temp_fader_at_mutation(PlaybackIdentity::physical(number)?, value)
    }

    pub fn set_temp_fader_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("playback Temp fader must be within 0-1".into());
        }
        let key = (identity, TemporaryPlaybackKind::TempFader);
        if value == 0.0 {
            let effect = if self.temporary.remove(&key).is_some() {
                PlaybackRuntimeEffect::Transient
            } else {
                PlaybackRuntimeEffect::None
            };
            return Ok(PlaybackMutation::new((), effect));
        }
        if let Some(playback) = self.temporary.get_mut(&key) {
            if playback.master == value && playback.fader_position == value {
                return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
            }
            playback.master = value;
            playback.fader_position = value;
        } else {
            let playback = self.temporary_playback_at(identity, value, false)?;
            self.temporary.insert(key, playback);
        }
        Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Transient))
    }

    pub fn set_swap(&mut self, number: u16, pressed: bool) -> Result<(), String> {
        self.set_swap_mutation(number, pressed).map(|_| ())
    }

    pub fn set_swap_mutation(
        &mut self,
        number: u16,
        pressed: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_swap_at_mutation(PlaybackIdentity::physical(number)?, pressed)
    }

    pub fn set_swap_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        pressed: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        self.definition_at(identity)
            .ok_or("playback does not exist")?;
        let key = (identity, TemporaryPlaybackKind::Swap);
        if pressed {
            let mut changed = self.swap_held.insert(identity);
            if !self.temporary.contains_key(&key) {
                let playback = self.temporary_playback_at(identity, 1.0, true)?;
                self.temporary.insert(key, playback);
                changed = true;
            }
            let effect = if changed {
                PlaybackRuntimeEffect::Transient
            } else {
                PlaybackRuntimeEffect::None
            };
            return Ok(PlaybackMutation::new((), effect));
        }
        let released = self.temporary.remove(&key);
        let changed = self.swap_held.remove(&identity) || released.is_some();
        if !changed {
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
        }
        let promoted = self.definition_at(identity).unwrap().flash_release
            == FlashReleaseMode::ReleaseIntensityOnly
            && released.is_some_and(|released| {
                self.promote_intensity_release_at(identity, released, false)
            });
        let effect = PlaybackRuntimeEffect::Transient.combine(if promoted {
            PlaybackRuntimeEffect::Durable
        } else {
            PlaybackRuntimeEffect::None
        });
        Ok(PlaybackMutation::new((), effect))
    }

    pub(crate) fn promote_intensity_release_at(
        &mut self,
        identity: PlaybackIdentity,
        mut released: ActivePlayback,
        clear_restore_off: bool,
    ) -> bool {
        let key = match identity {
            PlaybackIdentity::Physical(number) => PlaybackKey::Number(number.get()),
            PlaybackIdentity::Virtual(address) => PlaybackKey::Virtual(address),
        };
        let inserted = !self.active.contains_key(&key);
        released.temporary = false;
        released.flash = false;
        let active = self.active.entry(key).or_insert(released);
        let changed = inserted
            || !active.enabled
            || active.master != 0.0
            || active.temporary
            || active.flash
            || (clear_restore_off && active.flash_restore_off);
        active.enabled = true;
        active.master = 0.0;
        active.temporary = false;
        active.flash = false;
        if clear_restore_off {
            active.flash_restore_off = false;
        }
        changed
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
            PlaybackButtonAction::DynamicRestart => {
                self.restart_dynamic_mutation(number).map(|_| ())
            }
            PlaybackButtonAction::DynamicDoubleSpeed => {
                self.scale_dynamic_speed_mutation(number, true).map(|_| ())
            }
            PlaybackButtonAction::DynamicHalfSpeed => {
                self.scale_dynamic_speed_mutation(number, false).map(|_| ())
            }
            PlaybackButtonAction::DynamicLearnSpeed => {
                if pressed {
                    self.tap_dynamic_speed_mutation(number).map(|_| ())
                } else {
                    Ok(())
                }
            }
            PlaybackButtonAction::Pause if self.dynamic_assignment(number).is_some() => {
                self.toggle_dynamic_pause_mutation(number).map(|_| ())
            }
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
