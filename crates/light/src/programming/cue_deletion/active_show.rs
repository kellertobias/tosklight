use super::{
    ProgrammingCueDeletionOutcome, ProgrammingCueDeletionPorts, ProgrammingCueDeletionState,
    ResolvedCueDeletionRequest,
    candidate::{PreparedCueDeletion, prepare_deletion},
};
use crate::active_show::CompletedActiveShowTransaction;
use crate::{
    ActionContext, ActionError, ActiveShowObjectsChange, ActiveShowService, EventBus, EventDraft,
};

impl ActiveShowService {
    pub(in crate::programming) fn delete_programming_cue<P: ProgrammingCueDeletionPorts>(
        &self,
        context: &ActionContext,
        request: &ResolvedCueDeletionRequest,
        ports: &P,
    ) -> Result<ProgrammingCueDeletionOutcome, ActionError> {
        self.transact(
            context,
            request.show_id,
            ports,
            "programming-cue-delete",
            |document| prepare_deletion(document, request, context.expected_revision),
            complete_deletion,
        )
    }
}

fn complete_deletion<P: ProgrammingCueDeletionPorts>(
    events: &EventBus,
    ports: &P,
    context: &ActionContext,
    completed: CompletedActiveShowTransaction<PreparedCueDeletion>,
) -> ProgrammingCueDeletionOutcome {
    let mut prepared = completed.state;
    let commit = completed
        .commit
        .expect("a validated Cue deletion always commits one change");
    prepared.show_revision = commit.revision();
    ports.reconcile_cue_deletion(&prepared.changes);
    let sequence = events
        .publish(EventDraft::active_show_objects_changed(
            context,
            ActiveShowObjectsChange {
                show_id: prepared.show_id,
                show_revision: prepared.show_revision,
                changes: prepared.changes,
            },
        ))
        .sequence;
    ProgrammingCueDeletionOutcome {
        show_id: prepared.show_id,
        show_revision: prepared.show_revision,
        cue_list: prepared.projection,
        deleted_cue: prepared.deleted_cue,
        state: ProgrammingCueDeletionState::Changed {
            show_event_sequence: sequence,
        },
        persistence_warning: None,
    }
}
