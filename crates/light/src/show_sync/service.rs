//! The one ordered sync transaction: dedup, field compare-and-set, compile, commit, publish.

use super::staging::{SyncStage, stage_sync, validate_command};
use super::{
    PATCHED_FIXTURE_KIND, ShowSyncAppliedObject, ShowSyncCommand, ShowSyncPorts, ShowSyncResult,
};
use crate::active_show::{CompletedActiveShowTransaction, PreparedActiveShowTransaction};
use crate::show_compiler::prepare_normalized_show_candidate_incremental;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActiveShowObjectChange,
    ActiveShowObjectKind, ActiveShowObjectsChange, ActiveShowService, EventBus, EventDraft,
    PatchChange, PreparedShowCandidate, ShowPatchPorts, prepare_show_candidate,
};
use light_show::{PortableShowCandidate, PortableShowDocument, SyncRequestRecord};

enum SyncCompletion {
    Replayed(ShowSyncResult),
    Unchanged(ShowSyncResult),
    Committed(Box<CommittedSync>),
}

struct CommittedSync {
    result: ShowSyncResult,
    patch: Option<PatchChange>,
    object_changes: Vec<ActiveShowObjectChange>,
}

impl ActiveShowService {
    /// Applies one Control ↔ Architect sync transaction to the active show.
    ///
    /// Unlike desk object intents, which are last-write-wins, each field compares the client's
    /// base with the current value: untouched fields commit, fields another user changed are
    /// reported as conflicts and keep that user's value. The request identity is stored in the
    /// commit, so a retry — from any session, after any restart — returns the first outcome.
    pub fn synchronize<P: ShowSyncPorts>(
        &self,
        envelope: ActionEnvelope<ShowSyncCommand>,
        ports: &P,
    ) -> Result<ShowSyncResult, ActionError> {
        validate_command(&envelope.command)?;
        if has_patch_operations(&envelope.command) {
            ports.authorize_patch(&envelope.context)?;
        }
        let ActionEnvelope { context, command } = envelope;
        self.transact_with_unit(
            &context,
            command.show_id,
            ports,
            "show-sync",
            |unit| prepare(unit, &command, ports),
            complete,
        )
    }
}

fn has_patch_operations(command: &ShowSyncCommand) -> bool {
    use super::ShowSyncOperation as Operation;
    command.operations.iter().any(|operation| {
        matches!(
            operation,
            Operation::UpdatePatchFixture { .. }
                | Operation::CreatePatchFixture { .. }
                | Operation::RemovePatchFixture { .. }
                | Operation::RetainProfileRevision { .. }
        )
    })
}

fn prepare<P: ShowSyncPorts>(
    unit: &P::UnitOfWork,
    command: &ShowSyncCommand,
    ports: &P,
) -> Result<PreparedActiveShowTransaction<SyncCompletion>, ActionError> {
    use crate::ActiveShowUnitOfWork;
    if let Some(applied) =
        ports.applied_sync_request(unit, command.association_id, &command.request_id)?
    {
        return replay(command, applied).map(PreparedActiveShowTransaction::NoChange);
    }
    let document = unit.document();
    let stage = stage_sync(document, command, ports)?;
    if stage.transaction.is_empty() {
        let result = unchanged_result(document, command, stage);
        return Ok(PreparedActiveShowTransaction::NoChange(
            SyncCompletion::Unchanged(result),
        ));
    }
    commit_candidate(document, command, stage, ports)
}

fn replay(
    command: &ShowSyncCommand,
    applied: light_show::SyncAppliedRequest,
) -> Result<SyncCompletion, ActionError> {
    if applied.signature != command.signature {
        return Err(ActionError::new(
            ActionErrorKind::Conflict,
            super::REQUEST_REUSED_MESSAGE,
        ));
    }
    let mut result: ShowSyncResult = serde_json::from_value(applied.outcome).map_err(|error| {
        ActionError::new(
            ActionErrorKind::Internal,
            format!("stored sync outcome is unreadable: {error}"),
        )
    })?;
    result.replayed = true;
    result.event_sequence = None;
    Ok(SyncCompletion::Replayed(result))
}

fn unchanged_result(
    document: &PortableShowDocument,
    command: &ShowSyncCommand,
    stage: SyncStage,
) -> ShowSyncResult {
    ShowSyncResult {
        request_id: command.request_id.clone(),
        association_id: command.association_id,
        show_id: command.show_id.0,
        replayed: false,
        show_revision: document.revision().value(),
        patch_revision: document.patch_revision().value(),
        applied: Vec::new(),
        metadata: Vec::new(),
        conflicts: stage.conflicts,
        event_sequence: None,
    }
}

