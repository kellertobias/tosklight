use super::preload_playback_queue_projection::ProgrammingPreloadPlaybackQueueContent;
use super::preload_values_projection::ProgrammingPreloadValuesContent;
use super::values_projection::ProgrammingValuesContent;
use super::{
    ProgrammingPorts, ProgrammingPreloadPlaybackQueueChange, ProgrammingPreloadValuesChange,
    ProgrammingPriorityChange, ProgrammingService, ProgrammingValuesChange,
};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::SessionId;
use light_programmer::ProgrammerCaptureMode;
use std::sync::Arc;
use uuid::Uuid;

/// The session whose live Programmer authority is being replaced.
///
/// Desk IDs identify every live interaction scope that must be excluded after the desk's gate has
/// been acquired.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingLifecycleTarget {
    pub current_session_id: SessionId,
    desk_ids: Vec<Uuid>,
}

impl ProgrammingLifecycleTarget {
    pub fn new(current_session_id: SessionId, mut desk_ids: Vec<Uuid>) -> Self {
        desk_ids.sort_unstable();
        desk_ids.dedup();
        Self {
            current_session_id,
            desk_ids,
        }
    }
}

/// Adapter completion for a lifecycle mutation.
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

/// Result of replacing the Programmer authority and publishing its projections.
#[derive(Debug)]
pub struct ProgrammingLifecycleResult<T> {
    pub output: T,
    pub values_revision: u64,
    pub capture_mode_revision: u64,
    pub preload_values_revision: u64,
    pub preload_playback_queue_revision: u64,
    pub priority_revision: u64,
    pub values_event_sequence: Option<u64>,
    pub preload_values_event_sequence: Option<u64>,
    pub preload_playback_queue_event_sequence: Option<u64>,
    pub capture_mode_event_sequence: Option<u64>,
    pub priority_event_sequence: Option<u64>,
}

impl ProgrammingService {
    /// Replace the desk's Programmer.
    ///
    /// Server adapters must acquire their show-activation guard before entering this boundary.
    /// Lock order within it is the Programmer gate followed by the sorted live desk gates.
    pub fn replace_desk_programmer<T>(
        &self,
        actor_context: &ActionContext,
        ports: &dyn ProgrammingPorts,
        target: ProgrammingLifecycleTarget,
        operation: impl FnOnce() -> ProgrammingLifecycleCompletion<T>,
    ) -> Result<ProgrammingLifecycleResult<T>, ActionError> {
        ports.authorize_programming_change(actor_context)?;
        let desk_ids = target.desk_ids.clone();
        self.programmers.serialized(|| {
            self.with_desk_gates(&desk_ids, || {
                self.replace_desk_programmer_locked(actor_context, target, operation)
            })
        })
    }

    fn replace_desk_programmer_locked<T>(
        &self,
        actor_context: &ActionContext,
        target: ProgrammingLifecycleTarget,
        operation: impl FnOnce() -> ProgrammingLifecycleCompletion<T>,
    ) -> Result<ProgrammingLifecycleResult<T>, ActionError> {
        self.assert_lifecycle_target(&target)?;
        let lifecycle_before = self.active_lifecycle_programmer();
        let before_values =
            ProgrammingValuesContent::read(&self.programmers, target.current_session_id)?;
        let before_mode = self
            .programmers
            .capture_mode(target.current_session_id)
            .ok_or_else(lifecycle_target_unavailable)?;
        let before_priority = self.priority_projection(
            target.current_session_id,
            self.programmers.priority_revision(),
        )?;
        let before_preload_values =
            ProgrammingPreloadValuesContent::read(&self.programmers, target.current_session_id)?;
        let before_preload_playback_queue = ProgrammingPreloadPlaybackQueueContent::read(
            &self.programmers,
            target.current_session_id,
        )?;
        let completion = operation();
        self.invalidate_values_replay();
        self.invalidate_preload_values_replay();
        self.invalidate_priority_replay();
        self.invalidate_cue_recording_replay();
        self.invalidate_cue_deletion_replay();
        self.invalidate_cue_transfer_authority();
        self.invalidate_group_management_replay();
        self.invalidate_group_recording_replay();
        self.invalidate_preset_recording_replay();
        self.invalidate_update_replay();
        let after_values = self.lifecycle_values(completion.replacement_session_id)?;
        let after_preload_values =
            self.lifecycle_preload_values(completion.replacement_session_id)?;
        let after_preload_playback_queue =
            self.lifecycle_preload_playback_queue(completion.replacement_session_id)?;
        let after_mode = self.lifecycle_mode(completion.replacement_session_id)?;
        let values = self.lifecycle_values_change(before_values, after_values);
        let preload_values =
            self.lifecycle_preload_values_change(before_preload_values, after_preload_values);
        let capture_mode = self.capture_mode_change(before_mode, after_mode);
        let priority = self.lifecycle_priority_change(
            &target,
            completion.replacement_session_id,
            before_priority,
        )?;
        let preload_playback_queue = self.lifecycle_preload_playback_queue_change(
            before_preload_playback_queue,
            after_preload_playback_queue,
        );
        let capture_mode_event_sequence = self.publish_capture_mode(actor_context, capture_mode);
        let priority_event_sequence =
            priority.map(|change| self.publish_priority(actor_context, change));
        let values_event_sequence = self.publish_values(actor_context, values);
        let preload_values_event_sequence =
            self.publish_preload_values(actor_context, preload_values);
        let preload_playback_queue_event_sequence =
            self.publish_preload_playback_queue(actor_context, preload_playback_queue);
        self.publish_lifecycle(actor_context, lifecycle_before);
        Ok(ProgrammingLifecycleResult {
            output: completion.output,
            values_revision: self.programmers.normal_values_revision(),
            capture_mode_revision: self.programmers.capture_mode_revision(),
            preload_values_revision: self.programmers.preload_values_revision(),
            preload_playback_queue_revision: self.programmers.preload_playback_queue_revision(),
            priority_revision: self.programmers.priority_revision(),
            values_event_sequence,
            preload_values_event_sequence,
            preload_playback_queue_event_sequence,
            capture_mode_event_sequence,
            priority_event_sequence,
        })
    }

