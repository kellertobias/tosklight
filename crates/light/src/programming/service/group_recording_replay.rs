use crate::{
    ActionError, ActionErrorKind, ProgrammingGroupRecordRequest, ProgrammingGroupRecordResult,
};
use light_core::UserId;
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;

const ENTRY_LIMIT: usize = 1_024;
const BYTE_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    user_id: UserId,
    desk_id: uuid::Uuid,
    session_id: light_core::SessionId,
    request_id: String,
}

struct ReplayEntry {
    request: ProgrammingGroupRecordRequest,
    result: ProgrammingGroupRecordResult,
    retained_bytes: usize,
}

#[derive(Default)]
pub(super) struct GroupRecordingReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
}

impl GroupRecordingReplayCache {
    pub(super) fn invalidate_user(&mut self, user_id: UserId) {
        self.entries.retain(|key, _| key.user_id != user_id);
        self.order.retain(|key| key.user_id != user_id);
        self.retain_size();
    }

    pub(super) fn get(
        &self,
        user_id: UserId,
        desk_id: uuid::Uuid,
        session_id: light_core::SessionId,
        request_id: &str,
        request: &ProgrammingGroupRecordRequest,
    ) -> Result<Option<ProgrammingGroupRecordResult>, ActionError> {
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
                "request_id was already used for a different Group recording action",
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
        session_id: light_core::SessionId,
        request_id: String,
        request: ProgrammingGroupRecordRequest,
        result: ProgrammingGroupRecordResult,
    ) {
        let key = ReplayKey {
            user_id,
            desk_id,
            session_id,
            request_id,
        };
        let retained_bytes = retained_entry_bytes(&key, &request, &result);
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

    fn retain_size(&mut self) {
        self.retained_bytes = self
            .entries
            .values()
            .map(|entry| entry.retained_bytes)
            .sum();
    }
}

impl super::ProgrammingService {
    pub(in crate::programming) fn invalidate_group_recording_replay(&self, user_id: UserId) {
        self.group_recording_replay.lock().invalidate_user(user_id);
    }
}

fn retained_entry_bytes(
    key: &ReplayKey,
    request: &ProgrammingGroupRecordRequest,
    result: &ProgrammingGroupRecordResult,
) -> usize {
    size_of::<ReplayKey>()
        .saturating_add(size_of::<ReplayEntry>())
        .saturating_add(key.request_id.capacity())
        .saturating_add(request.group_id.capacity())
        .saturating_add(result.request_id.capacity())
        .saturating_add(
            result
                .outcome
                .projection()
                .raw_body
                .as_deref()
                .map_or(0, json_bytes),
        )
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
