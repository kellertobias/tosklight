use crate::*;

impl PlaybackEngine {
    pub(crate) fn auto_off_overwritten(&mut self) -> bool {
        let full: Vec<_> = self
            .active
            .iter()
            .filter(|(_, p)| p.enabled && p.master >= 1.0 && !p.flash && !p.temporary)
            .map(|(key, p)| (*key, p.cue_list_id, p.activated_at, p.transition_ordinal))
            .collect();
        let mut release = Vec::new();
        for (own_key, playback) in &self.active {
            if !playback.enabled {
                continue;
            }
            let Some(number) = playback.playback_number else {
                continue;
            };
            if !self.definitions.get(&number).is_some_and(|d| d.auto_off) {
                continue;
            }
            let own = self.cue_lists[&playback.cue_list_id].state_at_index(playback.cue_index);
            if own.is_empty() {
                continue;
            }
            let own_list = &self.cue_lists[&playback.cue_list_id];
            let covered = own.iter().all(|(address, own_value)| {
                full.iter()
                    .any(|(other_key, other, changed, transition_ordinal)| {
                        if other_key == own_key {
                            return false;
                        }
                        let other_list = &self.cue_lists[other];
                        let Some(other_value) = other_list
                            .state_at_index(self.active[other_key].cue_index)
                            .get(address)
                            .cloned()
                        else {
                            return false;
                        };
                        if other_list.priority != own_list.priority {
                            other_list.priority > own_list.priority
                        } else if address.1.is_intensity() {
                            other_value.normalized().unwrap_or(0.0)
                                > own_value.normalized().unwrap_or(0.0)
                        } else {
                            (*changed, *transition_ordinal)
                                > (playback.activated_at, playback.transition_ordinal)
                        }
                    })
            });
            if covered {
                release.push(*own_key);
            }
        }
        let changed = !release.is_empty();
        let released_cue_lists = release
            .iter()
            .filter_map(|key| self.active.get(key).map(|playback| playback.cue_list_id))
            .collect::<HashSet<_>>();
        for key in release {
            if let Some(playback) = self.active.get_mut(&key) {
                playback.enabled = false;
                playback.fader_zero_auto_off_armed = false;
                playback.activation = None;
            }
        }
        self.jump_counts
            .retain(|(cue_list_id, _), _| !released_cue_lists.contains(cue_list_id));
        changed
    }
    pub fn restore_active(&mut self, playbacks: impl IntoIterator<Item = ActivePlayback>) {
        self.control_states.clear();
        for mut playback in playbacks {
            let addressed_identity = playback.playback_identity.or_else(|| {
                playback
                    .playback_number
                    .and_then(|number| PlaybackIdentity::physical(number).ok())
            });
            if let Some(identity) = addressed_identity {
                let identity = if self.identity_targets_cue_list(identity, playback.cue_list_id) {
                    identity
                } else if let Some(surviving) =
                    self.first_identity_for_cue_list(playback.cue_list_id)
                {
                    surviving
                } else {
                    continue;
                };
                playback.playback_identity =
                    identity.virtual_address().map(PlaybackIdentity::Virtual);
                playback.playback_number = Some(identity.number());
            }
            let key = PlaybackKey::CueList(playback.cue_list_id);
            let Some(cue_list) = self.cue_lists.get(&playback.cue_list_id) else {
                continue;
            };
            let Some(last) = cue_list.cues.len().checked_sub(1) else {
                continue;
            };
            if playback.deleted_cue_hold.is_none()
                && let Some(index) = playback
                    .current_cue_id
                    .and_then(|id| cue_list.cues.iter().position(|cue| cue.id == id))
                    .or_else(|| {
                        playback.current_cue_number.and_then(|number| {
                            cue_list.cues.iter().position(|cue| cue.number == number)
                        })
                    })
            {
                playback.cue_index = index;
                playback.current_cue_id = Some(cue_list.cues[index].id);
                playback.current_cue_number = Some(cue_list.cues[index].number);
            } else {
                playback.cue_index = playback.cue_index.min(last);
            }
            playback.previous_index = playback.previous_index.map(|index| index.min(last));
            playback.manual_xfade_from_index = playback
                .manual_xfade_from_index
                .map(|index| index.min(last));
            playback.manual_xfade_to_index =
                playback.manual_xfade_to_index.map(|index| index.min(last));
            playback.manual_xfade_position = playback.manual_xfade_position.clamp(0.0, 1.0);
            playback.manual_xfade_progress = playback.manual_xfade_progress.clamp(0.0, 1.0);
            if let Some(loaded) = playback.loaded_cue_id
                && let Some(cue) = cue_list.cues.iter().find(|cue| cue.id == loaded)
            {
                playback.loaded_cue_number = Some(cue.number);
            } else if playback.loaded_cue_id.is_some() {
                playback.loaded_cue_id = None;
                playback.loaded_cue_number = None;
            }
            self.observe_restored_activation(playback.activation.as_ref());
            self.observe_restored_transition_ordinal(playback.transition_ordinal);
            match self.active.entry(key) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(playback);
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    if restored_runtime_wins(&playback, entry.get()) {
                        entry.insert(playback);
                    }
                }
            }
        }
        let restored_targets = self
            .active
            .values()
            .map(|playback| (playback.cue_list_id, playback.master))
            .collect::<Vec<_>>();
        for (cue_list_id, master) in restored_targets {
            self.retarget_physical_controls(cue_list_id, master, None);
        }
    }

    fn identity_targets_cue_list(
        &self,
        identity: PlaybackIdentity,
        cue_list_id: CueListId,
    ) -> bool {
        self.definition_at(identity).is_some_and(|definition| {
            matches!(
                definition.target,
                PlaybackTarget::CueList { cue_list_id: assigned } if assigned == cue_list_id
            )
        })
    }

    fn first_identity_for_cue_list(&self, cue_list_id: CueListId) -> Option<PlaybackIdentity> {
        self.definitions
            .iter()
            .filter(|(_, definition)| {
                matches!(
                    definition.target,
                    PlaybackTarget::CueList { cue_list_id: assigned } if assigned == cue_list_id
                )
            })
            .map(|(number, _)| {
                PlaybackIdentity::physical(*number).expect("registered physical number")
            })
            .chain(
                self.virtual_definitions
                    .iter()
                    .filter(|(_, definition)| {
                        matches!(
                            definition.target,
                            PlaybackTarget::CueList { cue_list_id: assigned } if assigned == cue_list_id
                        )
                    })
                    .map(|(address, _)| PlaybackIdentity::Virtual(*address)),
            )
            .min()
    }

    pub fn restore_active_dynamics(
        &mut self,
        playbacks: impl IntoIterator<Item = ActiveDynamicPlayback>,
    ) {
        self.active_dynamics.clear();
        self.dynamic_flash_states.clear();
        let mut restored_origins = HashMap::<Uuid, PlaybackIdentity>::new();
        for mut playback in playbacks {
            let persisted_identity = playback.playback_identity.unwrap_or_else(|| {
                PlaybackIdentity::physical(playback.playback_number)
                    .expect("persisted physical Dynamic Playback number")
            });
            let Some(target_id) = playback
                .dynamic_id
                .or_else(|| self.dynamic_target_id_at(persisted_identity))
            else {
                continue;
            };
            let Some(identity) = self
                .dynamic_target_id_at(persisted_identity)
                .filter(|candidate| *candidate == target_id)
                .map(|_| persisted_identity)
                .or_else(|| self.first_dynamic_identity(target_id))
            else {
                continue;
            };
            if !playback.fader_value.is_finite()
                || !playback.size.is_finite()
                || !playback.master.is_finite()
                || playback.local_speed_multiplier.denominator == 0
            {
                continue;
            }
            playback.fader_value = playback.fader_value.clamp(0.0, 1.0);
            playback.size = playback.size.clamp(0.0, 1.0);
            playback.master = playback.master.clamp(0.0, 1.0);
            if playback.flash_restore_off {
                playback.enabled = false;
            }
            playback.flash = false;
            playback.flash_restore_off = false;
            playback.fader_pickup_required = false;
            playback.fader_pickup_target = None;
            playback.dynamic_id = Some(target_id);
            playback.playback_number = identity.number();
            playback.playback_identity = identity.virtual_address().map(PlaybackIdentity::Virtual);
            let replace = self.active_dynamics.get(&target_id).is_none_or(|current| {
                playback.activated_at > current.activated_at
                    || (playback.activated_at == current.activated_at
                        && persisted_identity
                            < *restored_origins
                                .get(&target_id)
                                .expect("restored Dynamic origin accompanies runtime"))
            });
            if replace {
                self.active_dynamics.insert(target_id, playback);
                restored_origins.insert(target_id, persisted_identity);
            }
        }
    }

    pub fn active_dynamics_for_snapshot(
        &self,
        next: &PlaybackEngine,
    ) -> Vec<ActiveDynamicPlayback> {
        self.active_dynamics
            .iter()
            .filter_map(|(target_id, active)| {
                let identity = next.first_dynamic_identity(*target_id)?;
                let mut active = active.clone();
                if active.flash_restore_off {
                    active.enabled = false;
                }
                active.flash = false;
                active.flash_restore_off = false;
                active.fader_pickup_required = false;
                active.fader_pickup_target = None;
                active.dynamic_id = Some(*target_id);
                active.playback_number = identity.number();
                active.playback_identity =
                    identity.virtual_address().map(PlaybackIdentity::Virtual);
                Some(active)
            })
            .collect()
    }

    pub fn active_for_snapshot(
        &self,
        next_lists: &[CueList],
        now: DateTime<Utc>,
    ) -> Vec<ActivePlayback> {
        self.active
            .iter()
            .map(|(key, value)| {
                let mut playback = value.clone();
                let Some(old_list) = self.cue_lists.get(&playback.cue_list_id) else {
                    return playback;
                };
                let infer_legacy_current = playback.enabled
                    || playback.current_cue_number.is_some()
                    || playback.current_cue_id.is_some();
                let current_id = playback.current_cue_id.or_else(|| {
                    infer_legacy_current
                        .then(|| old_list.cues.get(playback.cue_index).map(|cue| cue.id))
                        .flatten()
                });
                playback.current_cue_id = current_id;
                let number = playback.current_cue_number.or_else(|| {
                    infer_legacy_current
                        .then(|| old_list.cues.get(playback.cue_index).map(|cue| cue.number))
                        .flatten()
                });
                playback.current_cue_number = number;
                let Some(number) = number else {
                    return playback;
                };
                let Some(next) = next_lists
                    .iter()
                    .find(|list| list.id == playback.cue_list_id)
                else {
                    return playback;
                };
                if let Some(index) =
                    current_id.and_then(|id| next.cues.iter().position(|cue| cue.id == id))
                {
                    playback.cue_index = index;
                    playback.current_cue_number = Some(next.cues[index].number);
                    return playback;
                }
                if !playback.enabled {
                    playback.cue_index = 0;
                    playback.previous_index = None;
                    playback.current_cue_id = None;
                    playback.current_cue_number = None;
                    playback.deleted_cue_hold = None;
                    playback.deleted_cue_transition_source = None;
                    return playback;
                }
                let previous_number = next
                    .cues
                    .iter()
                    .rfind(|cue| cue.number < number)
                    .map(|cue| cue.number);
                let next_number = next
                    .cues
                    .iter()
                    .find(|cue| cue.number > number)
                    .map(|cue| cue.number);
                let mut isolated = PlaybackEngine {
                    cue_lists: self.cue_lists.clone(),
                    compiled_cue_lists: self.compiled_cue_lists.clone(),
                    active: HashMap::from([(*key, playback.clone())]),
                    jump_counts: self.jump_counts.clone(),
                    jump_bypass_once: HashSet::new(),
                    control_states: HashMap::new(),
                    active_dynamics: HashMap::new(),
                    dynamic_flash_states: HashMap::new(),
                    cuelist_flash_states: HashMap::new(),
                    cuelist_swap_states: HashMap::new(),
                    temporary: HashMap::new(),
                    swap_held: HashSet::new(),
                    dynamics_paused_at: None,
                    speed_groups_bpm: self.speed_groups_bpm,
                    speed_groups_paused: self.speed_groups_paused,
                    sequence_master_fade_millis: self.sequence_master_fade_millis,
                    definitions: self.definitions.clone(),
                    virtual_definitions: self.virtual_definitions.clone(),
                    clock: Arc::clone(&self.clock),
                    next_activation_ordinal: self.next_activation_ordinal,
                    next_transition_ordinal: self.next_transition_ordinal,
                };
                isolated.active.get_mut(key).unwrap().deleted_cue_hold = None;
                isolated
                    .active
                    .get_mut(key)
                    .unwrap()
                    .deleted_cue_transition_source = None;
                playback.deleted_cue_transition_source = None;
                playback.deleted_cue_hold = Some(DeletedCueHold {
                    deleted_number: number,
                    previous_number,
                    next_number,
                    contributions: isolated.contributions_at(now),
                });
                playback
            })
            .collect()
    }
}

fn restored_runtime_wins(candidate: &ActivePlayback, current: &ActivePlayback) -> bool {
    let candidate_ordinal = candidate
        .activation
        .as_ref()
        .map(|activation| activation.ordinal)
        .unwrap_or(0);
    let current_ordinal = current
        .activation
        .as_ref()
        .map(|activation| activation.ordinal)
        .unwrap_or(0);
    if candidate_ordinal != current_ordinal {
        return candidate_ordinal > current_ordinal;
    }
    if candidate.activated_at != current.activated_at {
        return candidate.activated_at > current.activated_at;
    }
    if candidate.transition_ordinal != current.transition_ordinal {
        return candidate.transition_ordinal > current.transition_ordinal;
    }
    candidate.playback_identity < current.playback_identity
}
