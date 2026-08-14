use crate::{engine::CuelistFlashState, *};

impl PlaybackEngine {
    pub fn on_at(&mut self, identity: PlaybackIdentity) -> Result<(), String> {
        match identity {
            PlaybackIdentity::Physical(number) => self.on(number.get()),
            PlaybackIdentity::Virtual(address) => {
                let definition = self
                    .virtual_definitions
                    .get(&address)
                    .ok_or("virtual playback does not exist")?;
                let PlaybackTarget::CueList { cue_list_id } = definition.target else {
                    return Err(
                        "operation is not available for this virtual playback function".into(),
                    );
                };
                let key = PlaybackKey::CueList(cue_list_id);
                self.disarm_cuelist_flash(cue_list_id);
                let had_runtime = self.active.contains_key(&key);
                let was_enabled = self
                    .active
                    .get(&key)
                    .is_some_and(|playback| playback.enabled);
                if !had_runtime {
                    self.go_at_key(key, cue_list_id, self.clock.now())?;
                }
                if had_runtime {
                    self.restart_first_cue_if_needed(key, cue_list_id);
                }
                let transition_ordinal =
                    (had_runtime && !was_enabled).then(|| self.take_transition_ordinal());
                let active = self
                    .active
                    .get_mut(&key)
                    .expect("virtual playback activation inserted runtime");
                active.playback_identity = Some(identity);
                activate_normal(active, address.number().get());
                if let Some(transition_ordinal) = transition_ordinal {
                    active.transition_ordinal = transition_ordinal;
                }
                self.retarget_physical_controls(cue_list_id, 1.0, None);
                Ok(())
            }
        }
    }

    pub fn off_at(&mut self, identity: PlaybackIdentity) -> Result<bool, String> {
        match identity {
            PlaybackIdentity::Physical(number) => self.off(number.get()),
            PlaybackIdentity::Virtual(_address) => {
                let key = self.runtime_key_at(identity)?;
                let PlaybackKey::CueList(cue_list_id) = key;
                self.reset_jump_counts(cue_list_id);
                let Some(playback) = self.active.get_mut(&key) else {
                    return Ok(false);
                };
                let was_enabled = playback.enabled;
                deactivate(playback);
                self.retarget_physical_controls(cue_list_id, 0.0, None);
                Ok(was_enabled)
            }
        }
    }

    pub fn on(&mut self, number: u16) -> Result<(), String> {
        self.on_mutation(number).map(|_| ())
    }

    pub fn on_mutation(&mut self, number: u16) -> Result<PlaybackMutation<()>, String> {
        if self.dynamic_assignment(number).is_some() {
            return self.on_dynamic_mutation(number);
        }
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::CueList(id);
        self.disarm_cuelist_flash(id);
        let had_runtime = self.active.contains_key(&key);
        let was_enabled = self
            .active
            .get(&key)
            .is_some_and(|playback| playback.enabled);
        let mut changed = false;
        if !had_runtime {
            self.go_at_key(key, id, self.clock.now())?;
            changed = true;
        }
        changed |= self.restart_first_cue_if_needed(key, id);
        let transition_ordinal =
            (had_runtime && (!was_enabled || changed)).then(|| self.take_transition_ordinal());
        let active = self.active.get_mut(&key).unwrap();
        active.playback_identity = None;
        changed |= activate_normal(active, number);
        if let Some(transition_ordinal) = transition_ordinal {
            active.transition_ordinal = transition_ordinal;
        }
        let control_changed = self.retarget_physical_controls(id, 1.0, None);
        let addressed_effect = durable_effect(changed);
        let addressed_effect = addressed_effect.combine(if control_changed {
            PlaybackRuntimeEffect::Transient
        } else {
            PlaybackRuntimeEffect::None
        });
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
        active.completed_trigger_cue_id = None;
        self.reset_jump_counts(id);
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
        if self.dynamic_assignment(number).is_some() {
            return self.off_dynamic_mutation(number);
        }
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::CueList(id);
        self.reset_jump_counts(id);
        let Some(playback) = self.active.get_mut(&key) else {
            return Ok(PlaybackMutation::new(false, PlaybackRuntimeEffect::None));
        };
        let was_enabled = playback.enabled;
        let changed = deactivate(playback);
        let control_changed = self.retarget_physical_controls(id, 0.0, None);
        Ok(PlaybackMutation::new(
            was_enabled,
            durable_effect(changed).combine(if control_changed {
                PlaybackRuntimeEffect::Transient
            } else {
                PlaybackRuntimeEffect::None
            }),
        ))
    }

