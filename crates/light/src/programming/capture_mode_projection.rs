use super::{ProgrammingPorts, ProgrammingService};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::{SessionId, UserId};
use light_programmer::{ProgrammerCaptureMode, ProgrammerRegistry};
use std::sync::Arc;

/// Authoritative user-owned capture mode shared by every desk session for one user.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingCaptureModeProjection {
    pub user_id: UserId,
    pub revision: u64,
    pub blind: bool,
    pub preview: bool,
    pub preload_capture_programmer: bool,
}

impl ProgrammingCaptureModeProjection {
    pub const fn mode(&self) -> ProgrammerCaptureMode {
        ProgrammerCaptureMode {
            blind: self.blind,
            preview: self.preview,
            preload_capture_programmer: self.preload_capture_programmer,
        }
    }

    pub(super) fn read(
        programmers: &ProgrammerRegistry,
        session: SessionId,
        user_id: UserId,
    ) -> Result<Self, ActionError> {
        let mode = programmers
            .capture_mode(session)
            .ok_or_else(capture_mode_unavailable)?;
        if programmers.user_id(session) != Some(user_id) {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the requested user",
            ));
        }
        Ok(Self::from_mode(
            user_id,
            programmers.capture_mode_revision(user_id),
            mode,
        ))
    }

    pub(super) const fn from_mode(
        user_id: UserId,
        revision: u64,
        mode: ProgrammerCaptureMode,
    ) -> Self {
        Self {
            user_id,
            revision,
            blind: mode.blind,
            preview: mode.preview,
            preload_capture_programmer: mode.preload_capture_programmer,
        }
    }
}

/// One semantic capture-mode transition carrying a complete replaceable projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingCaptureModeChange {
    pub projection: Arc<ProgrammingCaptureModeProjection>,
}

/// Authoritative gap-repair snapshot for one authenticated user's capture mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingCaptureModeSnapshot {
    pub event_sequence: u64,
    pub projection: ProgrammingCaptureModeProjection,
}

impl ProgrammingService {
    pub fn capture_mode_snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingCaptureModeSnapshot, ActionError> {
        let (session, user_id) = capture_identity(context)?;
        self.with_user_and_desk_gate(context.desk_id, user_id, || {
            ports.authorize(context)?;
            // A cursor captured before the immutable projection can permit a duplicate after
            // repair, but cannot miss a same-user transition serialized by this user gate.
            let event_sequence = self.events.latest_sequence();
            let projection =
                ProgrammingCaptureModeProjection::read(&self.programmers, session, user_id)?;
            Ok(ProgrammingCaptureModeSnapshot {
                event_sequence,
                projection,
            })
        })
    }
}

fn capture_identity(context: &ActionContext) -> Result<(SessionId, UserId), ActionError> {
    let session = context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer capture-mode snapshots require an operator session",
        )
    })?;
    let user_id = context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer capture-mode snapshots require an authenticated user",
        )
    })?;
    Ok((session, user_id))
}

fn capture_mode_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "Programmer capture mode is unavailable",
    )
}
