use crate::*;

pub fn dynamic_playback_controller_id(target_id: Uuid) -> Uuid {
    Uuid::from_u128(0x4459_4e41_4d49_432d_504c_4159_4241_434b ^ target_id.as_u128())
}

impl PlaybackEngine {
    pub fn dynamic_assignment(&self, number: u16) -> Option<&DynamicPlaybackAssignment> {
        self.dynamic_assignment_at(PlaybackIdentity::physical(number).ok()?)
    }

    pub fn dynamic_assignment_at(
        &self,
        identity: PlaybackIdentity,
    ) -> Option<&DynamicPlaybackAssignment> {
        match &self.definition_at(identity)?.target {
            PlaybackTarget::Dynamic { assignment } => Some(assignment),
            _ => None,
        }
    }

    pub fn active_dynamic_playbacks(&self) -> Vec<ActiveDynamicPlayback> {
        let mut active = self.active_dynamics.values().cloned().collect::<Vec<_>>();
        active.sort_by_key(|playback| playback.playback_number);
        active
    }

    pub fn active_dynamic_playbacks_for_persistence(&self) -> Vec<ActiveDynamicPlayback> {
        self.active_dynamic_playbacks()
            .into_iter()
            .map(|mut active| {
                if active.flash_restore_off {
                    active.enabled = false;
                }
                active.flash = false;
                active.flash_restore_off = false;
                active.fader_pickup_required = false;
                active.fader_pickup_target = None;
                active
            })
            .collect()
    }

    pub fn active_dynamic_playback_at(
        &self,
        identity: PlaybackIdentity,
    ) -> Option<ActiveDynamicPlayback> {
        let target_id = self.dynamic_assignment_at(identity)?.target_id();
        let mut active = self.active_dynamics.get(&target_id)?.clone();
        let flash = self.dynamic_flash_states.get(&identity).copied();
        active.flash = flash.is_some();
        active.flash_restore_off = flash.is_some_and(|state| state.restore_off);
        if matches!(identity, PlaybackIdentity::Physical(_)) {
            let control = self.control_state_at(identity);
            active.fader_pickup_required = control.fader_pickup_required;
            active.fader_pickup_target = control.fader_pickup_target;
        }
        Some(active)
    }

    pub fn dynamic_target_id_at(&self, identity: PlaybackIdentity) -> Option<Uuid> {
        self.dynamic_assignment_at(identity)
            .map(DynamicPlaybackAssignment::target_id)
    }

    pub fn dynamic_identities(&self, target_id: Uuid) -> Vec<PlaybackIdentity> {
        let mut identities = self
            .definitions
            .iter()
            .filter_map(|(number, definition)| {
                let PlaybackTarget::Dynamic { assignment } = &definition.target else {
                    return None;
                };
                (assignment.target_id() == target_id)
                    .then(|| PlaybackIdentity::physical(*number).expect("registered playback"))
            })
            .chain(
                self.virtual_definitions
                    .iter()
                    .filter_map(|(address, definition)| {
                        let PlaybackTarget::Dynamic { assignment } = &definition.target else {
                            return None;
                        };
                        (assignment.target_id() == target_id)
                            .then_some(PlaybackIdentity::Virtual(*address))
                    }),
            )
            .collect::<Vec<_>>();
        identities.sort_unstable();
        identities
    }

    pub(crate) fn first_dynamic_identity(&self, target_id: Uuid) -> Option<PlaybackIdentity> {
        self.dynamic_identities(target_id).into_iter().next()
    }

    pub(crate) fn on_dynamic_mutation(
        &mut self,
        number: u16,
    ) -> Result<PlaybackMutation<()>, String> {
        self.on_dynamic_at_mutation(PlaybackIdentity::physical(number)?)
    }

    pub fn on_dynamic_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<()>, String> {
        let assignment = self
            .dynamic_assignment_at(identity)
            .ok_or("Playback is not assigned to a Dynamic")?
            .clone();
        let target_id = assignment.target_id();
        for peer in self.dynamic_identities(target_id) {
            if let Some(state) = self.dynamic_flash_states.get_mut(&peer) {
                state.restore_off = false;
            }
        }
        let now = self.clock.now();
        let active = self
            .active_dynamics
            .entry(target_id)
            .or_insert_with(|| active_dynamic_playback(identity, &assignment, now));
        let changed = !active.enabled || active.flash_restore_off;
        active.enabled = true;
        active.flash_restore_off = false;
        active.paused = false;
        if changed {
            active.activated_at = now;
        }
        Ok(PlaybackMutation::new((), durable_effect(changed)))
    }