    fn reset_jump_counts(&mut self, id: CueListId) {
        self.jump_counts
            .retain(|(cue_list_id, _), _| *cue_list_id != id);
    }

    pub fn toggle(&mut self, number: u16) -> Result<bool, String> {
        self.toggle_mutation(number).map(|mutation| mutation.value)
    }

    pub fn toggle_mutation(&mut self, number: u16) -> Result<PlaybackMutation<bool>, String> {
        if self.dynamic_assignment(number).is_some() {
            return self.toggle_dynamic_mutation(number);
        }
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

    /// Starts one engine-owned transition of the target's authoritative master.
    ///
    /// This is intentionally independent of the configured physical fader mode. Automation
    /// submits the final level and duration once; the Playback tick owns all intermediate values.
    pub fn set_master_transition_mutation(
        &mut self,
        number: u16,
        value: f32,
        duration_millis: u64,
    ) -> Result<PlaybackMutation<()>, String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("playback master must be within 0-1".into());
        }
        if duration_millis > 60_000 {
            return Err("playback master transition must be within 0-60000 milliseconds".into());
        }
        let started_at = self.clock.now();
        if self.dynamic_assignment(number).is_some() {
            let identity = PlaybackIdentity::physical(number)?;
            let target_id = self
                .dynamic_target_id_at(identity)
                .ok_or("Playback is not assigned to a Dynamic")?;
            if value > 0.0
                && !self
                    .active_dynamics
                    .get(&target_id)
                    .is_some_and(|active| active.enabled)
            {
                self.on_dynamic_at_mutation(identity)?;
            }
            let active = self
                .active_dynamics
                .get_mut(&target_id)
                .ok_or("Playback is not assigned to a Dynamic")?;
            let before = active.clone();
            if duration_millis == 0 {
                active.master = value;
                active.master_transition = None;
            } else {
                active.master_transition = Some(PlaybackMasterTransition {
                    from: active.master,
                    to: value,
                    started_at,
                    duration_millis,
                    release_after: false,
                });
            }
            let changed = *active != before;
            let control_changed = self.retarget_dynamic_physical_controls(target_id, value, None);
            return Ok(PlaybackMutation::new(
                (),
                durable_effect(changed).combine(if control_changed {
                    PlaybackRuntimeEffect::Transient
                } else {
                    PlaybackRuntimeEffect::None
                }),
            ));
        }

        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::CueList(id);
        if value > 0.0 && !self.active.contains_key(&key) {
            self.go_at_key(key, id, started_at)?;
        }
        let active = self
            .active
            .get_mut(&key)
            .ok_or("playback is inactive at zero master")?;
        let before = active.clone();
        if duration_millis == 0 {
            active.master = value;
            active.master_transition = None;
        } else {
            active.master_transition = Some(PlaybackMasterTransition {
                from: active.master,
                to: value,
                started_at,
                duration_millis,
                release_after: false,
            });
        }
        Ok(PlaybackMutation::new((), durable_effect(*active != before)))
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

