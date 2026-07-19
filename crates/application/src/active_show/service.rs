use super::{
    ActiveShowObjectsChange, ActiveShowPorts, ActiveShowUnitOfWork, BackupIdentity,
    MutateActiveShowObjectsCommand, MutateActiveShowObjectsResult, MutateOutputRouteCommand,
    MutateOutputRouteResult, OutputRouteChange, UndoActiveShowObjectCommand,
    UndoActiveShowObjectResult,
    objects::{PreparedObjectChanges, prepare_object_mutation},
    route::prepare_route_mutation,
    undo::{prepare_object_undo, validate_object_undo},
};
use crate::{ActionContext, ActionEnvelope, ActionError, EventBus, EventDraft};
use light_core::ShowId;
use light_show::{PortableShowCommit, PortableShowDocument, PortableShowRevision};
use parking_lot::Mutex;
use std::sync::Arc;

/// Ordered application boundary for mutations of the currently active portable show.
#[derive(Clone)]
pub struct ActiveShowService {
    operation: Arc<Mutex<()>>,
    events: EventBus,
}

impl ActiveShowService {
    pub fn new(events: EventBus) -> Self {
        Self {
            operation: Arc::new(Mutex::new(())),
            events,
        }
    }

    pub fn mutate_output_route<P: ActiveShowPorts>(
        &self,
        envelope: ActionEnvelope<MutateOutputRouteCommand>,
        ports: &P,
    ) -> Result<MutateOutputRouteResult, ActionError> {
        ports.authorize_mutation(&envelope.context)?;
        ports.run_active_show_lifecycle(&envelope.context, envelope.command.show_id, || {
            let _ordered = self.operation.lock();
            let mut unit = ports.begin_active_show(&envelope.context, envelope.command.show_id)?;
            let prepared = prepare_route_mutation(unit.document(), &envelope.command)?;
            let runtime = ports.prepare_runtime(prepared.snapshot)?;
            unit.backup(&backup_identity(
                &envelope.context,
                envelope.command.show_id,
                "route",
            ))?;
            let commit = unit.commit(prepared.transaction)?;
            let change = OutputRouteChange {
                show_id: envelope.command.show_id,
                show_revision: commit.revision(),
                route_id: envelope.command.route_id,
                object_revision: prepared.object_revision,
                route: prepared.route,
                deleted: prepared.deleted,
            };
            ports.install_runtime(&envelope.context, runtime);
            let event = self.events.publish(EventDraft::output_route_changed(
                &envelope.context,
                change.clone(),
            ));
            Ok(MutateOutputRouteResult {
                context: envelope.context.clone(),
                change,
                route_to_terminate: prepared.route_to_terminate,
                event_sequence: event.sequence,
            })
        })
    }

    pub fn mutate_objects<P: ActiveShowPorts>(
        &self,
        envelope: ActionEnvelope<MutateActiveShowObjectsCommand>,
        ports: &P,
    ) -> Result<MutateActiveShowObjectsResult, ActionError> {
        ports.authorize_mutation(&envelope.context)?;
        ports.run_active_show_lifecycle(&envelope.context, envelope.command.show_id, || {
            let _ordered = self.operation.lock();
            let unit = ports.begin_active_show(&envelope.context, envelope.command.show_id)?;
            let prepared = prepare_object_mutation(unit.document(), &envelope.command)?;
            let committed = self.commit_object_changes(
                &envelope.context,
                envelope.command.show_id,
                unit,
                ports,
                prepared,
                "show-object",
            )?;
            Ok(MutateActiveShowObjectsResult {
                context: envelope.context.clone(),
                show_revision: committed.show_revision,
                changes: committed.changes,
                event_sequence: committed.event_sequence,
            })
        })
    }

    pub fn undo_object<P: ActiveShowPorts>(
        &self,
        envelope: ActionEnvelope<UndoActiveShowObjectCommand>,
        ports: &P,
    ) -> Result<UndoActiveShowObjectResult, ActionError> {
        ports.authorize_mutation(&envelope.context)?;
        ports.run_active_show_lifecycle(&envelope.context, envelope.command.show_id, || {
            let _ordered = self.operation.lock();
            let unit = ports.begin_active_show(&envelope.context, envelope.command.show_id)?;
            let prepared = prepare_requested_undo(ports, &unit, &envelope.command)?;
            let committed = self.commit_object_changes(
                &envelope.context,
                envelope.command.show_id,
                unit,
                ports,
                prepared,
                "undo-show-object",
            )?;
            Ok(UndoActiveShowObjectResult {
                context: envelope.context.clone(),
                show_revision: committed.show_revision,
                change: single_change(committed.changes),
                event_sequence: committed.event_sequence,
            })
        })
    }

    fn commit_object_changes<P: ActiveShowPorts>(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        mut unit: P::UnitOfWork,
        ports: &P,
        prepared: PreparedObjectChanges,
        operation: &str,
    ) -> Result<CommittedObjectChanges, ActionError> {
        let runtime = ports.prepare_runtime(prepared.snapshot)?;
        unit.backup(&backup_identity(context, show_id, operation))?;
        let show_revision = unit.commit(prepared.transaction)?.revision();
        ports.install_runtime(context, runtime);
        ports.reconcile_object_changes(&prepared.changes);
        Ok(self.publish_object_changes(context, show_id, show_revision, prepared.changes))
    }

