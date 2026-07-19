use super::prepare::{PreparedMutation, PreparedPatch, prepare_patch};
use super::query::build_snapshot;
use super::replay::{ReplayCache, ReplayKey};
use super::validation::validate_action;
use super::{
    ActiveShowUnitOfWork, BackupIdentity, PatchFixturesCommand, PatchFixturesResult, PatchSnapshot,
    ShowPatchPorts,
};
use crate::{ActionEnvelope, ActionError, ActionErrorKind, EventBus, EventDraft};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone)]
pub struct ShowPatchService {
    operation: Arc<Mutex<()>>,
    replay: Arc<Mutex<ReplayCache>>,
    events: EventBus,
}

impl ShowPatchService {
    pub fn new(events: EventBus) -> Self {
        Self {
            operation: Arc::new(Mutex::new(())),
            replay: Arc::new(Mutex::new(ReplayCache::default())),
            events,
        }
    }

    pub fn events(&self) -> &EventBus {
        &self.events
    }

    pub fn snapshot<P: ShowPatchPorts>(
        &self,
        context: &crate::ActionContext,
        show_id: light_core::ShowId,
        ports: &P,
    ) -> Result<PatchSnapshot, ActionError> {
        ports.authorize_patch_read(context)?;
        let _ordered = self.operation.lock();
        let unit = ports.begin_active_show(context, show_id)?;
        validate_active_show_id(unit.document(), show_id)?;
        let mut snapshot = build_snapshot(unit.document())?;
        snapshot.event_sequence = self.events.latest_sequence();
        Ok(snapshot)
    }

    pub fn handle<P: ShowPatchPorts>(
        &self,
        envelope: ActionEnvelope<PatchFixturesCommand>,
        ports: &P,
    ) -> Result<PatchFixturesResult, ActionError> {
        ports.authorize_patch(&envelope.context)?;
        validate_action(&envelope.context, &envelope.command)?;
        let key = required_replay_key(&envelope)?;
        if let Some(result) = self.cached(&key, &envelope)? {
            return Ok(result);
        }
        let _ordered = self.operation.lock();
        if let Some(result) = self.cached(&key, &envelope)? {
            return Ok(result);
        }
        self.apply(key, envelope, ports)
    }

    fn apply<P: ShowPatchPorts>(
        &self,
        key: ReplayKey,
        envelope: ActionEnvelope<PatchFixturesCommand>,
        ports: &P,
    ) -> Result<PatchFixturesResult, ActionError> {
        let mut unit = ports.begin_active_show(&envelope.context, envelope.command.show_id)?;
        validate_active_document(unit.document(), &envelope)?;
        let prepared = prepare_patch(unit.document(), &envelope.command, ports)?;
        match prepared {
            PreparedPatch::Noop(change) => Ok(self.finish_noop(key, envelope, change)),
            PreparedPatch::Mutation(prepared) => {
                unit.backup(&backup_identity(&envelope, &key))?;
                self.commit_install_publish(key, envelope, unit, prepared, ports)
            }
        }
    }

    fn commit_install_publish<P: ShowPatchPorts>(
        &self,
        key: ReplayKey,
        envelope: ActionEnvelope<PatchFixturesCommand>,
        unit: P::UnitOfWork,
        prepared: PreparedMutation<P::PreparedRuntime>,
        ports: &P,
    ) -> Result<PatchFixturesResult, ActionError> {
        let commit = unit.commit(prepared.transaction)?;
        let mut change = prepared.change;
        change.show_revision = commit.revision();
        change.patch_revision = commit.patch_revision();
        ports.install_runtime(prepared.runtime);
        ports.reconcile_patch_change(&change);
        let event = self
            .events
            .publish(EventDraft::patch_changed(&envelope.context, change.clone()));
        let result = committed_result(&envelope, key.request_id(), change, event.sequence);
        self.remember(key, &envelope.context, envelope.command, &result);
        Ok(result)
    }

    fn finish_noop(
        &self,
        key: ReplayKey,
        envelope: ActionEnvelope<PatchFixturesCommand>,
        change: super::PatchChange,
    ) -> PatchFixturesResult {
        let result = PatchFixturesResult {
            context: envelope.context.clone(),
            request_id: key.request_id().to_owned(),
            replayed: false,
            changed: false,
            change,
            event_sequence: None,
        };
        self.remember(key, &envelope.context, envelope.command, &result);
        result
    }

    fn cached(
        &self,
        key: &ReplayKey,
        envelope: &ActionEnvelope<PatchFixturesCommand>,
    ) -> Result<Option<PatchFixturesResult>, ActionError> {
        self.replay
            .lock()
            .get(key, &envelope.context, &envelope.command)
    }

    fn remember(
        &self,
        key: ReplayKey,
        context: &crate::ActionContext,
        command: PatchFixturesCommand,
        result: &PatchFixturesResult,
    ) {
        self.replay
            .lock()
            .insert(key, context, command, result.clone());
    }
}

impl Default for ShowPatchService {
    fn default() -> Self {
        Self::new(EventBus::default())
    }
}

fn required_replay_key(
    envelope: &ActionEnvelope<PatchFixturesCommand>,
) -> Result<ReplayKey, ActionError> {
    ReplayKey::from_context(&envelope.context)
        .ok_or_else(|| ActionError::new(ActionErrorKind::Invalid, "patch request_id is required"))
}

fn validate_active_document(
    document: &light_show::PortableShowDocument,
    envelope: &ActionEnvelope<PatchFixturesCommand>,
) -> Result<(), ActionError> {
    validate_active_show_id(document, envelope.command.show_id)?;
    let Some(expected) = envelope.context.expected_revision else {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "patch operation requires a whole-show expected revision",
        ));
    };
    if document.revision().value() == expected {
        Ok(())
    } else {
        Err(
            ActionError::new(ActionErrorKind::Conflict, "stale show revision")
                .at_revision(document.revision().value()),
        )
    }
}

fn validate_active_show_id(
    document: &light_show::PortableShowDocument,
    show_id: light_core::ShowId,
) -> Result<(), ActionError> {
    if document.id() == show_id {
        Ok(())
    } else {
        Err(ActionError::new(
            ActionErrorKind::NotFound,
            "requested show is not active",
        ))
    }
}

fn backup_identity(
    envelope: &ActionEnvelope<PatchFixturesCommand>,
    key: &ReplayKey,
) -> BackupIdentity {
    BackupIdentity {
        show_id: envelope.command.show_id,
        correlation_id: envelope.context.correlation_id,
        request_id: key.request_id().to_owned(),
    }
}

fn committed_result(
    envelope: &ActionEnvelope<PatchFixturesCommand>,
    request_id: &str,
    change: super::PatchChange,
    event_sequence: u64,
) -> PatchFixturesResult {
    PatchFixturesResult {
        context: envelope.context.clone(),
        request_id: request_id.to_owned(),
        replayed: false,
        changed: true,
        change,
        event_sequence: Some(event_sequence),
    }
}
