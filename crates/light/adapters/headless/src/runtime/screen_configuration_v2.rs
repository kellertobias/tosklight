//! Typed replay-safe desk-store screen configuration routes.

use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::screen_configuration as wire;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/screens", get(snapshot))
        .route("/api/v2/screens/actions", post(apply_action))
}

async fn snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::ScreenConfigurationSnapshot>, ApiError> {
    let session = authenticate(&state, &headers)?;
    screen_snapshot(&state, &session).map(Json)
}

async fn apply_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ScreenConfigurationActionRequest>,
) -> Result<Json<wire::ScreenConfigurationActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show_objects_v2::validate_request_id(&request.request_id)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let mut replay = state.screen_configuration_replay.lock().await;
    if let Some(outcome) = replay.get(&key, &request.action)? {
        return Ok(Json(outcome));
    }
    let mut outcome = execute_action(&state, request.action.clone())?;
    outcome.request_id = request.request_id;
    replay.insert(key, request.action, outcome.clone());
    Ok(Json(outcome))
}

fn execute_action(
    state: &AppState,
    action: wire::ScreenConfigurationAction,
) -> Result<wire::ScreenConfigurationActionOutcome, ApiError> {
    match action {
        wire::ScreenConfigurationAction::Create { configuration } => {
            let store = state.desk.lock();
            if store
                .screen(configuration.id)
                .map_err(ApiError::store)?
                .is_some()
            {
                return Err(ApiError::conflict("screen already exists"));
            }
            let screen = store
                .put_screen(domain_screen(configuration)?)
                .map_err(ApiError::store)?;
            drop(store);
            emit_screen_changed(state, &screen);
            outcome(Some(screen), None)
        }
        wire::ScreenConfigurationAction::Update { screen_id, patch } => {
            let store = state.desk.lock();
            let existing = store
                .screen(screen_id)
                .map_err(ApiError::store)?
                .ok_or_else(|| ApiError::not_found("screen"))?;
            let screen = store
                .put_screen(apply_patch(existing, patch)?)
                .map_err(ApiError::store)?;
            drop(store);
            emit_screen_changed(state, &screen);
            outcome(Some(screen), None)
        }
        wire::ScreenConfigurationAction::Delete { screen_id } => {
            let store = state.desk.lock();
            if store.screen(screen_id).map_err(ApiError::store)?.is_none() {
                return Err(ApiError::not_found("screen"));
            }
            store.delete_screen(screen_id).map_err(ApiError::store)?;
            drop(store);
            emit(
                state,
                "screen_configuration_changed",
                serde_json::json!({"screen_id":screen_id,"deleted":true}),
            );
            outcome(None, None)
        }
        wire::ScreenConfigurationAction::SetPage { screen_id, page } => {
            let show = state
                .active_show
                .read()
                .clone()
                .ok_or_else(|| ApiError::bad_request("no show is open"))?;
            if !state
                .engine
                .snapshot()
                .playback_pages
                .iter()
                .any(|candidate| candidate.number == page)
            {
                return Err(ApiError::bad_request("playback page does not exist"));
            }
            let store = state.desk.lock();
            let screen = store
                .screen(screen_id)
                .map_err(ApiError::store)?
                .ok_or_else(|| ApiError::not_found("screen"))?;
            if screen.page_mode != "independent" {
                return Err(ApiError::bad_request("screen follows the main page"));
            }
            store
                .set_screen_page(screen_id, show.id, page)
                .map_err(ApiError::store)?;
            drop(store);
            emit(
                state,
                "screen_page_changed",
                serde_json::json!({"screen_id":screen_id,"show_id":show.id,"page":page}),
            );
            outcome(Some(screen), Some(page))
        }
    }
}

fn screen_snapshot(
    state: &AppState,
    session: &Session,
) -> Result<wire::ScreenConfigurationSnapshot, ApiError> {
    let show = state.active_show.read().clone();
    let store = state.desk.lock();
    let screens = store.screens().map_err(ApiError::store)?;
    let mut active_pages = std::collections::BTreeMap::new();
    if let Some(show) = show {
        for screen in &screens {
            let page = if screen.page_mode == "follow_main" {
                store.desk_page(session.desk.id, show.id)
            } else {
                store.screen_page(screen.id, show.id)
            }
            .map_err(ApiError::store)?;
            active_pages.insert(screen.id, page);
        }
    }
    Ok(wire::ScreenConfigurationSnapshot {
        screens: screens
            .into_iter()
            .map(wire_screen)
            .collect::<Result<_, _>>()?,
        active_pages,
    })
}