    fn publish_object_changes(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        show_revision: PortableShowRevision,
        changes: Vec<super::ActiveShowObjectChange>,
    ) -> CommittedObjectChanges {
        let event = self.events.publish(EventDraft::active_show_objects_changed(
            context,
            ActiveShowObjectsChange {
                show_id,
                show_revision,
                changes: changes.clone(),
            },
        ));
        CommittedObjectChanges {
            show_revision,
            changes,
            event_sequence: event.sequence,
        }
    }

    pub fn events(&self) -> &EventBus {
        &self.events
    }

    /// Clones one coherent active-show document while sharing the ordering gate used by every
    /// application-owned show mutation.
    ///
    /// The gate is released before this method returns. Callers may therefore perform expensive
    /// planning or adapter reads against the immutable snapshot without blocking unrelated show
    /// mutations. Any later transaction must still validate the snapshot revision while holding
    /// the gate.
    pub(crate) fn snapshot<P>(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        ports: &P,
    ) -> Result<PortableShowDocument, ActionError>
    where
        P: ActiveShowPorts,
    {
        self.snapshot_with_event_sequence(context, show_id, ports)
            .map(|(document, _)| document)
    }

    /// Captures a coherent document and application-event cursor while the mutation gate is held.
    /// Every application-owned show event is published before that gate is released, so callers
    /// can safely start replay strictly after the returned cursor without missing a committed
    /// change represented by the document.
    pub(crate) fn snapshot_with_event_sequence<P>(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        ports: &P,
    ) -> Result<(PortableShowDocument, u64), ActionError>
    where
        P: ActiveShowPorts,
    {
        ports.authorize_mutation(context)?;
        ports.run_active_show_lifecycle(context, show_id, || {
            let _ordered = self.operation.lock();
            let unit = ports.begin_active_show(context, show_id)?;
            Ok((unit.document().clone(), self.events.latest_sequence()))
        })
    }

    /// Commits a capability-specific transaction through the same ordered backup, candidate,
    /// persistence, and runtime-install lifecycle as the built-in active-show commands.
    ///
    /// `complete` is deliberately infallible and executes while the ordering gate is still held,
    /// so targeted reconciliation and event publication cannot be reordered behind a later show
    /// mutation.
    pub(crate) fn transact<P, T, R>(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        ports: &P,
        operation: &str,
        prepare: impl FnOnce(
            &PortableShowDocument,
        ) -> Result<PreparedActiveShowTransaction<T>, ActionError>,
        complete: impl FnOnce(&EventBus, &P, &ActionContext, CompletedActiveShowTransaction<T>) -> R,
    ) -> Result<R, ActionError>
    where
        P: ActiveShowPorts,
    {
        ports.authorize_mutation(context)?;
        ports.run_active_show_lifecycle(context, show_id, || {
            let _ordered = self.operation.lock();
            let mut unit = ports.begin_active_show(context, show_id)?;
            match prepare(unit.document())? {
                PreparedActiveShowTransaction::NoChange(state) => Ok(complete(
                    &self.events,
                    ports,
                    context,
                    CompletedActiveShowTransaction {
                        state,
                        commit: None,
                    },
                )),
                PreparedActiveShowTransaction::PreparedCommit { prepared, state } => {
                    let (transaction, snapshot) = (*prepared).into_parts();
                    let runtime = ports.prepare_runtime(snapshot)?;
                    unit.backup(&backup_identity(context, show_id, operation))?;
                    let commit = unit.commit(transaction)?;
                    ports.install_runtime(context, runtime);
                    Ok(complete(
                        &self.events,
                        ports,
                        context,
                        CompletedActiveShowTransaction {
                            state,
                            commit: Some(commit),
                        },
                    ))
                }
            }
        })
    }
}

pub(crate) enum PreparedActiveShowTransaction<T> {
    NoChange(T),
    /// A capability-prepared candidate whose fully migrated transaction has already passed
    /// capability-specific scope validation. The shared service still exclusively owns runtime
    /// preparation, backup, persistence, installation, and completion ordering.
    PreparedCommit {
        prepared: Box<crate::PreparedShowCandidate>,
        state: T,
    },
}

pub(crate) struct CompletedActiveShowTransaction<T> {
    pub state: T,
    pub commit: Option<PortableShowCommit>,
}

struct CommittedObjectChanges {
    show_revision: PortableShowRevision,
    changes: Vec<super::ActiveShowObjectChange>,
    event_sequence: u64,
}

fn prepare_requested_undo<P: ActiveShowPorts>(
    ports: &P,
    unit: &P::UnitOfWork,
    command: &UndoActiveShowObjectCommand,
) -> Result<PreparedObjectChanges, ActionError> {
    validate_object_undo(unit.document(), command)?;
    let undo = ports.prepare_object_undo(
        unit,
        command.kind.as_str(),
        &command.object_id,
        command.expected_object_revision,
    )?;
    prepare_object_undo(unit.document(), command, undo)
}

fn single_change(mut changes: Vec<super::ActiveShowObjectChange>) -> super::ActiveShowObjectChange {
    debug_assert_eq!(changes.len(), 1, "one Undo returns one object change");
    changes.pop().expect("one Undo returns one object change")
}

impl Default for ActiveShowService {
    fn default() -> Self {
        Self::new(EventBus::default())
    }
}

fn backup_identity(
    context: &ActionContext,
    show_id: light_core::ShowId,
    operation: &str,
) -> BackupIdentity {
    BackupIdentity {
        show_id,
        correlation_id: context.correlation_id,
        request_id: context
            .request_id
            .clone()
            .unwrap_or_else(|| format!("{operation}-{}", context.correlation_id)),
    }
}
