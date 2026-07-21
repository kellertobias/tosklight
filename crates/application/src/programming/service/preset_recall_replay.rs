use super::values_replay_memory::{
    ENTRY_CONTAINER_OVERHEAD, programming_values_projection_retained_bytes,
};
use crate::{
    ActionError, ActionErrorKind, ProgrammingPresetRecallOutcome, ProgrammingPresetRecallRequest,
    ProgrammingPresetRecallResult,
};
use light_core::{SessionId, UserId};
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;

const ENTRY_LIMIT: usize = 512;
const BYTE_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    user_id: UserId,
    desk_id: uuid::Uuid,
    session_id: SessionId,
    request_id: String,
}

struct ReplayEntry {
    request: ProgrammingPresetRecallRequest,
    result: ProgrammingPresetRecallResult,
    retained_bytes: usize,
}

pub(super) struct PresetRecallReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
    entry_limit: usize,
    byte_limit: usize,
}

impl Default for PresetRecallReplayCache {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            retained_bytes: 0,
            entry_limit: ENTRY_LIMIT,
            byte_limit: BYTE_LIMIT,
        }
    }
}

impl PresetRecallReplayCache {
    pub(super) fn invalidate_user(&mut self, user_id: UserId) {
        self.entries.retain(|key, _| key.user_id != user_id);
        self.order.retain(|key| key.user_id != user_id);
        self.retained_bytes = self
            .entries
            .values()
            .map(|entry| entry.retained_bytes)
            .sum();
    }

    pub(super) fn get(
        &self,
        user_id: UserId,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: &str,
        request: &ProgrammingPresetRecallRequest,
    ) -> Result<Option<ProgrammingPresetRecallResult>, ActionError> {
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
                "request_id was already used for a different Preset recall action",
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
        request: ProgrammingPresetRecallRequest,
        result: ProgrammingPresetRecallResult,
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
        while self.entries.len() > self.entry_limit || self.retained_bytes > self.byte_limit {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.retained_bytes = self.retained_bytes.saturating_sub(entry.retained_bytes);
            }
        }
    }

    #[cfg(test)]
    fn with_limits(entry_limit: usize, byte_limit: usize) -> Self {
        Self {
            entry_limit,
            byte_limit,
            ..Self::default()
        }
    }
}

