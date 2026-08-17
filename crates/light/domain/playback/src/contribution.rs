use crate::*;

mod attributes;
mod state;

use state::PlaybackFrame;

struct ContributionContext<'a> {
    engine: &'a PlaybackEngine,
    now: DateTime<Utc>,
    is_snap: &'a dyn Fn(FixtureId, &AttributeKey) -> bool,
}

impl PlaybackEngine {
    pub fn contributions(&self) -> Vec<TimedValue> {
        self.contributions_at(self.clock.now())
    }

    pub fn contributions_at(&self, now: DateTime<Utc>) -> Vec<TimedValue> {
        self.contributions_at_with_snap(now, |_, attribute| {
            attribute_uses_snap_transition(attribute)
        })
    }

    pub(crate) fn transition_source_at(
        &self,
        key: PlaybackKey,
        now: DateTime<Utc>,
    ) -> Option<Vec<TimedValue>> {
        let playback = self.active.get(&key)?;
        if !playback.enabled {
            return None;
        }
        let values = self
            .contributions_with_context_at(now, |_, attribute| {
                attribute_uses_snap_transition(attribute)
            })
            .into_iter()
            .filter(|contribution| {
                contribution.source.cue_list_id == playback.cue_list_id
                    && contribution.source.playback_identity == playback.playback_identity
                    && !contribution.source.temporary
            })
            .map(|contribution| contribution.value)
            .collect::<Vec<_>>();
        Some(values)
    }

    pub fn contributions_at_with_snap(
        &self,
        now: DateTime<Utc>,
        is_snap: impl Fn(FixtureId, &AttributeKey) -> bool,
    ) -> Vec<TimedValue> {
        self.contributions_with_context_at(now, is_snap)
            .into_iter()
            .map(|contribution| contribution.value)
            .collect()
    }

    // @tour playback-runtime:30 Build owned Cue contributions
    // Active and temporary Playbacks reconstruct tracked values here while retaining the exact
    // sequence-master owner needed after normal HTP/LTP arbitration.

    /// Resolve active Cue values while retaining the exact playback master which owns each
    /// contribution. The engine uses this metadata only after normal HTP/LTP arbitration.
    pub fn contributions_with_context_at(
        &self,
        now: DateTime<Utc>,
        is_snap: impl Fn(FixtureId, &AttributeKey) -> bool,
    ) -> Vec<PlaybackContribution> {
        ContributionContext {
            engine: self,
            now,
            is_snap: &is_snap,
        }
        .build()
    }
}

impl ContributionContext<'_> {
    fn build(&self) -> Vec<PlaybackContribution> {
        let mut values = Vec::new();
        for playback in self
            .engine
            .active
            .values()
            .chain(self.engine.temporary.values())
        {
            if playback.enabled && !self.suppressed(playback) {
                self.extend_playback(&mut values, playback);
            }
        }
        values
    }

    fn suppressed(&self, playback: &ActivePlayback) -> bool {
        let Some(number) = playback.playback_number else {
            return false;
        };
        let identity = playback.playback_identity.unwrap_or_else(|| {
            PlaybackIdentity::physical(number).expect("active physical playback number is valid")
        });
        self.engine.swap_held.iter().any(|source| {
            *source != identity
                && !self
                    .engine
                    .definition_at(identity)
                    .is_some_and(|definition| definition.protect_from_swap)
        })
    }

    fn extend_playback(&self, values: &mut Vec<PlaybackContribution>, playback: &ActivePlayback) {
        let source = source(playback);
        let (sequence_master, snap_sequence_master) = sequence_masters(playback);
        if let Some(hold) = &playback.deleted_cue_hold {
            self.extend_hold(
                values,
                hold,
                source,
                playback.transition_ordinal,
                sequence_master,
                snap_sequence_master,
            );
            return;
        }
        let frame = PlaybackFrame::new(
            self,
            playback,
            source,
            sequence_master,
            snap_sequence_master,
        );
        self.extend_attributes(values, &frame);
    }
}

fn source(playback: &ActivePlayback) -> SequenceMasterSource {
    SequenceMasterSource {
        playback_number: playback.playback_number,
        playback_identity: playback.playback_identity,
        cue_list_id: playback.cue_list_id,
        temporary: playback.temporary,
    }
}

fn sequence_masters(playback: &ActivePlayback) -> (f32, f32) {
    if playback.flash {
        return (1.0, 1.0);
    }
    let current = playback.master.clamp(0.0, 1.0);
    let snapped = playback
        .master_transition
        .as_ref()
        .map(|transition| transition.to)
        .unwrap_or(playback.master)
        .clamp(0.0, 1.0);
    (current, snapped)
}
