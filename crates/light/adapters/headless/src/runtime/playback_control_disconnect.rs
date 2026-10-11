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
    cancel(state, &source.surface_id)
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
        let snapshot = self.state.output.snapshot();
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
        let before =
            super::playback_service::read_runtime_projections(self.state, &context, &identities);
        let result = self
            .state
            .output
            .cancel_playback_control_surface(self.surface_id)
            .map_err(ApiError::bad_request);
        if let Err(error) = result {
            return PlaybackOperation::new(Err(error));
        }
        let (before, after) = match (
            before,
            super::playback_service::read_runtime_projections(self.state, &context, &identities),
        ) {
            (Ok(before), Ok(after)) => (before, after),
            (Err(error), _) | (_, Err(error)) => return PlaybackOperation::new(Err(error)),
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
        PlaybackOperation::with_events(Ok(()), events)
    }
}
