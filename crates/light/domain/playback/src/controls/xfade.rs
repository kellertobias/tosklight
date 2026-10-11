use crate::*;

impl PlaybackEngine {
    pub fn set_manual_xfade(&mut self, number: PlaybackAddress, value: f32) -> Result<(), String> {
        self.set_manual_xfade_mutation(number, value).map(|_| ())
    }

    pub fn set_manual_xfade_mutation(
        &mut self,
        number: PlaybackAddress,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        self.set_manual_xfade_inner_mutation(number, value, false)
    }

    pub(crate) fn set_manual_xfade_inner_mutation(
        &mut self,
        number: PlaybackAddress,
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
        self.apply_configured_manual_xfade_mutation(number, value)
    }

    /// The caller has validated the addressed configured fader (including wider right faders).
    pub(crate) fn apply_configured_manual_xfade_mutation(
        &mut self,
        number: PlaybackAddress,
        value: f32,
    ) -> Result<PlaybackMutation<()>, String> {
        let cue_list_id = self.cue_list_for(number)?;
        let key = PlaybackKey::CueList(cue_list_id);
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
            let ordinal = self.take_source_occurrence_ordinal();
            let compiled = &self.compiled_cue_lists[&cue_list_id];
            let active = self
                .active
                .get_mut(&key)
                .expect("X-fade activation inserted runtime");
            active.begin_source_history(active.activated_at, ordinal, compiled);
            changed = true;
        }
        let now = self.clock.now();
        let leg_indices = {
            let active = &self.active[&key];
            (active.manual_xfade_from_index.is_none()
                && manual_xfade_progress(active.manual_xfade_direction, value) > 0.0)
                .then(|| next_manual_xfade_index(active, &self.cue_lists[&cue_list_id]))
                .flatten()
                .map(|to| (active.cue_index, to))
        };
        let leg_history = if let Some((from, to)) = leg_indices {
            let ordinal = self.take_source_occurrence_ordinal();
            let active = &self.active[&key];
            let source = SequenceMasterSource {
                control_source_id: None,
                playback_address: Some(number),

                cue_list_id,
                temporary: false,
            };
            ordinal.map(|ordinal| {
                PlaybackSourceHistory::cue_leg(
                    active.source_history.as_ref(),
                    &self.compiled_cue_lists[&cue_list_id],
                    source,
                    now,
                    ordinal,
                    from,
                    to,
                    false,
                    true,
                )
            })
        } else {
            None
        };
        let cue_list = self
            .cue_lists
            .get(&cue_list_id)
            .ok_or("playback cue list does not exist")?;
        let active = self.active.get_mut(&key).expect("X-fade playback exists");
        changed |= apply_manual_xfade(
            active,
            cue_list,
            &self.compiled_cue_lists[&cue_list_id],
            number,
            value,
            now,
            leg_history,
        );
        let effect = if changed {
            PlaybackRuntimeEffect::Durable
        } else {
            PlaybackRuntimeEffect::None
        };
        Ok(PlaybackMutation::new((), effect))
    }

    pub fn xfade(&mut self, number: PlaybackAddress, on: bool) -> Result<(), String> {
        self.xfade_mutation(number, on).map(|_| ())
    }

    pub fn xfade_mutation(
        &mut self,
        number: PlaybackAddress,
        on: bool,
    ) -> Result<PlaybackMutation<()>, String> {
        let duration = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .xfade_millis;
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::CueList(id);
        let mut changed = false;
        if on && !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now(), Some(number))?;
            self.active.get_mut(&key).unwrap().master = 0.0;
            changed = true;
        }
        let now = self.clock.now();
        let active = self.active.get_mut(&key).ok_or("playback is not active")?;
        changed |= active.playback_address != Some(number) || (on && !active.enabled);
        if on {
            active.enabled = true;
            active.fader_zero_auto_off_armed = false;
        }
        active.playback_address = Some(number);
        if duration == 0 {
            let target = if on { 1.0 } else { 0.0 };
            changed |= active.master != target || (!on && active.enabled);
            active.master = target;
            if !on {
                active.enabled = false;
                active.fader_zero_auto_off_armed = false;
                active.activation = None;
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

    pub fn preload_timing_state(
        &self,
        identity: PlaybackAddress,
    ) -> Option<PlaybackPreloadTimingState> {
        let key = self.runtime_key(identity).ok()?;
        self.active
            .get(&key)
            .map(|playback| PlaybackPreloadTimingState {
                enabled: playback.enabled,
                master: playback.master,
                activation: playback.activation.clone(),
            })
    }

    pub fn apply_preload_timing(
        &mut self,
        number: PlaybackAddress,
        action: &str,
        started_at: DateTime<Utc>,
        fallback_millis: u64,
        previous: Option<PlaybackPreloadTimingState>,
    ) -> Result<(), String> {
        self.apply_preload_timing_mutation(number, action, started_at, fallback_millis, previous)
            .map(|_| ())
    }

    pub fn apply_preload_timing_mutation(
        &mut self,
        identity: PlaybackAddress,
        action: &str,
        started_at: DateTime<Utc>,
        fallback_millis: u64,
        previous: Option<PlaybackPreloadTimingState>,
    ) -> Result<PlaybackMutation<()>, String> {
        if !matches!(
            self.definition(identity)
                .map(|definition| &definition.target),
            Some(PlaybackTarget::CueList { .. })
        ) {
            return Err("preload timing is available only for Cuelist playbacks".into());
        }
        let key = self.runtime_key(identity)?;
        let durable = self.active.get_mut(&key).is_some_and(|playback| {
            apply_active_preload_timing(playback, action, started_at, fallback_millis, previous)
        });
        let transient = action == "temp-on"
            && self
                .temporary
                .iter_mut()
                .filter(|((held, kind), _)| {
                    held.address == identity && *kind == TemporaryPlaybackKind::TempButton
                })
                .any(|(_, playback)| {
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
    compiled: &Arc<CompiledCueList>,
    number: PlaybackAddress,
    value: f32,
    now: DateTime<Utc>,
    leg_history: Option<PlaybackSourceHistory>,
) -> bool {
    let mut changed = active.playback_address != Some(number)
        || !active.enabled
        || active.fader_position != value
        || active.manual_xfade_position != value;
    active.playback_address = Some(number);
    active.enabled = true;
    active.fader_zero_auto_off_armed = false;
    active.fader_position = value;
    active.manual_xfade_position = value;
    let progress = manual_xfade_progress(active.manual_xfade_direction, value);
    if active.manual_xfade_from_index.is_none()
        && progress > 0.0
        && let Some(next) = next_manual_xfade_index(active, cue_list)
    {
        active.manual_xfade_from_index = Some(active.cue_index);
        active.source_history = leg_history;
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
        complete_manual_xfade(active, cue_list, compiled, now);
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

fn complete_manual_xfade(
    active: &mut ActivePlayback,
    cue_list: &CueList,
    compiled: &Arc<CompiledCueList>,
    now: DateTime<Utc>,
) {
    let target = active
        .manual_xfade_to_index
        .expect("manual X-fade target accompanies source");
    active.cue_index = target;
    active.current_cue_id = Some(cue_list.cues[target].id);
    active.current_cue_number = Some(cue_list.cues[target].number.clone());
    active.previous_index = None;
    active.transition_timing_bypassed = true;
    active.tracking_wrap = target == 0 && cue_list.effective_wrap_mode() == WrapMode::Tracking;
    active.activated_at = now;
    active.completed_trigger_cue_id = None;
    active.manual_xfade_from_index = None;
    active.manual_xfade_to_index = None;
    active.manual_xfade_progress = 0.0;
    active.manual_xfade_direction = match active.manual_xfade_direction {
        ManualXFadeDirection::TowardsHigh => ManualXFadeDirection::TowardsLow,
        ManualXFadeDirection::TowardsLow => ManualXFadeDirection::TowardsHigh,
    };
    active.source_history = active
        .source_history
        .take()
        .and_then(|history| history.complete_manual(compiled, target, active.tracking_wrap));
}

fn apply_active_preload_timing(
    playback: &mut ActivePlayback,
    action: &str,
    started_at: DateTime<Utc>,
    fallback_millis: u64,
    previous: Option<PlaybackPreloadTimingState>,
) -> bool {
    let mut changed = false;
    // Activation uses one Cue value transition, retaining the captured live look. A separate
    // master envelope would multiply that transition and override explicit Cue timing.
    if playback.enabled && matches!(action, "go" | "go-minus" | "on" | "toggle") {
        changed |= set_transition_timing(playback, started_at, fallback_millis);
    }
    match (previous, playback.enabled) {
        (Some(previous), false)
            if previous.enabled && matches!(action, "off" | "toggle") && fallback_millis > 0 =>
        {
            changed |= set_master_transition(
                playback,
                PlaybackMasterTransition {
                    from: previous.master,
                    to: 0.0,
                    started_at,
                    duration_millis: fallback_millis,
                    release_after: true,
                },
                previous.master,
                true,
            );
            playback.activation = previous.activation;
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
    set_transition_timing(playback, started_at, fallback_millis)
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
    playback.completed_trigger_cue_id = None;
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
