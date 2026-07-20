use super::preload_playback_queue_projection::ProgrammingPreloadPlaybackQueueContent;
use super::preload_values_projection::ProgrammingPreloadValuesContent;
use super::values_projection::ProgrammingValuesContent;
use super::{
    ProgrammingPorts, ProgrammingPreloadPlaybackQueueChange, ProgrammingPreloadValuesChange,
    ProgrammingService, ProgrammingValuesChange,
};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::{SessionId, UserId};
use light_programmer::ProgrammerCaptureMode;
use std::sync::Arc;
use uuid::Uuid;

/// Explicit target identity for replacing one user's live Programmer authority.
///
/// The acting identity remains in `ActionContext`; it is deliberately not inferred from this
/// target. Desk IDs identify every live interaction scope that must be excluded after the target
/// user gate has been acquired.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingLifecycleTarget {
    pub user_id: UserId,
    pub current_session_id: SessionId,
    desk_ids: Vec<Uuid>,
}

impl ProgrammingLifecycleTarget {
    pub fn new(user_id: UserId, current_session_id: SessionId, mut desk_ids: Vec<Uuid>) -> Self {
        desk_ids.sort_unstable();
        desk_ids.dedup();
        Self {
            user_id,
            current_session_id,
            desk_ids,
        }
    }
}

/// Adapter completion for a target-user lifecycle mutation.
#[derive(Debug)]
pub struct ProgrammingLifecycleCompletion<T> {
    pub output: T,
    pub replacement_session_id: Option<SessionId>,
}

impl<T> ProgrammingLifecycleCompletion<T> {
    pub const fn new(output: T, replacement_session_id: Option<SessionId>) -> Self {
        Self {
            output,
            replacement_session_id,
        }
    }
}

/// Result of replacing a Programmer authority and publishing its user-owned projections.
#[derive(Debug)]
pub struct ProgrammingLifecycleResult<T> {
    pub output: T,
    pub values_revision: u64,
    pub capture_mode_revision: u64,
    pub preload_values_revision: u64,
    pub preload_playback_queue_revision: u64,
    pub values_event_sequence: Option<u64>,
    pub preload_values_event_sequence: Option<u64>,
    pub preload_playback_queue_event_sequence: Option<u64>,
    pub capture_mode_event_sequence: Option<u64>,
}

impl ProgrammingService {
    /// Replace one explicitly targeted user's Programmer while authorizing a separate actor.
    ///
    /// Server adapters must acquire their show-activation guard before entering this boundary.
    /// Lock order within it is target user followed by the target user's sorted live desk gates.
    pub fn replace_user_programmer<T>(
        &self,
        actor_context: &ActionContext,
        ports: &dyn ProgrammingPorts,
        target: ProgrammingLifecycleTarget,
        operation: impl FnOnce() -> ProgrammingLifecycleCompletion<T>,
    ) -> Result<ProgrammingLifecycleResult<T>, ActionError> {
        ports.authorize(actor_context)?;
        let user_id = target.user_id;
        let desk_ids = target.desk_ids.clone();
        self.programmers.with_user_serialized(user_id, || {
            self.with_desk_gates(&desk_ids, || {
                self.replace_user_programmer_locked(actor_context, target, operation)
            })
        })
    }

    fn replace_user_programmer_locked<T>(
        &self,
        actor_context: &ActionContext,
        target: ProgrammingLifecycleTarget,
        operation: impl FnOnce() -> ProgrammingLifecycleCompletion<T>,
    ) -> Result<ProgrammingLifecycleResult<T>, ActionError> {
        self.assert_lifecycle_target(&target)?;
        let lifecycle_before = self.active_lifecycle_programmer(target.user_id);
        let before_values = ProgrammingValuesContent::read(
            &self.programmers,
            target.current_session_id,
            target.user_id,
        )?;
        let before_mode = self
            .programmers
            .capture_mode(target.current_session_id)
            .ok_or_else(lifecycle_target_unavailable)?;
        let before_preload_values = ProgrammingPreloadValuesContent::read(
            &self.programmers,
            target.current_session_id,
            target.user_id,
        )?;
        let before_preload_playback_queue = ProgrammingPreloadPlaybackQueueContent::read(
            &self.programmers,
            target.current_session_id,
            target.user_id,
        )?;
        let completion = operation();
        self.invalidate_values_replay(target.user_id);
        self.invalidate_preload_values_replay(target.user_id);
        self.invalidate_group_recording_replay(target.user_id);
        self.invalidate_preset_recording_replay(target.user_id);
        let after_values = self.lifecycle_values(&target, completion.replacement_session_id)?;
        let after_preload_values =
            self.lifecycle_preload_values(&target, completion.replacement_session_id)?;
        let after_preload_playback_queue =
            self.lifecycle_preload_playback_queue(&target, completion.replacement_session_id)?;
        let after_mode = self.lifecycle_mode(&target, completion.replacement_session_id)?;
        let values = self.lifecycle_values_change(target.user_id, before_values, after_values);
        let preload_values = self.lifecycle_preload_values_change(
            target.user_id,
            before_preload_values,
            after_preload_values,
        );
        let capture_mode = self.capture_mode_change(target.user_id, before_mode, after_mode);
        let preload_playback_queue = self.lifecycle_preload_playback_queue_change(
            target.user_id,
            before_preload_playback_queue,
            after_preload_playback_queue,
        );
        let capture_mode_event_sequence = self.publish_capture_mode(actor_context, capture_mode);
        let values_event_sequence = self.publish_values(actor_context, values);
        let preload_values_event_sequence =
            self.publish_preload_values(actor_context, preload_values);
        let preload_playback_queue_event_sequence =
            self.publish_preload_playback_queue(actor_context, preload_playback_queue);
        self.publish_lifecycle_for_user(actor_context, target.user_id, lifecycle_before);
        Ok(ProgrammingLifecycleResult {
            output: completion.output,
            values_revision: self.programmers.normal_values_revision(target.user_id),
            capture_mode_revision: self.programmers.capture_mode_revision(target.user_id),
            preload_values_revision: self.programmers.preload_values_revision(target.user_id),
            preload_playback_queue_revision: self
                .programmers
                .preload_playback_queue_revision(target.user_id),
            values_event_sequence,
            preload_values_event_sequence,
            preload_playback_queue_event_sequence,
            capture_mode_event_sequence,
        })
    }

