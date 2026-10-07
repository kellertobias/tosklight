//! TL-659: the operator's change lead time reset.
//!
//! Resetting a diagnostic reading changes no playback, fixture, selection or command-line state,
//! so it is an object-intent update (api-rules §3): `POST …/update` with a typed body carrying
//! only what changes and a client request identity. A resend returns the first outcome instead of
//! wiping a lead measured since. The readings themselves travel with the output health snapshot.

use super::show_objects_v2::validate_request_id;
use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::runtime as wire;
use std::collections::VecDeque;

const REPLAY_LIMIT: usize = 64;

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v2/output/change-lead-time/update", post(update))
}

async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::RuntimeChangeLeadTimeUpdateRequest>,
) -> Result<Json<wire::RuntimeChangeLeadTimeUpdateOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let reset = request.reset;
    let outcome = state.replay.change_lead_update(key, request, || {
        if reset {
            state.output.change_lead_recorder().reset();
        }
        wire::RuntimeChangeLeadTimeUpdateOutcome {
            change_lead: runtime_wire::change_lead_time(state.output.change_lead_snapshot()),
            replayed: false,
        }
    })?;
    Ok(Json(outcome))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ReplayKey {
    session_id: Uuid,
    request_id: String,
}

struct ReplayEntry {
    key: ReplayKey,
    request: wire::RuntimeChangeLeadTimeUpdateRequest,
    outcome: wire::RuntimeChangeLeadTimeUpdateOutcome,
}

#[derive(Default)]
pub(in crate::runtime) struct ChangeLeadReplayCache {
    entries: VecDeque<ReplayEntry>,
}

impl ChangeLeadReplayCache {
    pub(in crate::runtime) fn get(
        &self,
        key: &ReplayKey,
        request: &wire::RuntimeChangeLeadTimeUpdateRequest,
    ) -> Result<Option<wire::RuntimeChangeLeadTimeUpdateOutcome>, ApiError> {
        let Some(entry) = self.entries.iter().find(|entry| &entry.key == key) else {
            return Ok(None);
        };
        if &entry.request != request {
            return Err(ApiError::conflict(
                "request_id was already used for a different change lead time update",
            ));
        }
        Ok(Some(wire::RuntimeChangeLeadTimeUpdateOutcome {
            replayed: true,
            ..entry.outcome.clone()
        }))
    }

    pub(in crate::runtime) fn insert(
        &mut self,
        key: ReplayKey,
        request: wire::RuntimeChangeLeadTimeUpdateRequest,
        outcome: wire::RuntimeChangeLeadTimeUpdateOutcome,
    ) {
        self.entries.push_back(ReplayEntry {
            key,
            request,
            outcome,
        });
        while self.entries.len() > REPLAY_LIMIT {
            self.entries.pop_front();
        }
    }
}
