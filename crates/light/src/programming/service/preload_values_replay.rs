use super::super::ProgrammingPreloadValuesResult;
use super::ProgrammingService;
use super::values_replay_fingerprint::RequestFingerprint;
use super::values_replay_memory::{
    ENTRY_CONTAINER_OVERHEAD, ReplayLimits, preload_result_retained_bytes,
};
use crate::{ActionError, ActionErrorKind};
use light_core::SessionId;
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    desk_id: uuid::Uuid,
    session_id: SessionId,
    request_id: String,
}

pub(super) struct PreloadReplayIdentity {
    pub(super) desk_id: uuid::Uuid,
    pub(super) session_id: SessionId,
    pub(super) request_id: String,
}

impl From<&PreloadReplayIdentity> for ReplayKey {
    fn from(identity: &PreloadReplayIdentity) -> Self {
        Self {
            desk_id: identity.desk_id,
            session_id: identity.session_id,
            request_id: identity.request_id.clone(),
        }
    }
}

struct ReplayEntry {
    fingerprint: RequestFingerprint,
    result: ProgrammingPreloadValuesResult,
    retained_bytes: usize,
}

#[derive(Default)]
pub(super) struct PreloadValuesReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
    limits: ReplayLimits,
}

impl PreloadValuesReplayCache {
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
        identity: &PreloadReplayIdentity,
        fingerprint: RequestFingerprint,
    ) -> Result<Option<ProgrammingPreloadValuesResult>, ActionError> {
        let key = ReplayKey::from(identity);
        let Some(entry) = self.entries.get(&key) else {
            return Ok(None);
        };
        if entry.fingerprint != fingerprint {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different Preload values action",
            ));
        }
        let mut replayed = entry.result.clone();
        replayed.replayed = true;
        Ok(Some(replayed))
    }

    pub(super) fn insert(
        &mut self,
        identity: PreloadReplayIdentity,
        fingerprint: RequestFingerprint,
        result: ProgrammingPreloadValuesResult,
    ) {
        let key = ReplayKey::from(&identity);
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

fn retained_entry_bytes(key: &ReplayKey, result: &ProgrammingPreloadValuesResult) -> usize {
    ENTRY_CONTAINER_OVERHEAD
        .saturating_add(2 * size_of::<ReplayKey>())
        .saturating_add(key.request_id.capacity() + key.request_id.len())
        .saturating_add(size_of::<ReplayEntry>())
        .saturating_add(preload_result_retained_bytes(result))
}

impl ProgrammingService {
    pub(in crate::programming) fn invalidate_preload_values_replay(&self) {
        self.preload_values_replay.lock().invalidate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActionContext, ActionSource, ProgrammingPreloadValuesOutcome,
        ProgrammingPreloadValuesProjection,
    };
    use light_core::{AttributeKey, AttributeValue};
    use light_programmer::PreloadProgrammerGroupValue;
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn byte_budget_evicts_oldest_large_projection() {
        let desk_id = Uuid::new_v4();
        let session_id = SessionId::new();
        let first = result(desk_id, session_id, "first", 4_096);
        let second = result(desk_id, session_id, "second", 4_096);
        let first_key = key(desk_id, session_id, "first");
        let second_key = key(desk_id, session_id, "second");
        let budget = retained_entry_bytes(&first_key, &first)
            + retained_entry_bytes(&second_key, &second)
            - 1;
        let mut cache = PreloadValuesReplayCache::with_limits(10, budget);
        let first_identity = identity(desk_id, session_id, "first");
        let second_identity = identity(desk_id, session_id, "second");

        cache.insert(first_identity, [1; 32], first);
        cache.insert(second_identity, [2; 32], second);

        assert_eq!(cache.entries.len(), 1);
        assert!(cache.retained_bytes <= budget);
        assert!(
            cache
                .get(&identity(desk_id, session_id, "first"), [1; 32])
                .unwrap()
                .is_none()
        );
        assert!(
            cache
                .get(&identity(desk_id, session_id, "second"), [2; 32])
                .unwrap()
                .is_some()
        );
    }

    fn key(desk_id: Uuid, session_id: SessionId, request_id: &str) -> ReplayKey {
        ReplayKey {
            desk_id,
            session_id,
            request_id: request_id.into(),
        }
    }

    fn identity(desk_id: Uuid, session_id: SessionId, request_id: &str) -> PreloadReplayIdentity {
        PreloadReplayIdentity {
            desk_id,
            session_id,
            request_id: request_id.into(),
        }
    }

    fn result(
        desk_id: Uuid,
        session_id: SessionId,
        request_id: &str,
        spread_len: usize,
    ) -> ProgrammingPreloadValuesResult {
        let projection = ProgrammingPreloadValuesProjection {
            dynamic_values: Vec::new(),
            revision: 1,
            fixture_values: Vec::new(),
            group_values: vec![PreloadProgrammerGroupValue {
                group_id: "front".into(),
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Spread(vec![0.5; spread_len]),
                programmer_order: 1,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            }],
        };
        ProgrammingPreloadValuesResult {
            context: ActionContext::operator(desk_id, session_id.0, ActionSource::Http)
                .with_request_id(request_id)
                .with_expected_revision(0),
            outcome: ProgrammingPreloadValuesOutcome::Changed {
                projection: Arc::new(projection),
                event_sequence: 1,
            },
            capture_mode_revision: 1,
            interaction_event_sequence: None,
            replayed: false,
            warning: None,
        }
    }
}
