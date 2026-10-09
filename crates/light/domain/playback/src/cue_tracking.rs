use crate::*;

impl PlaybackEngine {
    pub(crate) fn cue_list_for(&self, number: u16) -> Result<CueListId, String> {
        match &self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .target
        {
            PlaybackTarget::CueList { cue_list_id } => Ok(*cue_list_id),
            PlaybackTarget::Group { .. } => {
                Err("operation is not available for a group playback".into())
            }
            _ => Err("operation is not available for this playback function".into()),
        }
    }

    fn key_for_cue_list(&self, id: CueListId) -> Result<PlaybackKey, String> {
        if self.cue_lists.contains_key(&id) {
            Ok(PlaybackKey::CueList(id))
        } else {
            Err("cue list does not exist".into())
        }
    }

    pub fn go(&mut self, id: CueListId) -> Result<&ActivePlayback, String> {
        self.go_at(id, self.clock.now())
    }

    pub fn go_at(&mut self, id: CueListId, now: DateTime<Utc>) -> Result<&ActivePlayback, String> {
        self.timeline_controlled.remove(&id);
        let key = self.key_for_cue_list(id)?;
        self.go_at_key(key, id, now, None)
    }

    fn action_source(
        &self,
        key: PlaybackKey,
        id: CueListId,
        identity: Option<PlaybackIdentity>,
    ) -> SequenceMasterSource {
        let mut source = self
            .active
            .get(&key)
            .map(ActivePlayback::sequence_master_source)
            .unwrap_or(SequenceMasterSource {
                playback_number: None,
                playback_identity: None,
                cue_list_id: id,
                temporary: false,
            });
        if let Some(identity) = identity {
            source.playback_number = Some(identity.number());
            source.playback_identity = identity.virtual_address().map(PlaybackIdentity::Virtual);
        }
        source
    }

