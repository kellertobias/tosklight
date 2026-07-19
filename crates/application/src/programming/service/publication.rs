use super::ProgrammingService;
use crate::{ActionContext, EventDraft, ProgrammingInteractionChange};

impl ProgrammingService {
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
