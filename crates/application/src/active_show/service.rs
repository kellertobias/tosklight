use super::{
    ActiveShowPorts, ActiveShowUnitOfWork, BackupIdentity, MutateActiveShowObjectsCommand,
    MutateActiveShowObjectsResult, MutateOutputRouteCommand, MutateOutputRouteResult,
    OutputRouteChange,
};
use super::{objects::prepare_object_mutation, route::prepare_route_mutation};
use crate::{ActionContext, ActionEnvelope, ActionError, EventBus, EventDraft};
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
        ports.install_runtime(runtime);
        let event = self.events.publish(EventDraft::output_route_changed(
            &envelope.context,
            change.clone(),
        ));
        Ok(MutateOutputRouteResult {
            context: envelope.context,
            change,
            route_to_terminate: prepared.route_to_terminate,
            event_sequence: event.sequence,
        })
    }

    pub fn mutate_objects<P: ActiveShowPorts>(
        &self,
        envelope: ActionEnvelope<MutateActiveShowObjectsCommand>,
        ports: &P,
    ) -> Result<MutateActiveShowObjectsResult, ActionError> {
        ports.authorize_mutation(&envelope.context)?;
        let _ordered = self.operation.lock();
        let mut unit = ports.begin_active_show(&envelope.context, envelope.command.show_id)?;
        let prepared = prepare_object_mutation(unit.document(), &envelope.command)?;
        let runtime = ports.prepare_runtime(prepared.snapshot)?;
        unit.backup(&backup_identity(
            &envelope.context,
            envelope.command.show_id,
            "show-object",
        ))?;
        let commit = unit.commit(prepared.transaction)?;
        ports.install_runtime(runtime);
        ports.reconcile_object_changes(&prepared.changes);
        Ok(MutateActiveShowObjectsResult {
            context: envelope.context,
            show_revision: commit.revision(),
            changes: prepared.changes,
        })
    }

    pub fn events(&self) -> &EventBus {
        &self.events
    }
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