    pub(crate) fn go_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        now: DateTime<Utc>,
        identity: Option<PlaybackIdentity>,
    ) -> Result<&ActivePlayback, String> {
        let source = self.action_source(key, id, identity);
        let interrupted_source = self.transition_source_at(key, now);
        let transition_ordinal = self.take_transition_ordinal(now);
        let source_ordinal = self.take_source_occurrence_ordinal();
        let jump_index = self
            .active
            .get(&key)
            .filter(|playback| {
                !playback.paused
                    && playback.loaded_cue_id.is_none()
                    && playback.deleted_cue_hold.is_none()
                    && !self.jump_bypass_once.contains(&id)
            })
            .map(|playback| playback.cue_index)
            .and_then(|index| self.jump_destination(id, index));
        let cue_list = self.cue_lists.get(&id).ok_or("cue list does not exist")?;
        let compiled = &self.compiled_cue_lists[&id];
        let playback = match self.active.entry(key) {
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(first_go_playback(
                id,
                now,
                transition_ordinal,
                cue_list,
                compiled,
                source_ordinal,
                source,
            )),
            std::collections::hash_map::Entry::Occupied(entry) => {
                let playback = entry.into_mut();
                if let Some(loaded) = playback.loaded_cue_id.take() {
                    let index = cue_list
                        .cues
                        .iter()
                        .position(|cue| cue.id == loaded)
                        .ok_or("loaded cue no longer exists")?;
                    if playback.enabled && playback.current_cue_number.is_some() {
                        playback.deleted_cue_transition_source = interrupted_source;
                        playback.previous_index = Some(playback.cue_index);
                    } else {
                        playback.previous_index = None;
                    }
                    playback.cue_index = index;
                    playback.current_cue_id = Some(cue_list.cues[index].id);
                    playback.current_cue_number = Some(cue_list.cues[index].number.clone());
                    playback.loaded_cue_number = None;
                    playback.tracking_wrap = false;
                    playback.paused = false;
                    playback.paused_at = None;
                    playback.activated_at = now;
                    playback.completed_trigger_cue_id = None;
                    playback.transition_ordinal = transition_ordinal;
                    playback.begin_source_history_from(now, source_ordinal, compiled, source);
                    reset_manual_transition(playback);
                    return Ok(playback);
                }
                if let Some(hold) = playback.deleted_cue_hold.take() {
                    if let Some(next) = hold.next_number.as_ref()
                        && let Some(index) =
                            cue_list.cues.iter().position(|cue| cue.number == *next)
                    {
                        playback.deleted_cue_transition_source = Some(hold.contributions.clone());
                        playback.previous_index = None;
                        playback.cue_index = index;
                        playback.current_cue_id = Some(cue_list.cues[index].id);
                        playback.current_cue_number = Some(next.clone());
                        playback.tracking_wrap = false;
                        playback.activated_at = now;
                        playback.completed_trigger_cue_id = None;
                        playback.transition_ordinal = transition_ordinal;
                        playback.begin_source_history_from(now, source_ordinal, compiled, source);
                    } else {
                        playback.deleted_cue_hold = Some(hold);
                    }
                    reset_manual_transition(playback);
                    return Ok(playback);
                }
                let resumed = playback.paused;
                if playback.paused {
                    if let Some(paused_at) = playback.paused_at.take() {
                        playback.activated_at += now - paused_at;
                    }
                    playback.paused = false;
                } else if let Some(index) = jump_index {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index = index;
                    playback.tracking_wrap = false;
                } else if playback.cue_index + 1 < cue_list.cues.len() {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index += 1;
                } else if cue_list.effective_wrap_mode() != WrapMode::Off {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index = 0;
                    playback.tracking_wrap = cue_list.effective_wrap_mode() == WrapMode::Tracking;
                }
                if !resumed {
                    if interrupted_source.is_some() {
                        playback.deleted_cue_transition_source = interrupted_source;
                    }
                    playback.activated_at = now;
                    playback.completed_trigger_cue_id = None;
                }
                playback.transition_ordinal = transition_ordinal;
                if !resumed {
                    playback.begin_source_history_from(now, source_ordinal, compiled, source);
                }
                playback.current_cue_number =
                    Some(cue_list.cues[playback.cue_index].number.clone());
                playback.current_cue_id = Some(cue_list.cues[playback.cue_index].id);
                playback
            }
        };
        reset_manual_transition(playback);
        Ok(playback)
    }

    fn jump_destination(&mut self, id: CueListId, cue_index: usize) -> Option<usize> {
        let cue_list = self.cue_lists.get(&id)?;
        let cue = cue_list.cues.get(cue_index)?;
        let (destination_id, limit) = cue.actions.iter().find_map(|action| match action {
            CueAction::Jump { cue_id, count } => Some((*cue_id, *count)),
            _ => None,
        })?;
        let arrivals = self.jump_counts.entry((id, cue.id)).or_default();
        *arrivals = arrivals.saturating_add(1);
        (*arrivals <= limit).then(|| {
            cue_list
                .cues
                .iter()
                .position(|candidate| candidate.id == destination_id)
                .expect("validated jump destination remains in the Cuelist")
        })
    }

    pub fn jump(
        &mut self,
        id: CueListId,
        cue_number: CueNumber,
    ) -> Result<&ActivePlayback, String> {
        self.jump_at(id, cue_number, self.clock.now())
    }

    pub fn jump_to_cue_id(
        &mut self,
        id: CueListId,
        cue_id: Uuid,
    ) -> Result<&ActivePlayback, String> {
        let number = self
            .cue_lists
            .get(&id)
            .ok_or("cue list does not exist")?
            .cues
            .iter()
            .find(|cue| cue.id == cue_id)
            .map(|cue| cue.number.clone())
            .ok_or("cue does not exist")?;
        self.jump(id, number)
    }

    pub fn execute_timeline_cue_id(
        &mut self,
        id: CueListId,
        cue_id: Uuid,
    ) -> Result<&ActivePlayback, String> {
        self.jump_to_cue_id(id, cue_id)?;
        self.timeline_controlled.insert(id);
        let key = self.key_for_cue_list(id)?;
        let playback = self.active.get_mut(&key).expect("jump installed playback");
        playback.discrete_cue_actions_suppressed = false;
        Ok(playback)
    }

    pub fn reconstruct_to_cue_id(
        &mut self,
        id: CueListId,
        cue_id: Uuid,
        elapsed_millis: u64,
    ) -> Result<&ActivePlayback, String> {
        let number = self
            .cue_lists
            .get(&id)
            .ok_or("cue list does not exist")?
            .cues
            .iter()
            .find(|cue| cue.id == cue_id)
            .map(|cue| cue.number.clone())
            .ok_or("cue does not exist")?;
        let elapsed =
            chrono::Duration::milliseconds(i64::try_from(elapsed_millis).unwrap_or(i64::MAX));
        self.jump_at(id, number, self.clock.now() - elapsed)?;
        self.timeline_controlled.insert(id);
        let key = self.key_for_cue_list(id)?;
        let playback = self.active.get_mut(&key).expect("jump installed playback");
        playback.discrete_cue_actions_suppressed = true;
        playback.previous_index = None;
        playback.deleted_cue_transition_source = None;
        Ok(playback)
    }

    pub fn jump_at(
        &mut self,
        id: CueListId,
        cue_number: CueNumber,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let key = self.key_for_cue_list(id)?;
        self.jump_at_key(key, id, cue_number, now, None)
    }

    pub(crate) fn jump_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        cue_number: CueNumber,
        now: DateTime<Utc>,
        identity: Option<PlaybackIdentity>,
    ) -> Result<&ActivePlayback, String> {
        let source = self.action_source(key, id, identity);
        let interrupted_source = self.transition_source_at(key, now);
        let transition_ordinal = self.take_transition_ordinal(now);
        let source_ordinal = self.take_source_occurrence_ordinal();
        let cue_list = self.cue_lists.get(&id).ok_or("cue list does not exist")?;
        let index = cue_list
            .cues
            .iter()
            .position(|cue| cue.number == cue_number)
            .ok_or("cue does not exist")?;
        let compiled = &self.compiled_cue_lists[&id];
        let playback = self.active.entry(key).or_insert(ActivePlayback {
            playback_number: None,
            playback_identity: None,
            activation: None,
            transition_ordinal,
            cue_list_id: id,
            cue_index: index,
            previous_index: None,
            paused: false,
            activated_at: now,
            paused_at: None,
            completed_trigger_cue_id: None,
            master: 1.0,
            fader_position: 1.0,
            fader_pickup_required: false,
            fader_pickup_target: None,
            flash: false,
            master_transition: None,
            temporary: false,
            enabled: true,
            fader_zero_auto_off_armed: false,
            flash_restore_off: false,
            transition_timing_bypassed: false,
            discrete_cue_actions_suppressed: false,
            transition_fade_fallback_millis: None,
            external_completion_millis: 0,
            manual_xfade_position: 0.0,
            manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
            manual_xfade_from_index: None,
            manual_xfade_to_index: None,
            manual_xfade_progress: 0.0,
            tracking_wrap: false,
            current_cue_id: Some(cue_list.cues[index].id),
            current_cue_number: Some(cue_list.cues[index].number.clone()),
            deleted_cue_hold: None,
            deleted_cue_transition_source: None,
            source_history: None,
            loaded_cue_id: None,
            loaded_cue_number: None,
        });
        if interrupted_source.is_some() {
            playback.deleted_cue_transition_source = interrupted_source;
            playback.previous_index = Some(playback.cue_index);
        } else if playback.cue_index != index {
            playback.previous_index = Some(playback.cue_index);
        }
        playback.cue_index = index;
        playback.current_cue_id = Some(cue_list.cues[index].id);
        playback.current_cue_number = Some(cue_number);
        playback.deleted_cue_hold = None;
        playback.loaded_cue_id = None;
        playback.loaded_cue_number = None;
        playback.tracking_wrap = false;
        playback.paused = false;
        playback.paused_at = None;
        playback.activated_at = now;
        playback.completed_trigger_cue_id = None;
        playback.transition_timing_bypassed = false;
        playback.discrete_cue_actions_suppressed = false;
        playback.transition_ordinal = transition_ordinal;
        playback.begin_source_history_from(now, source_ordinal, compiled, source);
        reset_manual_transition(playback);
        Ok(playback)
    }

    pub fn back(&mut self, id: CueListId) -> Result<&ActivePlayback, String> {
        self.back_at(id, self.clock.now())
    }
    pub fn back_at(
        &mut self,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        self.timeline_controlled.remove(&id);
        let key = self.key_for_cue_list(id)?;
        self.back_at_key(key, id, now, None)
    }
    pub(crate) fn back_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        now: DateTime<Utc>,
        identity: Option<PlaybackIdentity>,
    ) -> Result<&ActivePlayback, String> {
        let source = self.action_source(key, id, identity);
        let interrupted_source = self.transition_source_at(key, now);
        let transition_ordinal = self.take_transition_ordinal(now);
        let source_ordinal = self.take_source_occurrence_ordinal();
        let compiled = &self.compiled_cue_lists[&id];
        let playback = self.active.get_mut(&key).ok_or("cue list is not active")?;
        reset_manual_transition(playback);
        if let Some(hold) = playback.deleted_cue_hold.take() {
            if let Some(previous) = hold.previous_number.as_ref()
                && let Some(index) = self.cue_lists[&id]
                    .cues
                    .iter()
                    .position(|cue| cue.number == *previous)
            {
                playback.deleted_cue_transition_source = Some(hold.contributions.clone());
                playback.previous_index = None;
                playback.cue_index = index;
                playback.current_cue_id = Some(self.cue_lists[&id].cues[index].id);
                playback.current_cue_number = Some(previous.clone());
                playback.tracking_wrap = false;
                playback.activated_at = now;
                playback.completed_trigger_cue_id = None;
                playback.transition_ordinal = transition_ordinal;
                playback.begin_source_history_from(now, source_ordinal, compiled, source);
                playback.paused = false;
                playback.paused_at = None;
            } else {
                playback.deleted_cue_hold = Some(hold);
            }
            return Ok(playback);
        }
        playback.deleted_cue_transition_source = interrupted_source;
        playback.previous_index = Some(playback.cue_index);
        playback.cue_index = playback.cue_index.saturating_sub(1);
        playback.current_cue_id = Some(self.cue_lists[&id].cues[playback.cue_index].id);
        playback.current_cue_number =
            Some(self.cue_lists[&id].cues[playback.cue_index].number.clone());
        playback.tracking_wrap = false;
        playback.activated_at = now;
        playback.completed_trigger_cue_id = None;
        playback.transition_ordinal = transition_ordinal;
        playback.begin_source_history_from(now, source_ordinal, compiled, source);
        playback.paused = false;
        playback.paused_at = None;
        Ok(playback)
    }
    pub fn pause(&mut self, id: CueListId) -> Result<(), String> {
        self.pause_mutation(id).map(|_| ())
    }
    pub fn pause_mutation(&mut self, id: CueListId) -> Result<PlaybackMutation<()>, String> {
        self.pause_at_mutation(id, self.clock.now())
    }
    pub fn pause_playback(&mut self, number: u16) -> Result<(), String> {
        self.pause_playback_mutation(number).map(|_| ())
    }
    pub fn pause_playback_mutation(&mut self, number: u16) -> Result<PlaybackMutation<()>, String> {
        let now = self.clock.now();
        let key = self.runtime_key(number)?;
        self.pause_key_at_mutation(key, now, "playback is not active")
    }
    pub fn pause_playback_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<()>, String> {
        match identity {
            PlaybackIdentity::Physical(number) => self.pause_playback_mutation(number.get()),
            PlaybackIdentity::Virtual(_) => {
                let key = self.runtime_key_at(identity)?;
                self.pause_key_at_mutation(key, self.clock.now(), "virtual playback is not active")
            }
        }
    }
    pub fn pause_at(&mut self, id: CueListId, now: DateTime<Utc>) -> Result<(), String> {
        self.pause_at_mutation(id, now).map(|_| ())
    }
    pub fn pause_at_mutation(
        &mut self,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<PlaybackMutation<()>, String> {
        let key = self.key_for_cue_list(id)?;
        self.pause_key_at_mutation(key, now, "cue list is not active")
    }
    pub fn resume(&mut self, id: CueListId) -> Result<(), String> {
        let key = self.key_for_cue_list(id)?;
        let now = self.clock.now();
        let playback = self.active.get_mut(&key).ok_or("cue list is not active")?;
        if let Some(paused_at) = playback.paused_at.take() {
            playback.activated_at += now - paused_at;
        }
        playback.paused = false;
        Ok(())
    }
    fn pause_key_at_mutation(
        &mut self,
        key: PlaybackKey,
        now: DateTime<Utc>,
        inactive_error: &'static str,
    ) -> Result<PlaybackMutation<()>, String> {
        let playback = self.active.get_mut(&key).ok_or(inactive_error)?;
        if playback.paused {
            return Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::None));
        }
        playback.paused = true;
        playback.paused_at = Some(now);
        Ok(PlaybackMutation::new((), PlaybackRuntimeEffect::Durable))
    }
    pub fn release(&mut self, id: CueListId) -> bool {
        self.timeline_controlled.remove(&id);
        self.jump_counts
            .retain(|(cue_list_id, _), _| *cue_list_id != id);
        self.key_for_cue_list(id)
            .ok()
            .is_some_and(|key| self.active.remove(&key).is_some())
    }
    pub fn active(&self) -> Vec<ActivePlayback> {
        self.active
            .values()
            .filter(|playback| playback.enabled)
            .chain(self.temporary.values())
            .cloned()
            .collect()
    }
    pub fn runtime(&self) -> Vec<ActivePlayback> {
        let mut runtime = self.active.values().cloned().collect::<Vec<_>>();
        runtime.sort_by_key(|playback| playback.playback_number.unwrap_or(u16::MAX));
        runtime
    }
    pub fn playback_runtime(&self, number: u16) -> Option<&ActivePlayback> {
        let key = self.runtime_key(number).ok()?;
        self.active.get(&key)
    }

    pub fn playback_runtime_at(&self, identity: PlaybackIdentity) -> Option<&ActivePlayback> {
        let key = self.runtime_key_at(identity).ok()?;
        self.active.get(&key)
    }

    pub fn is_active_at(&self, identity: PlaybackIdentity) -> bool {
        self.playback_runtime_at(identity)
            .is_some_and(|runtime| runtime.enabled)
            || self
                .active_dynamic_playback_at(identity)
                .is_some_and(|runtime| runtime.enabled)
            || self
                .temporary
                .keys()
                .any(|(candidate, _)| *candidate == identity)
    }

    /// Carry the current static Cue output of Solo peers into the incoming Cue's transition.
    /// Peers can become logically Off immediately without creating an unowned black interval.
    /// This is separate from ordinary Off/Release, which continue to remove ownership at once.
    pub fn adopt_solo_handover(
        &mut self,
        incoming: PlaybackIdentity,
        peers: &[PlaybackIdentity],
    ) -> Result<(), String> {
        if self.dynamic_assignment_at(incoming).is_some() {
            return Ok(());
        }
        let key = self.runtime_key_at(incoming)?;
        let peer_keys: HashSet<_> = peers
            .iter()
            .filter_map(|peer| self.runtime_key_at(*peer).ok())
            .filter(|peer| *peer != key)
            .collect();
        if peer_keys.is_empty() {
            return Ok(());
        }
        let now = self.clock.now();
        let mut retained: HashMap<AttributeAddress, PlaybackContribution> = HashMap::new();
        for row in self.contributions_with_context(now, None) {
            if !peer_keys.contains(&PlaybackKey::CueList(row.source.cue_list_id)) {
                continue;
            }
            let address = (row.value.fixture_id, row.value.attribute.clone());
            let replace = retained.get(&address).is_none_or(|old| {
                let incoming = &row.value;
                let previous = &old.value;
                if incoming.priority != previous.priority {
                    return incoming.priority > previous.priority;
                }
                if incoming.merge_mode == light_core::MergeMode::Htp {
                    return incoming.value.normalized().unwrap_or(0.)
                        > previous.value.normalized().unwrap_or(0.);
                }
                (incoming.changed_at, row.transition_ordinal)
                    > (previous.changed_at, old.transition_ordinal)
            });
            if replace {
                retained.insert(address, row);
            }
        }
        if retained.is_empty() {
            return Ok(());
        }
        let ordinal = crate::source_evidence::take_occurrence_ordinal(
            &mut self.next_source_occurrence_ordinal,
        );
        let playback = self
            .active
            .get_mut(&key)
            .ok_or("incoming Solo playback is not active")?;
        playback.deleted_cue_transition_source = Some(
            retained
                .into_values()
                .map(PlaybackRetainedValue::from)
                .collect(),
        );
        playback.begin_source_history(
            now,
            ordinal,
            &self.compiled_cue_lists[&playback.cue_list_id],
        );
        Ok(())
    }

    /// Adopt only Color pairs authored by the incoming Cue, from the last accepted output.
    /// This is an activation source, not an additional producer or fabricated author evidence.
    pub fn adopt_published_color_start(
        &mut self,
        incoming: PlaybackIdentity,
        value_at: &dyn Fn(FixtureId, &AttributeKey) -> Option<AttributeValue>,
    ) -> Result<(), String> {
        if self.dynamic_assignment_at(incoming).is_some() {
            return Ok(());
        }
        let key = self.runtime_key_at(incoming)?;
        let Some(active) = self.active.get(&key).filter(|active| active.enabled) else {
            return Ok(());
        };
        let compiled = &self.compiled_cue_lists[&active.cue_list_id];
        let priority = self.cue_lists[&active.cue_list_id].priority;
        let retained: Vec<_> = compiled
            .attributes_through(active.cue_index)
            .iter()
            .filter_map(|attribute| {
                let target = attribute.value(active.cue_index, active.tracking_wrap)?;
                let address = (attribute.fixture_id(), attribute.attribute().clone());
                let start = value_at(address.0, &address.1)?;
                // Cross-model/semantic-to-Direct crossings need appearance capture at the physical
                // adapter. Do not pretend an unsupported pair is a continuous native transition.
                use light_core::programming::ColorProgram;
                let compatible = match (&start, target) {
                    (AttributeValue::ColorProgram(a), AttributeValue::ColorProgram(b)) => {
                        match (a.as_ref(), b.as_ref()) {
                            (
                                ColorProgram::Direct { recipe: a, .. },
                                ColorProgram::Direct { recipe: b, .. },
                            ) => a.source == b.source,
                            (ColorProgram::Semantic { .. }, ColorProgram::Semantic { .. }) => true,
                            _ => false,
                        }
                    }
                    _ => false,
                };
                compatible.then(|| PlaybackRetainedValue {
                    replacement_projection: None,
                    timed: TimedValue {
                        fixture_id: address.0,
                        attribute: address.1,
                        value: start.clone(),
                        priority,
                        changed_at: active.activated_at,
                        programmer_order: 0,
                        merge_mode: light_core::MergeMode::Ltp,
                        fade: false,
                        fade_millis: None,
                        delay_millis: None,
                    },
                    family_evidence: None,
                    pending_transition: None,
                })
            })
            .collect();
        if retained.is_empty() {
            return Ok(());
        }
        let ordinal = crate::source_evidence::take_occurrence_ordinal(
            &mut self.next_source_occurrence_ordinal,
        );
        let active = self.active.get_mut(&key).expect("validated active Cue");
        active.deleted_cue_transition_source = Some(retained);
        active.begin_source_history(
            active.activated_at,
            ordinal,
            &self.compiled_cue_lists[&active.cue_list_id],
        );
        Ok(())
    }

    pub fn release_at_mutation(
        &mut self,
        identity: PlaybackIdentity,
    ) -> Result<PlaybackMutation<()>, String> {
        self.definition_at(identity)
            .ok_or("playback does not exist")?;
        let dynamic_flash_effect = if self.dynamic_flash_states.contains_key(&identity) {
            self.set_dynamic_flash_at_mutation(identity, false)?.effect
        } else {
            PlaybackRuntimeEffect::None
        };
        let durable = if self.dynamic_assignment_at(identity).is_some() {
            self.off_dynamic_at_mutation(identity)?.value
        } else {
            self.off_at(identity)?
        };
        let before = self.temporary.len();
        self.temporary
            .retain(|(candidate, _), _| *candidate != identity);
        let transient = before != self.temporary.len()
            || self.swap_held.remove(&identity)
            || self.cuelist_flash_states.remove(&identity).is_some()
            || self.cuelist_swap_states.remove(&identity).is_some()
            || dynamic_flash_effect.changed();
        Ok(PlaybackMutation::new(
            (),
            (if durable {
                PlaybackRuntimeEffect::Durable
            } else {
                PlaybackRuntimeEffect::None
            })
            .combine(if transient {
                PlaybackRuntimeEffect::Transient
            } else {
                PlaybackRuntimeEffect::None
            }),
        ))
    }
}

