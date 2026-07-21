use std::collections::{HashMap, VecDeque};

use uuid::Uuid;

use crate::{ActionEnvelope, ActionError, ActionErrorKind};

use super::{SpeedGroupCommand, SpeedGroupExpectation, SpeedGroupResult};

const REQUEST_CACHE_LIMIT: usize = 4_096;

pub(super) struct AuthorityState {
    pub id: Uuid,
    pub revision: u64,
    replay: ReplayCache,
}

impl AuthorityState {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            revision: 0,
            replay: ReplayCache::default(),
        }
    }

    pub fn next_revision(&self) -> Result<u64, ActionError> {
        self.revision.checked_add(1).ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Unavailable,
                "Speed Group runtime revision is exhausted",
            )
            .at_revision(self.revision)
        })
    }

    pub fn validate_expectation(
        &self,
        expectation: SpeedGroupExpectation,
    ) -> Result<(), ActionError> {
        let SpeedGroupExpectation::Exact {
            authority_id,
            revision,
        } = expectation
        else {
            return Ok(());
        };
        if self.id != authority_id {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "Speed Group authority was replaced",
            )
            .at_revision(self.revision));
        }
        if self.revision != revision {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "Speed Group runtime revision conflict",
            )
            .at_revision(self.revision));
        }
        Ok(())
    }

    pub fn cached(
        &self,
        envelope: &ActionEnvelope<SpeedGroupCommand>,
    ) -> Result<Option<SpeedGroupResult>, ActionError> {
        let Some(key) = ReplayKey::from_envelope(envelope, self.id) else {
            return Ok(None);
        };
        self.replay.get(&key, envelope.command, self.revision)
    }

    pub fn remember(
        &mut self,
        envelope: &ActionEnvelope<SpeedGroupCommand>,
        result: SpeedGroupResult,
    ) {
        let Some(key) = ReplayKey::from_envelope(envelope, self.id) else {
            return;
        };
        self.replay.insert(key, envelope.command, result);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    desk_id: Uuid,
    session_id: Option<Uuid>,
    request_id: String,
    authority_id: Uuid,
}

impl ReplayKey {
    fn from_envelope(
        envelope: &ActionEnvelope<SpeedGroupCommand>,
        authority_id: Uuid,
    ) -> Option<Self> {
        Some(Self {
            desk_id: envelope.context.desk_id,
            session_id: envelope.context.session_id,
            request_id: envelope.context.request_id.clone()?,
            authority_id,
        })
    }
}

#[derive(Clone)]
struct ReplayEntry {
    command: SpeedGroupCommand,
    result: SpeedGroupResult,
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
        command: SpeedGroupCommand,
        current_revision: u64,
    ) -> Result<Option<SpeedGroupResult>, ActionError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if entry.command != command {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different Speed Group operation",
            )
            .at_revision(current_revision));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    fn insert(&mut self, key: ReplayKey, command: SpeedGroupCommand, result: SpeedGroupResult) {
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
