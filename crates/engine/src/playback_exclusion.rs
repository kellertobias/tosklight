use crate::{Engine, playback::EnginePlaybackOutcome};
use light_playback::PlaybackEngine;
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Debug)]
pub struct PoolPlaybackTransition {
    pub outcome: EnginePlaybackOutcome,
    pub released_playbacks: Vec<u16>,
}

impl Engine {
    pub fn enabled_auto_off_playbacks(&self) -> Vec<u16> {
        let generation = self.generation.load();
        let auto_off = generation
            .snapshot()
            .playbacks
            .iter()
            .filter(|definition| definition.auto_off)
            .map(|definition| definition.number)
            .collect::<HashSet<_>>();
        let mut numbers = generation
            .playback()
            .read()
            .runtime()
            .into_iter()
            .filter(|runtime| runtime.enabled)
            .filter_map(|runtime| runtime.playback_number)
            .filter(|number| auto_off.contains(number))
            .collect::<Vec<_>>();
        numbers.sort_unstable();
        numbers.dedup();
        numbers
    }
}

pub(crate) fn apply_with_exclusions<T>(
    playback: &mut PlaybackEngine,
    activated_number: u16,
    zones: &[Vec<u16>],
    apply: impl FnOnce(&mut PlaybackEngine) -> Result<T, String>,
) -> Result<(T, Vec<u16>), String> {
    let was_enabled = is_enabled(playback, activated_number);
    let outcome = apply(playback)?;
    let released = if !was_enabled && is_enabled(playback, activated_number) {
        release_active_peers(playback, zones, activated_number)
    } else {
        Vec::new()
    };
    Ok((outcome, released))
}

fn is_enabled(playback: &PlaybackEngine, number: u16) -> bool {
    playback
        .playback_runtime(number)
        .is_some_and(|runtime| runtime.enabled)
}

fn release_active_peers(
    playback: &mut PlaybackEngine,
    zones: &[Vec<u16>],
    activated_number: u16,
) -> Vec<u16> {
    exclusion_peers(zones, activated_number)
        .into_iter()
        .filter(|number| {
            is_enabled(playback, *number) && playback.off(*number).is_ok_and(|changed| changed)
        })
        .collect()
}

fn exclusion_peers(zones: &[Vec<u16>], activated_number: u16) -> BTreeSet<u16> {
    zones
        .iter()
        .filter(|zone| zone.contains(&activated_number))
        .flat_map(|zone| zone.iter().copied())
        .filter(|number| *number != activated_number)
        .collect()
}