/// The playback a first Go starts on the Cuelist's first cue.
fn first_go_playback(
    id: CueListId,
    now: DateTime<Utc>,
    transition_ordinal: u64,
    cue_list: &CueList,
    compiled: &Arc<CompiledCueList>,
    source_ordinal: Option<u64>,
    source: SequenceMasterSource,
) -> ActivePlayback {
    ActivePlayback {
        playback_number: None,
        playback_identity: None,
        activation: None,
        transition_ordinal,
        cue_list_id: id,
        cue_index: 0,
        previous_index: None,
        paused: false,
        activated_at: now,
        paused_at: None,
        completed_trigger_cue_id: None,
        master: 1.0,
        fader_position: 1.0,
        fader_pickup_required: false,
        fader_pickup_target: None,
        flash: false,
        master_transition: None,
        temporary: false,
        enabled: true,
        fader_zero_auto_off_armed: false,
        flash_restore_off: false,
        transition_timing_bypassed: false,
        discrete_cue_actions_suppressed: false,
        transition_fade_fallback_millis: None,
        external_completion_millis: 0,
        manual_xfade_position: 0.0,
        manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
        manual_xfade_from_index: None,
        manual_xfade_to_index: None,
        manual_xfade_progress: 0.0,
        tracking_wrap: false,
        current_cue_id: Some(cue_list.cues[0].id),
        current_cue_number: Some(cue_list.cues[0].number.clone()),
        deleted_cue_hold: None,
        deleted_cue_transition_source: None,
        source_history: source_ordinal.map(|ordinal| {
            PlaybackSourceHistory::next(None, now, ordinal, compiled, 0, false, source, false)
        }),
        loaded_cue_id: None,
        loaded_cue_number: None,
    }
}
