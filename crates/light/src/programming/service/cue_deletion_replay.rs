use super::ProgrammingService;
use crate::{
    ActionError, ActionErrorKind, ProgrammingCueDeletionRequest, ProgrammingCueDeletionResult,
};
use light_core::{SessionId, UserId};
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;
use uuid::Uuid;

const ENTRY_LIMIT: usize = 1_024;
const BYTE_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct CueDeletionScope {
    pub user_id: UserId,
    pub desk_id: Uuid,
    pub session_id: SessionId,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    scope: CueDeletionScope,
    request_id: String,
}

struct ReplayEntry {
    request: ProgrammingCueDeletionRequest,
    expected_show_revision: Option<u64>,
    result: ProgrammingCueDeletionResult,
    retained_bytes: usize,
}

#[derive(Default)]
pub(super) struct CueDeletionReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
}

impl CueDeletionReplayCache {
    pub fn get(
        &self,
        scope: &CueDeletionScope,
        request_id: &str,
        request: &ProgrammingCueDeletionRequest,
        expected_show_revision: Option<u64>,
    ) -> Result<Option<ProgrammingCueDeletionResult>, ActionError> {
        let key = ReplayKey {
            scope: scope.clone(),
            request_id: request_id.to_owned(),
        };
        let Some(entry) = self.entries.get(&key) else {
            return Ok(None);
        };
        if entry.request != *request || entry.expected_show_revision != expected_show_revision {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different Cue deletion action",
            ));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    pub fn insert(
        &mut self,
        scope: CueDeletionScope,
        request_id: String,
        request: ProgrammingCueDeletionRequest,
        expected_show_revision: Option<u64>,
        result: ProgrammingCueDeletionResult,
    ) {
        let key = ReplayKey { scope, request_id };
        let retained_bytes = retained_bytes(&key, &result);
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        if let Some(previous) = self.entries.insert(
            key,
            ReplayEntry {
                request,
                expected_show_revision,
                result,
                retained_bytes,
            },
        ) {
            self.retained_bytes = self.retained_bytes.saturating_sub(previous.retained_bytes);
        }
        self.retained_bytes = self.retained_bytes.saturating_add(retained_bytes);
        self.truncate();
    }

    fn invalidate_user(&mut self, user_id: UserId) {
        self.entries.retain(|key, _| key.scope.user_id != user_id);
        self.order.retain(|key| key.scope.user_id != user_id);
        self.retained_bytes = self
            .entries
            .values()
            .map(|entry| entry.retained_bytes)
            .sum();
    }

    fn truncate(&mut self) {
        while self.entries.len() > ENTRY_LIMIT
            || (self.retained_bytes > BYTE_LIMIT && self.entries.len() > 1)
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.retained_bytes = self.retained_bytes.saturating_sub(entry.retained_bytes);
            }
        }
    }
}

impl ProgrammingService {
    pub(in crate::programming) fn invalidate_cue_deletion_replay(&self, user_id: UserId) {
        self.cue_deletion_replay.lock().invalidate_user(user_id);
    }
}

fn retained_bytes(key: &ReplayKey, result: &ProgrammingCueDeletionResult) -> usize {
    size_of::<ReplayKey>()
        .saturating_add(size_of::<ReplayEntry>())
        .saturating_add(key.request_id.capacity())
        .saturating_add(result.request_id.capacity())
        .saturating_add(result.outcome.cue_list.object_id.capacity())
        .saturating_add(json_bytes(&result.outcome.cue_list.raw_body))
}

fn json_bytes(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Null => 0,
        serde_json::Value::Bool(_) => 1,
        serde_json::Value::Number(_) => size_of::<serde_json::Number>(),
        serde_json::Value::String(value) => value.capacity(),
        serde_json::Value::Array(values) => values.iter().map(json_bytes).sum(),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| key.capacity().saturating_add(json_bytes(value)))
            .sum(),
    }
}
