use crate::*;
use std::collections::BTreeSet;

impl PlaybackEngine {
    /// Classifies the exact retained runtime difference from an isolated baseline.
    pub fn retained_runtime_effect_since(&self, before: &Self) -> PlaybackRuntimeEffect {
        if self.active != before.active {
            PlaybackRuntimeEffect::Durable
        } else if self.temporary != before.temporary || self.swap_held != before.swap_held {
            PlaybackRuntimeEffect::Transient
        } else {
            PlaybackRuntimeEffect::None
        }
    }

    /// Returns exact effects for numbered Playbacks whose final retained state differs.
    pub fn numbered_runtime_effects_since(
        &self,
        before: &Self,
    ) -> Vec<(u16, PlaybackRuntimeEffect)> {
        runtime_numbers(self, before)
            .into_iter()
            .filter_map(|number| {
                let effect = self.numbered_runtime_effect_since(before, number);
                effect.changed().then_some((number, effect))
            })
            .collect()
    }

    fn numbered_runtime_effect_since(&self, before: &Self, number: u16) -> PlaybackRuntimeEffect {
        if active_playback(self, number) != active_playback(before, number) {
            PlaybackRuntimeEffect::Durable
        } else if temporary_playbacks(self, number) != temporary_playbacks(before, number)
            || self.swap_held.contains(&number) != before.swap_held.contains(&number)
        {
            PlaybackRuntimeEffect::Transient
        } else {
            PlaybackRuntimeEffect::None
        }
    }
}

fn runtime_numbers(current: &PlaybackEngine, before: &PlaybackEngine) -> BTreeSet<u16> {
    current
        .active
        .keys()
        .chain(before.active.keys())
        .filter_map(|key| match key {
            PlaybackKey::Number(number) => Some(*number),
            PlaybackKey::CueList(_) => None,
        })
        .chain(current.temporary.keys().map(|(number, _)| *number))
        .chain(before.temporary.keys().map(|(number, _)| *number))
        .chain(current.swap_held.iter().copied())
        .chain(before.swap_held.iter().copied())
        .collect()
}

fn active_playback(engine: &PlaybackEngine, number: u16) -> Option<&ActivePlayback> {
    engine.active.get(&PlaybackKey::Number(number))
}

fn temporary_playbacks(
    engine: &PlaybackEngine,
    number: u16,
) -> Vec<(TemporaryPlaybackKind, &ActivePlayback)> {
    let mut playbacks = engine
        .temporary
        .iter()
        .filter(|((candidate, _), _)| *candidate == number)
        .map(|((_, kind), playback)| (*kind, playback))
        .collect::<Vec<_>>();
    playbacks.sort_by_key(|(kind, _)| temporary_kind_order(*kind));
    playbacks
}

const fn temporary_kind_order(kind: TemporaryPlaybackKind) -> u8 {
    match kind {
        TemporaryPlaybackKind::Flash => 0,
        TemporaryPlaybackKind::TempButton => 1,
        TemporaryPlaybackKind::TempFader => 2,
        TemporaryPlaybackKind::Swap => 3,
    }
}
