use super::ContributionContext;
use crate::*;

pub(super) enum PreviousState {
    Tracked(usize),
    Deleted(HashMap<AttributeAddress, PlaybackRetainedValue>),
    Empty,
}

pub(super) struct PlaybackFrame<'a> {
    pub(super) playback: &'a ActivePlayback,
    pub(super) cue_list: &'a CueList,
    pub(super) cue: &'a Cue,
    pub(super) outgoing_cue: Option<&'a Cue>,
    pub(super) outgoing_cue_fade_millis: Option<u64>,
    pub(super) compiled: &'a Arc<CompiledCueList>,
    pub(super) source: SequenceMasterSource,
    pub(super) sequence_master: f32,
    pub(super) snap_sequence_master: f32,
    pub(super) target_index: usize,
    pub(super) target_tracking_wrap: bool,
    pub(super) previous: PreviousState,
    pub(super) elapsed: u64,
    pub(super) cue_fade_millis: u64,
    pub(super) release_fade_millis: u64,
}

impl<'a> PlaybackFrame<'a> {
    // @tour cue-tracking-and-goto:40 Reconstruct the target stage
    // A frame combines the target Cue, previous state, tracking-wrap policy, timing, and compiled
    // history. Sequential GO, backward navigation, and direct GOTO read the same authority.
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
        let cue_fade_millis = effective_cue_fade_millis(
            cue_list,
            cue,
            playback,
            context.engine.sequence_master_fade_millis,
            &context.engine.speed_groups_bpm,
        );
        let outgoing_cue = playback
            .manual_xfade_from_index
            .or(playback.previous_index)
            .and_then(|index| cue_list.cues.get(index));
        let outgoing_cue_fade_millis = outgoing_cue.map(|outgoing| {
            let outgoing_fade = effective_cue_fade_millis(
                cue_list,
                outgoing,
                playback,
                context.engine.sequence_master_fade_millis,
                &context.engine.speed_groups_bpm,
            );
            if outgoing.fade_millis == 0 {
                cue_fade_millis
            } else {
                outgoing_fade
            }
        });
        let effective_now = playback.paused_at.unwrap_or(context.now);
        let elapsed = (effective_now - playback.activated_at)
            .num_milliseconds()
            .max(0) as u64;
        Self {
            playback,
            cue_list,
            cue,
            outgoing_cue,
            outgoing_cue_fade_millis,
            compiled,
            source,
            sequence_master,
            snap_sequence_master,
            target_index,
            target_tracking_wrap,
            previous,
            elapsed,
            cue_fade_millis,
            release_fade_millis: context.engine.release_fade_millis,
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
            PreviousState::Deleted(values) => values
                .get(&(attribute.fixture_id(), attribute.attribute().clone()))
                .map(|row| &row.timed.value),
            PreviousState::Empty => None,
        }
    }

    pub(super) fn deleted_previous(
        &self,
    ) -> Option<&HashMap<AttributeAddress, PlaybackRetainedValue>> {
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

    pub(super) fn evidence(
        &self,
        fixture: FixtureId,
        attribute: &AttributeKey,
        progress: f32,
    ) -> Option<Arc<PlaybackFamilyEvidence>> {
        use crate::source_evidence::{EvidencePhases, PlaybackEvidenceCache};
        let history = self.playback.source_history.as_ref()?;
        if !history.matches_manual_route(
            self.playback.manual_xfade_from_index,
            self.playback.manual_xfade_to_index,
        ) {
            return None;
        }
        let cache = history.cached(
            self.compiled,
            self.target_index,
            self.target_tracking_wrap,
            |history| {
                let mut cache = PlaybackEvidenceCache {
                    generation: Arc::downgrade(self.compiled.source_generation()),
                    target_index: self.target_index,
                    target_wrap: self.target_tracking_wrap,
                    targets: HashMap::new(),
                    phases: HashMap::new(),
                };
                for compiled in self.relevant_attributes() {
                    if [self.previous_value(compiled), self.target_value(compiled)]
                        .into_iter()
                        .flatten()
                        .all(|value| {
                            crate::source_evidence::family_owner(compiled.attribute(), value)
                                .is_none()
                        })
                    {
                        continue;
                    }
                    let address = (compiled.fixture_id(), compiled.attribute().clone());
                    let previous = self
                        .deleted_previous()
                        .and_then(|rows| rows.get(&address))
                        .and_then(|row| row.family_evidence.clone());
                    let target = history.target(
                        self.compiled,
                        compiled,
                        self.target_index,
                        self.target_tracking_wrap,
                    );
                    let phases = EvidencePhases::new(
                        compiled.attribute(),
                        self.previous_value(compiled),
                        self.target_value(compiled),
                        previous,
                        target.evidence.clone(),
                    );
                    cache.targets.insert(address.clone(), target);
                    cache.phases.insert(address, phases);
                }
                if let Some(previous) = self.deleted_previous() {
                    for (address, row) in previous {
                        if !self.compiled.contains(address.0, &address.1)
                            && crate::source_evidence::family_owner(&address.1, &row.timed.value)
                                .is_some()
                        {
                            cache.phases.insert(
                                address.clone(),
                                EvidencePhases::new(
                                    &address.1,
                                    Some(&row.timed.value),
                                    None,
                                    row.family_evidence.clone(),
                                    None,
                                ),
                            );
                        }
                    }
                }
                cache
            },
        )?;
        cache
            .phases
            .get(&(fixture, attribute.clone()))?
            .at(progress)
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
    source: &[PlaybackRetainedValue],
    playback: &ActivePlayback,
) -> HashMap<AttributeAddress, PlaybackRetainedValue> {
    let intensity_scale = if playback.flash { 1.0 } else { playback.master };
    source
        .iter()
        .map(|retained| {
            let mut row = retained.clone();
            row.timed.value = normalized_deleted_value(&retained.timed, intensity_scale);
            ((retained.fixture_id, retained.attribute.clone()), row)
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
