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
    OutputRuntimeCommand, OutputRuntimeIdentity, OutputRuntimeOutcome, OutputRuntimePorts,
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
        if let Some(result) = self.cached(&envelope, before.scope)? {
            return Ok(result);
        }
        let desired = envelope.command.desired(before);
        let result = if desired == before {
            unchanged(&envelope.context, before)
        } else {
            let durability = ports.apply(&envelope.context, envelope.command)?;
            let projection =
                ports.projection(&envelope.context, OutputRuntimeIdentity::GlobalMaster)?;
            if projection.scope != before.scope {
                return Err(ActionError::new(
                    ActionErrorKind::Busy,
                    "active output runtime changed while the command was applied",
                ));
            }
            changed(&self.events, &envelope.context, durability, projection)
        };
        self.remember(&envelope, before.scope, &result);
        Ok(result)
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
        scope: OutputRuntimeScope,
    ) -> Result<Option<OutputRuntimeResult>, ActionError> {
        let Some(key) = ReplayKey::from_envelope(envelope, scope) else {
            return Ok(None);
        };
        self.replay.lock().get(&key, envelope.command)
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

fn unchanged(
    context: &ActionContext,
    projection: super::OutputRuntimeProjection,
) -> OutputRuntimeResult {
    OutputRuntimeResult {
        context: context.clone(),
        outcome: OutputRuntimeOutcome::NoChange,
        durability: super::OutputRuntimeDurability::Durable,
        projection,
        event_sequence: None,
        replayed: false,
    }
}

fn changed(
    events: &EventBus,
    context: &ActionContext,
    durability: super::OutputRuntimeDurability,
    projection: super::OutputRuntimeProjection,
) -> OutputRuntimeResult {
    let event = events.publish(EventDraft::output_runtime_changed(
        context,
        OutputRuntimeChange { projection },
    ));
    OutputRuntimeResult {
        context: context.clone(),
        outcome: OutputRuntimeOutcome::Applied,
        durability,
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
    ) -> Result<Option<OutputRuntimeResult>, ActionError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if entry.command != command {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different output operation",
            ));
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
