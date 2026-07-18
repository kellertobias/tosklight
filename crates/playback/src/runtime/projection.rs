use crate::*;

impl PlaybackEngine {
    pub fn runtime_status(&self) -> Vec<PlaybackRuntimeStatus> {
        let mut runtime = self.runtime();
        for ((number, _), temporary) in &self.temporary {
            if runtime
                .iter()
                .any(|playback| playback.playback_number == Some(*number))
            {
                continue;
            }
            let mut inactive = temporary.clone();
            inactive.enabled = false;
            inactive.master = 0.0;
            inactive.temporary = false;
            inactive.flash = false;
            runtime.push(inactive);
        }
        runtime.sort_by_key(|playback| playback.playback_number.unwrap_or(u16::MAX));
        runtime
            .into_iter()
            .map(|mut playback| {
                let number = playback.playback_number;
                let temporary_master = number
                    .map(|number| {
                        self.temporary
                            .iter()
                            .filter(|((candidate, _), _)| *candidate == number)
                            .map(|(_, playback)| playback.master)
                            .fold(0.0_f32, f32::max)
                    })
                    .unwrap_or(0.0);
                let temporary_active = temporary_master > 0.0
                    || number.is_some_and(|number| {
                        self.temporary
                            .keys()
                            .any(|(candidate, _)| *candidate == number)
                    });
                let swap_active = number.is_some_and(|number| self.swap_held.contains(&number));
                playback.flash = number.is_some_and(|number| {
                    self.temporary
                        .contains_key(&(number, TemporaryPlaybackKind::Flash))
                });
                let cue_list = self.cue_lists.get(&playback.cue_list_id);
                let normal = cue_list.and_then(|list| {
                    if let Some(hold) = &playback.deleted_cue_hold {
                        return hold
                            .next_number
                            .and_then(|number| list.cues.iter().find(|cue| cue.number == number));
                    }
                    if playback.current_cue_id.is_none() && playback.current_cue_number.is_none() {
                        return list.cues.first();
                    }
                    let index = playback
                        .current_cue_id
                        .and_then(|id| list.cues.iter().position(|cue| cue.id == id))
                        .or_else(|| {
                            playback.current_cue_number.and_then(|number| {
                                list.cues.iter().position(|cue| cue.number == number)
                            })
                        })
                        .unwrap_or(playback.cue_index.min(list.cues.len().saturating_sub(1)));
                    list.cues.get(index + 1).or_else(|| {
                        (list.effective_wrap_mode() != WrapMode::Off)
                            .then(|| list.cues.first())
                            .flatten()
                    })
                });
                let loaded = cue_list.and_then(|list| {
                    playback
                        .loaded_cue_id
                        .and_then(|id| list.cues.iter().find(|cue| cue.id == id))
                });
                let effective = loaded.or(normal);
                PlaybackRuntimeStatus {
                    normal_next_cue_id: normal.map(|cue| cue.id),
                    normal_next_cue_number: normal.map(|cue| cue.number),
                    effective_next_cue_id: effective.map(|cue| cue.id),
                    effective_next_cue_number: effective.map(|cue| cue.number),
                    effective_next_is_loaded: loaded.is_some(),
                    temporary_active,
                    temporary_master,
                    swap_active,
                    playback,
                }
            })
            .collect()
    }

