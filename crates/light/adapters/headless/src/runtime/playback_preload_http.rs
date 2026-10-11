//! Show-owned Preload ranges, revision guarded and replay safe.
use super::*;
use light_wire::v2::playback as wire;
use std::collections::VecDeque;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/playback-preload-scope", get(snapshot))
        .route("/api/v2/playback-preload-scope/update", post(update))
}
fn scope_wire(
    scope: &light_playback::PlaybackPreloadScope,
) -> Result<wire::PlaybackPreloadScope, ApiError> {
    serde_json::from_value(
        serde_json::to_value(scope).map_err(|e| ApiError::internal(e.to_string()))?,
    )
    .map_err(|e| ApiError::internal(e.to_string()))
}
async fn snapshot(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Json<wire::PlaybackPreloadScopeSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    let _activation = state.active_show.acquire().await;
    let show_id = show.resolve(&state)?;
    let stored = if state.active_show.in_recovery() {
        Default::default()
    } else {
        super::playback_policy::read_playback_preload(&state, show_id)?
    };
    Ok(Json(wire::PlaybackPreloadScopeSnapshot {
        show_id: show_id.0,
        revision: stored.revision,
        scope: scope_wire(&stored.scope)?,
    }))
}
async fn update(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::PlaybackPreloadScopeUpdateRequest>,
) -> Result<Json<wire::PlaybackPreloadScopeUpdateOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show_objects_v2::validate_request_id(&request.request_id)?;
    let scope: light_playback::PlaybackPreloadScope = serde_json::from_value(
        serde_json::to_value(&request.scope).map_err(|e| ApiError::bad_request(e.to_string()))?,
    )
    .map_err(|e| ApiError::bad_request(e.to_string()))?;
    scope.validate().map_err(ApiError::bad_request)?;
    let _activation = state.active_show.acquire().await;
    let show_id = show.resolve_writable(&state)?;
    let key = (session.id.0, request.request_id.clone());
    let action = (show_id.0, request.clone());
    state
        .replay
        .playback_preload_update(key, action, || {
            let context = light_application::ActionContext::operator(
                session.desk.id,
                session.id.0,
                light_application::ActionSource::Http,
            );
            let (changed, stored) = super::playback_policy::update_playback_preload(
                &state,
                &context,
                show_id,
                request.expected_revision,
                scope,
                &request.request_id,
            )?;
            let outcome = wire::PlaybackPreloadScopeUpdateOutcome {
                request_id: request.request_id,
                show_id: show_id.0,
                revision: stored.revision,
                scope: scope_wire(&stored.scope)?,
                replayed: false,
                changed,
            };
            if changed {
                state.events.publish(
                    light_application::EventDraft::playback_preload_scope_changed(
                        light_application::PlaybackPreloadScopeChange {
                            show_id,
                            revision: stored.revision,
                        },
                    ),
                );
            }
            Ok(outcome)
        })
        .map(Json)
}
#[derive(Default)]
pub(super) struct PreloadReplayCache {
    entries: VecDeque<(
        (Uuid, String),
        (Uuid, wire::PlaybackPreloadScopeUpdateRequest),
        wire::PlaybackPreloadScopeUpdateOutcome,
    )>,
}
impl PreloadReplayCache {
    pub(super) fn get(
        &self,
        key: &(Uuid, String),
        action: &(Uuid, wire::PlaybackPreloadScopeUpdateRequest),
    ) -> Result<Option<wire::PlaybackPreloadScopeUpdateOutcome>, ApiError> {
        let Some((_, previous, outcome)) = self
            .entries
            .iter()
            .find(|(candidate, _, _)| candidate == key)
        else {
            return Ok(None);
        };
        if previous != action {
            return Err(ApiError::conflict(
                "Preload scope request identity was reused for a different update",
            ));
        }
        let mut outcome = outcome.clone();
        outcome.replayed = true;
        Ok(Some(outcome))
    }
    pub(super) fn insert(
        &mut self,
        key: (Uuid, String),
        action: (Uuid, wire::PlaybackPreloadScopeUpdateRequest),
        outcome: wire::PlaybackPreloadScopeUpdateOutcome,
    ) {
        if self.entries.len() == 1024 {
            self.entries.pop_front();
        }
        self.entries.push_back((key, action, outcome));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_preserve_outcome_and_reject_reused_identity_across_show_or_scope() {
        let session = Uuid::new_v4();
        let show = Uuid::new_v4();
        let key = (session, "scope-edit".to_owned());
        let scope = wire::PlaybackPreloadScope {
            position_ranges: vec![wire::PlaybackPositionRange {
                start: 101,
                end: 115,
            }],
            pages: None,
        };
        let action = (
            show,
            wire::PlaybackPreloadScopeUpdateRequest {
                request_id: key.1.clone(),
                expected_revision: 3,
                scope: scope.clone(),
            },
        );
        let outcome = wire::PlaybackPreloadScopeUpdateOutcome {
            request_id: key.1.clone(),
            show_id: show,
            revision: 4,
            scope: scope.clone(),
            changed: true,
            replayed: false,
        };
        let mut cache = PreloadReplayCache::default();
        cache.insert(key.clone(), action.clone(), outcome);
        let replay = cache.get(&key, &action).unwrap().unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.revision, 4);
        assert_eq!(replay.scope.pages, None);
        let mut other = action.clone();
        other.0 = Uuid::new_v4();
        assert!(cache.get(&key, &other).is_err());
        other = action.clone();
        other.1.scope.pages = Some(vec![2]);
        assert!(cache.get(&key, &other).is_err());
        assert!(
            cache
                .get(&(Uuid::new_v4(), key.1), &action)
                .unwrap()
                .is_none()
        );
    }
}