fn retained_entry_bytes(
    key: &ReplayKey,
    request: &ProgrammingPresetRecallRequest,
    result: &ProgrammingPresetRecallResult,
) -> usize {
    let mut bytes = ENTRY_CONTAINER_OVERHEAD
        .saturating_add(2 * size_of::<ReplayKey>())
        .saturating_add(key.request_id.capacity() + key.request_id.len())
        .saturating_add(size_of::<ReplayEntry>())
        .saturating_add(size_of_val(request))
        .saturating_add(result.request_id.capacity())
        .saturating_add(
            result
                .context
                .request_id
                .as_ref()
                .map_or(0, String::capacity),
        )
        .saturating_add(result.active_context.capacity())
        .saturating_add(result.preset.object_id.capacity())
        .saturating_add(result.warning.as_ref().map_or(0, String::capacity))
        .saturating_add(json_bytes(&result.preset.raw_body));
    if let ProgrammingPresetRecallOutcome::Changed {
        projection: Some(projection),
        ..
    } = &result.outcome
    {
        bytes = bytes.saturating_add(programming_values_projection_retained_bytes(projection));
    }
    bytes
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

impl super::ProgrammingService {
    pub(in crate::programming) fn invalidate_preset_recall_replay(&self, user_id: UserId) {
        self.preset_recall_replay.lock().invalidate_user(user_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActionContext, ActionSource, ProgrammingRecalledPresetProjection,
        ProgrammingValuesProjection,
    };
    use light_core::{AttributeKey, AttributeValue, ShowId};
    use light_programmer::{PresetAddress, PresetFamily, ProgrammerGroupUpdate};
    use light_show::PortableShowRevision;
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn byte_budget_evicts_oldest_large_projection_and_raw_body() {
        let user_id = UserId::new();
        let desk_id = Uuid::new_v4();
        let session_id = SessionId::new();
        let first_request = request();
        let second_request = request();
        let first = result(user_id, desk_id, session_id, "first", 4_096);
        let second = result(user_id, desk_id, session_id, "second", 4_096);
        let first_key = key(user_id, desk_id, session_id, "first");
        let second_key = key(user_id, desk_id, session_id, "second");
        let budget = retained_entry_bytes(&first_key, &first_request, &first)
            + retained_entry_bytes(&second_key, &second_request, &second)
            - 1;
        let mut cache = PresetRecallReplayCache::with_limits(10, budget);

        cache.insert(
            user_id,
            desk_id,
            session_id,
            "first".into(),
            first_request.clone(),
            first,
        );
        cache.insert(
            user_id,
            desk_id,
            session_id,
            "second".into(),
            second_request.clone(),
            second,
        );

        assert_eq!(cache.entries.len(), 1);
        assert!(cache.retained_bytes <= budget);
        assert!(
            cache
                .get(user_id, desk_id, session_id, "first", &first_request)
                .unwrap()
                .is_none()
        );
        assert!(
            cache
                .get(user_id, desk_id, session_id, "second", &second_request)
                .unwrap()
                .is_some()
        );
    }

    fn key(user_id: UserId, desk_id: Uuid, session_id: SessionId, request_id: &str) -> ReplayKey {
        ReplayKey {
            user_id,
            desk_id,
            session_id,
            request_id: request_id.into(),
        }
    }

    fn request() -> ProgrammingPresetRecallRequest {
        ProgrammingPresetRecallRequest {
            show_id: ShowId::new(),
            address: PresetAddress::new(PresetFamily::Mixed, 1).unwrap(),
            expected_preset_revision: crate::ProgrammingPresetRecallRevisionExpectation::Exact(1),
            expected_show_revision: crate::ProgrammingPresetRecallRevisionExpectation::Exact(1),
            expected_values_revision: crate::ProgrammingPresetRecallRevisionExpectation::Exact(0),
            expected_capture_mode_revision:
                crate::ProgrammingPresetRecallRevisionExpectation::Exact(0),
            expected_selection_revision: crate::ProgrammingPresetRecallRevisionExpectation::Exact(
                1,
            ),
        }
    }

    fn result(
        user_id: UserId,
        desk_id: Uuid,
        session_id: SessionId,
        request_id: &str,
        spread_len: usize,
    ) -> ProgrammingPresetRecallResult {
        let address = PresetAddress::new(PresetFamily::Mixed, 1).unwrap();
        let projection = ProgrammingValuesProjection {
            user_id,
            revision: 1,
            fixture_values: Vec::new(),
            group_values: vec![ProgrammerGroupUpdate {
                group_id: "front".into(),
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Spread(vec![0.5; spread_len]),
                programmer_order: 1,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            }],
        };
        ProgrammingPresetRecallResult {
            context: ActionContext::operator(desk_id, user_id.0, session_id.0, ActionSource::Http)
                .with_request_id(request_id),
            request_id: request_id.into(),
            replayed: false,
            applied_fixtures: 1,
            selection_revision: 1,
            interaction_event_sequence: None,
            capture_mode_revision: 0,
            active_context: "preset:mixed:1".into(),
            preset: ProgrammingRecalledPresetProjection {
                show_id: ShowId::new(),
                show_revision: PortableShowRevision::from_value(1),
                object_id: address.storage_key(),
                object_revision: 1,
                address,
                raw_body: Arc::new(serde_json::json!({
                    "future": "x".repeat(spread_len),
                })),
            },
            outcome: ProgrammingPresetRecallOutcome::Changed {
                values_revision: 1,
                projection: Some(Arc::new(projection)),
                values_event_sequence: Some(1),
            },
            warning: None,
        }
    }
}