    pub(crate) fn off_dynamic_mutation(
        &mut self,
        number: u16,
    ) -> Result<PlaybackMutation<bool>, String> {
        self.off_dynamic_at_mutation(PlaybackIdentity::physical(number)?)
    }

    pub fn off_dynamic_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<bool>, String> {
        let target_id = self
            .dynamic_target_id_at(identity)
            .ok_or("Playback is not assigned to a Dynamic")?;
        let held = self
            .dynamic_identities(target_id)
            .into_iter()
            .filter(|peer| self.dynamic_flash_states.contains_key(peer))
            .collect::<Vec<_>>();
        for peer in &held {
            if let Some(state) = self.dynamic_flash_states.get_mut(peer) {
                state.restore_off = true;
            }
        }
        let Some(active) = self.active_dynamics.get_mut(&target_id) else {
            return Ok(PlaybackMutation::new(false, PlaybackRuntimeEffect::None));
        };
        let changed = active.enabled && !active.flash_restore_off;
        active.enabled = !held.is_empty();
        active.flash = !held.is_empty();
        active.flash_restore_off = !held.is_empty();
        active.paused = false;
        Ok(PlaybackMutation::new(changed, durable_effect(changed)))
    }

    pub(crate) fn toggle_dynamic_mutation(
        &mut self,
        number: u16,
    ) -> Result<PlaybackMutation<bool>, String> {
        self.toggle_dynamic_at_mutation(PlaybackIdentity::physical(number)?)
    }

    pub fn toggle_dynamic_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<bool>, String> {
        let target_id = self
            .dynamic_target_id_at(identity)
            .ok_or("Playback is not assigned to a Dynamic")?;
        if self
            .active_dynamics
            .get(&target_id)
            .is_some_and(|active| active.enabled)
        {
            return self
                .off_dynamic_at_mutation(identity)
                .map(|mutation| mutation.map(|_| false));
        }
        self.on_dynamic_at_mutation(identity)
            .map(|mutation| mutation.map(|_| true))
    }

