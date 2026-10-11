//! Desk-isolated surface configuration and live page/context selection.
use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::playback_surfaces as wire;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/playback-surfaces", get(snapshot))
        .route(
            "/api/v2/playback-surfaces/{surface_id}/update",
            post(update),
        )
        .route(
            "/api/v2/playback-surfaces/{surface_id}/select",
            post(select),
        )
        .route("/api/v2/playback-surfaces/{surface_id}/page", post(page))
}
pub(in crate::runtime) fn surface_snapshot(
    state: &AppState,
    desk_id: Uuid,
) -> Result<wire::PlaybackSurfacesSnapshot, ApiError> {
    let surfaces = state
        .installation
        .playback_surfaces(desk_id)
        .map_err(ApiError::store)?;
    let resolved_pages =
        light_show::resolve_playback_surface_pages(&surfaces).map_err(ApiError::store)?;
    let selected_surface_id = state
        .installation
        .selected_playback_surface(desk_id)
        .map_err(ApiError::store)?;
    let command_page = *resolved_pages
        .get(&selected_surface_id)
        .ok_or_else(|| ApiError::bad_request("Selected Playback surface is missing"))?;
    let playback_surfaces = serde_json::from_value(
        serde_json::to_value(surfaces).map_err(|e| ApiError::internal(e.to_string()))?,
    )
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(wire::PlaybackSurfacesSnapshot {
        playback_surfaces,
        selected_surface_id,
        resolved_pages,
        command_page,
    })
}
async fn snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::PlaybackSurfacesSnapshot>, ApiError> {
    let session = authenticate(&state, &headers)?;
    surface_snapshot(&state, session.desk.id).map(Json)
}
async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(surface_id): Path<String>,
    TolerantJson(request): TolerantJson<wire::PlaybackSurfacesUpdateRequest>,
) -> Result<Json<wire::PlaybackSurfacesSnapshot>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show_objects_v2::validate_request_id(&request.request_id)?;
    let key = (session.id.0, request.request_id.clone());
    state
        .replay
        .playback_surfaces_update(key, (surface_id.clone(), request.clone()), || {
            state
                .installation
                .update_playback_surface(session.desk.id, &surface_id, |previous| {
                    let mut value = previous
                        .map(|surface| serde_json::to_value(surface))
                        .transpose()
                        .map_err(|e| light_show::StoreError::Invalid(e.to_string()))?
                        .unwrap_or_else(|| serde_json::json!({}));
                    if let Some(name) = request.name {
                        value["name"] = serde_json::json!(name);
                    }
                    if let Some(layout) = request.layout {
                        value["layout"] = serde_json::json!(layout);
                    }
                    if let Some(page_source) = request.page_source {
                        value["page_source"] = serde_json::json!(page_source);
                    }
                    serde_json::from_value(value).map_err(|e| {
                        light_show::StoreError::Invalid(format!("Playback surface: {e}"))
                    })
                })
                .map_err(ApiError::store)?;
            changed(&state, &session)?;
            surface_snapshot(&state, session.desk.id)
        })
        .map(Json)
}

async fn select(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(surface_id): Path<String>,
) -> Result<Json<wire::PlaybackSurfacesSnapshot>, ApiError> {
    let session = authenticate(&state, &headers)?;
    state
        .installation
        .select_playback_surface(session.desk.id, &surface_id)
        .map_err(ApiError::store)?;
    changed(&state, &session)?;
    surface_snapshot(&state, session.desk.id).map(Json)
}
async fn page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(surface_id): Path<String>,
    TolerantJson(request): TolerantJson<wire::PlaybackSurfacePageRequest>,
) -> Result<Json<wire::PlaybackSurfacesSnapshot>, ApiError> {
    let session = authenticate(&state, &headers)?;
    state
        .installation
        .set_playback_surface_page(session.desk.id, &surface_id, request.page)
        .map_err(ApiError::store)?;
    changed(&state, &session)?;
    surface_snapshot(&state, session.desk.id).map(Json)
}
pub(super) fn changed(state: &AppState, session: &Session) -> Result<(), ApiError> {
    let context = light_application::ActionContext::operator(
        session.desk.id,
        session.id.0,
        light_application::ActionSource::Http,
    );
    if state.active_show.current().is_some() {
        let projection = super::playback_service::desk::projection(state, &context)?;
        state
            .events
            .publish(light_application::EventDraft::playback_view_changed(
                &context, projection,
            ));
    }
    emit(
        state,
        "playback_surfaces_changed",
        serde_json::json!({"desk_id":session.desk.id}),
    );
    send_osc_feedback(state, false);
    Ok(())
}

