use super::{
    PlaybackAddress, PlaybackCommand, PlaybackExecution, PlaybackOutcome, PlaybackPorts,
    PlaybackResult, ResolvedPlaybackAddress,
};
use crate::{ActionEnvelope, ActionError, ActionErrorKind};
use parking_lot::{Mutex, MutexGuard};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use uuid::Uuid;

const REQUEST_CACHE_LIMIT: usize = 4_096;

#[derive(Clone, Default)]
pub struct PlaybackService {
    operation: Arc<Mutex<()>>,
    replay: Arc<Mutex<ReplayCache>>,
}

impl PlaybackService {
    /// Transitional access for page changes and deferred playback work that have not yet become
    /// application commands. Holding this guard orders them with all migrated playback actions.
    pub fn operation_lock(&self) -> MutexGuard<'_, ()> {
        self.operation.lock()
    }

    pub fn handle(
        &self,
        envelope: ActionEnvelope<PlaybackCommand>,
        ports: &dyn PlaybackPorts,
    ) -> Result<PlaybackResult, ActionError> {
        let _ordered = self.operation_lock();
        ports.authorize(&envelope.context)?;
        if let Some(result) = self.cached(&envelope)? {
            return Ok(result);
        }
        let result = self.apply(&envelope, ports)?;
        self.remember(&envelope, &result);
        Ok(result)
    }

    fn apply(
        &self,
        envelope: &ActionEnvelope<PlaybackCommand>,
        ports: &dyn PlaybackPorts,
    ) -> Result<PlaybackResult, ActionError> {
        let resolved = resolve(envelope.command.address, &envelope.context, ports)?;
        let execution = ports.execute(
            &envelope.context,
            resolved,
            envelope.command.action,
            envelope.command.surface,
        )?;
        Ok(PlaybackResult {
            context: envelope.context.clone(),
            requested: envelope.command.address,
            resolved,
            outcome: outcome(&execution),
            execution,
            replayed: false,
        })
    }

    fn cached(
        &self,
        envelope: &ActionEnvelope<PlaybackCommand>,
    ) -> Result<Option<PlaybackResult>, ActionError> {
        let Some(key) = ReplayKey::from_envelope(envelope) else {
            return Ok(None);
        };
        self.replay.lock().get(&key, &envelope.command)
    }

    fn remember(&self, envelope: &ActionEnvelope<PlaybackCommand>, result: &PlaybackResult) {
        let Some(key) = ReplayKey::from_envelope(envelope) else {
            return;
        };
        self.replay
            .lock()
            .insert(key, envelope.command, result.clone());
    }
}

fn resolve(
    address: PlaybackAddress,
    context: &crate::ActionContext,
    ports: &dyn PlaybackPorts,
) -> Result<ResolvedPlaybackAddress, ActionError> {
    match address {
        PlaybackAddress::CueList(id) => Ok(ResolvedPlaybackAddress::CueList(id)),
        PlaybackAddress::Pool(number) => Ok(pool(number, None, None)),
        PlaybackAddress::CurrentPage { slot } => {
            let page = ports.current_page(context)?;
            resolve_page(page, slot, ports)
        }
        PlaybackAddress::ExplicitPage { page, slot } => resolve_page(page, slot, ports),
    }
}

fn resolve_page(
    page: u8,
    slot: u8,
    ports: &dyn PlaybackPorts,
) -> Result<ResolvedPlaybackAddress, ActionError> {
    ports
        .playback_at(page, slot)?
        .map(|number| pool(number, Some(page), Some(slot)))
        .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "paged playback"))
}

const fn pool(number: u16, page: Option<u8>, slot: Option<u8>) -> ResolvedPlaybackAddress {
    ResolvedPlaybackAddress::Pool { number, page, slot }
}

fn outcome(execution: &PlaybackExecution) -> PlaybackOutcome {
    match execution {
        PlaybackExecution::Pool {
            pending: Some(action),
            ..
        } => PlaybackOutcome::Captured(*action),
        PlaybackExecution::Pool { changed: false, .. } | PlaybackExecution::Released(false) => {
            PlaybackOutcome::NoChange
        }
        _ => PlaybackOutcome::Applied,
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    desk_id: Uuid,
    session_id: Option<Uuid>,
    request_id: String,
}

impl ReplayKey {
    fn from_envelope(envelope: &ActionEnvelope<PlaybackCommand>) -> Option<Self> {
        Some(Self {
            desk_id: envelope.context.desk_id,
            session_id: envelope.context.session_id,
            request_id: envelope.context.request_id.clone()?,
        })
    }
}

struct ReplayEntry {
    command: PlaybackCommand,
    result: PlaybackResult,
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
        command: &PlaybackCommand,
    ) -> Result<Option<PlaybackResult>, ActionError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if entry.command != *command {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different playback operation",
            ));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    fn insert(&mut self, key: ReplayKey, command: PlaybackCommand, result: PlaybackResult) {
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