    pub fn set_dynamic_fader_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_dynamic_fader_at_mutation_inner(identity, value, false)
    }

    pub(crate) fn set_dynamic_fader_at_mutation_inner(
        &mut self,
        identity: PlaybackIdentity,
        value: f32,
        physical: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("Dynamic Playback fader must be within 0-1".into());
        }
        let assignment = self
            .dynamic_assignment_at(identity)
            .ok_or("Playback is not assigned to a Dynamic")?
            .clone();
        let target_id = assignment.target_id();
        if assignment.fader_mode == DynamicPlaybackFaderMode::None {
            return Err("Dynamic Playback has no fader assignment".into());
        }
        let mut control_changed = false;
        if physical {
            if !self.control_states.contains_key(&identity) {
                let authoritative = self
                    .active_dynamics
                    .get(&target_id)
                    .map(|active| active.fader_value);
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
                let target = state.fader_pickup_target.unwrap_or_default();
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
        if value > 0.0
            && !self
                .active_dynamics
                .get(&target_id)
                .is_some_and(|active| active.enabled)
        {
            self.on_dynamic_at_mutation(identity)?;
        }
        let now = self.clock.now();
        let active = self
            .active_dynamics
            .entry(target_id)
            .or_insert_with(|| active_dynamic_playback(identity, &assignment, now));
        let before = active.clone();
        active.fader_value = value;
        match assignment.fader_mode {
            DynamicPlaybackFaderMode::None => {}
            DynamicPlaybackFaderMode::Master => active.master = value,
            DynamicPlaybackFaderMode::Size => active.size = value,
            DynamicPlaybackFaderMode::SizeAndMaster => {
                active.size = value;
                active.master = value;
            }
        }
        if value == 0.0 && assignment.auto_off_at_zero {
            active.enabled = false;
            active.paused = false;
        }
        let changed = *active != before;
        control_changed |=
            self.retarget_dynamic_physical_controls(target_id, value, physical.then_some(identity));
        Ok(PlaybackMutation::new(
            (),
            durable_effect(changed).combine(if control_changed {
                PlaybackRuntimeEffect::Transient
            } else {
                PlaybackRuntimeEffect::None
            }),
        ))
    }

    pub fn toggle_dynamic_pause_mutation(
        &mut self,
        number: u16,
    ) -> Result<PlaybackMutation<()>, String> {
        self.toggle_dynamic_pause_at_mutation(PlaybackIdentity::physical(number)?)
    }

    pub fn toggle_dynamic_pause_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<()>, String> {
        let target_id = self
            .dynamic_target_id_at(identity)
            .ok_or("Playback is not assigned to a Dynamic")?;
        let active = self
            .active_dynamics
            .get_mut(&target_id)
            .filter(|active| active.enabled)
            .ok_or("Dynamic Playback is Off")?;
        active.paused = !active.paused;
        Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Durable))
    }

    pub fn restart_dynamic_mutation(
        &mut self,
        number: u16,
    ) -> Result<PlaybackMutation<()>, String> {
        self.restart_dynamic_at_mutation(PlaybackIdentity::physical(number)?)
    }

    pub fn restart_dynamic_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<()>, String> {
        self.on_dynamic_at_mutation(identity)?;
        let target_id = self
            .dynamic_target_id_at(identity)
            .ok_or("Playback is not assigned to a Dynamic")?;
        let now = self.clock.now();
        let active = self
            .active_dynamics
            .get_mut(&target_id)
            .expect("Dynamic On creates runtime state");
        active.activated_at = now;
        active.paused = false;
        Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Durable))
    }

    pub fn scale_dynamic_speed_mutation(
        &mut self,
        number: u16,
        double: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        self.scale_dynamic_speed_at_mutation(PlaybackIdentity::physical(number)?, double)
    }

    pub fn scale_dynamic_speed_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        double: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        let assignment = self
            .dynamic_assignment_at(identity)
            .ok_or("Playback is not assigned to a Dynamic")?
            .clone();
        let target_id = assignment.target_id();
        let now = self.clock.now();
        let active = self
            .active_dynamics
            .entry(target_id)
            .or_insert_with(|| active_dynamic_playback(identity, &assignment, now));
        if let Some(duration) = active.learned_duration_millis {
            active.learned_duration_millis = Some(if double {
                (duration / 2).max(1)
            } else {
                duration.saturating_mul(2).min(86_400_000)
            });
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Durable));
        }
        if double {
            active.local_speed_multiplier.numerator = active
                .local_speed_multiplier
                .numerator
                .saturating_mul(2)
                .min(1_024);
        } else {
            active.local_speed_multiplier.denominator = active
                .local_speed_multiplier
                .denominator
                .saturating_mul(2)
                .min(1_024);
        }
        Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Durable))
    }

    pub fn tap_dynamic_speed_mutation(
        &mut self,
        number: u16,
    ) -> Result<PlaybackMutation<Option<u64>>, String> {
        self.tap_dynamic_speed_at_mutation(PlaybackIdentity::physical(number)?)
    }

    pub fn tap_dynamic_speed_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<Option<u64>>, String> {
        let assignment = self
            .dynamic_assignment_at(identity)
            .ok_or("Playback is not assigned to a Dynamic")?
            .clone();
        let target_id = assignment.target_id();
        if !matches!(
            assignment.dynamic.embedded_fallback.definition.speed,
            light_dynamics::DynamicSpeed::Fixed { .. }
        ) {
            return Err("Tap/Learn Speed is available only for fixed-duration Dynamics".into());
        }
        let now = self.clock.now();
        let now_millis = u64::try_from(now.timestamp_millis()).unwrap_or_default();
        let active = self
            .active_dynamics
            .entry(target_id)
            .or_insert_with(|| active_dynamic_playback(identity, &assignment, now));
        let Some(previous) = active.last_learn_tap_millis.replace(now_millis) else {
            active.learn_intervals_millis.clear();
            return Ok(PlaybackMutation::new(None, PlaybackRuntimeEffect::Durable));
        };
        let interval = now_millis.saturating_sub(previous);
        if !(100..=30_000).contains(&interval) {
            active.learn_intervals_millis.clear();
            return Ok(PlaybackMutation::new(None, PlaybackRuntimeEffect::Durable));
        }
        if active.learn_intervals_millis.len() >= 4 {
            active.learn_intervals_millis.remove(0);
        }
        active.learn_intervals_millis.push(interval);
        let learned = active.learn_intervals_millis.iter().sum::<u64>()
            / active.learn_intervals_millis.len() as u64;
        active.learned_duration_millis = Some(learned.max(1));
        Ok(PlaybackMutation::new(
            active.learned_duration_millis,
            PlaybackRuntimeEffect::Durable,
        ))
    }

    pub fn clear_dynamic_learned_speed_mutation(
        &mut self,
        number: u16,
    ) -> Result<PlaybackMutation<()>, String> {
        let identity = PlaybackIdentity::physical(number)?;
        let target_id = self
            .dynamic_target_id_at(identity)
            .ok_or("Playback is not assigned to a Dynamic")?;
        let Some(active) = self.active_dynamics.get_mut(&target_id) else {
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
        };
        let changed = active.learned_duration_millis.take().is_some()
            || active.last_learn_tap_millis.take().is_some()
            || !active.learn_intervals_millis.is_empty();
        active.learn_intervals_millis.clear();
        Ok(PlaybackMutation::new((), durable_effect(changed)))
    }

    pub fn set_dynamic_flash_mutation(
        &mut self,
        number: u16,
        pressed: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_dynamic_flash_at_mutation(PlaybackIdentity::physical(number)?, pressed)
    }

    pub fn set_dynamic_flash_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        pressed: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        let assignment = self
            .dynamic_assignment_at(identity)
            .ok_or("Playback is not assigned to a Dynamic")?
            .clone();
        let target_id = assignment.target_id();
        let durable_enabled = self
            .active_dynamics
            .get(&target_id)
            .is_some_and(|active| active.enabled && !active.flash_restore_off);
        if pressed && !self.active_dynamics.contains_key(&target_id) {
            self.on_dynamic_at_mutation(identity)?;
        }
        let previous_state = self.dynamic_flash_states.get(&identity).copied();
        if pressed {
            self.dynamic_flash_states.insert(
                identity,
                DynamicFlashState {
                    restore_off: !durable_enabled,
                },
            );
        } else {
            self.dynamic_flash_states.remove(&identity);
        }
        let peer_identities = self.dynamic_identities(target_id);
        let held_peers = peer_identities
            .iter()
            .filter_map(|peer| self.dynamic_flash_states.get(&peer).copied())
            .collect::<Vec<_>>();
        let Some(active) = self.active_dynamics.get_mut(&target_id) else {
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
        };
        let before = active.clone();
        let promoted = !pressed
            && previous_state.is_some_and(|state| state.restore_off)
            && !assignment.auto_off_flash_release;
        if promoted {
            active.enabled = true;
            active.flash = !held_peers.is_empty();
            active.flash_restore_off = false;
            for peer in peer_identities {
                if let Some(state) = self.dynamic_flash_states.get_mut(&peer) {
                    state.restore_off = false;
                }
            }
        } else if held_peers.is_empty() {
            active.flash = false;
            if previous_state.is_some_and(|state| state.restore_off)
                && assignment.auto_off_flash_release
            {
                active.enabled = false;
                active.paused = false;
            }
            active.flash_restore_off = false;
        } else {
            active.enabled = true;
            active.flash = true;
            active.flash_restore_off = held_peers.iter().all(|state| state.restore_off);
        }
        let effect = if promoted {
            PlaybackRuntimeEffect::Durable
        } else if *active != before || previous_state.is_some() != pressed {
            PlaybackRuntimeEffect::Transient
        } else {
            PlaybackRuntimeEffect::None
        };
        Ok(PlaybackMutation::new((), effect))
    }
}

fn active_dynamic_playback(
    identity: PlaybackIdentity,
    assignment: &DynamicPlaybackAssignment,
    now: DateTime<Utc>,
) -> ActiveDynamicPlayback {
    ActiveDynamicPlayback {
        dynamic_id: Some(assignment.target_id()),
        playback_number: identity.number(),
        playback_identity: identity.virtual_address().map(PlaybackIdentity::Virtual),
        enabled: true,
        paused: false,
        flash: false,
        flash_restore_off: false,
        activated_at: now,
        fader_value: 1.0,
        fader_pickup_required: false,
        fader_pickup_target: None,
        size: 1.0,
        master: 1.0,
        master_transition: None,
        local_speed_multiplier: assignment.local_speed_multiplier,
        learned_duration_millis: assignment.learned_duration_millis,
        last_learn_tap_millis: None,
        learn_intervals_millis: Vec::new(),
    }
}

const fn durable_effect(changed: bool) -> PlaybackRuntimeEffect {
    if changed {
        PlaybackRuntimeEffect::Durable
    } else {
        PlaybackRuntimeEffect::None
    }
}
