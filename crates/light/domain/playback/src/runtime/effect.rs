use crate::*;
use std::collections::BTreeSet;

impl PlaybackEngine {
    /// Classifies the exact retained runtime difference from an isolated baseline.
    pub fn retained_runtime_effect_since(&self, before: &Self) -> PlaybackRuntimeEffect {
        if self.active != before.active
            || self.active_dynamics != before.active_dynamics
            || self.jump_counts != before.jump_counts
        {
            PlaybackRuntimeEffect::Durable
        } else if self.control_states != before.control_states
            || self.temporary != before.temporary
            || self.swap_held != before.swap_held
        {
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
        if active_playback(self, number) != active_playback(before, number)
            || dynamic_playback(self, number) != dynamic_playback(before, number)
            || cue_jump_counts_differ(self, before, number)
        {
            PlaybackRuntimeEffect::Durable
        } else if control_state(self, number) != control_state(before, number)
            || temporary_playbacks(self, number) != temporary_playbacks(before, number)
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

fn control_state(engine: &PlaybackEngine, number: u16) -> PlaybackControlState {
    PlaybackIdentity::physical(number)
        .ok()
        .map(|identity| engine.control_state_at(identity))
        .unwrap_or_default()
}

fn cue_jump_counts_differ(current: &PlaybackEngine, before: &PlaybackEngine, number: u16) -> bool {
    let cue_list_id = current
        .definitions
        .get(&number)
        .or_else(|| before.definitions.get(&number))
        .and_then(|definition| match &definition.target {
            PlaybackTarget::CueList { cue_list_id } => Some(*cue_list_id),
            _ => None,
        });
    let Some(cue_list_id) = cue_list_id else {
        return false;
    };
    current
        .jump_counts
        .iter()
        .filter(|((id, _), _)| *id == cue_list_id)
        .any(|(key, count)| before.jump_counts.get(key) != Some(count))
        || before
            .jump_counts
            .iter()
            .filter(|((id, _), _)| *id == cue_list_id)
            .any(|(key, count)| current.jump_counts.get(key) != Some(count))
}

fn has_cue_jump_counts(engine: &PlaybackEngine, cue_list_id: CueListId) -> bool {
    engine.jump_counts.keys().any(|(id, _)| *id == cue_list_id)
}

fn runtime_numbers(current: &PlaybackEngine, before: &PlaybackEngine) -> BTreeSet<u16> {
    current
        .definitions
        .values()
        .chain(before.definitions.values())
        .filter_map(|definition| match &definition.target {
            PlaybackTarget::CueList { cue_list_id }
                if current
                    .active
                    .contains_key(&PlaybackKey::CueList(*cue_list_id))
                    || before
                        .active
                        .contains_key(&PlaybackKey::CueList(*cue_list_id))
                    || has_cue_jump_counts(current, *cue_list_id)
                    || has_cue_jump_counts(before, *cue_list_id) =>
            {
                Some(definition.number)
            }
            PlaybackTarget::Dynamic { assignment }
                if current
                    .active_dynamics
                    .contains_key(&assignment.target_id())
                    || before.active_dynamics.contains_key(&assignment.target_id()) =>
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

fn dynamic_playback(engine: &PlaybackEngine, number: u16) -> Option<&ActiveDynamicPlayback> {
    let target = engine.dynamic_assignment(number)?.target_id();
    engine.active_dynamics.get(&target)
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
