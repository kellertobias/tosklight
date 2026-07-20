use super::{ProgrammingPorts, ProgrammingService};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::{SessionId, UserId};
use light_programmer::{
    PreloadPlaybackAction, PreloadPlaybackQueueAction, PreloadPlaybackQueueSurface,
    ProgrammerRegistry,
};
use std::sync::Arc;

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static PROJECTION_READS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingPreloadPlaybackAction {
    Toggle,
    Go,
    Back,
    Off,
    On,
    TemporaryOn,
    TemporaryOff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingPreloadPlaybackSurface {
    Physical,
    Virtual,
    Osc,
    Matter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammingPreloadPlaybackQueueItem {
    pub playback_number: u16,
    pub page: Option<u8>,
    pub action: ProgrammingPreloadPlaybackAction,
    pub surface: ProgrammingPreloadPlaybackSurface,
}

/// Exact-user authority for the ordered playback actions queued by Preload capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingPreloadPlaybackQueueProjection {
    pub user_id: UserId,
    pub revision: u64,
    pub actions: Vec<ProgrammingPreloadPlaybackQueueItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingPreloadPlaybackQueueChange {
    pub projection: Arc<ProgrammingPreloadPlaybackQueueProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingPreloadPlaybackQueueSnapshot {
    pub event_sequence: u64,
    pub projection: ProgrammingPreloadPlaybackQueueProjection,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ProgrammingPreloadPlaybackQueueContent {
    actions: Vec<ProgrammingPreloadPlaybackQueueItem>,
}

impl ProgrammingPreloadPlaybackQueueContent {
    pub(super) fn read(
        programmers: &ProgrammerRegistry,
        session: SessionId,
        user_id: UserId,
    ) -> Result<Self, ActionError> {
        #[cfg(test)]
        PROJECTION_READS.set(PROJECTION_READS.get() + 1);
        if programmers.user_id(session) != Some(user_id) {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the requested user",
            ));
        }
        let actions = programmers
            .preload_playback_actions(session)
            .ok_or_else(queue_unavailable)?
            .iter()
            .map(queue_item)
            .collect();
        Ok(Self { actions })
    }

    pub(super) fn projection(
        self,
        user_id: UserId,
        revision: u64,
    ) -> ProgrammingPreloadPlaybackQueueProjection {
        ProgrammingPreloadPlaybackQueueProjection {
            user_id,
            revision,
            actions: self.actions,
        }
    }
}

#[cfg(test)]
pub(super) fn reset_projection_read_count() {
    PROJECTION_READS.set(0);
}

#[cfg(test)]
pub(super) fn projection_read_count() -> usize {
    PROJECTION_READS.get()
}

impl ProgrammingService {
    pub fn preload_playback_queue_snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingPreloadPlaybackQueueSnapshot, ActionError> {
        let (session, user_id) = queue_identity(context)?;
        self.with_user_and_desk_gate(context.desk_id, user_id, || {
            ports.authorize(context)?;
            let event_sequence = self.events.latest_sequence();
            let content =
                ProgrammingPreloadPlaybackQueueContent::read(&self.programmers, session, user_id)?;
            Ok(ProgrammingPreloadPlaybackQueueSnapshot {
                event_sequence,
                projection: content.projection(
                    user_id,
                    self.programmers.preload_playback_queue_revision(user_id),
                ),
            })
        })
    }
}

fn queue_identity(context: &ActionContext) -> Result<(SessionId, UserId), ActionError> {
    let session = context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preload playback queue snapshots require an operator session",
        )
    })?;
    let user_id = context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preload playback queue snapshots require an authenticated user",
        )
    })?;
    Ok((session, user_id))
}

fn queue_item(action: &PreloadPlaybackAction) -> ProgrammingPreloadPlaybackQueueItem {
    ProgrammingPreloadPlaybackQueueItem {
        playback_number: action.playback_number,
        page: action.page,
        action: match action.action {
            PreloadPlaybackQueueAction::Toggle => ProgrammingPreloadPlaybackAction::Toggle,
            PreloadPlaybackQueueAction::Go => ProgrammingPreloadPlaybackAction::Go,
            PreloadPlaybackQueueAction::Back => ProgrammingPreloadPlaybackAction::Back,
            PreloadPlaybackQueueAction::Off => ProgrammingPreloadPlaybackAction::Off,
            PreloadPlaybackQueueAction::On => ProgrammingPreloadPlaybackAction::On,
            PreloadPlaybackQueueAction::TemporaryOn => {
                ProgrammingPreloadPlaybackAction::TemporaryOn
            }
            PreloadPlaybackQueueAction::TemporaryOff => {
                ProgrammingPreloadPlaybackAction::TemporaryOff
            }
        },
        surface: match action.surface {
            PreloadPlaybackQueueSurface::Physical => ProgrammingPreloadPlaybackSurface::Physical,
            PreloadPlaybackQueueSurface::Virtual => ProgrammingPreloadPlaybackSurface::Virtual,
            PreloadPlaybackQueueSurface::Osc => ProgrammingPreloadPlaybackSurface::Osc,
            PreloadPlaybackQueueSurface::Matter => ProgrammingPreloadPlaybackSurface::Matter,
        },
    }
}

fn queue_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "Preload playback queue is unavailable",
    )
}
