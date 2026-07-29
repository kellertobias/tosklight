use super::*;
use light_application::{
    ActionContext, ActionSource, EventDraft, PlaybackAction, PlaybackOperation,
    PlaybackRuntimeIdentity, PlaybackUnitOfWork, committed_playback_event,
};
use std::sync::Arc;

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct RestoredExclusionOutcome {
    pub(super) released_playbacks: Vec<u16>,
    pub(super) provenance_migrated: bool,
    pub(super) persistence_pending: bool,
}

pub(super) fn normalize_restored_virtual_playback_exclusions(
    state: &AppState,
) -> Result<RestoredExclusionOutcome, ApiError> {
    let context = ActionContext::system(Uuid::nil(), ActionSource::System);
    state
        .playback
        .run_unit_of_work(RestoredExclusionNormalization { state, context })
        .output
}

struct RestoredExclusionNormalization<'a> {
    state: &'a AppState,
    context: ActionContext,
}

impl PlaybackUnitOfWork for RestoredExclusionNormalization<'_> {
    type Output = Result<RestoredExclusionOutcome, ApiError>;

    fn execute(self) -> PlaybackOperation<Self::Output> {
        match self.apply() {
            Ok((outcome, events)) => PlaybackOperation::with_events(Ok(outcome), events),
            Err(error) => PlaybackOperation::new(Err(error)),
        }
    }
}

impl RestoredExclusionNormalization<'_> {
    fn apply(self) -> Result<(RestoredExclusionOutcome, Vec<EventDraft>), ApiError> {
        let provenance_migrated = migrate_activation_provenance(self.state)?;
        let candidates = restored_exclusion_losers(self.state)?;
        let before = projections(self.state, &self.context, &candidates)?;
        let released = release_candidates(self.state, candidates)?;
        let persistence_pending =
            persist_normalized_runtime(self.state, provenance_migrated || !released.is_empty());
        let after = projections(self.state, &self.context, &released)?;
        let events = changed_events(&self.context, before, after);
        Ok((
            RestoredExclusionOutcome {
                released_playbacks: released.iter().map(|identity| identity.number()).collect(),
                provenance_migrated,
                persistence_pending,
            },
            events,
        ))
    }
}

fn migrate_activation_provenance(state: &AppState) -> Result<bool, ApiError> {
    let mut runtime = state.output.playback_runtime();
    if !activation_migration_required(&runtime) {
        return Ok(false);
    }
    let mut ordered = runtime
        .iter()
        .enumerate()
        .filter(|(_, playback)| playback.enabled && playback.playback_number.is_some())
        .map(|(index, playback)| (index, activation_time(playback), playback.playback_number))
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(_, at, number)| (*at, *number));
    for (offset, (index, _, _)) in ordered.into_iter().enumerate() {
        migrate_activation(&mut runtime[index], offset as u64 + 1);
    }
    for playback in runtime.iter_mut().filter(|playback| !playback.enabled) {
        playback.activation = None;
    }
    state
        .output
        .execute_playback(EnginePlaybackCommand::RestoreActive(runtime))
        .map_err(ApiError::internal)?;
    Ok(true)
}

fn activation_migration_required(runtime: &[light_playback::ActivePlayback]) -> bool {
    let mut ordinals = HashSet::new();
    for playback in runtime {
        if !playback.enabled && playback.activation.is_some() {
            return true;
        }
        if !playback.enabled || playback.playback_number.is_none() {
            continue;
        }
        let Some(activation) = &playback.activation else {
            return true;
        };
        if activation.ordinal == 0 || !ordinals.insert(activation.ordinal) {
            return true;
        }
    }
    false
}

fn migrate_activation(playback: &mut light_playback::ActivePlayback, ordinal: u64) {
    let activation = playback.activation.take();
    playback.activation = Some(light_playback::PlaybackActivationProvenance {
        ordinal,
        at: activation
            .as_ref()
            .map_or(playback.activated_at, |activation| activation.at),
        desk_id: activation
            .as_ref()
            .and_then(|activation| activation.desk_id),
        surface: activation.as_ref().map_or(
            light_playback::PlaybackActivationSurface::Unknown,
            |activation| activation.surface,
        ),
        exclusion_scope: activation.as_ref().map_or(
            light_playback::PlaybackExclusionScope::LegacyAllDesks,
            |activation| activation.exclusion_scope,
        ),
    });
}

