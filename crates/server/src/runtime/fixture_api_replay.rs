use super::api_error::ApiError;
use light_wire::v2::fixture_library as wire;
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    pub(super) session_id: Uuid,
    pub(super) request_id: String,
}

struct ReplayEntry {
    signature: [u8; 32],
    outcome: wire::FixtureLibraryActionOutcome,
}

#[derive(Default)]
pub(super) struct FixtureLibraryReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl FixtureLibraryReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        signature: &[u8; 32],
    ) -> Result<Option<wire::FixtureLibraryActionOutcome>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.signature != signature {
            return Err(ApiError::conflict(
                "request_id was already used for a different fixture-library action",
            ));
        }
        let mut outcome = entry.outcome.clone();
        outcome.replayed = true;
        Ok(Some(outcome))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        signature: [u8; 32],
        outcome: wire::FixtureLibraryActionOutcome,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { signature, outcome });
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}
