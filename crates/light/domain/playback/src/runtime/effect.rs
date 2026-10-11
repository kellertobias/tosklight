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
            || self.cuelist_flash_states != before.cuelist_flash_states
            || self.cuelist_swap_states != before.cuelist_swap_states
            || self.dynamic_flash_states != before.dynamic_flash_states
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
    ) -> Vec<(PlaybackAddress, PlaybackRuntimeEffect)> {
        runtime_numbers(self, before)
            .into_iter()
            .filter_map(|number| {
                let effect = self.numbered_runtime_effect_since(before, number);
                effect.changed().then_some((number, effect))
            })
            .collect()
    }

    fn numbered_runtime_effect_since(
        &self,
        before: &Self,
        number: PlaybackAddress,
    ) -> PlaybackRuntimeEffect {
        if active_playback(self, number) != active_playback(before, number)
            || dynamic_playback(self, number) != dynamic_playback(before, number)
            || cue_jump_counts_differ(self, before, number)
        {
            PlaybackRuntimeEffect::Durable
        } else if control_state(self, number) != control_state(before, number)
            || temporary_playbacks(self, number) != temporary_playbacks(before, number)
            || self.swap_held.iter().any(|held| held.address == number)
                != before.swap_held.iter().any(|held| held.address == number)
            || held_target_states(self, &self.cuelist_flash_states, number)
                != held_target_states(before, &before.cuelist_flash_states, number)
            || held_target_states(self, &self.cuelist_swap_states, number)
                != held_target_states(before, &before.cuelist_swap_states, number)
            || held_target_states(self, &self.dynamic_flash_states, number)
                != held_target_states(before, &before.dynamic_flash_states, number)
        {
            PlaybackRuntimeEffect::Transient
        } else {
            PlaybackRuntimeEffect::None
        }
    }
}

fn control_state(engine: &PlaybackEngine, number: PlaybackAddress) -> PlaybackControlState {
    engine.control_state_at(number)
}

fn cue_jump_counts_differ(
    current: &PlaybackEngine,
    before: &PlaybackEngine,
    number: PlaybackAddress,
) -> bool {
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

fn runtime_numbers(current: &PlaybackEngine, before: &PlaybackEngine) -> BTreeSet<PlaybackAddress> {
    current
        .definitions
        .keys()
        .chain(before.definitions.keys())
        .copied()
        .chain(current.temporary.keys().map(|(held, _)| held.address))
        .chain(before.temporary.keys().map(|(held, _)| held.address))
        .chain(current.swap_held.iter().map(|held| held.address))
        .chain(before.swap_held.iter().map(|held| held.address))
        .collect()
}

fn active_playback(engine: &PlaybackEngine, number: PlaybackAddress) -> Option<&ActivePlayback> {
    let key = engine.runtime_key(number).ok()?;
    engine.active.get(&key)
}

fn dynamic_playback(
    engine: &PlaybackEngine,
    number: PlaybackAddress,
) -> Option<&ActiveDynamicPlayback> {
    let target = engine.dynamic_assignment(number)?.target_id();
    engine.active_dynamics.get(&target)
}

fn temporary_playbacks(
    engine: &PlaybackEngine,
    number: PlaybackAddress,
) -> Vec<(TemporaryPlaybackKind, &ActivePlayback)> {
    let mut playbacks = engine
        .temporary
        .iter()
        .filter(|((candidate, _), _)| candidate.address == number)
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

// Release behavior belongs to the target shared by every assigned alias, while each
// held source remains independently identifiable within that target.
fn held_target_states<'a, T>(
    engine: &PlaybackEngine,
    states: &'a HashMap<PlaybackControlKey, T>,
    address: PlaybackAddress,
) -> HashMap<&'a PlaybackControlKey, &'a T> {
    states
        .iter()
        .filter(|(held, _)| engine.same_target(held.address, address))
        .collect()
}
