use super::{ProgrammingPorts, ProgrammingService};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::SessionId;
use light_programmer::{ProgrammerCaptureMode, ProgrammerRegistry};
use std::sync::Arc;

/// Authoritative user-owned capture mode shared by every desk session for one user.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingCaptureModeProjection {
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
    ) -> Result<Self, ActionError> {
        let mode = programmers
            .capture_mode(session)
            .ok_or_else(capture_mode_unavailable)?;
        Ok(Self::from_mode(programmers.capture_mode_revision(), mode))
    }

    pub(super) const fn from_mode(revision: u64, mode: ProgrammerCaptureMode) -> Self {
        Self {
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
        let session = capture_identity(context)?;
        self.with_programmer_and_desk_gate(context.desk_id, || {
            ports.authorize(context)?;
            // A cursor captured before the immutable projection can permit a duplicate after
            // repair, but cannot miss a transition serialized by this gate.
            let event_sequence = self.events.latest_sequence();
            let projection = ProgrammingCaptureModeProjection::read(&self.programmers, session)?;
            Ok(ProgrammingCaptureModeSnapshot {
                event_sequence,
                projection,
            })
        })
    }
}

fn capture_identity(context: &ActionContext) -> Result<SessionId, ActionError> {
    let session = context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer capture-mode snapshots require an operator session",
        )
    })?;
    Ok(session)
}

fn capture_mode_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "Programmer capture mode is unavailable",
    )
}
