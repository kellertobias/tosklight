use super::ProgrammingService;
use crate::programming::update::{ProgrammingUpdateCommand, ProgrammingUpdateResult};
use crate::{ActionError, ActionErrorKind};
use light_core::{SessionId, UserId};
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;

const ENTRY_LIMIT: usize = 1_024;
const BYTE_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    user_id: UserId,
    desk_id: uuid::Uuid,
    session_id: SessionId,
    request_id: String,
}

struct ReplayEntry {
    request: ProgrammingUpdateCommand,
    result: ProgrammingUpdateResult,
    retained_bytes: usize,
}

#[derive(Default)]
pub(super) struct UpdateReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
}

impl UpdateReplayCache {
    pub(super) fn get(
        &self,
        user_id: UserId,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: &str,
        request: &ProgrammingUpdateCommand,
    ) -> Result<Option<ProgrammingUpdateResult>, ActionError> {
        let key = ReplayKey {
            user_id,
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
                "request_id was already used for a different Programming Update action",
            ));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    pub(super) fn insert(
        &mut self,
        user_id: UserId,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: String,
        request: ProgrammingUpdateCommand,
        result: ProgrammingUpdateResult,
    ) {
        let key = ReplayKey {
            user_id,
            desk_id,
            session_id,
            request_id,
        };
        let retained_bytes = retained_bytes(&key, &request, &result);
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        if let Some(previous) = self.entries.insert(
            key,
            ReplayEntry {
                request,
                result,
                retained_bytes,
            },
        ) {
            self.retained_bytes = self.retained_bytes.saturating_sub(previous.retained_bytes);
        }
        self.retained_bytes = self.retained_bytes.saturating_add(retained_bytes);
        self.truncate();
    }

    pub(super) fn invalidate_user(&mut self, user_id: UserId) {
        self.entries.retain(|key, _| key.user_id != user_id);
        self.order.retain(|key| key.user_id != user_id);
        self.retained_bytes = self
            .entries
            .values()
            .map(|entry| entry.retained_bytes)
            .sum();
    }

    fn truncate(&mut self) {
        while self.entries.len() > ENTRY_LIMIT || self.retained_bytes > BYTE_LIMIT {
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
    pub(in crate::programming) fn invalidate_update_replay(&self, user_id: UserId) {
        self.update_replay.lock().invalidate_user(user_id);
    }
}

fn retained_bytes(
    key: &ReplayKey,
    request: &ProgrammingUpdateCommand,
    result: &ProgrammingUpdateResult,
) -> usize {
    size_of::<ReplayKey>()
        .saturating_add(size_of::<ReplayEntry>())
        .saturating_add(key.request_id.capacity())
        .saturating_add(
            request
                .expected_programmer_revision
                .as_ref()
                .map_or(0, String::capacity),
        )
        .saturating_add(result.request_id.capacity())
        .saturating_add(json_bytes(&result.outcome.projection.raw_body))
}

fn json_bytes(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            size_of::<serde_json::Value>()
        }
        serde_json::Value::String(value) => value.capacity(),
        serde_json::Value::Array(values) => values
            .iter()
            .map(json_bytes)
            .sum::<usize>()
            .saturating_add(values.capacity() * size_of::<serde_json::Value>()),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| key.capacity().saturating_add(json_bytes(value)))
            .sum(),
    }
}
