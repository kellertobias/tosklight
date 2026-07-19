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
            let Some(compiled) = self.compiled_cue_lists.get(&playback.cue_list_id) else {
                continue;
            };
            let Some(current_index) = current_cue_index(playback, cue_list) else {
                continue;
            };
            for fixture_id in compiled.fixture_ids_through(current_index) {
                let Some(candidate) = self.move_in_black_candidate(
                    playback,
                    cue_list,
                    compiled,
                    current_index,
                    fixture_id,
                ) else {
                    continue;
                };
                candidates.push(candidate);
            }
        }
        candidates.sort_by(|left, right| {
            left.playback_number
                .cmp(&right.playback_number)
                .then_with(|| left.fixture_id.0.cmp(&right.fixture_id.0))
        });
        candidates
    }

    fn move_in_black_candidate(
        &self,
        playback: &ActivePlayback,
        cue_list: &CueList,
        compiled: &CompiledCueList,
        current_index: usize,
        fixture_id: FixtureId,
    ) -> Option<MoveInBlackCandidate> {
        if !fixture_has_state(compiled, fixture_id, current_index)
            || fixture_intensity(compiled, fixture_id, current_index) != 0.0
        {
            return None;
        }
        let target_index = (current_index + 1..cue_list.cues.len())
            .find(|index| fixture_intensity(compiled, fixture_id, *index) > 0.0)?;
        let target_cue = &cue_list.cues[target_index];
        let cue_fade_millis = cue_fade_millis(
            cue_list,
            target_cue,
            &self.speed_groups_bpm,
            self.sequence_master_fade_millis,
        );
        let mut values = move_in_black_values(
            compiled,
            cue_list,
            fixture_id,
            current_index,
            target_index,
            cue_fade_millis,
        );
        if values.is_empty() {
            return None;
        }
        values.sort_by(|left, right| left.attribute.cmp(&right.attribute));
        let current_cue = &cue_list.cues[current_index];
        Some(MoveInBlackCandidate {
            playback_number: playback.playback_number,
            cue_list_id: cue_list.id,
            current_cue_id: current_cue.id,
            current_cue_number: current_cue.number,
            target_cue_id: target_cue.id,
            target_cue_number: target_cue.number,
            fixture_id,
            priority: cue_list.priority,
            values,
        })
    }
}

fn current_cue_index(playback: &ActivePlayback, cue_list: &CueList) -> Option<usize> {
    playback
        .current_cue_id
        .and_then(|id| cue_list.cues.iter().position(|cue| cue.id == id))
        .or_else(|| {
            playback
                .current_cue_number
                .and_then(|number| cue_list.cues.iter().position(|cue| cue.number == number))
        })
        .or_else(|| {
            cue_list
                .cues
                .get(playback.cue_index)
                .map(|_| playback.cue_index)
        })
}

fn fixture_has_state(compiled: &CompiledCueList, fixture_id: FixtureId, cue_index: usize) -> bool {
    compiled
        .attributes_for_fixture(fixture_id)
        .any(|attribute| attribute.value(cue_index, false).is_some())
}

fn fixture_intensity(compiled: &CompiledCueList, fixture_id: FixtureId, cue_index: usize) -> f32 {
    compiled
        .attributes_for_fixture(fixture_id)
        .filter(|attribute| attribute.attribute().is_intensity())
        .filter_map(|attribute| attribute.value(cue_index, false))
        .filter_map(AttributeValue::normalized)
        .fold(0.0_f32, f32::max)
}

fn cue_fade_millis(
    cue_list: &CueList,
    target_cue: &Cue,
    speed_groups_bpm: &[f64; 5],
    sequence_master_fade_millis: u64,
) -> u64 {
    if cue_list.disable_cue_timing {
        0
    } else if cue_list.mode == CueListMode::Chaser {
        effective_chaser_xfade_millis(cue_list, speed_groups_bpm)
    } else if target_cue.fade_millis == 0 {
        sequence_master_fade_millis
    } else {
        target_cue.fade_millis
    }
}

fn move_in_black_values(
    compiled: &CompiledCueList,
    cue_list: &CueList,
    fixture_id: FixtureId,
    current_index: usize,
    target_index: usize,
    cue_fade_millis: u64,
) -> Vec<MoveInBlackTargetValue> {
    compiled
        .attributes_for_fixture(fixture_id)
        .filter(|attribute| attribute.attribute().is_position())
        .filter_map(|attribute| {
            let current = attribute
                .value(current_index, false)
                .cloned()
                .unwrap_or(AttributeValue::Normalized(0.0));
            let target = attribute
                .value(target_index, false)
                .cloned()
                .unwrap_or(AttributeValue::Normalized(0.0));
            (current != target).then(|| MoveInBlackTargetValue {
                attribute: attribute.attribute().clone(),
                current,
                target,
                fade_millis: position_fade_millis(
                    cue_list,
                    attribute.timing(target_index),
                    cue_fade_millis,
                ),
            })
        })
        .collect()
}

fn position_fade_millis(
    cue_list: &CueList,
    timing: Option<(Option<u64>, Option<u64>)>,
    cue_fade_millis: u64,
) -> u64 {
    if cue_list.disable_cue_timing {
        0
    } else if cue_list.force_cue_timing {
        cue_fade_millis
    } else {
        timing
            .and_then(|(fade_millis, _)| fade_millis)
            .unwrap_or(cue_fade_millis)
    }
}
