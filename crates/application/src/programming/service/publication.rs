use super::ProgrammingService;
use crate::{
    ActionContext, ActionError, EventDraft, ProgrammingCaptureModeChange,
    ProgrammingCaptureModeProjection, ProgrammingInteractionChange,
    ProgrammingPreloadPlaybackQueueChange, ProgrammingPreloadValuesChange,
    ProgrammingPriorityChange, ProgrammingValuesChange,
};
use light_core::{SessionId, UserId};
use std::sync::Arc;

use super::super::preload_playback_queue_projection::ProgrammingPreloadPlaybackQueueContent;
use super::super::preload_values_projection::ProgrammingPreloadValuesContent;
use super::super::values_projection::ProgrammingValuesContent;

impl ProgrammingService {
    pub(in crate::programming) fn publish_priority(
        &self,
        context: &ActionContext,
        change: ProgrammingPriorityChange,
    ) -> u64 {
        self.events
            .publish(EventDraft::programming_priority_changed(context, change))
            .sequence
    }

    pub(super) fn publish_interaction(
        &self,
        context: &ActionContext,
        interaction: Option<ProgrammingInteractionChange>,
    ) -> Option<u64> {
        interaction
            .and_then(|change| self.suppress_nested_selection(change))
            .map(|change| {
                self.events
                    .publish(EventDraft::programming_interaction_changed(context, change))
                    .sequence
            })
    }

    pub(in crate::programming) fn publish_values(
        &self,
        context: &ActionContext,
        values: Option<ProgrammingValuesChange>,
    ) -> Option<u64> {
        values.map(|change| {
            self.events
                .publish(EventDraft::programming_values_changed(context, change))
                .sequence
        })
    }

    pub(in crate::programming) fn publish_capture_mode(
        &self,
        context: &ActionContext,
        change: Option<ProgrammingCaptureModeChange>,
    ) -> Option<u64> {
        change.map(|change| {
            self.events
                .publish(EventDraft::programming_capture_mode_changed(
                    context, change,
                ))
                .sequence
        })
    }

    pub(in crate::programming) fn publish_preload_values(
        &self,
        context: &ActionContext,
        values: Option<ProgrammingPreloadValuesChange>,
    ) -> Option<u64> {
        values.map(|change| {
            self.events
                .publish(EventDraft::programming_preload_values_changed(
                    context, change,
                ))
                .sequence
        })
    }

    pub(in crate::programming) fn publish_preload_playback_queue(
        &self,
        context: &ActionContext,
        change: Option<ProgrammingPreloadPlaybackQueueChange>,
    ) -> Option<u64> {
        change.map(|change| {
            self.events
                .publish(EventDraft::programming_preload_playback_queue_changed(
                    context, change,
                ))
                .sequence
        })
    }

    pub(in crate::programming) fn capture_mode_change(
        &self,
        user_id: UserId,
        before: light_programmer::ProgrammerCaptureMode,
        after: light_programmer::ProgrammerCaptureMode,
    ) -> Option<ProgrammingCaptureModeChange> {
        if before == after {
            return None;
        }
        let revision = self.programmers.advance_capture_mode_revision(user_id);
        Some(ProgrammingCaptureModeChange {
            projection: Arc::new(ProgrammingCaptureModeProjection::from_mode(
                user_id, revision, after,
            )),
        })
    }

    pub(super) fn values_change(
        &self,
        user_id: UserId,
        session: SessionId,
        before_generation: u64,
        after_generation: u64,
    ) -> Result<Option<ProgrammingValuesChange>, ActionError> {
        if before_generation == after_generation {
            return Ok(None);
        }
        let content = ProgrammingValuesContent::read(&self.programmers, session, user_id)?;
        let revision = self.programmers.advance_normal_values_revision(user_id);
        Ok(Some(ProgrammingValuesChange {
            projection: Arc::new(content.projection(user_id, revision)),
        }))
    }

    pub(super) fn preload_values_change(
        &self,
        user_id: UserId,
        session: SessionId,
        before_generation: u64,
        after_generation: u64,
    ) -> Result<Option<ProgrammingPreloadValuesChange>, ActionError> {
        if before_generation == after_generation {
            return Ok(None);
        }
        let content = ProgrammingPreloadValuesContent::read(&self.programmers, session, user_id)?;
        let revision = self.programmers.advance_preload_values_revision(user_id);
        Ok(Some(ProgrammingPreloadValuesChange {
            projection: Arc::new(content.projection(user_id, revision)),
        }))
    }

    pub(super) fn preload_playback_queue_change(
        &self,
        user_id: UserId,
        session: SessionId,
        before_generation: u64,
        after_generation: u64,
    ) -> Result<Option<ProgrammingPreloadPlaybackQueueChange>, ActionError> {
        if before_generation == after_generation {
            return Ok(None);
        }
        let content =
            ProgrammingPreloadPlaybackQueueContent::read(&self.programmers, session, user_id)?;
        let revision = self
            .programmers
            .advance_preload_playback_queue_revision(user_id);
        Ok(Some(ProgrammingPreloadPlaybackQueueChange {
            projection: Arc::new(content.projection(user_id, revision)),
        }))
    }

    fn suppress_nested_selection(
        &self,
        change: ProgrammingInteractionChange,
    ) -> Option<ProgrammingInteractionChange> {
        let Some(revision) = change.selection().map(|selection| selection.revision) else {
            return Some(change);
        };
        let desk_id = change.desk_id();
        if self.nested_selection_publications.lock().remove(&desk_id) == Some(revision) {
            return change.without_selection();
        }
        Some(change)
    }

    pub(super) fn publish_selection_refresh(
        &self,
        context: &ActionContext,
        change: ProgrammingInteractionChange,
        suppress_outer: bool,
    ) -> u64 {
        let desk_id = change.desk_id();
        let selection_revision = change.selection().map(|selection| selection.revision);
        let sequence = self
            .events
            .publish(EventDraft::programming_interaction_changed(context, change))
            .sequence;
        if suppress_outer && let Some(revision) = selection_revision {
            self.nested_selection_publications
                .lock()
                .insert(desk_id, revision);
        }
        sequence
    }
}