fn commit_candidate<P: ShowSyncPorts>(
    document: &PortableShowDocument,
    command: &ShowSyncCommand,
    mut stage: SyncStage,
    ports: &P,
) -> Result<PreparedActiveShowTransaction<SyncCompletion>, ActionError> {
    let transaction = std::mem::replace(&mut stage.transaction, document.transaction());
    let mut candidate = compile(document, transaction, stage.patch.is_some(), ports)?;
    let projection = document
        .candidate(candidate.transaction())
        .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
    let (patch, mut object_changes) = match &stage.patch {
        Some(patch) => {
            let (change, groups) = patch.finish(document, projection, &stage.extra_keys())?;
            (Some(change), groups)
        }
        None => (None, Vec::new()),
    };
    object_changes.extend(typed_object_changes(projection, &stage)?);
    let result = committed_result(command, projection, stage);
    let outcome = serde_json::to_value(&result)
        .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))?;
    candidate
        .transaction_mut()
        .record_sync_request(SyncRequestRecord {
            association_id: command.association_id,
            request_id: command.request_id.clone(),
            signature: command.signature.clone(),
            outcome,
        });
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        prepared: Box::new(candidate),
        state: SyncCompletion::Committed(Box::new(CommittedSync {
            result,
            patch,
            object_changes,
        })),
    })
}

/// A patch change recompiles the patch exactly as the patch capability does; object and metadata
/// changes alone touch no compiled projection and share the live runtime.
fn compile<P: ShowSyncPorts>(
    document: &PortableShowDocument,
    transaction: light_show::PortableShowTransaction,
    patch_changed: bool,
    ports: &P,
) -> Result<PreparedShowCandidate, ActionError> {
    match ports.installed_snapshot() {
        Some(previous) if !patch_changed => {
            prepare_normalized_show_candidate_incremental(document, transaction, &previous)
        }
        _ => prepare_show_candidate(document, transaction),
    }
}

fn committed_result(
    command: &ShowSyncCommand,
    projection: PortableShowCandidate<'_>,
    stage: SyncStage,
) -> ShowSyncResult {
    let mut applied: Vec<ShowSyncAppliedObject> = stage
        .objects
        .iter()
        .map(|(key, deleted)| ShowSyncAppliedObject {
            kind: key.kind().to_owned(),
            id: key.id().to_owned(),
            revision: projection.object_revision(key.kind(), key.id()),
            deleted: *deleted,
        })
        .collect();
    applied.extend(stage.patched_fixtures.iter().map(|(fixture_id, deleted)| {
        let id = fixture_id.0.to_string();
        ShowSyncAppliedObject {
            revision: projection.object_revision(PATCHED_FIXTURE_KIND, &id),
            kind: PATCHED_FIXTURE_KIND.into(),
            id,
            deleted: *deleted,
        }
    }));
    ShowSyncResult {
        request_id: command.request_id.clone(),
        association_id: command.association_id,
        show_id: command.show_id.0,
        replayed: false,
        show_revision: projection.revision().value(),
        patch_revision: projection.patch_revision().value(),
        applied,
        metadata: stage.metadata,
        conflicts: stage.conflicts,
        event_sequence: None,
    }
}

/// Synchronized kinds the desk also owns as typed families publish their ordinary object change,
/// so every desk surface showing them updates as it does for a desk edit.
fn typed_object_changes(
    projection: PortableShowCandidate<'_>,
    stage: &SyncStage,
) -> Result<Vec<ActiveShowObjectChange>, ActionError> {
    let mut changes = Vec::new();
    for (key, deleted) in &stage.objects {
        let Some(kind) = ActiveShowObjectKind::from_storage_kind(key.kind()) else {
            continue;
        };
        if *deleted {
            changes.push(ActiveShowObjectChange::deleted(
                kind,
                key.id().to_owned(),
                0,
            ));
            continue;
        }
        let object = projection.object(key.kind(), key.id()).ok_or_else(|| {
            ActionError::new(ActionErrorKind::Internal, "staged object is absent")
        })?;
        changes.push(
            ActiveShowObjectChange::present(
                kind,
                key.id().to_owned(),
                object.revision(),
                object.body().clone(),
            )
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?,
        );
    }
    Ok(changes)
}

fn complete<P: ShowSyncPorts>(
    events: &EventBus,
    ports: &P,
    context: &ActionContext,
    completed: CompletedActiveShowTransaction<SyncCompletion>,
) -> ShowSyncResult {
    match completed.state {
        SyncCompletion::Replayed(result) | SyncCompletion::Unchanged(result) => result,
        SyncCompletion::Committed(committed) => {
            let CommittedSync {
                mut result,
                patch,
                object_changes,
            } = *committed;
            let commit = completed
                .commit
                .expect("a prepared sync commit always completes with its commit");
            debug_assert_eq!(commit.revision().value(), result.show_revision);
            let mut sequence = None;
            if let Some(mut change) = patch {
                change.show_revision = commit.revision();
                change.patch_revision = commit.patch_revision();
                ShowPatchPorts::reconcile_patch_change(ports, &change);
                sequence = Some(
                    events
                        .publish(EventDraft::patch_changed(context, change))
                        .sequence,
                );
            }
            if !object_changes.is_empty() {
                ports.reconcile_object_changes(&object_changes);
                let draft = EventDraft::active_show_objects_changed(
                    context,
                    ActiveShowObjectsChange {
                        show_id: commit_show_id(&result),
                        show_revision: commit.revision(),
                        changes: object_changes,
                    },
                );
                sequence = Some(events.publish(draft).sequence);
            }
            result.event_sequence = sequence;
            result
        }
    }
}

fn commit_show_id(result: &ShowSyncResult) -> light_core::ShowId {
    light_core::ShowId(result.show_id)
}
