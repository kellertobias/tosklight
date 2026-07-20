use crate::*;

impl PlaybackEngine {
    pub fn set_manual_xfade(&mut self, number: u16, value: f32) -> Result<(), String> {
        self.set_manual_xfade_mutation(number, value).map(|_| ())
    }

    pub fn set_manual_xfade_mutation(
        &mut self,
        number: u16,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_manual_xfade_inner_mutation(number, value, false)
    }

    pub(crate) fn set_manual_xfade_inner_mutation(
        &mut self,
        number: u16,
        value: f32,
        allow_faderless: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("manual X-fade must be within 0-1".into());
        }
        let definition = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?;
        if definition.fader != PlaybackFaderMode::XFade
            || (!definition.has_fader && !allow_faderless)
        {
            return Err("playback is not configured for manual X-fade".into());
        }
        let cue_list_id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        let mut changed = false;
        if !self.active.contains_key(&key) {
            let cue_list = self
                .cue_lists
                .get(&cue_list_id)
                .ok_or("playback cue list does not exist")?;
            self.active.insert(
                key,
                new_active_playback(Some(number), cue_list, self.clock.now(), 1.0, true),
            );
            changed = true;
        }
        let cue_list = self
            .cue_lists
            .get(&cue_list_id)
            .ok_or("playback cue list does not exist")?;
        let active = self.active.get_mut(&key).expect("X-fade playback exists");
        changed |= apply_manual_xfade(active, cue_list, number, value, self.clock.now());
        let effect = if changed {
            PlaybackRuntimeEffect::Durable
        } else {
            PlaybackRuntimeEffect::None
        };
        Ok(PlaybackMutation::new((), effect))
    }

    pub fn xfade(&mut self, number: u16, on: bool) -> Result<(), String> {
        self.xfade_mutation(number, on).map(|_| ())
    }

    pub fn xfade_mutation(
        &mut self,
        number: u16,
        on: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        let duration = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .xfade_millis;
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        let mut changed = false;
        if on && !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now())?;
            self.active.get_mut(&key).unwrap().master = 0.0;
            changed = true;
        }
        let now = self.clock.now();
        let active = self.active.get_mut(&key).ok_or("playback is not active")?;
        changed |= active.playback_number != Some(number) || (on && !active.enabled);
        if on {
            active.enabled = true;
        }
        active.playback_number = Some(number);
        if duration == 0 {
            let target = if on { 1.0 } else { 0.0 };
            changed |= active.master != target || (!on && active.enabled);
            active.master = target;
            if !on {
                active.enabled = false;
            }
        } else {
            active.master_transition = Some(PlaybackMasterTransition {
                from: active.master,
                to: if on { 1.0 } else { 0.0 },
                started_at: now,
                duration_millis: duration,
                release_after: !on,
            });
            changed = true;
        }
        let effect = if changed {
            PlaybackRuntimeEffect::Durable
        } else {
            PlaybackRuntimeEffect::None
        };
        Ok(PlaybackMutation::new((), effect))
    }

    /// Applies the timing envelope owned by one atomic Preload GO after the retained action verb
    /// has executed against the playback's then-current state. This does not rewrite Cue data:
    /// explicit Cue/attribute timing still wins, while a zero Cue time falls back to Programmer
    /// Fade for this transition only.
    pub fn apply_preload_timing(
        &mut self,
        number: u16,
        action: &str,
        started_at: DateTime<Utc>,
        fallback_millis: u64,
        previous: Option<(bool, f32)>,
    ) -> Result<(), String> {
        self.apply_preload_timing_mutation(number, action, started_at, fallback_millis, previous)
            .map(|_| ())
    }

    pub fn apply_preload_timing_mutation(
        &mut self,
        number: u16,
        action: &str,
        started_at: DateTime<Utc>,
        fallback_millis: u64,
        previous: Option<(bool, f32)>,
    ) -> Result<PlaybackMutation<()>, String> {
        self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        let durable = self.active.get_mut(&key).is_some_and(|playback| {
            apply_active_preload_timing(playback, action, started_at, fallback_millis, previous)
        });
        let transient = action == "temp-on"
            && self
                .temporary
                .get_mut(&(number, TemporaryPlaybackKind::TempButton))
                .is_some_and(|playback| {
                    apply_temporary_preload_timing(playback, started_at, fallback_millis)
                });
        let effect = effect_if(durable, PlaybackRuntimeEffect::Durable)
            .combine(effect_if(transient, PlaybackRuntimeEffect::Transient));
        Ok(PlaybackMutation::new((), effect))
    }
}

fn apply_manual_xfade(
    active: &mut ActivePlayback,
    cue_list: &CueList,
    number: u16,
    value: f32,
    now: DateTime<Utc>,
) -> bool {
    let mut changed = active.playback_number != Some(number)
        || !active.enabled
        || active.fader_position != value
        || active.manual_xfade_position != value;
    active.playback_number = Some(number);
    active.enabled = true;
    active.fader_position = value;
    active.manual_xfade_position = value;
    let progress = manual_xfade_progress(active.manual_xfade_direction, value);
    if active.manual_xfade_from_index.is_none()
        && progress > 0.0
        && let Some(next) = next_manual_xfade_index(active, cue_list)
    {
        active.manual_xfade_from_index = Some(active.cue_index);
        active.manual_xfade_to_index = Some(next);
        active.transition_timing_bypassed = false;
        changed = true;
    }
    if active.manual_xfade_from_index.is_none() {
        changed |= active.manual_xfade_progress != 0.0;
        active.manual_xfade_progress = 0.0;
        return changed;
    }
    let next_progress = progress.clamp(0.0, 1.0);
    changed |= active.manual_xfade_progress != next_progress;
    active.manual_xfade_progress = next_progress;
    if progress >= 1.0 {
        complete_manual_xfade(active, cue_list, now);
        changed = true;
    }
    changed
}