    pub fn set_virtual_master_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        if let PlaybackIdentity::Physical(number) = identity {
            return self.set_virtual_master_mutation(number.get(), value);
        }
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("playback master must be within 0-1".into());
        }
        let address = identity
            .virtual_address()
            .expect("physical identity returned above");
        let definition = self
            .definition_at(identity)
            .ok_or("virtual playback does not exist")?;
        if definition.fader != PlaybackFaderMode::Master {
            return Err("virtual master requires the Master fader mode".into());
        }
        let PlaybackTarget::CueList { cue_list_id } = definition.target else {
            return Err("virtual playback master is unavailable for this target".into());
        };
        let key = PlaybackKey::CueList(cue_list_id);
        let auto_off_at_zero = self.cue_lists[&cue_list_id].auto_off_at_zero;
        if !self.active.contains_key(&key) {
            self.go_at_key(key, cue_list_id, self.clock.now())?;
        }
        let mut changed = false;
        if let Some(active) = self.active.get_mut(&key) {
            active.playback_identity = Some(identity);
            changed |=
                apply_cuelist_master(active, address.number().get(), value, auto_off_at_zero);
        }
        if value == 0.0 && auto_off_at_zero {
            self.reset_jump_counts(cue_list_id);
        }
        let control_changed = self.retarget_physical_controls(cue_list_id, value, None);
        let addressed_effect = durable_effect(changed);
        let addressed_effect = addressed_effect.combine(if control_changed {
            PlaybackRuntimeEffect::Transient
        } else {
            PlaybackRuntimeEffect::None
        });
        let related_effect = durable_effect(self.auto_off_overwritten());
        Ok(PlaybackMutation::with_related_effect(
            (),
            addressed_effect,
            related_effect,
        ))
    }

    /// Apply an externally authoritative fader value with explicit start/stop semantics.
    ///
    /// Unlike a physical desk fader, an automation light does not inherit the authored
    /// `auto_off_at_zero` policy: positive values mean On and zero means Off. Keeping the level
    /// and activation change inside one Playback mutation boundary prevents transports from
    /// synthesizing a racy Master + On/Off command sequence.
    pub fn set_master_with_explicit_activation_mutation(
        &mut self,
        number: u16,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        let target = self
            .definitions
            .get(&number)
            .map(|definition| definition.target.clone())
            .ok_or("playback does not exist")?;
        if !matches!(
            target,
            PlaybackTarget::CueList { .. } | PlaybackTarget::Dynamic { .. }
        ) {
            return Err("explicit fader activation requires a Cuelist or Dynamic Playback".into());
        }

        let fader = self.set_virtual_master_mutation(number, value)?;
        let activation = if value > 0.0 {
            // The virtual-master path starts both Cuelists and Dynamics above zero without
            // engaging physical-fader pickup.
            PlaybackMutation::new((), PlaybackRuntimeEffect::None)
        } else {
            let activation = if matches!(target, PlaybackTarget::Dynamic { .. }) {
                self.off_dynamic_mutation(number)?.map(|_| ())
            } else {
                let off = self.off_mutation(number)?.map(|_| ());
                // Zero is authoritative Off for this surface, but the runtime stays present and
                // disabled. Arm the same restart the auto-off-at-zero fader uses so the next
                // non-zero value turns the Cuelist back on instead of moving a dark master.
                if let Ok(id) = self.cue_list_for(number)
                    && let Some(active) = self.active.get_mut(&PlaybackKey::CueList(id))
                {
                    active.fader_zero_auto_off_armed = true;
                }
                off
            };
            activation
        };

        Ok(PlaybackMutation {
            value: (),
            addressed_effect: fader.addressed_effect.combine(activation.addressed_effect),
            effect: fader.effect.combine(activation.effect),
        })
    }

    fn set_master_inner_mutation(
        &mut self,
        number: u16,
        value: f32,
        allow_faderless: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        if self.dynamic_assignment(number).is_some() {
            return self.set_dynamic_fader_at_mutation_inner(
                PlaybackIdentity::physical(number)?,
                value,
                !allow_faderless,
            );
        }
        let mode = self.validate_master(number, value, allow_faderless)?;
        match mode {
            PlaybackFaderMode::Temp => return self.set_temp_fader_mutation(number, value),
            PlaybackFaderMode::XFade => {
                return self.set_manual_xfade_inner_mutation(number, value, allow_faderless);
            }
            PlaybackFaderMode::Master => {}
            _ => return Err("fader mode is not handled by the Cuelist engine".into()),
        }
        self.set_cuelist_master_mutation(number, value, !allow_faderless)
    }

    pub fn set_configured_fader_mutation(
        &mut self,
        number: u16,
        mode: PlaybackFaderMode,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("playback master must be within 0-1".into());
        }
        match mode {
            PlaybackFaderMode::Temp => self.set_temp_fader_mutation(number, value),
            PlaybackFaderMode::XFade => self.set_manual_xfade_inner_mutation(number, value, true),
            PlaybackFaderMode::Master => self.set_cuelist_master_mutation(number, value, true),
            _ => Err("fader mode is not handled by the Cuelist engine".into()),
        }
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
        physical: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::CueList(id);
        let auto_off_at_zero = self.cue_lists[&id].auto_off_at_zero;
        let identity = PlaybackIdentity::physical(number)?;
        let mut control_changed = false;
        if physical {
            if !self.control_states.contains_key(&identity) {
                let authoritative = self.active.get(&key).map(|active| active.master);
                self.control_states.insert(
                    identity,
                    PlaybackControlState {
                        fader_pickup_required: authoritative.is_some(),
                        fader_pickup_target: authoritative,
                        ..PlaybackControlState::default()
                    },
                );
                control_changed = true;
            }
            let state = self.control_states.entry(identity).or_default();
            let previous = state.fader_position;
            let was_observed = state.observed;
            control_changed |= !was_observed || previous != value;
            state.fader_position = value;
            state.observed = true;
            if state.fader_pickup_required {
                let target = state.fader_pickup_target.unwrap_or(0.0);
                let crossed = value == target
                    || (was_observed && previous == target)
                    || (was_observed
                        && ((previous < target && value > target)
                            || (previous > target && value < target)));
                if !crossed {
                    return Ok(PlaybackMutation::new(
                        (),
                        if control_changed {
                            PlaybackRuntimeEffect::Transient
                        } else {
                            PlaybackRuntimeEffect::None
                        },
                    ));
                }
                state.fader_pickup_required = false;
                state.fader_pickup_target = None;
                control_changed = true;
            }
        }
        if !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now())?;
        }
        let mut changed = false;
        if let Some(active) = self.active.get_mut(&key) {
            changed |= apply_cuelist_master(active, number, value, auto_off_at_zero);
        }
        if value == 0.0 && auto_off_at_zero {
            self.reset_jump_counts(id);
        }
        control_changed |= self.retarget_physical_controls(id, value, physical.then_some(identity));
        let addressed_effect = durable_effect(changed).combine(if control_changed {
            PlaybackRuntimeEffect::Transient
        } else {
            PlaybackRuntimeEffect::None
        });
        let related_effect = durable_effect(self.auto_off_overwritten());
        Ok(PlaybackMutation::with_related_effect(
            (),
            addressed_effect,
            related_effect,
        ))
    }

    pub fn set_flash(&mut self, number: u16, pressed: bool) -> Result<(), String> {
        self.set_flash_mutation(number, pressed).map(|_| ())
    }

    pub fn set_flash_mutation(
        &mut self,
        number: u16,
        pressed: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_flash_at_mutation(PlaybackIdentity::physical(number)?, pressed)
    }

    pub fn set_flash_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        pressed: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        let definition = self
            .definition_at(identity)
            .ok_or("playback does not exist")?
            .clone();
        let PlaybackTarget::CueList { cue_list_id } = definition.target else {
            return Err("Flash is available only for Cuelist playbacks".into());
        };
        let key = (identity, TemporaryPlaybackKind::Flash);
        if pressed {
            if self.temporary.contains_key(&key) {
                return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
            }
            let restore_off = !self
                .active
                .get(&PlaybackKey::CueList(cue_list_id))
                .is_some_and(|playback| playback.enabled);
            self.cuelist_flash_states
                .insert(identity, CuelistFlashState { restore_off });
            let playback = self.temporary_playback_at(identity, 1.0, true)?;
            self.temporary.insert(key, playback);
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Transient));
        }
        let flash_state = self.cuelist_flash_states.remove(&identity);
        let Some(released) = self.temporary.remove(&key) else {
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
        };
        let held_peer = self
            .cuelist_flash_states
            .keys()
            .chain(self.cuelist_swap_states.keys())
            .copied()
            .any(|peer| self.runtime_key_at(peer).ok() == Some(PlaybackKey::CueList(cue_list_id)));
        let auto_off = self.cue_lists[&cue_list_id].auto_off_flash_release
            && flash_state.is_some_and(|state| state.restore_off)
            && !held_peer;
        let promoted = !auto_off
            && flash_state.is_some_and(|state| state.restore_off)
            && !held_peer
            && self.promote_intensity_release_at(identity, released, true);
        let turned_off = if auto_off {
            self.reset_jump_counts(cue_list_id);
            self.active
                .get_mut(&PlaybackKey::CueList(cue_list_id))
                .is_some_and(deactivate)
        } else {
            false
        };
        let effect =
            PlaybackRuntimeEffect::Transient.combine(durable_effect(promoted || turned_off));
        Ok(PlaybackMutation::new((), effect))
    }

    pub(crate) fn disarm_cuelist_flash(&mut self, cue_list_id: CueListId) {
        let identities = self
            .cuelist_flash_states
            .keys()
            .copied()
            .filter(|identity| {
                self.runtime_key_at(*identity).ok() == Some(PlaybackKey::CueList(cue_list_id))
            })
            .collect::<Vec<_>>();
        for identity in identities {
            if let Some(state) = self.cuelist_flash_states.get_mut(&identity) {
                state.restore_off = false;
            }
        }
        let swap_identities = self
            .cuelist_swap_states
            .keys()
            .copied()
            .filter(|identity| {
                self.runtime_key_at(*identity).ok() == Some(PlaybackKey::CueList(cue_list_id))
            })
            .collect::<Vec<_>>();
        for identity in swap_identities {
            if let Some(state) = self.cuelist_swap_states.get_mut(&identity) {
                state.restore_off = false;
            }
        }
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
    // ON is idempotent while this Playback is already at its normal master.
    // In particular, do not discard an in-flight Cue transition source merely
    // because ON is repeated while that transition is running.
    if playback.playback_number == Some(number)
        && playback.master == 1.0
        && playback.enabled
        && !playback.temporary
        && !playback.fader_zero_auto_off_armed
        && playback.master_transition.is_none()
        && !playback.transition_timing_bypassed
        && playback.transition_fade_fallback_millis.is_none()
        && playback.manual_xfade_from_index.is_none()
        && playback.manual_xfade_to_index.is_none()
        && playback.manual_xfade_progress == 0.0
    {
        return false;
    }
    let changed = playback.playback_number != Some(number)
        || playback.master != 1.0
        || !playback.enabled
        || playback.temporary
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
    playback.fader_zero_auto_off_armed = false;
    playback.temporary = false;
    playback.master_transition = None;
    playback.deleted_cue_transition_source = None;
    reset_manual_transition(playback);
    changed
}

