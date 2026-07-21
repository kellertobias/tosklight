use super::ProgrammingService;
use crate::programming::cue_transfer::{
    CueTransferAuthority, ProgrammingCueTransferRequest, ProgrammingCueTransferResult,
};
use crate::{ActionError, ActionErrorKind};
use light_core::{SessionId, UserId};
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;
use uuid::Uuid;

const ENTRY_LIMIT: usize = 1_024;
const BYTE_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct CueTransferScope {
    pub user_id: UserId,
    pub desk_id: Uuid,
    pub session_id: SessionId,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    scope: CueTransferScope,
    request_id: String,
}

struct ReplayEntry {
    request: ProgrammingCueTransferRequest,
    result: ProgrammingCueTransferResult,
    retained_bytes: usize,
}

#[derive(Default)]
pub(super) struct CueTransferReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
}

impl CueTransferReplayCache {
    pub fn get(
        &self,
        scope: &CueTransferScope,
        request_id: &str,
        request: &ProgrammingCueTransferRequest,
    ) -> Result<Option<ProgrammingCueTransferResult>, ActionError> {
        let key = ReplayKey {
            scope: scope.clone(),
            request_id: request_id.to_owned(),
        };
        let Some(entry) = self.entries.get(&key) else {
            return Ok(None);
        };
        if entry.request != *request {
            return Err(conflict(
                "request_id was already used for a different Cue transfer action",
            ));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    pub fn insert(
        &mut self,
        scope: CueTransferScope,
        request_id: String,
        request: ProgrammingCueTransferRequest,
        result: ProgrammingCueTransferResult,
    ) {
        let key = ReplayKey { scope, request_id };
        let retained_bytes = retained_bytes(&key, &result);
        if retained_bytes > BYTE_LIMIT {
            return;
        }
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

    fn invalidate_user(&mut self, user_id: UserId) {
        self.entries.retain(|key, _| key.scope.user_id != user_id);
        self.order.retain(|key| key.scope.user_id != user_id);
        self.recount();
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

    fn recount(&mut self) {
        self.retained_bytes = self
            .entries
            .values()
            .map(|entry| entry.retained_bytes)
            .sum();
    }
}

#[derive(Default)]
pub(super) struct CueTransferChoiceCache {
    entries: HashMap<Uuid, (CueTransferScope, CueTransferAuthority)>,
    order: VecDeque<Uuid>,
}

impl CueTransferChoiceCache {
    pub fn insert(&mut self, scope: CueTransferScope, authority: CueTransferAuthority) {
        let choice_id = authority.choice_id;
        if !self.entries.contains_key(&choice_id) {
            self.order.push_back(choice_id);
        }
        self.entries.insert(choice_id, (scope, authority));
        while self.entries.len() > ENTRY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    pub fn get(
        &self,
        scope: &CueTransferScope,
        choice_id: Uuid,
    ) -> Result<CueTransferAuthority, ActionError> {
        let (owner, authority) = self
            .entries
            .get(&choice_id)
            .ok_or_else(|| conflict("Cue transfer choice is no longer authoritative"))?;
        if owner != scope {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "Cue transfer choice belongs to another operator context",
            ));
        }
        Ok(authority.clone())
    }

    fn invalidate_user(&mut self, user_id: UserId) {
        self.entries
            .retain(|_, (scope, _)| scope.user_id != user_id);
        self.order
            .retain(|choice_id| self.entries.contains_key(choice_id));
    }
}

impl ProgrammingService {
    pub(in crate::programming) fn invalidate_cue_transfer_authority(&self, user_id: UserId) {
        self.cue_transfer_choices.lock().invalidate_user(user_id);
        self.cue_transfer_replay.lock().invalidate_user(user_id);
    }
}

fn retained_bytes(key: &ReplayKey, result: &ProgrammingCueTransferResult) -> usize {
    size_of::<ReplayKey>()
        .saturating_add(size_of::<ReplayEntry>())
        .saturating_add(key.request_id.capacity())
        .saturating_add(result.request_id.capacity())
        .saturating_add(
            result
                .outcome
                .projections
                .iter()
                .map(|projection| {
                    projection.object_id.capacity().saturating_add(
                        serde_json::to_vec(&projection.raw_body).map_or(0, |encoded| encoded.len()),
                    )
                })
                .sum::<usize>(),
        )
}

fn conflict(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message)
}
