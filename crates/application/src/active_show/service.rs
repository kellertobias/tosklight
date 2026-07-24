use super::{
    ActiveShowObjectsChange, ActiveShowPorts, ActiveShowUnitOfWork, BackupIdentity,
    MutateActiveShowObjectsCommand, MutateActiveShowObjectsResult, MutateOutputRouteCommand,
    MutateOutputRouteResult, OutputRouteChange, UndoActiveShowObjectCommand,
    UndoActiveShowObjectResult, UndoActiveShowRecordingCommand, UndoActiveShowRecordingOperation,
    objects::{PreparedObjectChanges, prepare_object_mutation},
    route::prepare_route_mutation,
    undo::{prepare_object_undo, prepare_recording_undo, validate_object_undo},
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
            let migration_changes = migration_changes(&commit, &[]);
            let migrated_routes =
                migrated_route_changes(envelope.command.show_id, &commit, Some(&change.route_id));
            ports.install_runtime(&envelope.context, runtime);
            let event = self.events.publish(EventDraft::output_route_changed(
                &envelope.context,
                change.clone(),
            ));
            self.publish_migration_riders(
                &envelope.context,
                envelope.command.show_id,
                commit.revision(),
                &migration_changes,
                &migrated_routes,
            );
            Ok(MutateOutputRouteResult {
                context: envelope.context.clone(),
                change,
                migration_changes,
                migrated_routes,
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
                migration_changes: committed.migration_changes,
                migrated_routes: committed.migrated_routes,
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
                migration_changes: committed.migration_changes,
                migrated_routes: committed.migrated_routes,
                event_sequence: committed.event_sequence,
            })
        })
    }

    pub fn undo_recording<P: ActiveShowPorts>(
        &self,
        envelope: ActionEnvelope<UndoActiveShowRecordingCommand>,
        ports: &P,
    ) -> Result<MutateActiveShowObjectsResult, ActionError> {
        ports.authorize_mutation(&envelope.context)?;
        ports.run_active_show_lifecycle(&envelope.context, envelope.command.show_id, || {
            let _ordered = self.operation.lock();
            let unit = ports.begin_active_show(&envelope.context, envelope.command.show_id)?;
            let undoes = envelope
                .command
                .objects
                .iter()
                .filter(|object| {
                    matches!(
                        object.operation,
                        UndoActiveShowRecordingOperation::RestorePrevious
                    )
                })
                .map(|object| {
                    ports.prepare_object_undo(
                        &unit,
                        object.kind.as_str(),
                        &object.object_id,
                        object.expected_object_revision,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let prepared = prepare_recording_undo(unit.document(), &envelope.command, undoes)?;
            let committed = self.commit_object_changes(
                &envelope.context,
                envelope.command.show_id,
                unit,
                ports,
                prepared,
                "undo-show-recording",
            )?;
            Ok(MutateActiveShowObjectsResult {
                context: envelope.context.clone(),
                show_revision: committed.show_revision,
                changes: committed.changes,
                migration_changes: committed.migration_changes,
                migrated_routes: committed.migrated_routes,
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
        let commit = unit.commit(prepared.transaction)?;
        let show_revision = commit.revision();
        let migration_changes = migration_changes(&commit, &prepared.changes);
        let migrated_routes = migrated_route_changes(show_id, &commit, None);
        ports.install_runtime(context, runtime);
        ports.reconcile_object_changes(&prepared.changes);
        let committed = self.publish_object_changes(
            context,
            show_id,
            show_revision,
            prepared.changes,
            migration_changes,
        );
        for migrated in &migrated_routes {
            self.events
                .publish(EventDraft::output_route_changed(context, migrated.clone()));
        }
        Ok(CommittedObjectChanges {
            migrated_routes,
            ..committed
        })
    }

    /// Publishes events for migration write-backs that rode along a route mutation's commit.
    fn publish_migration_riders(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        show_revision: PortableShowRevision,
        migration_changes: &[super::ActiveShowObjectChange],
        migrated_routes: &[OutputRouteChange],
    ) {
        if !migration_changes.is_empty() {
            self.events.publish(EventDraft::active_show_objects_changed(
                context,
                ActiveShowObjectsChange {
                    show_id,
                    show_revision,
                    changes: migration_changes.to_vec(),
                },
            ));
        }
        for migrated in migrated_routes {
            self.events
                .publish(EventDraft::output_route_changed(context, migrated.clone()));
        }
    }

    fn publish_object_changes(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        show_revision: PortableShowRevision,
        changes: Vec<super::ActiveShowObjectChange>,
        migration_changes: Vec<super::ActiveShowObjectChange>,
    ) -> CommittedObjectChanges {
        let mut published = changes.clone();
        published.extend(migration_changes.iter().cloned());
        let event = self.events.publish(EventDraft::active_show_objects_changed(
            context,
            ActiveShowObjectsChange {
                show_id,
                show_revision,
                changes: published,
            },
        ));
        CommittedObjectChanges {
            show_revision,
            changes,
            migration_changes,
            migrated_routes: Vec::new(),
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
        self.transact_with_unit(
            context,
            show_id,
            ports,
            operation,
            |unit| prepare(unit.document()),
            complete,
        )
    }

    /// Same ordered lifecycle as [`Self::transact`], but `prepare` observes the open unit of work
    /// so a capability can read adapter-owned history, such as object undo, inside the one
    /// transaction that later commits it.
    pub(crate) fn transact_with_unit<P, T, R>(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        ports: &P,
        operation: &str,
        prepare: impl FnOnce(&P::UnitOfWork) -> Result<PreparedActiveShowTransaction<T>, ActionError>,
        complete: impl FnOnce(&EventBus, &P, &ActionContext, CompletedActiveShowTransaction<T>) -> R,
    ) -> Result<R, ActionError>
    where
        P: ActiveShowPorts,
    {
        ports.authorize_mutation(context)?;
        ports.run_active_show_lifecycle(context, show_id, || {
            let _ordered = self.operation.lock();
            let mut unit = ports.begin_active_show(context, show_id)?;
            match prepare(&unit)? {
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
    migration_changes: Vec<super::ActiveShowObjectChange>,
    migrated_routes: Vec<OutputRouteChange>,
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

/// Route writes committed by staged compatibility migrations rather than the request itself.
/// Routes have their own change/event family, so they are reported as `OutputRouteChange`s.
fn migrated_route_changes(
    show_id: ShowId,
    commit: &PortableShowCommit,
    exclude_route_id: Option<&str>,
) -> Vec<OutputRouteChange> {
    commit
        .written_objects()
        .iter()
        .filter(|object| object.key().kind() == "route")
        .filter(|object| exclude_route_id != Some(object.key().id()))
        .map(|object| OutputRouteChange {
            show_id,
            show_revision: commit.revision(),
            route_id: object.key().id().to_string(),
            object_revision: object.revision(),
            route: serde_json::from_value(object.body().clone()).ok(),
            deleted: false,
        })
        .collect()
}

/// Object writes committed by staged compatibility migrations rather than the request itself.
/// Reporting them keeps every persisted revision bump observable instead of silent.
fn migration_changes(
    commit: &PortableShowCommit,
    requested: &[super::ActiveShowObjectChange],
) -> Vec<super::ActiveShowObjectChange> {
    commit
        .written_objects()
        .iter()
        .filter_map(|object| {
            let kind = super::ActiveShowObjectKind::from_storage_kind(object.key().kind())?;
            let object_id = object.key().id();
            if requested
                .iter()
                .any(|change| change.kind == kind && change.object_id == object_id)
            {
                return None;
            }
            Some(super::ActiveShowObjectChange {
                kind,
                object_id: object_id.to_string(),
                object_revision: object.revision(),
                body: Some(object.body().clone()),
                deleted: false,
            })
        })
        .collect()
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