pub(crate) fn deactivate(playback: &mut ActivePlayback) -> bool {
    let changed = playback.enabled
        || playback.flash
        || playback.master_transition.is_some()
        || playback.deleted_cue_hold.is_some()
        || playback.deleted_cue_transition_source.is_some()
        || playback.loaded_cue_id.is_some()
        || playback.loaded_cue_number.is_some()
        || playback.fader_zero_auto_off_armed;
    playback.enabled = false;
    playback.fader_zero_auto_off_armed = false;
    playback.activation = None;
    playback.flash = false;
    playback.master_transition = None;
    playback.deleted_cue_hold = None;
    playback.deleted_cue_transition_source = None;
    playback.loaded_cue_id = None;
    playback.loaded_cue_number = None;
    changed
}

fn apply_cuelist_master(
    playback: &mut ActivePlayback,
    number: u16,
    value: f32,
    auto_off_at_zero: bool,
) -> bool {
    let before = playback.clone();
    let restart = value > 0.0 && !playback.enabled && playback.fader_zero_auto_off_armed;
    let auto_off = value == 0.0 && playback.enabled && auto_off_at_zero;
    if auto_off {
        deactivate(playback);
        playback.fader_zero_auto_off_armed = true;
    }
    playback.playback_number = Some(number);
    playback.master = value;
    playback.master_transition = None;
    playback.temporary = false;
    if restart {
        playback.enabled = true;
        playback.fader_zero_auto_off_armed = false;
    }
    *playback != before
}
