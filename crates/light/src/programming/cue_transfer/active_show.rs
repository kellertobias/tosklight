use super::{
    CueTransferAuthority, ProgrammingCueTransferChoiceRequest, ProgrammingCueTransferMode,
    ProgrammingCueTransferOutcome, ProgrammingCueTransferPorts,
    candidate::{PreparedCueTransfer, prepare_current_transfer, prepare_transfer},
    resolution::resolve_choice,
};
use crate::active_show::{CompletedActiveShowTransaction, PreparedActiveShowTransaction};
use crate::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowObjectsChange, ActiveShowService,
    EventBus, EventDraft,
};
use light_show::PortableShowDocument;

impl ActiveShowService {
    pub(crate) fn prepare_programming_cue_transfer_choice<P: ProgrammingCueTransferPorts>(
        &self,
        context: &ActionContext,
        request: &ProgrammingCueTransferChoiceRequest,
        ports: &P,
    ) -> Result<CueTransferAuthority, ActionError> {
        self.transact(
            context,
            request.show_id,
            ports,
            "programming-cue-transfer-choice",
            |document| {
                resolve_choice(document, request).map(PreparedActiveShowTransaction::NoChange)
            },
            |_, _, _, completed| completed.state,
        )
    }

    pub(crate) fn commit_programming_cue_transfer<P: ProgrammingCueTransferPorts>(
        &self,
        context: &ActionContext,
        authority: &CueTransferAuthority,
        mode: ProgrammingCueTransferMode,
        ports: &P,
    ) -> Result<ProgrammingCueTransferOutcome, ActionError> {
        self.transact(
            context,
            authority.show_id,
            ports,
            "programming-cue-transfer",
            |document| prepare_expected_transfer(document, context, authority, mode),
            complete_transfer,
        )
    }

    pub(crate) fn commit_current_programming_cue_transfer<P: ProgrammingCueTransferPorts>(
        &self,
        context: &ActionContext,
        request: &ProgrammingCueTransferChoiceRequest,
        mode: ProgrammingCueTransferMode,
        ports: &P,
    ) -> Result<ProgrammingCueTransferOutcome, ActionError> {
        self.transact(
            context,
            request.show_id,
            ports,
            "programming-cue-transfer",
            |document| prepare_current_transfer(document, request, mode),
            complete_transfer,
        )
    }
}

fn prepare_expected_transfer(
    document: &PortableShowDocument,
    context: &ActionContext,
    authority: &CueTransferAuthority,
    mode: ProgrammingCueTransferMode,
) -> Result<PreparedActiveShowTransaction<PreparedCueTransfer>, ActionError> {
    let expected = context.expected_revision.ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Cue transfer requires an expected Show revision",
        )
    })?;
    if expected != document.revision().value() {
        return Err(
            ActionError::new(ActionErrorKind::Conflict, "stale active-show revision")
                .at_revision(document.revision().value()),
        );
    }
    prepare_transfer(document, authority, mode)
}

fn complete_transfer<P: ProgrammingCueTransferPorts>(
    events: &EventBus,
    ports: &P,
    context: &ActionContext,
    completed: CompletedActiveShowTransaction<PreparedCueTransfer>,
) -> ProgrammingCueTransferOutcome {
    let mut prepared = completed.state;
    let commit = completed
        .commit
        .expect("a validated Cue transfer always commits one change");
    prepared.show_revision = commit.revision();
    ports.reconcile_cue_transfer(&prepared.changes);
    let show_event_sequence = events
        .publish(EventDraft::active_show_objects_changed(
            context,
            ActiveShowObjectsChange {
                show_id: prepared.show_id,
                show_revision: prepared.show_revision,
                changes: prepared.changes,
            },
        ))
        .sequence;
    ProgrammingCueTransferOutcome {
        show_id: prepared.show_id,
        summary: prepared.summary,
        show_revision: prepared.show_revision,
        projections: prepared.projections,
        show_event_sequence,
        command_line: light_programmer::CommandLineState::default(),
        interaction_event_sequence: None,
        persistence_warning: None,
    }
}