#[derive(Default)]
pub(super) struct SurfaceReplayCache {
    entries: std::collections::VecDeque<(
        (Uuid, String),
        (String, wire::PlaybackSurfacesUpdateRequest),
        wire::PlaybackSurfacesSnapshot,
    )>,
}
impl SurfaceReplayCache {
    pub(super) fn apply(
        &mut self,
        key: (Uuid, String),
        request: (String, wire::PlaybackSurfacesUpdateRequest),
        apply: impl FnOnce() -> Result<wire::PlaybackSurfacesSnapshot, ApiError>,
    ) -> Result<wire::PlaybackSurfacesSnapshot, ApiError> {
        if let Some((_, previous, outcome)) = self
            .entries
            .iter()
            .find(|(candidate, _, _)| *candidate == key)
        {
            if previous != &request {
                return Err(ApiError::conflict(
                    "Surface update request identity was reused for different content",
                ));
            }
            return Ok(outcome.clone());
        }
        let outcome = apply()?;
        if self.entries.len() == 1024 {
            self.entries.pop_front();
        }
        self.entries.push_back((key, request, outcome.clone()));
        Ok(outcome)
    }
}

pub(super) fn select_live(
    state: &AppState,
    session: &Session,
    id: &str,
) -> Result<serde_json::Value, String> {
    state
        .installation
        .select_playback_surface(session.desk.id, id)
        .map_err(|e| e.to_string())?;
    changed(state, session).map_err(|e| e.message)?;
    serde_json::to_value(surface_snapshot(state, session.desk.id).map_err(|e| e.message)?)
        .map_err(|e| e.to_string())
}
pub(super) fn page_live(
    state: &AppState,
    session: &Session,
    id: &str,
    page: u8,
) -> Result<serde_json::Value, String> {
    state
        .installation
        .set_playback_surface_page(session.desk.id, id, page)
        .map_err(|e| e.to_string())?;
    changed(state, session).map_err(|e| e.message)?;
    serde_json::to_value(surface_snapshot(state, session.desk.id).map_err(|e| e.message)?)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_identity_includes_surface_and_changed_fields() {
        let key = (Uuid::new_v4(), "rename".to_owned());
        let patch = wire::PlaybackSurfacesUpdateRequest {
            request_id: key.1.clone(),
            name: Some("Main renamed".into()),
            layout: None,
            page_source: None,
        };
        let action = ("main".to_owned(), patch.clone());
        let snapshot = wire::PlaybackSurfacesSnapshot {
            playback_surfaces: Default::default(),
            selected_surface_id: "main".into(),
            resolved_pages: Default::default(),
            command_page: 1,
        };
        let mut cache = SurfaceReplayCache::default();
        cache
            .apply(key.clone(), action.clone(), || Ok(snapshot.clone()))
            .unwrap();
        let replay = cache
            .apply(key.clone(), action, || {
                panic!("an accepted edit must not execute again")
            })
            .unwrap();
        assert_eq!(replay, snapshot);
        assert!(
            cache
                .apply(key.clone(), ("second".into(), patch.clone()), || panic!(
                    "reused identity must not mutate another surface"
                ))
                .is_err()
        );
        let mut changed = patch;
        changed.name = Some("Another rename".into());
        assert!(
            cache
                .apply(key, ("main".into(), changed), || panic!(
                    "changed retry must not execute"
                ))
                .is_err()
        );
    }
}
