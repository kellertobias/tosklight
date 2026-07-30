use super::super::ProgrammingValuesResult;
use super::ProgrammingService;
use super::values_replay_fingerprint::RequestFingerprint;
use super::values_replay_memory::{
    ENTRY_CONTAINER_OVERHEAD, ReplayLimits, values_result_retained_bytes,
};
use crate::{ActionError, ActionErrorKind};
use light_core::{SessionId, UserId};
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    user_id: UserId,
    desk_id: uuid::Uuid,
    session_id: SessionId,
    request_id: String,
}

struct ReplayEntry {
    fingerprint: RequestFingerprint,
    result: ProgrammingValuesResult,
    retained_bytes: usize,
}

#[derive(Default)]
pub(super) struct ValuesReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
    limits: ReplayLimits,
}

impl ValuesReplayCache {
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
        fingerprint: RequestFingerprint,
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
        if entry.fingerprint != fingerprint {
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
        fingerprint: RequestFingerprint,
        result: ProgrammingValuesResult,
    ) {
        let key = ReplayKey {
            user_id,
            desk_id,
            session_id,
            request_id,
        };
        let retained_bytes = retained_entry_bytes(&key, &result);
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        if let Some(replaced) = self.entries.insert(
            key,
            ReplayEntry {
                fingerprint,
                result,
                retained_bytes,
            },
        ) {
            self.retained_bytes = self.retained_bytes.saturating_sub(replaced.retained_bytes);
        }
        self.retained_bytes = self.retained_bytes.saturating_add(retained_bytes);
        self.evict_to_limits();
    }

    fn evict_to_limits(&mut self) {
        while self.entries.len() > self.limits.entries || self.retained_bytes > self.limits.bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.retained_bytes = self.retained_bytes.saturating_sub(removed.retained_bytes);
            }
        }
    }

    #[cfg(test)]
    fn with_limits(entries: usize, bytes: usize) -> Self {
        Self {
            limits: ReplayLimits { entries, bytes },
            ..Self::default()
        }
    }
}

fn retained_entry_bytes(key: &ReplayKey, result: &ProgrammingValuesResult) -> usize {
    ENTRY_CONTAINER_OVERHEAD
        .saturating_add(2 * size_of::<ReplayKey>())
        .saturating_add(key.request_id.capacity() + key.request_id.len())
        .saturating_add(size_of::<ReplayEntry>())
        .saturating_add(values_result_retained_bytes(result))
}

impl ProgrammingService {
    pub(in crate::programming) fn invalidate_values_replay(&self, user_id: UserId) {
        self.values_replay.lock().invalidate_user(user_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActionContext, ActionSource, ProgrammingValuesOutcome, ProgrammingValuesProjection,
    };
    use light_core::{AttributeKey, AttributeValue};
    use light_programmer::ProgrammerGroupUpdate;
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn byte_budget_evicts_oldest_large_projection() {
        let user_id = UserId::new();
        let desk_id = Uuid::new_v4();
        let session_id = SessionId::new();
        let first = result(user_id, desk_id, session_id, "first", 4_096);
        let second = result(user_id, desk_id, session_id, "second", 4_096);
        let first_key = key(user_id, desk_id, session_id, "first");
        let second_key = key(user_id, desk_id, session_id, "second");
        let budget = retained_entry_bytes(&first_key, &first)
            + retained_entry_bytes(&second_key, &second)
            - 1;
        let mut cache = ValuesReplayCache::with_limits(10, budget);

        cache.insert(user_id, desk_id, session_id, "first".into(), [1; 32], first);
        cache.insert(
            user_id,
            desk_id,
            session_id,
            "second".into(),
            [2; 32],
            second,
        );

        assert_eq!(cache.entries.len(), 1);
        assert!(cache.retained_bytes <= budget);
        assert!(
            cache
                .get(user_id, desk_id, session_id, "first", [1; 32])
                .unwrap()
                .is_none()
        );
        assert!(
            cache
                .get(user_id, desk_id, session_id, "second", [2; 32])
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

    fn result(
        user_id: UserId,
        desk_id: Uuid,
        session_id: SessionId,
        request_id: &str,
        spread_len: usize,
    ) -> ProgrammingValuesResult {
        let projection = ProgrammingValuesProjection {
            dynamic_values: Vec::new().into(),
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
        ProgrammingValuesResult {
            context: ActionContext::operator(desk_id, user_id.0, session_id.0, ActionSource::Http)
                .with_request_id(request_id)
                .with_expected_revision(0),
            outcome: ProgrammingValuesOutcome::Changed {
                projection: Arc::new(projection),
                event_sequence: 1,
            },
            capture_mode_revision: 0,
            interaction_event_sequence: None,
            replayed: false,
            warning: None,
        }
    }
}
