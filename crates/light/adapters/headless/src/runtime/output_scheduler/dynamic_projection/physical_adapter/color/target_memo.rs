//! The last semantic resolve of one Color target (TL-639 round 3), one level above the
//! per-head memo (`resolve::ColorHeadMemo`).
//!
//! A head replays its memo when the intent and its fit inputs of the seeded `current` are
//! unchanged and `current` passes the fitter's whole-vector check. Seeding writes only controls
//! the head owns, from the previous continuity. So when the intent, the previous continuity and
//! the native raw values of every head's inputs and controls are unchanged, every head replays
//! its memo, provided `current` still passes the whole-vector check: the raw vector differs
//! from the seeded one only in owned controls, which held the same seeded values when the memo
//! was made. Under those conditions the result is the kept one, with every head's work zeroed
//! as a head replay publishes it, and the counters move as the per-head replays would move them.
//! Native channels outside the key (for example Intensity) may change freely.

use super::resolve::HeadsResolution;
use super::*;

pub(super) struct ColorTargetMemo {
    intent: ColorIntent,
    /// Kept from the second resolve of an unchanged intent on: an animated target changes its
    /// intent every frame and would only pay for copies it never replays.
    kept: Option<KeptResolve>,
}

struct KeptResolve {
    previous: Option<ColorContinuity>,
    /// Whether every head's fitter accepted the complete native vector the result was kept
    /// from. A capture resolves every non-static channel through `scale_channel_raw`, which
    /// never exceeds the channel's maximum, and every static channel to its fixed default, so
    /// the answer is the same for every capture of this destination while this descriptor
    /// (one runtime generation) lives (TL-639 round 4).
    accepted: bool,
    /// Native raw value of every key channel, in `ColorDescriptor::key_channels` order.
    key: Vec<u32>,
    resolved: HeadsResolution,
}

impl ColorDescriptor {
    /// Every raw channel a head of this target reads or writes: fit inputs and controls.
    pub(super) fn key_channels(&self) -> &[usize] {
        self.key_channels.get_or_init(|| {
            let mut channels = self
                .heads
                .iter()
                .flat_map(|head| {
                    head.inputs.iter().copied().chain(
                        head.controls
                            .iter()
                            .map(|control| control.channel_index as usize),
                    )
                })
                .collect::<Vec<_>>();
            channels.sort_unstable();
            channels.dedup();
            channels.into_boxed_slice()
        })
    }
}

impl ColorTargetMemo {
    /// The memo after a full resolve of `intent`; `last` is the memo before it.
    pub(super) fn after_resolve(
        last: Option<Self>,
        descriptor: &ColorDescriptor,
        intent: &ColorIntent,
        previous: Option<&ColorContinuity>,
        raw: &[u32],
        resolved: &HeadsResolution,
    ) -> Self {
        let repeated = last.as_ref().is_some_and(|last| last.intent == *intent);
        let kept = repeated
            .then(|| {
                let key = key(descriptor, raw)?;
                let mut resolved = resolved.clone();
                for outcome in &mut resolved.outcomes {
                    outcome.quality.work = ColorSolveWork::default();
                }
                Some(KeptResolve {
                    previous: previous.cloned(),
                    accepted: descriptor
                        .heads
                        .iter()
                        .all(|head| head.fitting.accepts_raw(raw)),
                    key,
                    resolved,
                })
            })
            .flatten();
        match last {
            Some(mut last) if repeated => {
                last.kept = kept;
                last
            }
            _ => Self {
                intent: intent.clone(),
                kept,
            },
        }
    }

    /// Whether [`Self::replay_key`] can answer for this intent and continuity.
    pub(super) fn may_replay_key(
        &self,
        intent: &ColorIntent,
        previous: Option<&ColorContinuity>,
    ) -> bool {
        self.kept.as_ref().is_some_and(|kept| {
            kept.accepted && self.intent == *intent && kept.previous.as_ref() == previous
        })
    }

    /// [`Self::replay`] from the key channels' raw values alone (`key`, in
    /// `ColorDescriptor::key_channels` order), after [`Self::may_replay_key`]: the whole-vector
    /// check is the kept `accepted` answer (see `KeptResolve::accepted`).
    pub(super) fn replay_key(&self, key: &[u32]) -> Option<HeadsResolution> {
        let kept = self.kept.as_ref()?;
        (kept.accepted && kept.key == key).then(|| kept.resolved.clone())
    }

    /// The kept result when every head would replay its memo (see the module comment).
    pub(super) fn replay(
        &self,
        descriptor: &ColorDescriptor,
        intent: &ColorIntent,
        previous: Option<&ColorContinuity>,
        raw: &[u32],
    ) -> Option<HeadsResolution> {
        let kept = self.kept.as_ref()?;
        let channels = descriptor.key_channels();
        let unchanged = self.intent == *intent
            && kept.previous.as_ref() == previous
            && channels.len() == kept.key.len()
            && channels
                .iter()
                .zip(&kept.key)
                .all(|(&channel, &value)| raw.get(channel) == Some(&value))
            && descriptor
                .heads
                .iter()
                .all(|head| head.fitting.accepts_raw(raw));
        unchanged.then(|| kept.resolved.clone())
    }
}

fn key(descriptor: &ColorDescriptor, raw: &[u32]) -> Option<Vec<u32>> {
    descriptor
        .key_channels()
        .iter()
        .map(|&channel| raw.get(channel).copied())
        .collect()
}
