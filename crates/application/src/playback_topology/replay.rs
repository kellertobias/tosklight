use super::{
    PlaybackTopologyAction, PlaybackTopologyCommand, PlaybackTopologyObjectProjection,
    PlaybackTopologyResult,
};
use crate::{ActionContext, ActionError, ActionErrorKind};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    mem::size_of,
};
use uuid::Uuid;

const ENTRY_LIMIT: usize = 1_024;
const BYTE_LIMIT: usize = 64 * 1024 * 1024;

pub(super) type RequestFingerprint = [u8; 32];

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    user_id: Uuid,
    desk_id: Uuid,
    session_id: Uuid,
    request_id: String,
}

impl ReplayKey {
    pub(super) fn from_context(context: &ActionContext) -> Result<Self, ActionError> {
        let request_id = context
            .request_id
            .as_ref()
            .ok_or_else(|| invalid("Playback topology action requires request_id"))?;
        if request_id.is_empty() || request_id.len() > 128 {
            return Err(invalid("request_id must contain 1-128 bytes"));
        }
        Ok(Self {
            user_id: context
                .user_id
                .ok_or_else(|| unauthorized("Playback topology action requires a user"))?,
            desk_id: context.desk_id,
            session_id: context
                .session_id
                .ok_or_else(|| unauthorized("Playback topology action requires a session"))?,
            request_id: request_id.clone(),
        })
    }

    pub(super) fn request_id(&self) -> &str {
        &self.request_id
    }
}

struct ReplayEntry {
    fingerprint: RequestFingerprint,
    result: PlaybackTopologyResult,
    retained_bytes: usize,
}

#[derive(Default)]
pub(super) struct ReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
}

impl ReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        fingerprint: RequestFingerprint,
    ) -> Result<Option<PlaybackTopologyResult>, ActionError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if entry.fingerprint != fingerprint {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different Playback topology action",
            ));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        fingerprint: RequestFingerprint,
        result: PlaybackTopologyResult,
    ) {
        let retained_bytes = retained_entry_bytes(&key, &result);
        if retained_bytes > BYTE_LIMIT {
            return;
        }
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        if let Some(previous) = self.entries.insert(
            key,
            ReplayEntry {
                fingerprint,
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

pub(super) fn fingerprint(
    command: &PlaybackTopologyCommand,
    expected_show_revision: u64,
) -> Result<RequestFingerprint, ActionError> {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, b"playback-topology-v2");
    hasher.update(command.show_id.0.as_bytes());
    hasher.update(expected_show_revision.to_le_bytes());
    match &command.action {
        PlaybackTopologyAction::SaveCueList {
            cue_list_id,
            expected_revision,
            expected_object_id,
            cue_list,
            raw_body,
        } => {
            hasher.update([0]);
            hasher.update(cue_list_id.0.as_bytes());
            hasher.update(expected_revision.to_le_bytes());
            hash_json(&mut hasher, expected_object_id)?;
            hash_json(&mut hasher, cue_list)?;
            hash_json(&mut hasher, raw_body)?;
        }
        PlaybackTopologyAction::ConfigureSlot {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id,
            expected_playback_revision,
            expected_playback_object_id,
            playback,
        } => {
            hasher.update([1, *page, *slot]);
            hasher.update(expected_page_revision.to_le_bytes());
            hash_json(&mut hasher, expected_page_object_id)?;
            hasher.update(expected_playback_revision.to_le_bytes());
            hash_json(&mut hasher, expected_playback_object_id)?;
            hash_json(&mut hasher, playback)?;
        }
        PlaybackTopologyAction::ClearMappedPlayback {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id,
            expected_playback_revision,
            expected_playback_object_id,
        } => {
            hasher.update([2, *page, *slot]);
            hasher.update(expected_page_revision.to_le_bytes());
            hash_json(&mut hasher, expected_page_object_id)?;
            hasher.update(expected_playback_revision.to_le_bytes());
            hash_json(&mut hasher, expected_playback_object_id)?;
        }
    }
    Ok(hasher.finalize().into())
}

fn hash_json(hasher: &mut Sha256, value: &impl serde::Serialize) -> Result<(), ActionError> {
    let encoded = serde_json::to_vec(value)
        .map_err(|error| invalid(format!("invalid topology action: {error}")))?;
    hash_bytes(hasher, &encoded);
    Ok(())
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(bytes.len().to_le_bytes());
    hasher.update(bytes);
}

fn retained_entry_bytes(key: &ReplayKey, result: &PlaybackTopologyResult) -> usize {
    size_of::<ReplayKey>()
        .saturating_add(size_of::<ReplayEntry>())
        .saturating_add(key.request_id.capacity())
        .saturating_add(result.request_id.capacity())
        .saturating_add(
            result
                .outcome
                .objects()
                .iter()
                .map(projection_bytes)
                .sum::<usize>(),
        )
}

fn projection_bytes(projection: &PlaybackTopologyObjectProjection) -> usize {
    size_of::<PlaybackTopologyObjectProjection>()
        .saturating_add(projection.object_id().len())
        .saturating_add(projection.raw_body().map_or(0, |body| json_bytes(body)))
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

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn unauthorized(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Unauthorized, message)
}