fn activation_time(playback: &light_playback::ActivePlayback) -> chrono::DateTime<chrono::Utc> {
    playback
        .activation
        .as_ref()
        .map_or(playback.activated_at, |activation| activation.at)
}

fn restored_exclusion_losers(
    state: &AppState,
) -> Result<Vec<light_playback::PlaybackIdentity>, ApiError> {
    let mut active = state
        .output
        .playback_runtime()
        .into_iter()
        .filter(|playback| playback.enabled && playback.playback_number.is_some())
        .collect::<Vec<_>>();
    active.sort_by_key(|playback| {
        (
            playback
                .activation
                .as_ref()
                .map_or(u64::MAX, |activation| activation.ordinal),
            playback.playback_number,
        )
    });
    losing_playbacks(&active, |playback| activation_zones(state, playback))
}

fn activation_zones(
    state: &AppState,
    playback: &light_playback::ActivePlayback,
) -> Result<Arc<[Vec<light_playback::PlaybackIdentity>]>, ApiError> {
    let identity = runtime_identity(playback).ok_or_else(|| {
        ApiError::internal("restored assigned Playback has no stable runtime identity")
    })?;
    match identity {
        light_playback::PlaybackIdentity::Virtual(_) => all_restored_zones(state, identity),
        light_playback::PlaybackIdentity::Physical(_) => Ok(Arc::default()),
    }
}

fn all_restored_zones(
    state: &AppState,
    identity: light_playback::PlaybackIdentity,
) -> Result<Arc<[Vec<light_playback::PlaybackIdentity>]>, ApiError> {
    Ok(qualified_zone_identities(state, identity)?.into())
}

fn losing_playbacks(
    active: &[light_playback::ActivePlayback],
    mut zones_for: impl FnMut(
        &light_playback::ActivePlayback,
    ) -> Result<Arc<[Vec<light_playback::PlaybackIdentity>]>, ApiError>,
) -> Result<Vec<light_playback::PlaybackIdentity>, ApiError> {
    let mut retained = HashSet::new();
    for playback in active {
        let Some(identity) = runtime_identity(playback) else {
            continue;
        };
        let zones = zones_for(playback)?;
        for peer in zones
            .iter()
            .filter(|zone| zone.contains(&identity))
            .flatten()
        {
            retained.remove(peer);
        }
        retained.insert(identity);
    }
    let mut losers = active
        .iter()
        .filter_map(runtime_identity)
        .filter(|identity| !retained.contains(identity))
        .collect::<Vec<_>>();
    losers.sort_unstable();
    losers.dedup();
    Ok(losers)
}

fn release_candidates(
    state: &AppState,
    candidates: Vec<light_playback::PlaybackIdentity>,
) -> Result<Vec<light_playback::PlaybackIdentity>, ApiError> {
    let requested = candidates.clone();
    match state
        .output
        .execute_playback(EnginePlaybackCommand::ReleaseIdentityBatch(candidates))
        .map_err(ApiError::bad_request)?
    {
        EnginePlaybackOutcome::ChangedPlaybacks(released) => Ok(requested
            .into_iter()
            .filter(|identity| released.contains(&identity.number()))
            .collect()),
        _ => Err(ApiError::internal(
            "unexpected restored Playback exclusion outcome",
        )),
    }
}

fn persist_normalized_runtime(state: &AppState, changed: bool) -> bool {
    changed
        && persist_active_playbacks(state)
            .inspect_err(|error| tracing::warn!(error=%error.message, "restored Playback exclusion persistence is pending"))
            .is_err()
}

fn projections(
    state: &AppState,
    context: &ActionContext,
    identities: &[light_playback::PlaybackIdentity],
) -> Result<HashMap<PlaybackRuntimeIdentity, light_application::PlaybackRuntimeProjection>, ApiError>
{
    if identities.is_empty() {
        return Ok(HashMap::new());
    }
    let requested = identities
        .iter()
        .copied()
        .map(application_identity)
        .collect::<Vec<_>>();
    let projections = playback_service::read_runtime_projections(state, context, &requested)?;
    Ok(requested.into_iter().zip(projections).collect())
}

