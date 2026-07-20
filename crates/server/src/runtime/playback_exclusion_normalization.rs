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
        .playback_service
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
                released_playbacks: released,
                provenance_migrated,
                persistence_pending,
            },
            events,
        ))
    }
}

fn migrate_activation_provenance(state: &AppState) -> Result<bool, ApiError> {
    let mut runtime = state.engine.playback_runtime();
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
        .engine
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

fn restored_exclusion_losers(state: &AppState) -> Result<Vec<u16>, ApiError> {
    let legacy_zones: Arc<[Vec<u16>]> = all_restored_zones(state).into();
    let mut desk_zones = HashMap::<Uuid, Arc<[Vec<u16>]>>::new();
    let mut active = state
        .engine
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
    losing_playbacks(&active, |playback| {
        activation_zones(state, playback, &legacy_zones, &mut desk_zones)
    })
}

fn activation_zones(
    state: &AppState,
    playback: &light_playback::ActivePlayback,
    legacy_zones: &Arc<[Vec<u16>]>,
    desk_zones: &mut HashMap<Uuid, Arc<[Vec<u16>]>>,
) -> Result<Arc<[Vec<u16>]>, ApiError> {
    let Some(activation) = &playback.activation else {
        return Ok(Arc::clone(legacy_zones));
    };
    match activation.exclusion_scope {
        light_playback::PlaybackExclusionScope::None => Ok(Arc::default()),
        light_playback::PlaybackExclusionScope::LegacyAllDesks => Ok(Arc::clone(legacy_zones)),
        light_playback::PlaybackExclusionScope::OriginatingDesk => {
            let Some(desk_id) = activation.desk_id else {
                return Ok(Arc::default());
            };
            if let Some(zones) = desk_zones.get(&desk_id) {
                return Ok(Arc::clone(zones));
            }
            let exists = state
                .desk
                .lock()
                .control_desk(desk_id)
                .map_err(ApiError::store)?
                .is_some();
            let zones: Arc<[Vec<u16>]> = if exists {
                virtual_playback_zone_numbers(state, desk_id).into()
            } else {
                Arc::default()
            };
            desk_zones.insert(desk_id, Arc::clone(&zones));
            Ok(zones)
        }
    }
}

fn all_restored_zones(state: &AppState) -> Vec<Vec<u16>> {
    let Some(show) = state.active_show.read().clone() else {
        return Vec::new();
    };
    let desks = read_virtual_playback_exclusion_store(&state.desk.lock(), show.id)
        .keys()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect::<Vec<_>>();
    desks
        .into_iter()
        .flat_map(|desk_id| virtual_playback_zone_numbers(state, desk_id))
        .collect()
}

fn losing_playbacks(
    active: &[light_playback::ActivePlayback],
    mut zones_for: impl FnMut(&light_playback::ActivePlayback) -> Result<Arc<[Vec<u16>]>, ApiError>,
) -> Result<Vec<u16>, ApiError> {
    let mut retained = HashSet::new();
    for playback in active {
        let number = playback.playback_number.expect("active pool Playback");
        let zones = zones_for(playback)?;
        for peer in zones.iter().filter(|zone| zone.contains(&number)).flatten() {
            retained.remove(peer);
        }
        retained.insert(number);
    }
    let mut losers = active
        .iter()
        .filter_map(|playback| playback.playback_number)
        .filter(|number| !retained.contains(number))
        .collect::<Vec<_>>();
    losers.sort_unstable();
    losers.dedup();
    Ok(losers)
}

fn release_candidates(state: &AppState, candidates: Vec<u16>) -> Result<Vec<u16>, ApiError> {
    match state
        .engine
        .execute_playback(EnginePlaybackCommand::ReleasePoolBatch(candidates))
        .map_err(ApiError::bad_request)?
    {
        EnginePlaybackOutcome::ChangedPlaybacks(released) => Ok(released),
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
    numbers: &[u16],
) -> Result<HashMap<u16, light_application::PlaybackRuntimeProjection>, ApiError> {
    if numbers.is_empty() {
        return Ok(HashMap::new());
    }
    let identities = numbers
        .iter()
        .copied()
        .map(PlaybackRuntimeIdentity::Playback)
        .collect::<Vec<_>>();
    let projections = playback_service::read_runtime_projections(state, context, &identities)?;
    Ok(numbers.iter().copied().zip(projections).collect())
}

fn changed_events(
    context: &ActionContext,
    before: HashMap<u16, light_application::PlaybackRuntimeProjection>,
    after: HashMap<u16, light_application::PlaybackRuntimeProjection>,
) -> Vec<EventDraft> {
    let mut numbers = after.keys().copied().collect::<Vec<_>>();
    numbers.sort_unstable();
    numbers
        .into_iter()
        .filter_map(|number| {
            committed_playback_event(
                context,
                PlaybackAction::Off { pressed: true },
                None,
                before.get(&number)?.clone(),
                after.get(&number)?.clone(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::CueListId;

    #[test]
    fn overlapping_zones_retain_the_last_activation_independent_of_zone_order() {
        let active = vec![active(1, 1), active(3, 2), active(2, 3), active(9, 4)];
        let zones: Arc<[Vec<u16>]> = vec![vec![1, 2], vec![2, 3]].into();

        assert_eq!(
            losing_playbacks(&active, |_| Ok(Arc::clone(&zones))).unwrap(),
            vec![1, 3]
        );

        let reversed: Arc<[Vec<u16>]> = zones.iter().cloned().rev().collect::<Vec<_>>().into();
        assert_eq!(
            losing_playbacks(&active, |_| Ok(Arc::clone(&reversed))).unwrap(),
            vec![1, 3]
        );
    }

    #[test]
    fn loser_numbers_are_stably_sorted() {
        let active = vec![active(10, 1), active(2, 2), active(11, 3), active(3, 4)];
        let zones: Arc<[Vec<u16>]> = vec![vec![10, 11], vec![2, 3]].into();

        assert_eq!(
            losing_playbacks(&active, |_| Ok(Arc::clone(&zones))).unwrap(),
            vec![2, 10]
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
}
