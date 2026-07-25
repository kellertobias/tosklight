use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use parking_lot::Mutex;
use uuid::Uuid;

use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, EventBus, EventDraft,
    OutputRuntimeChange,
};

use super::{
    OutputRuntimeApplication, OutputRuntimeCommand, OutputRuntimeExpectation,
    OutputRuntimeIdentity, OutputRuntimeOutcome, OutputRuntimePorts, OutputRuntimeProjection,
    OutputRuntimeResult, OutputRuntimeScope, OutputRuntimeSnapshot,
};

const REQUEST_CACHE_LIMIT: usize = 4_096;

#[derive(Clone)]
pub struct OutputRuntimeService {
    operation: Arc<Mutex<()>>,
    replay: Arc<Mutex<ReplayCache>>,
    events: EventBus,
}

impl OutputRuntimeService {
    pub fn new(events: EventBus) -> Self {
        Self {
            operation: Arc::default(),
            replay: Arc::default(),
            events,
        }
    }

    pub const fn events(&self) -> &EventBus {
        &self.events
    }

    pub fn handle(
        &self,
        envelope: ActionEnvelope<OutputRuntimeCommand>,
        ports: &dyn OutputRuntimePorts,
    ) -> Result<OutputRuntimeResult, ActionError> {
        let _ordered = self.operation.lock();
        ports.authorize(&envelope.context)?;
        let before = ports.projection(&envelope.context, OutputRuntimeIdentity::GlobalMaster)?;
        if let Some(result) = self.cached(&envelope, before)? {
            return Ok(result);
        }
        validate_expectation(envelope.command.expectation, before)?;
        let desired = envelope.command.desired(before);
        let result = if desired == before {
            unchanged(&envelope.context, before)
        } else {
            let next_revision = before.revision.checked_add(1).ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::Internal,
                    "output runtime revision exhausted",
                )
                .at_revision(before.revision)
            })?;
            let application = ports.apply(&envelope.context, envelope.command)?;
            let projection =
                ports.projection(&envelope.context, OutputRuntimeIdentity::GlobalMaster)?;
            validate_applied_projection(before, desired, next_revision, projection)?;
            changed(&self.events, &envelope.context, application, projection)
        };
        self.remember(&envelope, before.scope, &result);
        Ok(result)
    }

    /// Active-Show installation replaces the persisted Output authority. Cached request outcomes
    /// cannot cross that installation, including when the same Show ID is reopened after a prior
    /// persistence-pending action.
    pub fn clear_replay(&self) {
        *self.replay.lock() = ReplayCache::default();
    }

    pub fn snapshot(
        &self,
        context: &ActionContext,
        identity: OutputRuntimeIdentity,
        ports: &dyn OutputRuntimePorts,
    ) -> Result<OutputRuntimeSnapshot, ActionError> {
        let _ordered = self.operation.lock();
        ports.authorize(context)?;
        let event_sequence = self.events.latest_sequence();
        let projection = ports.projection(context, identity)?;
        Ok(OutputRuntimeSnapshot {
            event_sequence,
            projection,
        })
    }

    fn cached(
        &self,
        envelope: &ActionEnvelope<OutputRuntimeCommand>,
        projection: OutputRuntimeProjection,
    ) -> Result<Option<OutputRuntimeResult>, ActionError> {
        let Some(key) = ReplayKey::from_envelope(envelope, projection.scope) else {
            return Ok(None);
        };
        self.replay
            .lock()
            .get(&key, envelope.command, projection.revision)
    }

    fn remember(
        &self,
        envelope: &ActionEnvelope<OutputRuntimeCommand>,
        scope: OutputRuntimeScope,
        result: &OutputRuntimeResult,
    ) {
        let Some(key) = ReplayKey::from_envelope(envelope, scope) else {
            return;
        };
        self.replay
            .lock()
            .insert(key, envelope.command, result.clone());
    }
}

fn validate_applied_projection(
    before: OutputRuntimeProjection,
    mut desired: OutputRuntimeProjection,
    next_revision: u64,
    applied: OutputRuntimeProjection,
) -> Result<(), ActionError> {
    if applied.scope != before.scope {
        return Err(ActionError::new(
            ActionErrorKind::Busy,
            "active output runtime changed while the command was applied",
        )
        .at_revision(applied.revision));
    }
    desired.revision = next_revision;
    if applied != desired {
        return Err(ActionError::new(
            ActionErrorKind::Internal,
            "output runtime adapter returned a non-authoritative projection",
        )
        .at_revision(applied.revision));
    }
    Ok(())
}

fn validate_expectation(
    expectation: OutputRuntimeExpectation,
    projection: OutputRuntimeProjection,
) -> Result<(), ActionError> {
    let OutputRuntimeExpectation::Exact { show_id, revision } = expectation else {
        return Ok(());
    };
    if projection.scope.show_id != show_id {
        return Err(ActionError::new(
            ActionErrorKind::Conflict,
            "active Show changed before the output action",
        )
        .at_revision(projection.revision));
    }
    if projection.revision != revision {
        return Err(ActionError::new(
            ActionErrorKind::Conflict,
            "output runtime revision conflict",
        )
        .at_revision(projection.revision));
    }
    Ok(())
}

fn unchanged(
    context: &ActionContext,
    projection: super::OutputRuntimeProjection,
) -> OutputRuntimeResult {
    OutputRuntimeResult {
        context: context.clone(),
        outcome: OutputRuntimeOutcome::NoChange,
        durability: super::OutputRuntimeDurability::Durable,
        warning: None,
        projection,
        event_sequence: None,
        replayed: false,
    }
}

fn changed(
    events: &EventBus,
    context: &ActionContext,
    application: OutputRuntimeApplication,
    projection: super::OutputRuntimeProjection,
) -> OutputRuntimeResult {
    let event = events.publish(EventDraft::output_runtime_changed(
        context,
        OutputRuntimeChange { projection },
    ));
    OutputRuntimeResult {
        context: context.clone(),
        outcome: OutputRuntimeOutcome::Applied,
        durability: application.durability,
        warning: application.warning,
        projection,
        event_sequence: Some(event.sequence),
        replayed: false,
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    desk_id: Uuid,
    session_id: Option<Uuid>,
    request_id: String,
    scope: OutputRuntimeScope,
}

impl ReplayKey {
    fn from_envelope(
        envelope: &ActionEnvelope<OutputRuntimeCommand>,
        scope: OutputRuntimeScope,
    ) -> Option<Self> {
        Some(Self {
            desk_id: envelope.context.desk_id,
            session_id: envelope.context.session_id,
            request_id: envelope.context.request_id.clone()?,
            scope,
        })
    }
}

#[derive(Clone)]
struct ReplayEntry {
    command: OutputRuntimeCommand,
    result: OutputRuntimeResult,
}

#[derive(Default)]
struct ReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl ReplayCache {
    fn get(
        &self,
        key: &ReplayKey,
        command: OutputRuntimeCommand,
        current_revision: u64,
    ) -> Result<Option<OutputRuntimeResult>, ActionError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if entry.command != command {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different output operation",
            )
            .at_revision(current_revision));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    fn insert(
        &mut self,
        key: ReplayKey,
        command: OutputRuntimeCommand,
        result: OutputRuntimeResult,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { command, result });
        while self.entries.len() > REQUEST_CACHE_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}
