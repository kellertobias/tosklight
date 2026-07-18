use super::{ContributionContext, PlaybackFrame};
use crate::*;

impl ContributionContext<'_> {
    pub(super) fn extend_hold(
        &self,
        values: &mut Vec<PlaybackContribution>,
        hold: &DeletedCueHold,
        source: SequenceMasterSource,
        sequence_master: f32,
        snap_sequence_master: f32,
    ) {
        values.extend(hold.contributions.iter().cloned().map(|value| {
            let sequence_master = if (self.is_snap)(value.fixture_id, &value.attribute) {
                snap_sequence_master
            } else {
                sequence_master
            };
            PlaybackContribution {
                value,
                sequence_master,
                source,
            }
        }));
    }

    pub(super) fn extend_attributes(
        &self,
        values: &mut Vec<PlaybackContribution>,
        frame: &PlaybackFrame<'_>,
    ) {
        let timing = cue_timing(frame.cue);
        let addresses: HashSet<_> = frame
            .previous
            .keys()
            .chain(frame.target.keys())
            .cloned()
            .collect();
        for (fixture_id, attribute) in addresses {
            let snap = (self.is_snap)(fixture_id, &attribute);
            let progress = progress(frame, timing.get(&(fixture_id, attribute.clone())), snap);
            let Some(value) = interpolate(
                frame.previous.get(&(fixture_id, attribute.clone())),
                frame.target.get(&(fixture_id, attribute.clone())),
                progress,
            ) else {
                continue;
            };
            values.push(attribute_contribution(
                frame, fixture_id, attribute, value, snap,
            ));
        }
    }
}

fn cue_timing(cue: &Cue) -> HashMap<AttributeAddress, (Option<u64>, Option<u64>)> {
    cue.changes
        .iter()
        .map(|change| (change.address(), (change.fade_millis, change.delay_millis)))
        .collect()
}

fn progress(
    frame: &PlaybackFrame<'_>,
    timing: Option<&(Option<u64>, Option<u64>)>,
    snap: bool,
) -> f32 {
    let (fade_millis, delay_millis) = effective_timing(frame, timing);
    if frame.playback.manual_xfade_from_index.is_some() {
        return if snap {
            1.0
        } else {
            frame.playback.manual_xfade_progress
        };
    }
    if frame.playback.transition_timing_bypassed {
        1.0
    } else if frame.elapsed < delay_millis {
        0.0
    } else if snap || fade_millis == 0 {
        1.0
    } else {
        ((frame.elapsed - delay_millis) as f32 / fade_millis as f32).clamp(0.0, 1.0)
    }
}

fn effective_timing(
    frame: &PlaybackFrame<'_>,
    timing: Option<&(Option<u64>, Option<u64>)>,
) -> (u64, u64) {
    let (fade_override, delay_override) = timing.copied().unwrap_or((None, None));
    if frame.cue_list.disable_cue_timing {
        (0, 0)
    } else if frame.cue_list.force_cue_timing {
        (frame.cue_fade_millis, frame.cue.delay_millis)
    } else {
        (
            fade_override.unwrap_or(frame.cue_fade_millis),
            delay_override.unwrap_or(frame.cue.delay_millis),
        )
    }
}

fn attribute_contribution(
    frame: &PlaybackFrame<'_>,
    fixture_id: FixtureId,
    attribute: AttributeKey,
    value: AttributeValue,
    snap: bool,
) -> PlaybackContribution {
    let sequence_master = frame.master_for(snap);
    let value = apply_intensity_master(value, &attribute, sequence_master);
    PlaybackContribution {
        value: timed_value(frame, fixture_id, attribute, value),
        sequence_master,
        source: frame.source,
    }
}

fn apply_intensity_master(
    value: AttributeValue,
    attribute: &AttributeKey,
    master: f32,
) -> AttributeValue {
    if !attribute.is_intensity() {
        return value;
    }
    value
        .normalized()
        .map(|level| AttributeValue::Normalized(level * master))
        .unwrap_or(value)
}

pub(super) fn timed_value(
    frame: &PlaybackFrame<'_>,
    fixture_id: FixtureId,
    attribute: AttributeKey,
    value: AttributeValue,
) -> TimedValue {
    TimedValue {
        fixture_id,
        merge_mode: intensity_merge_mode(frame.cue_list, &attribute),
        attribute,
        value,
        priority: frame.cue_list.priority,
        changed_at: frame.playback.activated_at,
        programmer_order: 0,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    }
}

fn intensity_merge_mode(cue_list: &CueList, attribute: &AttributeKey) -> MergeMode {
    if !attribute.is_intensity() {
        return MergeMode::Ltp;
    }
    match cue_list.intensity_priority_mode {
        IntensityPriorityMode::Htp => MergeMode::Htp,
        IntensityPriorityMode::Ltp => MergeMode::Ltp,
    }
}