fn manual_xfade_progress(direction: ManualXFadeDirection, value: f32) -> f32 {
    match direction {
        ManualXFadeDirection::TowardsHigh => value,
        ManualXFadeDirection::TowardsLow => 1.0 - value,
    }
}

fn next_manual_xfade_index(active: &ActivePlayback, cue_list: &CueList) -> Option<usize> {
    if active.cue_index + 1 < cue_list.cues.len() {
        Some(active.cue_index + 1)
    } else if cue_list.effective_wrap_mode() != WrapMode::Off {
        Some(0)
    } else {
        None
    }
}

fn complete_manual_xfade(active: &mut ActivePlayback, cue_list: &CueList, now: DateTime<Utc>) {
    let target = active
        .manual_xfade_to_index
        .expect("manual X-fade target accompanies source");
    active.cue_index = target;
    active.current_cue_id = Some(cue_list.cues[target].id);
    active.current_cue_number = Some(cue_list.cues[target].number);
    active.previous_index = None;
    active.transition_timing_bypassed = true;
    active.tracking_wrap = target == 0 && cue_list.effective_wrap_mode() == WrapMode::Tracking;
    active.activated_at = now;
    active.manual_xfade_from_index = None;
    active.manual_xfade_to_index = None;
    active.manual_xfade_progress = 0.0;
    active.manual_xfade_direction = match active.manual_xfade_direction {
        ManualXFadeDirection::TowardsHigh => ManualXFadeDirection::TowardsLow,
        ManualXFadeDirection::TowardsLow => ManualXFadeDirection::TowardsHigh,
    };
}

fn apply_active_preload_timing(
    playback: &mut ActivePlayback,
    action: &str,
    started_at: DateTime<Utc>,
    fallback_millis: u64,
    previous: Option<(bool, f32)>,
) -> bool {
    let mut changed = false;
    if playback.enabled && matches!(action, "go" | "go-minus" | "on" | "toggle") {
        changed |= set_transition_timing(playback, started_at, fallback_millis);
    }
    match (previous, playback.enabled) {
        (Some((false, _)), true)
            if matches!(action, "go" | "on" | "toggle") && fallback_millis > 0 =>
        {
            let target = playback.master;
            changed |= set_master_transition(
                playback,
                PlaybackMasterTransition {
                    from: 0.0,
                    to: target,
                    started_at,
                    duration_millis: fallback_millis,
                    release_after: false,
                },
                0.0,
                true,
            );
        }
        (Some((true, previous_master)), false)
            if matches!(action, "off" | "toggle") && fallback_millis > 0 =>
        {
            changed |= set_master_transition(
                playback,
                PlaybackMasterTransition {
                    from: previous_master,
                    to: 0.0,
                    started_at,
                    duration_millis: fallback_millis,
                    release_after: true,
                },
                previous_master,
                true,
            );
        }
        _ => {}
    }
    changed
}

fn apply_temporary_preload_timing(
    playback: &mut ActivePlayback,
    started_at: DateTime<Utc>,
    fallback_millis: u64,
) -> bool {
    let target = playback.master;
    let mut changed = set_transition_timing(playback, started_at, fallback_millis);
    if fallback_millis > 0 {
        changed |= set_master_transition(
            playback,
            PlaybackMasterTransition {
                from: 0.0,
                to: target,
                started_at,
                duration_millis: fallback_millis,
                release_after: false,
            },
            0.0,
            playback.enabled,
        );
    }
    changed
}

fn set_transition_timing(
    playback: &mut ActivePlayback,
    started_at: DateTime<Utc>,
    fallback_millis: u64,
) -> bool {
    let changed = playback.activated_at != started_at
        || playback.transition_timing_bypassed
        || playback.transition_fade_fallback_millis != Some(fallback_millis);
    playback.activated_at = started_at;
    playback.transition_timing_bypassed = false;
    playback.transition_fade_fallback_millis = Some(fallback_millis);
    changed
}

fn set_master_transition(
    playback: &mut ActivePlayback,
    transition: PlaybackMasterTransition,
    master: f32,
    enabled: bool,
) -> bool {
    let changed = playback.master != master
        || playback.enabled != enabled
        || !transition_matches(playback.master_transition.as_ref(), &transition);
    playback.master = master;
    playback.enabled = enabled;
    playback.master_transition = Some(transition);
    changed
}

fn transition_matches(
    current: Option<&PlaybackMasterTransition>,
    next: &PlaybackMasterTransition,
) -> bool {
    current.is_some_and(|current| {
        current.from == next.from
            && current.to == next.to
            && current.started_at == next.started_at
            && current.duration_millis == next.duration_millis
            && current.release_after == next.release_after
    })
}

const fn effect_if(changed: bool, effect: PlaybackRuntimeEffect) -> PlaybackRuntimeEffect {
    if changed {
        effect
    } else {
        PlaybackRuntimeEffect::None
    }
}
