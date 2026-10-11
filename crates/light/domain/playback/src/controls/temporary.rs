use crate::{controls::activation::deactivate, engine::CuelistFlashState, *};

impl PlaybackEngine {
    pub(crate) fn temporary_playback(
        &mut self,
        held: &PlaybackControlKey,
        master: f32,
        flash: bool,
    ) -> Result<ActivePlayback, String> {
        let identity = held.address;
        let number = identity;
        let definition = self.definition(identity).ok_or("playback does not exist")?;
        let PlaybackTarget::CueList { cue_list_id } = definition.target else {
            return Err("temporary control is available only for Cuelist playbacks".into());
        };
        let cue_list = self
            .cue_lists
            .get(&cue_list_id)
            .ok_or("playback cue list does not exist")?;
        let now = self.clock.now();
        let active_key = PlaybackKey::CueList(cue_list_id);
        let mut playback = self
            .active
            .get(&active_key)
            .cloned()
            .unwrap_or_else(|| new_active_playback(Some(number), cue_list, now, master, true));
        playback.playback_address = Some(number);

        playback.control_source_id = Some(held.source_id());
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
        // A Temp or Flash plays the Cue with its own timing, as a GO would.
        playback.transition_timing_bypassed = false;
        playback.transition_fade_fallback_millis = None;
        playback.manual_xfade_from_index = None;
        playback.manual_xfade_to_index = None;
        playback.manual_xfade_progress = 0.0;
        let ordinal = self.take_source_occurrence_ordinal();
        // The replay selects its own Cue endpoint. Existing cloned holds and transition rows
        // remain the evaluator's outgoing values and keep their own paired evidence.
        playback.source_history = ordinal.map(|ordinal| {
            PlaybackSourceHistory::next(
                None,
                now,
                ordinal,
                &self.compiled_cue_lists[&cue_list_id],
                playback.cue_index,
                playback.tracking_wrap,
                playback.sequence_master_source(),
                false,
            )
        });
        Ok(playback)
    }

    pub fn toggle_temp(
        &mut self,
        number: PlaybackAddress,
        source: &PlaybackControlSource,
    ) -> Result<bool, String> {
        self.toggle_temp_mutation(number, source)
            .map(|mutation| mutation.value)
    }

    pub fn toggle_temp_mutation(
        &mut self,
        identity: PlaybackAddress,
        source: &PlaybackControlSource,
    ) -> Result<PlaybackMutation<bool>, String> {
        let held = PlaybackControlKey::new(identity, source)?;
        let key = (held.clone(), TemporaryPlaybackKind::TempButton);
        if self.temporary.remove(&key).is_some() {
            return Ok(PlaybackMutation::new(
                false,
                PlaybackRuntimeEffect::Transient,
            ));
        }
        let playback = self.temporary_playback(&held, 1.0, false)?;
        self.temporary.insert(key, playback);
        Ok(PlaybackMutation::new(
            true,
            PlaybackRuntimeEffect::Transient,
        ))
    }

    pub fn set_temp_button(
        &mut self,
        number: PlaybackAddress,
        source: &PlaybackControlSource,
        active: bool,
    ) -> Result<(), String> {
        self.set_temp_button_mutation(number, source, active)
            .map(|_| ())
    }

