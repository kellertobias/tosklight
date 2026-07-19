use crate::*;

impl PlaybackEngine {
    pub(crate) fn auto_off_overwritten(&mut self) {
        let full: Vec<_> = self
            .active
            .iter()
            .filter(|(_, p)| p.enabled && p.master >= 1.0 && !p.flash && !p.temporary)
            .map(|(key, p)| (*key, p.cue_list_id, p.activated_at))
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
                full.iter().any(|(other_key, other, changed)| {
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
                        *changed > playback.activated_at
                    }
                })
            });
            if covered {
                release.push(*own_key);
            }
        }
        for key in release {
            if let Some(playback) = self.active.get_mut(&key) {
                playback.enabled = false;
            }
        }
    }
    pub fn restore_active(&mut self, playbacks: impl IntoIterator<Item = ActivePlayback>) {
        for mut playback in playbacks {
            if let Some(number) = playback.playback_number
                && !self.definitions.get(&number).is_some_and(|definition| {
                    matches!(
                        definition.target,
                        PlaybackTarget::CueList { cue_list_id }
                            if cue_list_id == playback.cue_list_id
                    )
                })
            {
                continue;
            }
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
            let key = playback
                .playback_number
                .map(PlaybackKey::Number)
                .unwrap_or(PlaybackKey::CueList(playback.cue_list_id));
            self.active.insert(key, playback);
        }
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
                    temporary: HashMap::new(),
                    swap_held: HashSet::new(),
                    dynamics_paused_at: None,
                    speed_groups_bpm: self.speed_groups_bpm,
                    speed_groups_paused: self.speed_groups_paused,
                    sequence_master_fade_millis: self.sequence_master_fade_millis,
                    definitions: self.definitions.clone(),
                    clock: Arc::clone(&self.clock),
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
