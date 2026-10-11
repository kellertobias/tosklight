//! Typed replay-safe control-desk settings and page-assignment routes.

use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::control_desk_configuration as wire;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v2/control-desk/actions", post(apply_action))
}

async fn apply_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ControlDeskConfigurationActionRequest>,
) -> Result<Json<wire::ControlDeskConfigurationActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let desk_id = session.desk.id;
    show_objects_v2::validate_request_id(&request.request_id)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    if let Some(outcome) = state
        .replay
        .lookup_control_desk_configuration(&key, &request.action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let mut outcome = execute_action(&state, &session, desk_id, request.action.clone()).await?;
    outcome.request_id = request.request_id;
    state
        .replay
        .insert_control_desk_configuration(key, request.action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn execute_action(
    state: &AppState,
    session: &Session,
    desk_id: Uuid,
    action: wire::ControlDeskConfigurationAction,
) -> Result<wire::ControlDeskConfigurationActionOutcome, ApiError> {
    match action {
        wire::ControlDeskConfigurationAction::Update { patch } => {
            update_control_desk(state, desk_id, patch)
        }
        wire::ControlDeskConfigurationAction::RemoveClient { client_id } => {
            remove_historical_client(state, session, client_id)
        }
    }
}

fn update_control_desk(
    state: &AppState,
    desk_id: Uuid,
    patch: wire::ControlDeskConfigurationPatch,
) -> Result<wire::ControlDeskConfigurationActionOutcome, ApiError> {
    let current = state.installation.desk().map_err(ApiError::store)?;
    let desk = state
        .installation
        .update_desk(
            desk_id,
            patch.name.as_deref().unwrap_or(&current.name),
            patch.columns.unwrap_or(current.columns),
            patch.rows.unwrap_or(current.rows),
            patch.buttons.unwrap_or(current.buttons),
            patch.surface_id.unwrap_or(current.surface_id),
            Some([
                patch
                    .hardware_led_brightness
                    .unwrap_or(current.hardware_led_brightness),
                patch
                    .hardware_gooseneck_brightness
                    .unwrap_or(current.hardware_gooseneck_brightness),
                patch
                    .hardware_gooseneck_color
                    .unwrap_or(current.hardware_gooseneck_color),
            ]),
        )
        .map_err(ApiError::store)?;
    state.sessions.update_desk_sessions(&desk);
    emit(
        state,
        "control_desk_changed",
        serde_json::json!({"desk":desk}),
    );
    send_osc_feedback(state, false);
    Ok(outcome(desk, None))
}

fn remove_historical_client(
    state: &AppState,
    session: &Session,
    client_id: Uuid,
) -> Result<wire::ControlDeskConfigurationActionOutcome, ApiError> {
    let target = state
        .installation
        .client_desks()
        .map_err(ApiError::store)?
        .into_iter()
        .find(|entry| entry.client_id == Some(client_id))
        .ok_or_else(|| ApiError::not_found("client"))?;
    ensure_client_is_removable(state, session, client_id)?;
    if !state
        .installation
        .remove_client(client_id)
        .map_err(ApiError::store)?
    {
        return Err(ApiError::not_found("client"));
    }
    emit(
        state,
        "client_removed",
        serde_json::json!({"client_id":client_id,"desk_id":target.desk.id}),
    );
    Ok(removed_outcome(target.desk))
}

fn ensure_client_is_removable(
    state: &AppState,
    session: &Session,
    target_client_id: Uuid,
) -> Result<(), ApiError> {
    if state.sessions.client_id(session.id) == Some(target_client_id) {
        return Err(ApiError::conflict(
            "the current client cannot remove itself",
        ));
    }
    // A window is removable while it is not connected. The desk it operated stays: every window
    // shares the one desk, so somebody else standing at it is not a reason to keep this record.
    if state.sessions.client_connected(target_client_id) {
        return Err(ApiError::conflict(
            "an actively connected client cannot be removed",
        ));
    }
    Ok(())
}

fn outcome(
    desk: ControlDesk,
    event_sequence: Option<u64>,
) -> wire::ControlDeskConfigurationActionOutcome {
    wire::ControlDeskConfigurationActionOutcome {
        request_id: String::new(),
        replayed: false,
        desk: runtime_wire::desk(desk),
        removed: false,
        event_sequence,
    }
}

fn removed_outcome(desk: ControlDesk) -> wire::ControlDeskConfigurationActionOutcome {
    let mut outcome = outcome(desk, None);
    outcome.removed = true;
    outcome
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    session_id: Uuid,
    request_id: String,
}

#[derive(Clone)]
struct ReplayEntry {
    key: ReplayKey,
    action: wire::ControlDeskConfigurationAction,
    outcome: wire::ControlDeskConfigurationActionOutcome,
}

#[derive(Default)]
pub(super) struct ControlDeskConfigurationReplayCache {
    entries: VecDeque<ReplayEntry>,
}

impl ControlDeskConfigurationReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        action: &wire::ControlDeskConfigurationAction,
    ) -> Result<Option<wire::ControlDeskConfigurationActionOutcome>, ApiError> {
        let Some(entry) = self.entries.iter().find(|entry| &entry.key == key) else {
            return Ok(None);
        };
        if &entry.action != action {
            return Err(ApiError::conflict(
                "request_id was already used for a different control-desk action",
            ));
        }
        let mut outcome = entry.outcome.clone();
        outcome.replayed = true;
        Ok(Some(outcome))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        action: wire::ControlDeskConfigurationAction,
        outcome: wire::ControlDeskConfigurationActionOutcome,
    ) {
        self.entries.push_back(ReplayEntry {
            key,
            action,
            outcome,
        });
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT {
            self.entries.pop_front();
        }
    }
}