    fn assert_lifecycle_target(
        &self,
        target: &ProgrammingLifecycleTarget,
    ) -> Result<(), ActionError> {
        // The desk has one Programmer, so a session it knows operates that one.
        if self.programmers.knows_session(target.current_session_id) {
            Ok(())
        } else {
            Err(lifecycle_target_unavailable())
        }
    }

    fn lifecycle_values(
        &self,
        session: Option<SessionId>,
    ) -> Result<ProgrammingValuesContent, ActionError> {
        session.map_or_else(
            || Ok(ProgrammingValuesContent::default()),
            |session| ProgrammingValuesContent::read(&self.programmers, session),
        )
    }

    fn lifecycle_mode(
        &self,
        session: Option<SessionId>,
    ) -> Result<ProgrammerCaptureMode, ActionError> {
        session.map_or(Ok(ProgrammerCaptureMode::default()), |session| {
            if !self.programmers.knows_session(session) {
                return Err(ActionError::new(
                    ActionErrorKind::Internal,
                    "replacement Programmer session does not exist",
                ));
            }
            self.programmers
                .capture_mode(session)
                .ok_or_else(lifecycle_target_unavailable)
        })
    }

    fn lifecycle_preload_values(
        &self,
        session: Option<SessionId>,
    ) -> Result<ProgrammingPreloadValuesContent, ActionError> {
        session.map_or_else(
            || Ok(ProgrammingPreloadValuesContent::default()),
            |session| ProgrammingPreloadValuesContent::read(&self.programmers, session),
        )
    }

    fn lifecycle_preload_playback_queue(
        &self,
        session: Option<SessionId>,
    ) -> Result<ProgrammingPreloadPlaybackQueueContent, ActionError> {
        session.map_or_else(
            || Ok(ProgrammingPreloadPlaybackQueueContent::default()),
            |session| ProgrammingPreloadPlaybackQueueContent::read(&self.programmers, session),
        )
    }

    fn lifecycle_priority_change(
        &self,
        _target: &ProgrammingLifecycleTarget,
        replacement: Option<SessionId>,
        before: super::ProgrammingPriorityProjection,
    ) -> Result<Option<ProgrammingPriorityChange>, ActionError> {
        let Some(session) = replacement else {
            let mut revision = self.programmers.priority_revision();
            if revision <= before.revision {
                revision = self.programmers.advance_priority_revision();
            }
            return Ok(Some(ProgrammingPriorityChange::Remove { revision }));
        };
        if !self.programmers.knows_session(session) {
            return Err(ActionError::new(
                ActionErrorKind::Internal,
                "replacement Programmer session does not exist",
            ));
        }
        let mut revision = self.programmers.priority_revision();
        let mut after = self.priority_projection(session, revision)?;
        if after != before && revision <= before.revision {
            revision = self.programmers.advance_priority_revision();
            after.revision = revision;
        }
        Ok((before != after).then_some(ProgrammingPriorityChange::Upsert { projection: after }))
    }

    fn lifecycle_values_change(
        &self,
        before: ProgrammingValuesContent,
        after: ProgrammingValuesContent,
    ) -> Option<ProgrammingValuesChange> {
        if before == after {
            return None;
        }
        let revision = self.programmers.advance_normal_values_revision();
        Some(after.change(&before, revision))
    }

    fn lifecycle_preload_values_change(
        &self,
        before: ProgrammingPreloadValuesContent,
        after: ProgrammingPreloadValuesContent,
    ) -> Option<ProgrammingPreloadValuesChange> {
        if before == after {
            return None;
        }
        let revision = self.programmers.advance_preload_values_revision();
        Some(ProgrammingPreloadValuesChange {
            projection: Arc::new(after.projection(revision)),
        })
    }

    fn lifecycle_preload_playback_queue_change(
        &self,
        before: ProgrammingPreloadPlaybackQueueContent,
        after: ProgrammingPreloadPlaybackQueueContent,
    ) -> Option<ProgrammingPreloadPlaybackQueueChange> {
        if before == after {
            return None;
        }
        let revision = self.programmers.advance_preload_playback_queue_revision();
        Some(ProgrammingPreloadPlaybackQueueChange {
            projection: Arc::new(after.projection(revision)),
        })
    }
}

fn lifecycle_target_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "target Programmer authority is unavailable",
    )
}
