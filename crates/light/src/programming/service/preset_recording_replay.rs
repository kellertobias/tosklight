use crate::{
    ActionError, ActionErrorKind, ProgrammingPresetRecordRequest, ProgrammingPresetRecordResult,
};
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;

const ENTRY_LIMIT: usize = 1_024;
const BYTE_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    desk_id: uuid::Uuid,
    session_id: light_core::SessionId,
    request_id: String,
}

struct ReplayEntry {
    request: ProgrammingPresetRecordRequest,
    result: ProgrammingPresetRecordResult,
    retained_bytes: usize,
}

#[derive(Default)]
pub(super) struct PresetRecordingReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
}

impl PresetRecordingReplayCache {
    pub(super) fn invalidate(&mut self) {
        // One desk, one Programmer: an invalidation clears the cache rather than one user's part.
        self.entries.clear();
        self.order.clear();
        self.retained_bytes = self
            .entries
            .values()
            .map(|entry| entry.retained_bytes)
            .sum();
    }

    pub(super) fn get(
        &self,
        desk_id: uuid::Uuid,
        session_id: light_core::SessionId,
        request_id: &str,
        request: &ProgrammingPresetRecordRequest,
    ) -> Result<Option<ProgrammingPresetRecordResult>, ActionError> {
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
                "request_id was already used for a different Preset recording action",
            ));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    pub(super) fn insert(
        &mut self,
        desk_id: uuid::Uuid,
        session_id: light_core::SessionId,
        request_id: String,
        request: ProgrammingPresetRecordRequest,
        result: ProgrammingPresetRecordResult,
    ) {
        let key = ReplayKey {
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
}

impl super::ProgrammingService {
    pub(in crate::programming) fn invalidate_preset_recording_replay(&self) {
        self.preset_recording_replay.lock().invalidate();
    }
}

fn retained_entry_bytes(
    key: &ReplayKey,
    request: &ProgrammingPresetRecordRequest,
    result: &ProgrammingPresetRecordResult,
) -> usize {
    size_of::<ReplayKey>()
        .saturating_add(size_of::<ReplayEntry>())
        .saturating_add(key.request_id.capacity())
        .saturating_add(request.name.capacity())
        .saturating_add(result.request_id.capacity())
        .saturating_add(json_bytes(&result.outcome.projection().raw_body))
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
