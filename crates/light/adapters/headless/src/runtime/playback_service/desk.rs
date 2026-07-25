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
    let ports = ServerPlaybackPorts::new(state, None, None);
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

impl<'a> ChangePage<'a> {
    pub(in crate::runtime) fn existing(
        state: &'a AppState,
        show_id: light_core::ShowId,
        context: ActionContext,
        desk_id: uuid::Uuid,
        page: u8,
    ) -> impl PlaybackUnitOfWork<Output = Result<super::super::PlaybackPageAvailability, ApiError>> + 'a
    {
        ChangeExistingPage {
            state,
            show_id,
            context,
            desk_id,
            page,
        }
    }
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
        change_page(
            self.state,
            self.show.id,
            self.context,
            self.desk_id,
            self.page,
            before,
            availability,
        )
    }
}

struct ChangeExistingPage<'a> {
    pub state: &'a AppState,
    pub show_id: light_core::ShowId,
    pub context: ActionContext,
    pub desk_id: uuid::Uuid,
    pub page: u8,
}

impl PlaybackUnitOfWork for ChangeExistingPage<'_> {
    type Output = Result<super::super::PlaybackPageAvailability, ApiError>;

    fn execute(self) -> PlaybackOperation<Self::Output> {
        let before = match projection(self.state, &self.context) {
            Ok(before) => before,
            Err(error) => return PlaybackOperation::new(Err(error)),
        };
        let availability = existing_page(self.state, self.page);
        change_page(
            self.state,
            self.show_id,
            self.context,
            self.desk_id,
            self.page,
            before,
            availability,
        )
    }
}

fn existing_page(state: &AppState, number: u8) -> super::super::PlaybackPageAvailability {
    if state
        .engine
        .snapshot()
        .playback_pages
        .iter()
        .any(|page| page.number == number)
    {
        super::super::PlaybackPageAvailability::Existing
    } else {
        super::super::PlaybackPageAvailability::Missing
    }
}

fn change_page(
    state: &AppState,
    show_id: light_core::ShowId,
    context: ActionContext,
    desk_id: uuid::Uuid,
    page: u8,
    before: PlaybackDeskProjection,
    availability: super::super::PlaybackPageAvailability,
) -> PlaybackOperation<Result<super::super::PlaybackPageAvailability, ApiError>> {
    if !availability.available() {
        return PlaybackOperation::new(Ok(availability));
    }
    if let Err(error) = set_page(state, desk_id, show_id, page) {
        return PlaybackOperation::new(Err(error));
    }
    match change_event(state, &context, before) {
        Ok(event) => PlaybackOperation::with_events(Ok(availability), event.into_iter().collect()),
        Err(error) => PlaybackOperation::new(Err(error)),
    }
}

fn set_page(
    state: &AppState,
    desk_id: uuid::Uuid,
    show_id: light_core::ShowId,
    page: u8,
) -> Result<(), ApiError> {
    state
        .desk
        .lock()
        .set_desk_page(desk_id, show_id, page)
        .map_err(ApiError::store)
}
