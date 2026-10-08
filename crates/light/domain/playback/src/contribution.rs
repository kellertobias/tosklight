use crate::*;

mod attributes;
mod state;

use state::PlaybackFrame;

struct ContributionContext<'a> {
    engine: &'a PlaybackEngine,
    now: DateTime<Utc>,
    /// An override for whether an attribute snaps. `None` means the compiled cue already decided,
    /// which is the ordinary case: the answer depends only on the attribute's name, and that was
    /// settled when the cue list compiled rather than per contribution per frame.
    is_snap: Option<SnapOverride<'a>>,
    /// Where a family that fades in with nothing before it starts (see [`FamilyStartSource`]).
    family_start: Option<&'a dyn FamilyStartSource>,
}

/// A caller's own answer to whether an attribute snaps rather than fades.
pub type SnapOverride<'a> = &'a dyn Fn(FixtureId, &AttributeKey) -> bool;

/// TL-552: where a Position that fades in with no previous Cue value starts. Angles cannot fade
/// from nothing (0° is a real pose, never an invented owner), so without a start the fade renders
/// no Position until it completes. The engine installs each generation's declared default poses:
/// a frame-local start with no provenance. Live frames, previews and the interrupted source a
/// GO captures mid-fade all read the same start, so an interrupted fade continues from where it
/// was rather than jumping back.
pub trait FamilyStartSource: Send + Sync {
    fn family_start(&self, fixture: FixtureId, attribute: &AttributeKey) -> Option<AttributeValue>;

    /// Sample a representation requiring this generation's pinned physical source model.
    /// Unsupported pairs retain their compatibility hold; no destination model is guessed.
    fn sample_native_transition(
        &self,
        _from: &AttributeValue,
        _to: &AttributeValue,
        _progress: f32,
    ) -> Option<AttributeValue> {
        None
    }
}

/// The installed [`FamilyStartSource`]; empty keeps the hold-until-complete behaviour.
#[derive(Clone, Default)]
pub(crate) struct FamilyStartSlot(pub(crate) Option<Arc<dyn FamilyStartSource>>);

impl std::fmt::Debug for FamilyStartSlot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("FamilyStartSlot")
            .field(&self.0.is_some())
            .finish()
    }
}

impl PlaybackEngine {
    pub fn contributions(&self) -> Vec<TimedValue> {
        self.contributions_at(self.clock.now())
    }

    pub fn contributions_at(&self, now: DateTime<Utc>) -> Vec<TimedValue> {
        self.contributions_with_context(now, None)
            .into_iter()
            .map(|contribution| contribution.value)
            .collect()
    }

    pub(crate) fn transition_source_at(
        &self,
        key: PlaybackKey,
        now: DateTime<Utc>,
    ) -> Option<Vec<PlaybackRetainedValue>> {
        let playback = self.active.get(&key)?;
        if !playback.enabled {
            return None;
        }
        // Only this playback's values are wanted, so only this playback is built. Building every
        // playback and keeping one used to run the whole contribution pass once more per active
        // playback per tick.
        let context = ContributionContext {
            engine: self,
            now,
            is_snap: None,
            family_start: self.family_start.0.as_deref(),
        };
        if context.suppressed(playback) {
            return Some(Vec::new());
        }
        let mut values = Vec::new();
        context.extend_playback(&mut values, playback);
        Some(
            values
                .into_iter()
                .map(PlaybackRetainedValue::from)
                .collect(),
        )
    }

    pub fn contributions_at_with_snap(
        &self,
        now: DateTime<Utc>,
        is_snap: impl Fn(FixtureId, &AttributeKey) -> bool,
    ) -> Vec<TimedValue> {
        self.contributions_with_context(now, Some(&is_snap))
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
        self.contributions_with_context(now, Some(&is_snap))
    }

    /// Build this frame's Cue contributions, letting the compiled cue answer whether an attribute
    /// snaps unless a caller overrides it.
    pub fn contributions_with_context(
        &self,
        now: DateTime<Utc>,
        is_snap: Option<SnapOverride<'_>>,
    ) -> Vec<PlaybackContribution> {
        let mut values = Vec::new();
        self.extend_contributions(now, is_snap, &mut values);
        values
    }

    /// Fill a caller's buffer with this frame's Cue contributions.
    ///
    /// A desk builds these forty times a second from data whose shape changes only when the show
    /// does, so the buffer is worth keeping between frames rather than growing a new one each tick.
    /// Whatever the buffer already held is discarded.
    pub fn extend_contributions(
        &self,
        now: DateTime<Utc>,
        is_snap: Option<SnapOverride<'_>>,
        values: &mut Vec<PlaybackContribution>,
    ) {
        values.clear();
        ContributionContext {
            engine: self,
            now,
            is_snap,
            family_start: self.family_start.0.as_deref(),
        }
        .build_into(values);
    }

    /// Ordinary and Dynamic Cue projections must agree about temporarily suppressed sources.
    pub(crate) fn playback_source_suppressed(&self, playback: &ActivePlayback) -> bool {
        let Some(number) = playback.playback_number else {
            return false;
        };
        let identity = playback.playback_identity.unwrap_or_else(|| {
            PlaybackIdentity::physical(number).expect("active physical playback number is valid")
        });
        self.swap_held.iter().any(|source| {
            *source != identity
                && !self
                    .definition_at(identity)
                    .is_some_and(|definition| definition.protect_from_swap)
        })
    }
}

impl ContributionContext<'_> {
    fn build_into(&self, values: &mut Vec<PlaybackContribution>) {
        for playback in self
            .engine
            .active
            .values()
            .chain(self.engine.temporary.values())
        {
            if playback.enabled && !self.suppressed(playback) {
                self.extend_playback(values, playback);
            }
        }
    }

    fn suppressed(&self, playback: &ActivePlayback) -> bool {
        self.engine.playback_source_suppressed(playback)
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
    playback.sequence_master_source()
}

fn sequence_masters(playback: &ActivePlayback) -> (f32, f32) {
    playback.sequence_masters()
}
