//! Show-owned Solo arbitration under the same runtime guard as the incoming action.
use crate::playback::{EnginePlaybackOutcome, PlaybackAction, execute_addressed};
use light_playback::{
    PlaybackActivationOrigin, PlaybackAddress, PlaybackControlSource, PlaybackEngine,
    PlaybackRuntimeEffect, PlaybackSoloRegion,
};
use std::collections::BTreeSet;

pub(crate) fn apply_with_solo(
    playback: &mut PlaybackEngine,
    address: PlaybackAddress,
    action: &PlaybackAction,
    regions: &[PlaybackSoloRegion],
    origin: Option<PlaybackActivationOrigin>,
    source: &PlaybackControlSource,
) -> Result<EnginePlaybackOutcome, String> {
    let was_enabled = permanently_enabled(playback, address);
    let outcome = execute_addressed(playback, address, action.clone(), source)?;
    // Flash holds never participate in Solo; turning a hold on cannot evict a permanent peer.
    let activated = !was_enabled
        && permanently_enabled(playback, address)
        && !matches!(action, PlaybackAction::SetFlash(_));
    if !activated {
        return Ok(outcome);
    }
    if let Some(origin) = origin {
        playback.record_activation(address, origin);
    }
    let related = enforce_target_solo(playback, address, regions)?;
    let EnginePlaybackOutcome::Changed(effect) = outcome else {
        return Err("unexpected Playback action outcome".into());
    };
    Ok(EnginePlaybackOutcome::Changed(effect.with_related(related)))
}

fn permanently_enabled(playback: &PlaybackEngine, address: PlaybackAddress) -> bool {
    playback.permanent_enabled(address)
}

/// Direct Cuelist control has no preferred assignment: every assigned alias participates.
pub(crate) fn enforce_direct_cue_solo(
    playback: &mut PlaybackEngine,
    id: light_core::CueListId,
    regions: &[PlaybackSoloRegion],
) -> Result<PlaybackRuntimeEffect, String> {
    let aliases: BTreeSet<_> = regions.iter().flat_map(|region| region.members.iter().copied())
        .filter(|address| playback.definition(*address).is_some_and(|definition|
            matches!(definition.target, light_playback::PlaybackTarget::CueList { cue_list_id } if cue_list_id == id)))
        .collect();
    let Some(address) = aliases.first().copied() else {
        return Ok(PlaybackRuntimeEffect::None);
    };
    let selected: Vec<_> = regions
        .iter()
        .filter(|region| region.members.iter().any(|member| aliases.contains(member)))
        .cloned()
        .collect();
    release_region_peers(playback, address, &selected)
}

pub(crate) fn enforce_target_solo(
    playback: &mut PlaybackEngine,
    address: PlaybackAddress,
    regions: &[PlaybackSoloRegion],
) -> Result<PlaybackRuntimeEffect, String> {
    if playback.definition(address).is_none() {
        return Err("Playback does not exist".into());
    }
    let selected: Vec<_> = regions
        .iter()
        .filter(|region| {
            region
                .members
                .iter()
                .any(|member| playback.same_target(address, *member))
        })
        .cloned()
        .collect();
    release_region_peers(playback, address, &selected)
}

pub(crate) fn target_solo_members(
    playback: &PlaybackEngine,
    address: PlaybackAddress,
    regions: &[PlaybackSoloRegion],
) -> BTreeSet<PlaybackAddress> {
    regions
        .iter()
        .filter(|region| {
            region
                .members
                .iter()
                .any(|member| playback.same_target(address, *member))
        })
        .flat_map(|region| region.members.iter().copied())
        .collect()
}

fn release_region_peers(
    playback: &mut PlaybackEngine,
    address: PlaybackAddress,
    regions: &[PlaybackSoloRegion],
) -> Result<PlaybackRuntimeEffect, String> {
    let mut peers = Vec::new();
    let mut seen = Vec::new();
    let mut related = PlaybackRuntimeEffect::None;
    for candidate in regions
        .iter()
        .flat_map(|region| region.members.iter().copied())
        .collect::<BTreeSet<_>>()
    {
        if candidate == address
            || playback.definition(candidate).is_none()
            || playback.same_target(address, candidate)
            || seen
                .iter()
                .any(|peer| playback.same_target(*peer, candidate))
        {
            continue;
        }
        seen.push(candidate);
        // A held, formerly-Off target must also stay Off when its surviving hold releases.
        related = related.combine(playback.inhibit_held_release_activation(candidate)?.effect);
        if permanently_enabled(playback, candidate) {
            peers.push(candidate);
        }
    }
    if playback.definition(address).is_some_and(|definition| {
        matches!(
            definition.target,
            light_playback::PlaybackTarget::CueList { .. }
        )
    }) {
        playback.adopt_solo_handover(address, &peers)?;
    }
    for peer in peers {
        related = related.combine(playback.release_permanent_mutation(peer)?.effect);
    }
    Ok(related)
}

/// Resolve saved permanent winners in activation order; temporary holds do not take part.
pub(crate) fn enforce_restored_solo(
    playback: &mut PlaybackEngine,
    regions: &[PlaybackSoloRegion],
) -> Result<PlaybackRuntimeEffect, String> {
    let mut candidates: Vec<_> = regions
        .iter()
        .flat_map(|region| region.members.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|address| {
            if !permanently_enabled(playback, address) {
                return None;
            }
            playback
                .playback_runtime(address)
                .map(|runtime| (runtime.activated_at, address))
                .or_else(|| {
                    playback
                        .active_dynamic_playback(address)
                        .map(|runtime| (runtime.activated_at, address))
                })
        })
        .collect();
    candidates.sort_unstable();
    let mut effect = PlaybackRuntimeEffect::None;
    // Process newest winners first; an older winner already released cannot release a newer peer.
    for (_, address) in candidates.into_iter().rev() {
        if permanently_enabled(playback, address) {
            effect = effect.combine(enforce_target_solo(playback, address, regions)?);
        }
    }
    Ok(effect)
}
