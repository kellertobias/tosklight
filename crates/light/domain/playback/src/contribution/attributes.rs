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
        values.extend(hold.contributions.iter().cloned().map(|retained| {
            let value = retained.timed;
            let snaps = match self.is_snap {
                Some(is_snap) => is_snap(value.fixture_id, &value.attribute),
                None => crate::attribute_uses_snap_transition(&value.attribute),
            };
            let sequence_master = if snaps {
                snap_sequence_master
            } else {
                sequence_master
            };
            PlaybackContribution {
                value,
                family_evidence: retained.family_evidence,
                authored_target: false,
                transition_ordinal,
                sequence_master,
                source,
                address: None,
                pending_transition: retained.pending_transition,
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
                    self.extend_deleted_attribute(
                        values,
                        frame,
                        *fixture_id,
                        attribute,
                        &value.timed.value,
                    );
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
        // The compiled cue already knows; a caller only overrides it deliberately.
        let snap = match self.is_snap {
            Some(is_snap) => is_snap(fixture_id, key),
            None => attribute.uses_snap_transition(),
        };
        let progress = progress_for(
            frame,
            attribute.is_intensity(),
            previous,
            target,
            attribute.timing(frame.target_index),
            snap,
        );
        // A semantic family fading in over nothing starts from the fixture's declared default
        // (Position TL-552; Color, Zoom and Focus TL-544 G2). Only the frame value uses it;
        // evidence still names the authored endpoints alone.
        let declared = match (previous, target, self.family_start) {
            (None, Some(to), Some(start)) if progress < 1.0 && fades_from_declared(key, to) => {
                start
                    .family_start(fixture_id, key)
                    .filter(|from| declared_start_matches(from, to))
            }
            _ => None,
        };
        // TL-544 G1: an interrupted crossing starts from its live pose, not its held source.
        let native_sample = previous.zip(target).and_then(|(from, to)| {
            self.family_start?
                .sample_native_transition(from, to, progress)
        });
        let Some((value, pending)) = native_sample.map(|value| (value, None)).or_else(|| {
            interpolate_pending(
                previous.or(declared.as_ref()),
                frame
                    .previous_pending(attribute)
                    .filter(|_| declared.is_none()),
                target,
                progress,
            )
        }) else {
            return;
        };
        let family_evidence = crate::source_evidence::family_owner(key, &value)
            .and_then(|_| frame.evidence(fixture_id, key, progress));
        values.push(attribute_contribution(
            frame,
            fixture_id,
            key.clone(),
            value,
            snap,
            attribute.frame_address(),
            progress >= 1.0 && target.is_some(),
            family_evidence,
        ));
        if let Some(contribution) = values.last_mut() {
            contribution.pending_transition = pending;
        }
    }

    fn extend_deleted_attribute(
        &self,
        values: &mut Vec<PlaybackContribution>,
        frame: &PlaybackFrame<'_>,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
        previous: &AttributeValue,
    ) {
        let snap = match self.is_snap {
            Some(is_snap) => is_snap(fixture_id, attribute),
            None => crate::attribute_uses_snap_transition(attribute),
        };
        let progress = progress(frame, attribute, Some(previous), None, None, snap);
        let Some(value) = interpolate(Some(previous), None, progress) else {
            return;
        };
        let family_evidence = crate::source_evidence::family_owner(attribute, &value)
            .and_then(|_| frame.evidence(fixture_id, attribute, progress));
        values.push(attribute_contribution(
            frame,
            fixture_id,
            attribute.clone(),
            value,
            snap,
            None,
            false,
            family_evidence,
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
    progress_for(
        frame,
        attribute.is_intensity(),
        previous,
        target,
        timing,
        snap,
    )
}

/// The same question with the intensity test already answered, for callers holding a compiled
/// attribute that settled it when the cue list compiled.
fn progress_for(
    frame: &PlaybackFrame<'_>,
    is_intensity: bool,
    previous: Option<&AttributeValue>,
    target: Option<&AttributeValue>,
    timing: Option<(Option<u64>, Option<u64>)>,
    snap: bool,
) -> f32 {
    let outgoing_intensity = is_intensity && {
        let previous = previous.and_then(AttributeValue::normalized).unwrap_or(0.0);
        let target = target.and_then(AttributeValue::normalized).unwrap_or(0.0);
        target < previous
    };
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
        frame.release_fade_millis,
    )
}

#[allow(clippy::too_many_arguments)]
fn attribute_contribution(
    frame: &PlaybackFrame<'_>,
    fixture_id: FixtureId,
    attribute: AttributeKey,
    value: AttributeValue,
    snap: bool,
    address: Option<light_core::FrameAddress>,
    authored_target: bool,
    family_evidence: Option<Arc<PlaybackFamilyEvidence>>,
) -> PlaybackContribution {
    let sequence_master = frame.master_for(snap);
    let value = apply_level_master(value, &attribute, sequence_master);
    PlaybackContribution {
        value: timed_value(frame, fixture_id, attribute, value),
        family_evidence,
        authored_target,
        transition_ordinal: frame.playback.transition_ordinal,
        sequence_master,
        source: frame.source,
        address,
        pending_transition: None,
    }
}

/// The Cue master scales the Cue's level parameters (Intensity, Volume) before arbitration.
fn apply_level_master(
    value: AttributeValue,
    attribute: &AttributeKey,
    master: f32,
) -> AttributeValue {
    if !attribute.is_level() {
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

/// The families whose fade-in from nothing starts from a declared default: Position, semantic
/// Color, Zoom and Focus. A Direct Color keeps its hold (its native recipe has no default start).
fn fades_from_declared(key: &AttributeKey, to: &AttributeValue) -> bool {
    match to {
        AttributeValue::Position(_) | AttributeValue::Zoom(_) => true,
        AttributeValue::ColorProgram(program) => matches!(
            program.as_ref(),
            light_core::programming::ColorProgram::Semantic { .. }
        ),
        AttributeValue::Normalized(_) => key.0.as_ref() == "focus",
        _ => false,
    }
}

/// A start is only used when the fade can interpolate from it: same family representation, and
/// for Zoom the same opening convention (a convention is never converted).
fn declared_start_matches(from: &AttributeValue, to: &AttributeValue) -> bool {
    match (from, to) {
        (AttributeValue::Position(_), AttributeValue::Position(_))
        | (AttributeValue::ColorProgram(_), AttributeValue::ColorProgram(_))
        | (AttributeValue::Normalized(_), AttributeValue::Normalized(_)) => true,
        (AttributeValue::Zoom(from), AttributeValue::Zoom(to)) => from.convention == to.convention,
        _ => false,
    }
}