    /// Reconstructs the next eventual lit Position state for every fixture whose current tracked
    /// state is dark. Look-ahead deliberately stops at the end of the Cuelist; wrap behavior needs
    /// separate boundary tests before it may cross that edge.
    pub fn move_in_black_candidates(&self) -> Vec<MoveInBlackCandidate> {
        let mut candidates = Vec::new();
        for playback in self.active.values().filter(|playback| playback.enabled) {
            let Some(cue_list) = self.cue_lists.get(&playback.cue_list_id) else {
                continue;
            };
            let Some(current_index) = playback
                .current_cue_id
                .and_then(|id| cue_list.cues.iter().position(|cue| cue.id == id))
                .or_else(|| {
                    playback.current_cue_number.and_then(|number| {
                        cue_list.cues.iter().position(|cue| cue.number == number)
                    })
                })
                .or_else(|| {
                    cue_list
                        .cues
                        .get(playback.cue_index)
                        .map(|_| playback.cue_index)
                })
            else {
                continue;
            };
            let current_cue = &cue_list.cues[current_index];
            let current_state = cue_list.state_at_index(current_index);
            let fixtures = current_state
                .keys()
                .map(|(fixture_id, _)| *fixture_id)
                .collect::<HashSet<_>>();
            for fixture_id in fixtures {
                let current_intensity = current_state
                    .iter()
                    .filter(|((candidate, attribute), _)| {
                        *candidate == fixture_id && attribute.is_intensity()
                    })
                    .filter_map(|(_, value)| value.normalized())
                    .fold(0.0_f32, f32::max);
                if current_intensity != 0.0 {
                    continue;
                }
                let Some((target_index, target_state)) = cue_list
                    .cues
                    .iter()
                    .enumerate()
                    .skip(current_index + 1)
                    .find_map(|(index, _)| {
                        let state = cue_list.state_at_index(index);
                        let intensity = state
                            .iter()
                            .filter(|((candidate, attribute), _)| {
                                *candidate == fixture_id && attribute.is_intensity()
                            })
                            .filter_map(|(_, value)| value.normalized())
                            .fold(0.0_f32, f32::max);
                        (intensity > 0.0).then_some((index, state))
                    })
                else {
                    continue;
                };
                let target_cue = &cue_list.cues[target_index];
                let cue_fade_millis = if cue_list.disable_cue_timing {
                    0
                } else if cue_list.mode == CueListMode::Chaser {
                    effective_chaser_xfade_millis(cue_list, &self.speed_groups_bpm)
                } else if target_cue.fade_millis == 0 {
                    self.sequence_master_fade_millis
                } else {
                    target_cue.fade_millis
                };
                let timing = target_cue
                    .changes
                    .iter()
                    .filter(|change| change.fixture_id == fixture_id)
                    .map(|change| (change.attribute.clone(), change.fade_millis))
                    .collect::<HashMap<_, _>>();
                let position_attributes = current_state
                    .keys()
                    .chain(target_state.keys())
                    .filter(|(candidate, attribute)| {
                        *candidate == fixture_id && attribute.is_position()
                    })
                    .map(|(_, attribute)| attribute.clone())
                    .collect::<HashSet<_>>();
                let mut values = position_attributes
                    .into_iter()
                    .filter_map(|attribute| {
                        let current = current_state
                            .get(&(fixture_id, attribute.clone()))
                            .cloned()
                            .unwrap_or(AttributeValue::Normalized(0.0));
                        let target = target_state
                            .get(&(fixture_id, attribute.clone()))
                            .cloned()
                            .unwrap_or(AttributeValue::Normalized(0.0));
                        if current == target {
                            return None;
                        }
                        let fade_millis = if cue_list.disable_cue_timing {
                            0
                        } else if cue_list.force_cue_timing {
                            cue_fade_millis
                        } else {
                            timing
                                .get(&attribute)
                                .copied()
                                .flatten()
                                .unwrap_or(cue_fade_millis)
                        };
                        Some(MoveInBlackTargetValue {
                            attribute,
                            current,
                            target,
                            fade_millis,
                        })
                    })
                    .collect::<Vec<_>>();
                values.sort_by(|left, right| left.attribute.cmp(&right.attribute));
                if !values.is_empty() {
                    candidates.push(MoveInBlackCandidate {
                        playback_number: playback.playback_number,
                        cue_list_id: cue_list.id,
                        current_cue_id: current_cue.id,
                        current_cue_number: current_cue.number,
                        target_cue_id: target_cue.id,
                        target_cue_number: target_cue.number,
                        fixture_id,
                        priority: cue_list.priority,
                        values,
                    });
                }
            }
        }
        candidates.sort_by(|left, right| {
            left.playback_number
                .cmp(&right.playback_number)
                .then_with(|| left.fixture_id.0.cmp(&right.fixture_id.0))
        });
        candidates
    }
}