fn apply_patch(
    mut screen: ScreenConfiguration,
    patch: wire::ScreenConfigurationPatch,
) -> Result<ScreenConfiguration, ApiError> {
    macro_rules! patch {
        ($field:ident) => {
            if let Some(value) = patch.$field {
                screen.$field = value;
            }
        };
    }
    patch!(name);
    patch!(layout);
    patch!(show_dock);
    patch!(show_playbacks);
    patch!(playback_count);
    patch!(playback_rows);
    patch!(first_playback_slot);
    if let Some(page_mode) = patch.page_mode {
        screen.page_mode = page_mode_string(page_mode).to_owned();
    }
    patch!(show_page_controls);
    patch!(desired_open);
    if patch.clear_display_id {
        screen.display_id = None;
    } else if let Some(display_id) = patch.display_id {
        screen.display_id = Some(display_id);
    }
    if patch.clear_bounds {
        screen.bounds = None;
    } else if let Some(bounds) = patch.bounds {
        screen.bounds = Some(bounds);
    }
    patch!(fullscreen);
    if patch.clear_playback_layout {
        screen.playback_layout = None;
    } else if let Some(layout) = patch.playback_layout {
        screen.playback_layout = Some(domain_layout(layout));
    }
    Ok(screen)
}

fn domain_screen(screen: wire::ScreenConfiguration) -> Result<ScreenConfiguration, ApiError> {
    Ok(ScreenConfiguration {
        id: screen.id,
        name: screen.name,
        layout: screen.layout,
        show_dock: screen.show_dock,
        show_playbacks: screen.show_playbacks,
        playback_count: screen.playback_count,
        playback_rows: screen.playback_rows,
        first_playback_slot: screen.first_playback_slot,
        page_mode: page_mode_string(screen.page_mode).to_owned(),
        show_page_controls: screen.show_page_controls,
        desired_open: screen.desired_open,
        display_id: screen.display_id,
        bounds: screen.bounds,
        fullscreen: screen.fullscreen,
        playback_layout: screen.playback_layout.map(domain_layout),
    })
}

fn wire_screen(screen: ScreenConfiguration) -> Result<wire::ScreenConfiguration, ApiError> {
    Ok(wire::ScreenConfiguration {
        id: screen.id,
        name: screen.name,
        layout: screen.layout,
        show_dock: screen.show_dock,
        show_playbacks: screen.show_playbacks,
        playback_count: screen.playback_count,
        playback_rows: screen.playback_rows,
        first_playback_slot: screen.first_playback_slot,
        page_mode: match screen.page_mode.as_str() {
            "follow_main" => wire::ScreenPageMode::FollowMain,
            "independent" => wire::ScreenPageMode::Independent,
            _ => return Err(ApiError::internal("stored screen has an invalid page mode")),
        },
        show_page_controls: screen.show_page_controls,
        desired_open: screen.desired_open,
        display_id: screen.display_id,
        bounds: screen.bounds,
        fullscreen: screen.fullscreen,
        playback_layout: screen.playback_layout.map(wire_layout),
    })
}

fn domain_layout(layout: wire::ScreenPlaybackSurfaceLayout) -> light_show::PlaybackSurfaceLayout {
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

fn wire_layout(layout: light_show::PlaybackSurfaceLayout) -> wire::ScreenPlaybackSurfaceLayout {
    wire::ScreenPlaybackSurfaceLayout {
        playbacks_per_row: layout.playbacks_per_row,
        rows: layout
            .rows
            .into_iter()
            .map(|row| wire::ScreenPlaybackSurfaceRow {
                first_playback_slot: row.first_playback_slot,
                has_fader: row.has_fader,
                button_count: row.button_count,
            })
            .collect(),
    }
}

const fn page_mode_string(mode: wire::ScreenPageMode) -> &'static str {
    match mode {
        wire::ScreenPageMode::FollowMain => "follow_main",
        wire::ScreenPageMode::Independent => "independent",
    }
}

fn emit_screen_changed(state: &AppState, screen: &ScreenConfiguration) {
    emit(
        state,
        "screen_configuration_changed",
        serde_json::json!({"screen":screen}),
    );
}

fn outcome(
    screen: Option<ScreenConfiguration>,
    active_page: Option<u8>,
) -> Result<wire::ScreenConfigurationActionOutcome, ApiError> {
    Ok(wire::ScreenConfigurationActionOutcome {
        request_id: String::new(),
        replayed: false,
        screen: screen.map(wire_screen).transpose()?,
        active_page,
    })
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    session_id: Uuid,
    request_id: String,
}

#[derive(Clone)]
struct ReplayEntry {
    key: ReplayKey,
    action: wire::ScreenConfigurationAction,
    outcome: wire::ScreenConfigurationActionOutcome,
}

#[derive(Default)]
pub(super) struct ScreenConfigurationReplayCache {
    entries: VecDeque<ReplayEntry>,
}

impl ScreenConfigurationReplayCache {
    fn get(
        &self,
        key: &ReplayKey,
        action: &wire::ScreenConfigurationAction,
    ) -> Result<Option<wire::ScreenConfigurationActionOutcome>, ApiError> {
        let Some(entry) = self.entries.iter().find(|entry| &entry.key == key) else {
            return Ok(None);
        };
        if &entry.action != action {
            return Err(ApiError::conflict(
                "request_id was already used for a different screen action",
            ));
        }
        let mut outcome = entry.outcome.clone();
        outcome.replayed = true;
        Ok(Some(outcome))
    }

    fn insert(
        &mut self,
        key: ReplayKey,
        action: wire::ScreenConfigurationAction,
        outcome: wire::ScreenConfigurationActionOutcome,
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
