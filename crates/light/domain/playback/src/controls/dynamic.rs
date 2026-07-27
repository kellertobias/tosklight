use crate::*;

impl PlaybackEngine {
    pub fn dynamic_assignment(&self, number: u16) -> Option<&DynamicPlaybackAssignment> {
        match &self.definitions.get(&number)?.target {
            PlaybackTarget::Dynamic { assignment } => Some(assignment),
            _ => None,
        }
    }

    pub fn active_dynamic_playbacks(&self) -> Vec<ActiveDynamicPlayback> {
        let mut active = self.active_dynamics.values().cloned().collect::<Vec<_>>();
        active.sort_by_key(|playback| playback.playback_number);
        active
    }

    pub(crate) fn on_dynamic_mutation(
        &mut self,
        number: u16,
    ) -> Result<PlaybackMutation<()>, String> {
        let assignment = self
            .dynamic_assignment(number)
            .ok_or("Playback is not assigned to a Dynamic")?
            .clone();
        let now = self.clock.now();
        let active = self
            .active_dynamics
            .entry(number)
            .or_insert_with(|| active_dynamic_playback(number, &assignment, now));
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
        self.dynamic_assignment(number)
            .ok_or("Playback is not assigned to a Dynamic")?;
        let Some(active) = self.active_dynamics.get_mut(&number) else {
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
        if self
            .active_dynamics
            .get(&number)
            .is_some_and(|active| active.enabled)
        {
            return self
                .off_dynamic_mutation(number)
                .map(|mutation| mutation.map(|_| false));
        }
        self.on_dynamic_mutation(number)
            .map(|mutation| mutation.map(|_| true))
    }

    pub(crate) fn set_dynamic_fader_mutation(
        &mut self,
        number: u16,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("Dynamic Playback fader must be within 0-1".into());
        }
        let assignment = self
            .dynamic_assignment(number)
            .ok_or("Playback is not assigned to a Dynamic")?
            .clone();
        if assignment.fader_mode == DynamicPlaybackFaderMode::None {
            return Err("Dynamic Playback has no fader assignment".into());
        }
        if value > 0.0
            && !self
                .active_dynamics
                .get(&number)
                .is_some_and(|active| active.enabled)
        {
            self.on_dynamic_mutation(number)?;
        }
        let now = self.clock.now();
        let active = self
            .active_dynamics
            .entry(number)
            .or_insert_with(|| active_dynamic_playback(number, &assignment, now));
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
        self.dynamic_assignment(number)
            .ok_or("Playback is not assigned to a Dynamic")?;
        let active = self
            .active_dynamics
            .get_mut(&number)
            .filter(|active| active.enabled)
            .ok_or("Dynamic Playback is Off")?;
        active.paused = !active.paused;
        Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Durable))
    }

    pub fn restart_dynamic_mutation(
        &mut self,
        number: u16,
    ) -> Result<PlaybackMutation<()>, String> {
        self.on_dynamic_mutation(number)?;
        let now = self.clock.now();
        let active = self
            .active_dynamics
            .get_mut(&number)
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
        let assignment = self
            .dynamic_assignment(number)
            .ok_or("Playback is not assigned to a Dynamic")?
            .clone();
        let now = self.clock.now();
        let active = self
            .active_dynamics
            .entry(number)
            .or_insert_with(|| active_dynamic_playback(number, &assignment, now));
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
        let assignment = self
            .dynamic_assignment(number)
            .ok_or("Playback is not assigned to a Dynamic")?
            .clone();
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
            .entry(number)
            .or_insert_with(|| active_dynamic_playback(number, &assignment, now));
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
        self.dynamic_assignment(number)
            .ok_or("Playback is not assigned to a Dynamic")?;
        let Some(active) = self.active_dynamics.get_mut(&number) else {
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
        let assignment = self
            .dynamic_assignment(number)
            .ok_or("Playback is not assigned to a Dynamic")?
            .clone();
        let was_enabled = self
            .active_dynamics
            .get(&number)
            .is_some_and(|active| active.enabled);
        if pressed && !was_enabled {
            self.on_dynamic_mutation(number)?;
        }
        let Some(active) = self.active_dynamics.get_mut(&number) else {
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
    number: u16,
    assignment: &DynamicPlaybackAssignment,
    now: DateTime<Utc>,
) -> ActiveDynamicPlayback {
    ActiveDynamicPlayback {
        playback_number: number,
        enabled: true,
        paused: false,
        flash: false,
        flash_restore_off: false,
        activated_at: now,
        fader_value: 1.0,
        size: 1.0,
        master: 1.0,
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
