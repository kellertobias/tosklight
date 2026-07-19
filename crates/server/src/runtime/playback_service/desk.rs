//! Desk-local Playback view projection events for mutations outside Playback commands.

use light_application::{
    ActionContext, EventDraft, PlaybackDeskProjection, PlaybackOperation, PlaybackPorts,
    PlaybackUnitOfWork,
};
use light_show::ShowEntry;

use super::{ApiError, AppState, ServerPlaybackPorts, action_error};

pub(in crate::runtime) fn projection(
    state: &AppState,
    context: &ActionContext,
) -> Result<PlaybackDeskProjection, ApiError> {
    let ports = ServerPlaybackPorts {
        state,
        session: None,
        desk: None,
        persistence_pending: std::sync::atomic::AtomicBool::new(false),
    };
    PlaybackPorts::desk_projection(&ports, context)
        .map_err(action_error)?
        .ok_or_else(|| ApiError::internal("playback desk projection unavailable"))
}

fn change_event(
    state: &AppState,
    context: &ActionContext,
    before: PlaybackDeskProjection,
) -> Result<Option<EventDraft>, ApiError> {
    let after = projection(state, context)?;
    if after == before {
        return Ok(None);
    }
    Ok(Some(EventDraft::playback_view_changed(context, after)))
}

pub(in crate::runtime) struct ChangePage<'a> {
    pub state: &'a AppState,
    pub show: &'a ShowEntry,
    pub context: ActionContext,
    pub desk_id: uuid::Uuid,
    pub page: u8,
}

impl PlaybackUnitOfWork for ChangePage<'_> {
    type Output = Result<super::super::PlaybackPageAvailability, ApiError>;

    fn execute(self) -> PlaybackOperation<Self::Output> {
        let before = match projection(self.state, &self.context) {
            Ok(before) => before,
            Err(error) => return PlaybackOperation::new(Err(error)),
        };
        let availability = match super::super::ensure_playback_page_for_advance(
            self.state,
            self.show,
            self.page,
            &self.context,
        ) {
            Ok(availability) => availability,
            Err(error) => return PlaybackOperation::new(Err(error)),
        };
        if !availability.available() {
            return PlaybackOperation::new(Ok(availability));
        }
        if let Err(error) = self
            .state
            .desk
            .lock()
            .set_desk_page(self.desk_id, self.show.id, self.page)
            .map_err(ApiError::store)
        {
            return PlaybackOperation::new(Err(error));
        }
        match change_event(self.state, &self.context, before) {
            Ok(event) => {
                PlaybackOperation::with_events(Ok(availability), event.into_iter().collect())
            }
            Err(error) => PlaybackOperation::new(Err(error)),
        }
    }
}
