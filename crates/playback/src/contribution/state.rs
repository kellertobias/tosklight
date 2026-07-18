use super::ContributionContext;
use crate::*;

pub(super) struct PlaybackFrame<'a> {
    pub(super) playback: &'a ActivePlayback,
    pub(super) cue_list: &'a CueList,
    pub(super) cue: &'a Cue,
    pub(super) source: SequenceMasterSource,
    pub(super) sequence_master: f32,
    pub(super) snap_sequence_master: f32,
    pub(super) target: HashMap<AttributeAddress, AttributeValue>,
    pub(super) previous: HashMap<AttributeAddress, AttributeValue>,
    pub(super) effective_now: DateTime<Utc>,
    pub(super) elapsed: u64,
    pub(super) cue_fade_millis: u64,
}

impl<'a> PlaybackFrame<'a> {
    pub(super) fn new(
        context: &ContributionContext<'a>,
        playback: &'a ActivePlayback,
        source: SequenceMasterSource,
        sequence_master: f32,
        snap_sequence_master: f32,
    ) -> Self {
        let cue_list = &context.engine.cue_lists[&playback.cue_list_id];
        let target_index = playback.manual_xfade_to_index.unwrap_or(playback.cue_index);
        let target = target_state(cue_list, playback, target_index);
        let previous = previous_state(cue_list, playback);
        let cue = &cue_list.cues[target_index];
        let effective_now = playback.paused_at.unwrap_or(context.dynamics_now);
        let elapsed = (effective_now - playback.activated_at)
            .num_milliseconds()
            .max(0) as u64;
        let cue_fade_millis = cue_fade_millis(context.engine, cue_list, cue, playback);
        Self {
            playback,
            cue_list,
            cue,
            source,
            sequence_master,
            snap_sequence_master,
            target,
            previous,
            effective_now,
            elapsed,
            cue_fade_millis,
        }
    }

    pub(super) fn master_for(&self, snap: bool) -> f32 {
        if snap {
            self.snap_sequence_master
        } else {
            self.sequence_master
        }
    }
}

fn target_state(
    cue_list: &CueList,
    playback: &ActivePlayback,
    target_index: usize,
) -> HashMap<AttributeAddress, AttributeValue> {
    if !playback.tracking_wrap || playback.manual_xfade_to_index.is_some() {
        return cue_list.state_at_index(target_index);
    }
    let mut state = cue_list.state_at_index(cue_list.cues.len() - 1);
    for cue in cue_list.cues.iter().take(target_index + 1) {
        apply_changes(&mut state, &cue.changes);
    }
    state
}

fn previous_state(
    cue_list: &CueList,
    playback: &ActivePlayback,
) -> HashMap<AttributeAddress, AttributeValue> {
    if let Some(index) = playback.manual_xfade_from_index {
        return cue_list.state_at_index(index);
    }
    if let Some(source) = &playback.deleted_cue_transition_source {
        return normalized_deleted_source(source, playback);
    }
    playback
        .previous_index
        .map(|index| cue_list.state_at_index(index))
        .unwrap_or_default()
}

fn normalized_deleted_source(
    source: &[TimedValue],
    playback: &ActivePlayback,
) -> HashMap<AttributeAddress, AttributeValue> {
    let intensity_scale = if playback.flash { 1.0 } else { playback.master };
    source
        .iter()
        .map(|timed| {
            let value = normalized_deleted_value(timed, intensity_scale);
            ((timed.fixture_id, timed.attribute.clone()), value)
        })
        .collect()
}

fn normalized_deleted_value(timed: &TimedValue, intensity_scale: f32) -> AttributeValue {
    if !timed.attribute.is_intensity() {
        return timed.value.clone();
    }
    timed
        .value
        .normalized()
        .map(|level| {
            AttributeValue::Normalized(if intensity_scale > 0.0 {
                (level / intensity_scale).clamp(0.0, 1.0)
            } else {
                0.0
            })
        })
        .unwrap_or_else(|| timed.value.clone())
}

fn cue_fade_millis(
    engine: &PlaybackEngine,
    cue_list: &CueList,
    cue: &Cue,
    playback: &ActivePlayback,
) -> u64 {
    if cue_list.disable_cue_timing {
        0
    } else if cue_list.mode == CueListMode::Chaser {
        effective_chaser_xfade_millis(cue_list, &engine.speed_groups_bpm)
    } else if cue_list.mode == CueListMode::Sequence && cue.fade_millis == 0 {
        playback
            .transition_fade_fallback_millis
            .unwrap_or(engine.sequence_master_fade_millis)
    } else {
        cue.fade_millis
    }
}