    fn assert_lifecycle_target(
        &self,
        target: &ProgrammingLifecycleTarget,
    ) -> Result<(), ActionError> {
        match self.programmers.user_id(target.current_session_id) {
            Some(user_id) if user_id == target.user_id => Ok(()),
            Some(_) => Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the target Programmer session belongs to another user",
            )),
            None => Err(lifecycle_target_unavailable()),
        }
    }

    fn lifecycle_values(
        &self,
        target: &ProgrammingLifecycleTarget,
        session: Option<SessionId>,
    ) -> Result<ProgrammingValuesContent, ActionError> {
        session.map_or_else(
            || Ok(ProgrammingValuesContent::default()),
            |session| ProgrammingValuesContent::read(&self.programmers, session, target.user_id),
        )
    }

    fn lifecycle_mode(
        &self,
        target: &ProgrammingLifecycleTarget,
        session: Option<SessionId>,
    ) -> Result<ProgrammerCaptureMode, ActionError> {
        session.map_or(Ok(ProgrammerCaptureMode::default()), |session| {
            if self.programmers.user_id(session) != Some(target.user_id) {
                return Err(ActionError::new(
                    ActionErrorKind::Internal,
                    "replacement Programmer session does not belong to the target user",
                ));
            }
            self.programmers
                .capture_mode(session)
                .ok_or_else(lifecycle_target_unavailable)
        })
    }

    fn lifecycle_preload_values(
        &self,
        target: &ProgrammingLifecycleTarget,
        session: Option<SessionId>,
    ) -> Result<ProgrammingPreloadValuesContent, ActionError> {
        session.map_or_else(
            || Ok(ProgrammingPreloadValuesContent::default()),
            |session| {
                ProgrammingPreloadValuesContent::read(&self.programmers, session, target.user_id)
            },
        )
    }

    fn lifecycle_preload_playback_queue(
        &self,
        target: &ProgrammingLifecycleTarget,
        session: Option<SessionId>,
    ) -> Result<ProgrammingPreloadPlaybackQueueContent, ActionError> {
        session.map_or_else(
            || Ok(ProgrammingPreloadPlaybackQueueContent::default()),
            |session| {
                ProgrammingPreloadPlaybackQueueContent::read(
                    &self.programmers,
                    session,
                    target.user_id,
                )
            },
        )
    }

    fn lifecycle_values_change(
        &self,
        user_id: UserId,
        before: ProgrammingValuesContent,
        after: ProgrammingValuesContent,
    ) -> Option<ProgrammingValuesChange> {
        if before == after {
            return None;
        }
        let revision = self.programmers.advance_normal_values_revision(user_id);
        Some(ProgrammingValuesChange {
            projection: Arc::new(after.projection(user_id, revision)),
        })
    }

    fn lifecycle_preload_values_change(
        &self,
        user_id: UserId,
        before: ProgrammingPreloadValuesContent,
        after: ProgrammingPreloadValuesContent,
    ) -> Option<ProgrammingPreloadValuesChange> {
        if before == after {
            return None;
        }
        let revision = self.programmers.advance_preload_values_revision(user_id);
        Some(ProgrammingPreloadValuesChange {
            projection: Arc::new(after.projection(user_id, revision)),
        })
    }

    fn lifecycle_preload_playback_queue_change(
        &self,
        user_id: UserId,
        before: ProgrammingPreloadPlaybackQueueContent,
        after: ProgrammingPreloadPlaybackQueueContent,
    ) -> Option<ProgrammingPreloadPlaybackQueueChange> {
        if before == after {
            return None;
        }
        let revision = self
            .programmers
            .advance_preload_playback_queue_revision(user_id);
        Some(ProgrammingPreloadPlaybackQueueChange {
            projection: Arc::new(after.projection(user_id, revision)),
        })
    }
}

fn lifecycle_target_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "target Programmer authority is unavailable",
    )
}
