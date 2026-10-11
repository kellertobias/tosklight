//! Release only the held controls owned by a disconnected surface, in Playback order.
use super::{ApiError, AppState};
use light_application::{
    ActionContext, ActionSource, EventDraft, EventSource, PlaybackOperation, PlaybackRuntimeChange,
    PlaybackRuntimeIdentity, PlaybackUnitOfWork,
};

pub(in crate::runtime) fn cancel_for_desk(
    state: &AppState,
    desk_id: uuid::Uuid,
    surface_id: &str,
) -> Result<(), ApiError> {
    let source = light_playback::PlaybackControlSource {
        surface_id: surface_id.into(),
        control_id: 0,
    }
    .scoped_to_desk(desk_id)
    .map_err(ApiError::bad_request)?;
    // Same lock order as OSC ingress: receipt capture precedes the Playback unit of work.
    let mut holds = state.integrations.osc_playback_holds();
    cancel(state, &source.surface_id)?;
    holds.remove_surface(desk_id, surface_id);
    Ok(())
}

fn cancel(state: &AppState, surface_id: &str) -> Result<(), ApiError> {
    state
        .playback
        .run_unit_of_work(Disconnect { state, surface_id })
        .output
}
struct Disconnect<'a> {
    state: &'a AppState,
    surface_id: &'a str,
}
impl PlaybackUnitOfWork for Disconnect<'_> {
    type Output = Result<(), ApiError>;
    fn execute(self) -> PlaybackOperation<Self::Output> {
        match cancel_scoped_in_order(self.state, self.surface_id) {
            Ok(events) => PlaybackOperation::with_events(Ok(()), events),
            Err(error) => PlaybackOperation::new(Err(error)),
        }
    }
}

pub(in crate::runtime) fn cancel_scoped_in_order(
    state: &AppState,
    surface_id: &str,
) -> Result<Vec<EventDraft>, ApiError> {
    let snapshot = state.output.snapshot();
    let identities = snapshot
        .playback_pages
        .iter()
        .flat_map(|page| {
            page.assignments
                .keys()
                .filter_map(move |position| {
                    light_playback::PlaybackAddress::new(page.number, *position).ok()
                })
                .map(PlaybackRuntimeIdentity::Playback)
        })
        .collect::<Vec<_>>();
    let context = ActionContext::system(uuid::Uuid::nil(), ActionSource::System);
    let before = super::playback_service::read_runtime_projections(state, &context, &identities);
    let result = state
        .output
        .cancel_playback_control_surface(surface_id)
        .map_err(ApiError::bad_request);
    if let Err(error) = result {
        return Err(error);
    }
    let (before, after) = match (
        before,
        super::playback_service::read_runtime_projections(state, &context, &identities),
    ) {
        (Ok(before), Ok(after)) => (before, after),
        (Err(error), _) | (_, Err(error)) => return Err(error),
    };
    let events = before
        .into_iter()
        .zip(after)
        .filter_map(|(before, projection)| {
            (before != projection).then(|| {
                EventDraft::playback_runtime_changed(
                    None,
                    PlaybackRuntimeChange {
                        projection,
                        transition: None,
                    },
                    EventSource::Runtime,
                    None,
                )
            })
        })
        .collect();
    Ok(events)
}
