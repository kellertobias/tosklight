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

    pub fn active_dynamic_playback_at(
        &self,
        identity: PlaybackIdentity,
    ) -> Option<&ActiveDynamicPlayback> {
        let target_id = self.dynamic_assignment_at(identity)?.target_id();
        self.active_dynamics.get(&target_id)
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
        let now = self.clock.now();
        let active = self
            .active_dynamics
            .entry(target_id)
            .or_insert_with(|| active_dynamic_playback(identity, &assignment, now));
        let changed = !active.enabled;
        active.enabled = true;
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
        let Some(active) = self.active_dynamics.get_mut(&target_id) else {
            return Ok(PlaybackMutation::new(false, PlaybackRuntimeEffect::None));
        };
        let changed = active.enabled;
        active.enabled = false;
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

    pub(crate) fn set_dynamic_fader_mutation(
        &mut self,
        number: u16,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_dynamic_fader_at_mutation(PlaybackIdentity::physical(number)?, value)
    }

    pub fn set_dynamic_fader_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
        value: f32,
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
        Ok(PlaybackMutation::new((), durable_effect(*active != before)))
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
        let was_enabled = self
            .active_dynamics
            .get(&target_id)
            .is_some_and(|active| active.enabled);
        if pressed && !was_enabled {
            self.on_dynamic_at_mutation(identity)?;
        }
        let Some(active) = self.active_dynamics.get_mut(&target_id) else {
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
        };
        let before = active.clone();
        if pressed {
            active.flash = true;
            active.flash_restore_off = !was_enabled;
        } else {
            active.flash = false;
            if active.flash_restore_off && assignment.auto_off_flash_release {
                active.enabled = false;
                active.paused = false;
            }
            active.flash_restore_off = false;
        }
        Ok(PlaybackMutation::new((), durable_effect(*active != before)))
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
