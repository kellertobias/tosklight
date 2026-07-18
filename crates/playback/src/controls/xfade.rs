use crate::*;

impl PlaybackEngine {
    pub fn set_manual_xfade(&mut self, number: u16, value: f32) -> Result<(), String> {
        self.set_manual_xfade_inner(number, value, false)
    }

    pub(crate) fn set_manual_xfade_inner(
        &mut self,
        number: u16,
        value: f32,
        allow_faderless: bool,
    ) -> Result<(), String> {
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
        if !self.active.contains_key(&key) {
            let cue_list = self
                .cue_lists
                .get(&cue_list_id)
                .ok_or("playback cue list does not exist")?;
            self.active.insert(
                key,
                new_active_playback(Some(number), cue_list, self.clock.now(), 1.0, true),
            );
        }
        let cue_list = self
            .cue_lists
            .get(&cue_list_id)
            .ok_or("playback cue list does not exist")?;
        let active = self.active.get_mut(&key).expect("X-fade playback exists");
        active.playback_number = Some(number);
        active.enabled = true;
        active.fader_position = value;
        active.manual_xfade_position = value;
        let progress = match active.manual_xfade_direction {
            ManualXFadeDirection::TowardsHigh => value,
            ManualXFadeDirection::TowardsLow => 1.0 - value,
        };
        if active.manual_xfade_from_index.is_none() && progress > 0.0 {
            let next = if active.cue_index + 1 < cue_list.cues.len() {
                Some(active.cue_index + 1)
            } else if cue_list.effective_wrap_mode() != WrapMode::Off {
                Some(0)
            } else {
                None
            };
            if let Some(next) = next {
                active.manual_xfade_from_index = Some(active.cue_index);
                active.manual_xfade_to_index = Some(next);
                active.transition_timing_bypassed = false;
            }
        }
        if active.manual_xfade_from_index.is_none() {
            active.manual_xfade_progress = 0.0;
            return Ok(());
        }
        active.manual_xfade_progress = progress.clamp(0.0, 1.0);
        if progress >= 1.0 {
            let target = active
                .manual_xfade_to_index
                .expect("manual X-fade target accompanies source");
            active.cue_index = target;
            active.current_cue_id = Some(cue_list.cues[target].id);
            active.current_cue_number = Some(cue_list.cues[target].number);
            active.previous_index = None;
            active.transition_timing_bypassed = true;
            active.tracking_wrap =
                target == 0 && cue_list.effective_wrap_mode() == WrapMode::Tracking;
            active.activated_at = self.clock.now();
            active.manual_xfade_from_index = None;
            active.manual_xfade_to_index = None;
            active.manual_xfade_progress = 0.0;
            active.manual_xfade_direction = match active.manual_xfade_direction {
                ManualXFadeDirection::TowardsHigh => ManualXFadeDirection::TowardsLow,
                ManualXFadeDirection::TowardsLow => ManualXFadeDirection::TowardsHigh,
            };
        }
        Ok(())
    }

    pub fn xfade(&mut self, number: u16, on: bool) -> Result<(), String> {
        let duration = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .xfade_millis;
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        if on && !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now())?;
            self.active.get_mut(&key).unwrap().master = 0.0;
        }
        let active = self.active.get_mut(&key).ok_or("playback is not active")?;
        if on {
            active.enabled = true;
        }
        active.playback_number = Some(number);
        if duration == 0 {
            active.master = if on { 1.0 } else { 0.0 };
            if !on {
                self.active.get_mut(&key).unwrap().enabled = false;
            }
        } else {
            active.master_transition = Some(PlaybackMasterTransition {
                from: active.master,
                to: if on { 1.0 } else { 0.0 },
                started_at: self.clock.now(),
                duration_millis: duration,
                release_after: !on,
            });
        }
        Ok(())
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
        self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        if let Some(playback) = self.active.get_mut(&key) {
            if playback.enabled && matches!(action, "go" | "go-minus" | "on" | "toggle") {
                playback.activated_at = started_at;
                playback.transition_timing_bypassed = false;
                playback.transition_fade_fallback_millis = Some(fallback_millis);
            }

            match (previous, playback.enabled) {
                (Some((false, _)), true)
                    if matches!(action, "go" | "on" | "toggle") && fallback_millis > 0 =>
                {
                    let target = playback.master;
                    playback.master = 0.0;
                    playback.master_transition = Some(PlaybackMasterTransition {
                        from: 0.0,
                        to: target,
                        started_at,
                        duration_millis: fallback_millis,
                        release_after: false,
                    });
                }
                (Some((true, previous_master)), false)
                    if matches!(action, "off" | "toggle") && fallback_millis > 0 =>
                {
                    playback.enabled = true;
                    playback.master = previous_master;
                    playback.master_transition = Some(PlaybackMasterTransition {
                        from: previous_master,
                        to: 0.0,
                        started_at,
                        duration_millis: fallback_millis,
                        release_after: true,
                    });
                }
                _ => {}
            }
        }

        if action == "temp-on"
            && let Some(playback) = self
                .temporary
                .get_mut(&(number, TemporaryPlaybackKind::TempButton))
        {
            let target = playback.master;
            playback.activated_at = started_at;
            playback.transition_timing_bypassed = false;
            playback.transition_fade_fallback_millis = Some(fallback_millis);
            if fallback_millis > 0 {
                playback.master = 0.0;
                playback.master_transition = Some(PlaybackMasterTransition {
                    from: 0.0,
                    to: target,
                    started_at,
                    duration_millis: fallback_millis,
                    release_after: false,
                });
            }
        }
        Ok(())
    }
}
