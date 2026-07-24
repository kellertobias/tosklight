//! Typed replay-safe control-desk settings and page-assignment routes.

use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::control_desk_configuration as wire;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v2/control-desks/{desk_id}/actions",
        post(apply_action),
    )
}

async fn apply_action(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ControlDeskConfigurationActionRequest>,
) -> Result<Json<wire::ControlDeskConfigurationActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if session.desk.id != desk_id {
        return Err(ApiError::forbidden(
            "session is not authorized for this desk",
        ));
    }
    show_objects_v2::validate_request_id(&request.request_id)?;
    let key = ReplayKey {
        session_id: session.id.0,
        desk_id,
        request_id: request.request_id.clone(),
    };
    let mut replay = state.control_desk_configuration_replay.lock().await;
    if let Some(outcome) = replay.get(&key, &request.action)? {
        return Ok(Json(outcome));
    }
    let mut outcome = execute_action(&state, &session, request.action.clone()).await?;
    outcome.request_id = request.request_id;
    replay.insert(key, request.action, outcome.clone());
    Ok(Json(outcome))
}

async fn execute_action(
    state: &AppState,
    session: &Session,
    action: wire::ControlDeskConfigurationAction,
) -> Result<wire::ControlDeskConfigurationActionOutcome, ApiError> {
    match action {
        wire::ControlDeskConfigurationAction::Update { patch } => {
            let current = state
                .desk
                .lock()
                .control_desk(session.desk.id)
                .map_err(ApiError::store)?
                .ok_or_else(|| ApiError::not_found("control desk"))?;
            let layout = patch
                .playback_layout
                .map(domain_layout)
                .or(current.playback_layout);
            let desk = state
                .desk
                .lock()
                .update_desk(
                    session.desk.id,
                    patch.name.as_deref().unwrap_or(&current.name),
                    patch.osc_alias.as_deref().unwrap_or(&current.osc_alias),
                    patch.columns.unwrap_or(current.columns),
                    patch.rows.unwrap_or(current.rows),
                    patch.buttons.unwrap_or(current.buttons),
                    layout,
                )
                .map_err(ApiError::store)?;
            for active in state
                .sessions
                .write()
                .values_mut()
                .filter(|active| active.desk.id == desk.id)
            {
                active.desk = desk.clone();
            }
            emit(
                state,
                "control_desk_changed",
                serde_json::json!({"desk":desk}),
            );
            Ok(outcome(desk, None, None, None))
        }
        wire::ControlDeskConfigurationAction::SetPage {
            page,
            existing_only,
        } => {
            let _activation = state.activation_lock.clone().lock_owned().await;
            let show = state
                .active_show
                .read()
                .clone()
                .ok_or_else(|| ApiError::bad_request("no show is open"))?;
            let context = operator_action_context(session, light_application::ActionSource::Http);
            let completed = if existing_only {
                state
                    .playback_service
                    .run_unit_of_work(playback_service::ChangePage::existing(
                        state,
                        show.id,
                        context,
                        session.desk.id,
                        page,
                    ))
            } else {
                state
                    .playback_service
                    .run_unit_of_work(playback_service::ChangePage {
                        state,
                        show: &show,
                        context,
                        desk_id: session.desk.id,
                        page,
                    })
            };
            let availability = completed.output?;
            if !availability.available() {
                return Err(ApiError::bad_request("playback page does not exist"));
            }
            let page_creation_event_sequence = availability.event_sequence();
            let event_sequence = completed.event_sequences.first().copied();
            emit(
                state,
                "playback_page_changed",
                serde_json::json!({"desk_id":session.desk.id,"show_id":show.id,"page":page}),
            );
            send_osc_feedback(state, false);
            let desk = state
                .desk
                .lock()
                .control_desk(session.desk.id)
                .map_err(ApiError::store)?
                .ok_or_else(|| ApiError::not_found("control desk"))?;
            Ok(outcome(
                desk,
                Some(page),
                event_sequence,
                page_creation_event_sequence,
            ))
        }
    }
}

fn outcome(
    desk: ControlDesk,
    page: Option<u8>,
    event_sequence: Option<u64>,
    page_creation_event_sequence: Option<u64>,
) -> wire::ControlDeskConfigurationActionOutcome {
    wire::ControlDeskConfigurationActionOutcome {
        request_id: String::new(),
        replayed: false,
        desk: runtime_wire::desk(desk),
        page,
        event_sequence,
        page_creation_event_sequence,
    }
}

fn domain_layout(
    layout: light_wire::v2::runtime::RuntimePlaybackSurfaceLayout,
) -> light_show::PlaybackSurfaceLayout {
    light_show::PlaybackSurfaceLayout {
        playbacks_per_row: layout.playbacks_per_row,
        rows: layout
            .rows
            .into_iter()
            .map(|row| light_show::PlaybackSurfaceRow {
                first_playback_slot: row.first_playback_slot,
                has_fader: row.has_fader,
                button_count: row.button_count,
            })
            .collect(),
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    session_id: Uuid,
    desk_id: Uuid,
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
    fn get(
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

    fn insert(
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