    pub fn set_temp_button_mutation(
        &mut self,
        identity: PlaybackAddress,
        source: &PlaybackControlSource,
        active: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        let held = PlaybackControlKey::new(identity, source)?;
        let key = (held.clone(), TemporaryPlaybackKind::TempButton);
        if active {
            if self.temporary.contains_key(&key) {
                return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
            }
            let playback = self.temporary_playback(&held, 1.0, false)?;
            self.temporary.insert(key, playback);
        } else {
            self.definition(identity).ok_or("playback does not exist")?;
            if self.temporary.remove(&key).is_none() {
                return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
            }
        }
        Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Transient))
    }

    pub fn set_temp_fader(
        &mut self,
        number: PlaybackAddress,
        source: &PlaybackControlSource,
        value: f32,
    ) -> Result<(), String> {
        self.set_temp_fader_mutation(number, source, value)
            .map(|_| ())
    }

    pub fn set_temp_fader_mutation(
        &mut self,
        identity: PlaybackAddress,
        source: &PlaybackControlSource,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        let held = PlaybackControlKey::new(identity, source)?;
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("playback Temp fader must be within 0-1".into());
        }
        let key = (held.clone(), TemporaryPlaybackKind::TempFader);
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
            let playback = self.temporary_playback(&held, value, false)?;
            self.temporary.insert(key, playback);
        }
        Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Transient))
    }

    pub fn set_swap(
        &mut self,
        number: PlaybackAddress,
        source: &PlaybackControlSource,
        pressed: bool,
    ) -> Result<(), String> {
        self.set_swap_mutation(number, source, pressed).map(|_| ())
    }

    pub fn set_swap_mutation(
        &mut self,
        identity: PlaybackAddress,
        source: &PlaybackControlSource,
        pressed: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        let held = PlaybackControlKey::new(identity, source)?;
        let cue_list_id = match self
            .definition(identity)
            .ok_or("playback does not exist")?
            .target
        {
            PlaybackTarget::CueList { cue_list_id } => cue_list_id,
            _ => return Err("Swap is available only for Cuelist playbacks".into()),
        };
        let key = (held.clone(), TemporaryPlaybackKind::Swap);
        if pressed {
            let mut changed = self.swap_held.insert(held.clone());
            if !self.temporary.contains_key(&key) {
                let restore_off = !self
                    .active
                    .get(&PlaybackKey::CueList(cue_list_id))
                    .is_some_and(|playback| playback.enabled);
                self.cuelist_swap_states.insert(
                    held.clone(),
                    CuelistFlashState {
                        restore_off,
                        release_activation_inhibited: false,
                    },
                );
                let playback = self.temporary_playback(&held, 1.0, true)?;
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
        let swap_state = self.cuelist_swap_states.remove(&held);
        let released = self.temporary.remove(&key);
        let changed = self.swap_held.remove(&held) || released.is_some();
        if !changed {
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
        }
        let held_peer = self
            .cuelist_swap_states
            .keys()
            .chain(self.cuelist_flash_states.keys())
            .cloned()
            .any(|peer| {
                self.runtime_key(peer.address).ok() == Some(PlaybackKey::CueList(cue_list_id))
            });
        let auto_off = self.cue_lists[&cue_list_id].auto_off_flash_release
            && swap_state.is_some_and(|state| state.restore_off)
            && !held_peer;
        let promoted = !auto_off
            && swap_state
                .is_some_and(|state| state.restore_off && !state.release_activation_inhibited)
            && !held_peer
            && released
                .is_some_and(|released| self.promote_intensity_release(identity, released, false));
        let turned_off = if auto_off {
            self.active
                .get_mut(&PlaybackKey::CueList(cue_list_id))
                .is_some_and(deactivate)
        } else {
            false
        };
        let effect = PlaybackRuntimeEffect::Transient.combine(if promoted || turned_off {
            PlaybackRuntimeEffect::Durable
        } else {
            PlaybackRuntimeEffect::None
        });
        Ok(PlaybackMutation::new((), effect))
    }

    pub(crate) fn promote_intensity_release(
        &mut self,
        identity: PlaybackAddress,
        mut released: ActivePlayback,
        clear_restore_off: bool,
    ) -> bool {
        let Ok(key) = self.runtime_key(identity) else {
            return false;
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
        active.fader_zero_auto_off_armed = false;
        active.master = 0.0;
        active.temporary = false;
        active.flash = false;
        if clear_restore_off {
            active.flash_restore_off = false;
        }
        changed
    }
    pub fn button(
        &mut self,
        number: PlaybackAddress,
        source: &PlaybackControlSource,
        button: u8,
        pressed: bool,
    ) -> Result<(), String> {
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
            PlaybackButtonAction::Flash => self.set_flash(number, source, pressed),
            PlaybackButtonAction::Temp => {
                if pressed {
                    self.toggle_temp(number, source).map(|_| ())
                } else {
                    Ok(())
                }
            }
            PlaybackButtonAction::Swap => self.set_swap(number, source, pressed),
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
