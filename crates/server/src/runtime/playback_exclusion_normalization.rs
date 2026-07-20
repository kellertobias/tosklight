use super::*;
use light_application::{
    ActionContext, ActionSource, EventDraft, PlaybackAction, PlaybackOperation,
    PlaybackRuntimeIdentity, PlaybackUnitOfWork, committed_playback_event,
};

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct RestoredExclusionOutcome {
    pub(super) released_playbacks: Vec<u16>,
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
        let candidates = restored_exclusion_losers(self.state);
        let before = projections(self.state, &self.context, &candidates)?;
        let released = release_candidates(self.state, candidates)?;
        if released.is_empty() {
            return Ok((RestoredExclusionOutcome::default(), Vec::new()));
        }
        let persistence_pending = persist_active_playbacks(self.state)
            .inspect_err(|error| tracing::warn!(error=%error.message, "restored Playback exclusion persistence is pending"))
            .is_err();
        let after = projections(self.state, &self.context, &released)?;
        let events = changed_events(&self.context, before, after);
        Ok((
            RestoredExclusionOutcome {
                released_playbacks: released,
                persistence_pending,
            },
            events,
        ))
    }
}

fn restored_exclusion_losers(state: &AppState) -> Vec<u16> {
    let zones = all_restored_zones(state);
    let mut active = state
        .engine
        .playback_runtime()
        .into_iter()
        .filter(|playback| playback.enabled)
        .filter(|playback| {
            playback
                .playback_number
                .is_some_and(|number| zones.iter().any(|zone| zone.contains(&number)))
        })
        .collect::<Vec<_>>();
    active.sort_by_key(|playback| (playback.activated_at, playback.playback_number));
    losing_playbacks(&zones, &active)
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

fn losing_playbacks(zones: &[Vec<u16>], active: &[light_playback::ActivePlayback]) -> Vec<u16> {
    let mut retained = HashSet::new();
    for number in active
        .iter()
        .filter_map(|playback| playback.playback_number)
    {
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
    losers
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

fn projections(
    state: &AppState,
    context: &ActionContext,
    numbers: &[u16],
) -> Result<HashMap<u16, light_application::PlaybackRuntimeProjection>, ApiError> {
    numbers
        .iter()
        .map(|number| {
            playback_service::read_runtime_projection(
                state,
                context,
                PlaybackRuntimeIdentity::Playback(*number),
            )
            .map(|projection| (*number, projection))
        })
        .collect()
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
        let zones = vec![vec![1, 2], vec![2, 3]];

        assert_eq!(losing_playbacks(&zones, &active), vec![1, 3]);

        let reversed = zones.into_iter().rev().collect::<Vec<_>>();
        assert_eq!(losing_playbacks(&reversed, &active), vec![1, 3]);
    }

    #[test]
    fn loser_numbers_are_stably_sorted() {
        let active = vec![active(10, 1), active(2, 2), active(11, 3), active(3, 4)];
        let zones = vec![vec![10, 11], vec![2, 3]];

        assert_eq!(losing_playbacks(&zones, &active), vec![2, 10]);
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
