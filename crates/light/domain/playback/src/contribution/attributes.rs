use super::{ContributionContext, PlaybackFrame};
use crate::*;

impl ContributionContext<'_> {
    pub(super) fn extend_hold(
        &self,
        values: &mut Vec<PlaybackContribution>,
        hold: &DeletedCueHold,
        source: SequenceMasterSource,
        transition_ordinal: u64,
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
                transition_ordinal,
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
        let attributes = frame.relevant_attributes();
        values.reserve(attributes.len());
        for attribute in attributes {
            let previous = frame.previous_value(attribute);
            let target = frame.target_value(attribute);
            self.extend_one_attribute(values, frame, attribute, previous, target);
        }
        if let Some(previous) = frame.deleted_previous() {
            for ((fixture_id, attribute), value) in previous {
                if !frame.compiled.contains(*fixture_id, attribute) {
                    self.extend_deleted_attribute(values, frame, *fixture_id, attribute, value);
                }
            }
        }
    }

    fn extend_one_attribute(
        &self,
        values: &mut Vec<PlaybackContribution>,
        frame: &PlaybackFrame<'_>,
        attribute: &CompiledAttribute,
        previous: Option<&AttributeValue>,
        target: Option<&AttributeValue>,
    ) {
        if previous.is_none() && target.is_none() {
            return;
        }
        let fixture_id = attribute.fixture_id();
        let key = attribute.attribute();
        let snap = (self.is_snap)(fixture_id, key);
        let progress = progress(
            frame,
            key,
            previous,
            target,
            attribute.timing(frame.target_index),
            snap,
        );
        let Some(value) = interpolate(previous, target, progress) else {
            return;
        };
        values.push(attribute_contribution(
            frame,
            fixture_id,
            key.clone(),
            value,
            snap,
        ));
    }

    fn extend_deleted_attribute(
        &self,
        values: &mut Vec<PlaybackContribution>,
        frame: &PlaybackFrame<'_>,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
        previous: &AttributeValue,
    ) {
        let snap = (self.is_snap)(fixture_id, attribute);
        let progress = progress(frame, attribute, Some(previous), None, None, snap);
        let Some(value) = interpolate(Some(previous), None, progress) else {
            return;
        };
        values.push(attribute_contribution(
            frame,
            fixture_id,
            attribute.clone(),
            value,
            snap,
        ));
    }
}

fn progress(
    frame: &PlaybackFrame<'_>,
    attribute: &AttributeKey,
    previous: Option<&AttributeValue>,
    target: Option<&AttributeValue>,
    timing: Option<(Option<u64>, Option<u64>)>,
    snap: bool,
) -> f32 {
    let outgoing_intensity = is_outgoing_intensity(attribute, previous, target);
    let (fade_millis, delay_millis) = effective_timing(frame, timing, outgoing_intensity);
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
    timing: Option<(Option<u64>, Option<u64>)>,
    outgoing_intensity: bool,
) -> (u64, u64) {
    effective_attribute_timing(
        frame.cue_list,
        frame.cue,
        frame.cue_fade_millis,
        frame.outgoing_cue.zip(frame.outgoing_cue_fade_millis),
        timing,
        outgoing_intensity,
    )
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
        transition_ordinal: frame.playback.transition_ordinal,
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
