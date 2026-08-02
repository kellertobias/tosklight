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
            || self
                .swap_held
                .contains(&PlaybackIdentity::physical(number).expect("valid physical number"))
                != before
                    .swap_held
                    .contains(&PlaybackIdentity::physical(number).expect("valid physical number"))
        {
            PlaybackRuntimeEffect::Transient
        } else {
            PlaybackRuntimeEffect::None
        }
    }
}

fn runtime_numbers(current: &PlaybackEngine, before: &PlaybackEngine) -> BTreeSet<u16> {
    current
        .definitions
        .values()
        .chain(before.definitions.values())
        .filter_map(|definition| match definition.target {
            PlaybackTarget::CueList { cue_list_id }
                if current
                    .active
                    .contains_key(&PlaybackKey::CueList(cue_list_id))
                    || before
                        .active
                        .contains_key(&PlaybackKey::CueList(cue_list_id)) =>
            {
                Some(definition.number)
            }
            _ => None,
        })
        .chain(
            current
                .temporary
                .keys()
                .filter_map(|(identity, _)| match identity {
                    PlaybackIdentity::Physical(number) => Some(number.get()),
                    PlaybackIdentity::Virtual(_) => None,
                }),
        )
        .chain(
            before
                .temporary
                .keys()
                .filter_map(|(identity, _)| match identity {
                    PlaybackIdentity::Physical(number) => Some(number.get()),
                    PlaybackIdentity::Virtual(_) => None,
                }),
        )
        .chain(
            current
                .swap_held
                .iter()
                .filter_map(|identity| match identity {
                    PlaybackIdentity::Physical(number) => Some(number.get()),
                    PlaybackIdentity::Virtual(_) => None,
                }),
        )
        .chain(
            before
                .swap_held
                .iter()
                .filter_map(|identity| match identity {
                    PlaybackIdentity::Physical(number) => Some(number.get()),
                    PlaybackIdentity::Virtual(_) => None,
                }),
        )
        .collect()
}

fn active_playback(engine: &PlaybackEngine, number: u16) -> Option<&ActivePlayback> {
    let key = engine.runtime_key(number).ok()?;
    engine.active.get(&key)
}

fn temporary_playbacks(
    engine: &PlaybackEngine,
    number: u16,
) -> Vec<(TemporaryPlaybackKind, &ActivePlayback)> {
    let mut playbacks = engine
        .temporary
        .iter()
        .filter(|((candidate, _), _)| {
            *candidate == PlaybackIdentity::physical(number).expect("valid physical number")
        })
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
