use crate::{ActionError, ActionErrorKind, ProgrammingPriorityRequest, ProgrammingPriorityResult};
use light_core::SessionId;
use std::collections::{HashMap, VecDeque};

const ENTRY_LIMIT: usize = 2_048;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    desk_id: uuid::Uuid,
    session_id: SessionId,
    request_id: String,
}

struct ReplayEntry {
    request: ProgrammingPriorityRequest,
    result: ProgrammingPriorityResult,
}

#[derive(Default)]
pub(super) struct PriorityReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl PriorityReplayCache {
    pub(super) fn invalidate(&mut self) {
        // One desk, one Programmer: an invalidation clears the cache rather than one user's part.
        self.entries.clear();
        self.order.clear();
    }

    pub(super) fn get(
        &self,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: &str,
        request: &ProgrammingPriorityRequest,
    ) -> Result<Option<ProgrammingPriorityResult>, ActionError> {
        let key = ReplayKey {
            desk_id,
            session_id,
            request_id: request_id.to_owned(),
        };
        let Some(entry) = self.entries.get(&key) else {
            return Ok(None);
        };
        if entry.request != *request {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different Programmer priority action",
            ));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    pub(super) fn insert(
        &mut self,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: String,
        request: ProgrammingPriorityRequest,
        result: ProgrammingPriorityResult,
    ) {
        let key = ReplayKey {
            desk_id,
            session_id,
            request_id,
        };
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { request, result });
        while self.entries.len() > ENTRY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

impl super::ProgrammingService {
    pub(in crate::programming) fn invalidate_priority_replay(&self) {
        self.priority_replay.lock().invalidate();
    }
}
