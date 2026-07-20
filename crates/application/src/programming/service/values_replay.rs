use super::super::{ProgrammingValuesRequest, ProgrammingValuesResult};
use super::ProgrammingService;
use crate::{ActionError, ActionErrorKind};
use light_core::{SessionId, UserId};
use std::collections::{HashMap, VecDeque};

const REQUEST_CACHE_LIMIT: usize = 4_096;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    user_id: UserId,
    desk_id: uuid::Uuid,
    session_id: SessionId,
    request_id: String,
}

struct ReplayEntry {
    expected_revision: u64,
    request: ProgrammingValuesRequest,
    result: ProgrammingValuesResult,
}

#[derive(Default)]
pub(super) struct ValuesReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl ValuesReplayCache {
    pub(super) fn invalidate_user(&mut self, user_id: UserId) {
        self.entries.retain(|key, _| key.user_id != user_id);
        self.order.retain(|key| key.user_id != user_id);
    }

    pub(super) fn get(
        &self,
        user_id: UserId,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: &str,
        expected_revision: u64,
        request: &ProgrammingValuesRequest,
    ) -> Result<Option<ProgrammingValuesResult>, ActionError> {
        let key = ReplayKey {
            user_id,
            desk_id,
            session_id,
            request_id: request_id.to_owned(),
        };
        let Some(entry) = self.entries.get(&key) else {
            return Ok(None);
        };
        if entry.expected_revision != expected_revision || entry.request != *request {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different Programmer values action",
            ));
        }
        let mut replayed = entry.result.clone();
        replayed.replayed = true;
        Ok(Some(replayed))
    }

    pub(super) fn insert(
        &mut self,
        user_id: UserId,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: String,
        expected_revision: u64,
        request: ProgrammingValuesRequest,
        result: ProgrammingValuesResult,
    ) {
        let key = ReplayKey {
            user_id,
            desk_id,
            session_id,
            request_id,
        };
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(
            key,
            ReplayEntry {
                expected_revision,
                request,
                result,
            },
        );
        while self.entries.len() > REQUEST_CACHE_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

impl ProgrammingService {
    pub(in crate::programming) fn invalidate_values_replay(&self, user_id: UserId) {
        self.values_replay.lock().invalidate_user(user_id);
    }
}