fn changed_events(
    context: &ActionContext,
    before: HashMap<PlaybackRuntimeIdentity, light_application::PlaybackRuntimeProjection>,
    after: HashMap<PlaybackRuntimeIdentity, light_application::PlaybackRuntimeProjection>,
) -> Vec<EventDraft> {
    after
        .keys()
        .into_iter()
        .filter_map(|identity| {
            committed_playback_event(
                context,
                PlaybackAction::Off { pressed: true },
                None,
                before.get(identity)?.clone(),
                after.get(identity)?.clone(),
            )
        })
        .collect()
}

fn runtime_identity(
    playback: &light_playback::ActivePlayback,
) -> Option<light_playback::PlaybackIdentity> {
    playback.playback_identity.or_else(|| {
        playback
            .playback_number
            .and_then(|number| light_playback::PlaybackIdentity::physical(number).ok())
    })
}

fn qualified_zone_identities(
    state: &AppState,
    identity: light_playback::PlaybackIdentity,
) -> Result<Vec<Vec<light_playback::PlaybackIdentity>>, ApiError> {
    Ok(VirtualPlaybackExclusionResolver::read(state)?
        .zone_numbers()
        .into_iter()
        .filter_map(|zone| {
            zone.into_iter()
                .map(|number| match identity {
                    light_playback::PlaybackIdentity::Virtual(_) => {
                        light_playback::VirtualPlaybackAddress::from_number(number)
                            .map(light_playback::PlaybackIdentity::Virtual)
                    }
                    light_playback::PlaybackIdentity::Physical(_) => {
                        light_playback::PlaybackIdentity::physical(number)
                    }
                })
                .collect::<Result<Vec<_>, _>>()
                .ok()
        })
        .collect())
}

const fn application_identity(
    identity: light_playback::PlaybackIdentity,
) -> PlaybackRuntimeIdentity {
    match identity {
        light_playback::PlaybackIdentity::Physical(number) => {
            PlaybackRuntimeIdentity::Playback(number.get())
        }
        light_playback::PlaybackIdentity::Virtual(address) => {
            PlaybackRuntimeIdentity::Virtual(address)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::CueListId;

    #[test]
    fn overlapping_zones_retain_the_last_activation_independent_of_zone_order() {
        let active = vec![active(1, 1), active(3, 2), active(2, 3), active(9, 4)];
        let zones: Arc<[Vec<light_playback::PlaybackIdentity>]> =
            vec![zone(&[1, 2]), zone(&[2, 3])].into();

        assert_eq!(
            losing_playbacks(&active, |_| Ok(Arc::clone(&zones))).unwrap(),
            zone(&[1, 3])
        );

        let reversed: Arc<[Vec<light_playback::PlaybackIdentity>]> =
            zones.iter().cloned().rev().collect::<Vec<_>>().into();
        assert_eq!(
            losing_playbacks(&active, |_| Ok(Arc::clone(&reversed))).unwrap(),
            zone(&[1, 3])
        );
    }

    #[test]
    fn loser_numbers_are_stably_sorted() {
        let active = vec![active(10, 1), active(2, 2), active(11, 3), active(3, 4)];
        let zones: Arc<[Vec<light_playback::PlaybackIdentity>]> =
            vec![zone(&[10, 11]), zone(&[2, 3])].into();

        assert_eq!(
            losing_playbacks(&active, |_| Ok(Arc::clone(&zones))).unwrap(),
            zone(&[2, 10])
        );
    }

    fn active(number: u16, second: u8) -> light_playback::ActivePlayback {
        serde_json::from_value(serde_json::json!({
            "playback_number": number,
            "cue_list_id": CueListId::new(),
            "cue_index": 0,
            "previous_index": null,
            "paused": false,
            "activated_at": format!("2026-01-01T00:00:{second:02}Z"),
            "paused_at": null
        }))
        .expect("minimal restored Playback runtime must decode")
    }

    fn zone(numbers: &[u16]) -> Vec<light_playback::PlaybackIdentity> {
        numbers
            .iter()
            .copied()
            .map(|number| light_playback::PlaybackIdentity::physical(number).unwrap())
            .collect()
    }
}
