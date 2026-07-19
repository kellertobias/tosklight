use super::ContributionContext;
use crate::*;

pub(super) enum PreviousState {
    Tracked(usize),
    Deleted(HashMap<AttributeAddress, AttributeValue>),
    Empty,
}

pub(super) struct PlaybackFrame<'a> {
    pub(super) playback: &'a ActivePlayback,
    pub(super) cue_list: &'a CueList,
    pub(super) cue: &'a Cue,
    pub(super) compiled: &'a CompiledCueList,
    pub(super) source: SequenceMasterSource,
    pub(super) sequence_master: f32,
    pub(super) snap_sequence_master: f32,
    pub(super) target_index: usize,
    pub(super) target_tracking_wrap: bool,
    pub(super) previous: PreviousState,
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
        let compiled = &context.engine.compiled_cue_lists[&playback.cue_list_id];
        let target_index = playback.manual_xfade_to_index.unwrap_or(playback.cue_index);
        let target_tracking_wrap =
            playback.tracking_wrap && playback.manual_xfade_to_index.is_none();
        let previous = previous_state(playback);
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
            compiled,
            source,
            sequence_master,
            snap_sequence_master,
            target_index,
            target_tracking_wrap,
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

    pub(super) fn target_value<'b>(
        &self,
        attribute: &'b CompiledAttribute,
    ) -> Option<&'b AttributeValue> {
        attribute.value(self.target_index, self.target_tracking_wrap)
    }

    pub(super) fn previous_value<'b>(
        &'b self,
        attribute: &'b CompiledAttribute,
    ) -> Option<&'b AttributeValue> {
        match &self.previous {
            PreviousState::Tracked(index) => attribute.value(*index, false),
            PreviousState::Deleted(values) => {
                values.get(&(attribute.fixture_id(), attribute.attribute().clone()))
            }
            PreviousState::Empty => None,
        }
    }

    pub(super) fn target_value_for(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&AttributeValue> {
        self.compiled.value(
            fixture_id,
            attribute,
            self.target_index,
            self.target_tracking_wrap,
        )
    }

    pub(super) fn deleted_previous(&self) -> Option<&HashMap<AttributeAddress, AttributeValue>> {
        match &self.previous {
            PreviousState::Deleted(values) => Some(values),
            _ => None,
        }
    }

    pub(super) fn relevant_attributes(&self) -> &[CompiledAttribute] {
        if self.target_tracking_wrap || matches!(&self.previous, PreviousState::Deleted(_)) {
            return self.compiled.attributes();
        }
        let latest_index = match &self.previous {
            PreviousState::Tracked(index) => self.target_index.max(*index),
            PreviousState::Empty | PreviousState::Deleted(_) => self.target_index,
        };
        self.compiled.attributes_through(latest_index)
    }
}

fn previous_state(playback: &ActivePlayback) -> PreviousState {
    if let Some(index) = playback.manual_xfade_from_index {
        return PreviousState::Tracked(index);
    }
    if let Some(source) = &playback.deleted_cue_transition_source {
        return PreviousState::Deleted(normalized_deleted_source(source, playback));
    }
    playback
        .previous_index
        .map(PreviousState::Tracked)
        .unwrap_or(PreviousState::Empty)
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
