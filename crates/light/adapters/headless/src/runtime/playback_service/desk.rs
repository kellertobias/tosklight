//! Desk-local Playback view projection events for mutations outside Playback commands.

use light_application::{
    ActionContext, EventDraft, PlaybackDeskProjection, PlaybackOperation, PlaybackOperationResult,
    PlaybackPorts, PlaybackUnitOfWork,
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
    /// The caller owns activation. Portable page creation can finalize a new runtime and must
    /// therefore happen outside ordered Playback, while retaining the original view comparison.
    pub(in crate::runtime) fn run(
        self,
    ) -> PlaybackOperationResult<Result<super::super::PlaybackPageAvailability, ApiError>> {
        let state = self.state;
        state.programming.run_active_show_boundary(|| {
            let before = state.playback.run_unit_of_work(CaptureDeskProjection {
                state,
                context: &self.context,
            });
            let before = match before.output {
                Ok(before) => before,
                Err(error) => {
                    return PlaybackOperationResult {
                        output: Err(error),
                        event_sequences: Vec::new(),
                    };
                }
            };
            let availability = match super::super::ensure_playback_page_for_advance(
                state,
                self.show,
                self.page,
                &self.context,
            ) {
                Ok(availability) => availability,
                Err(error) => {
                    return PlaybackOperationResult {
                        output: Err(error),
                        event_sequences: Vec::new(),
                    };
                }
            };
            state.playback.run_unit_of_work(ChangeAvailablePage {
                change: self,
                before,
                availability,
            })
        })
    }

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

struct CaptureDeskProjection<'a> {
    state: &'a AppState,
    context: &'a ActionContext,
}

impl PlaybackUnitOfWork for CaptureDeskProjection<'_> {
    type Output = Result<PlaybackDeskProjection, ApiError>;
    fn execute(self) -> PlaybackOperation<Self::Output> {
        PlaybackOperation::new(projection(self.state, self.context))
    }
}

struct ChangeAvailablePage<'a> {
    change: ChangePage<'a>,
    before: PlaybackDeskProjection,
    availability: super::super::PlaybackPageAvailability,
}

impl PlaybackUnitOfWork for ChangeAvailablePage<'_> {
    type Output = Result<super::super::PlaybackPageAvailability, ApiError>;

    fn execute(self) -> PlaybackOperation<Self::Output> {
        let change = self.change;
        change_page(
            change.state,
            change.show.id,
            change.context,
            change.desk_id,
            change.page,
            self.before,
            self.availability,
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
        .output
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
        .installation
        .set_desk_page(desk_id, show_id, page)
        .map_err(ApiError::store)
}
