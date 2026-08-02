use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) enum ReplayAction {
    DynamicCreate(serde_json::Value),
    DynamicMove(Uuid, wire::DynamicPoolActionRequest),
    DynamicCopy(Uuid, wire::DynamicPoolActionRequest),
    DynamicDelete(Uuid, wire::DynamicDeleteActionRequest),
    DynamicUpdate(Uuid, wire::DynamicUpdateActionRequest),
    UserLayout(wire::UserLayoutAction),
    PatchLayer(wire::PatchLayerAction),
    Preload(wire::PreloadRecordAction),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::runtime) struct ReplayKey {
    session_id: Uuid,
    show_id: light_core::ShowId,
    request_id: String,
}

impl ReplayKey {
    pub(in crate::runtime) fn new(
        session: &Session,
        show_id: light_core::ShowId,
        request_id: &str,
    ) -> Self {
        Self {
            session_id: session.id.0,
            show_id,
            request_id: request_id.to_owned(),
        }
    }
}

struct ReplayEntry {
    action: ReplayAction,
    outcome: wire::ShowObjectActionOutcome,
}

#[derive(Default)]
pub(in crate::runtime) struct ShowObjectIntentReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl ShowObjectIntentReplayCache {
    pub(in crate::runtime) fn get(
        &self,
        key: &ReplayKey,
        action: &ReplayAction,
    ) -> Result<Option<wire::ShowObjectActionOutcome>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.action != action {
            return Err(ApiError::conflict(
                "request_id was already used for a different show-object action",
            ));
        }
        let mut outcome = entry.outcome.clone();
        outcome.replayed = true;
        Ok(Some(outcome))
    }

    pub(in crate::runtime) fn insert(
        &mut self,
        key: ReplayKey,
        action: ReplayAction,
        outcome: wire::ShowObjectActionOutcome,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { action, outcome });
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}
