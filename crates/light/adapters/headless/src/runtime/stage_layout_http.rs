//! v2 stage-layout intent route: multi-fixture position edits as one server-side fan-out.
//!
//! The client sends the ordered selection plus one typed operation; the server resolves each
//! fixture's stored position (migrating legacy 2D entries with the same formula the stage views
//! use), applies the operation, and persists only the changed `positions3d` entries of the
//! active show's `stage_layout/main` object. Edits carry a client `request_id` absorbed by a
//! replay window (api-rules §3).

use super::show_objects_v2::active_entry;
use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::stage_layout::{
    StageLayoutAction, StageLayoutActionOutcome, StageLayoutActionRequest,
    StageLayoutErrorResponse, StagePositionAxis,
};
use std::collections::VecDeque;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v2/stage-layout/actions", post(stage_layout_action))
}

async fn stage_layout_action(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<StageLayoutActionRequest>,
) -> Result<Response, StageLayoutHttpError> {
    let session = authenticate(&state, &headers).map_err(StageLayoutHttpError::api)?;
    validate_request(&request)?;
    let show_id = context.resolve(&state).map_err(StageLayoutHttpError::api)?;
    let key = ReplayKey {
        desk_id: session.desk.id,
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let _activation = state.activation_lock.clone().lock_owned().await;
    if let Some(replayed) = state
        .stage_layout_replay
        .lock()
        .get(&key, &request.action)?
    {
        return Ok(json_with_etag(replayed.revision, replayed));
    }
    let entry = active_entry(&state, show_id).map_err(StageLayoutHttpError::api)?;
    let store = ShowStore::open(&entry.path)
        .map_err(|error| StageLayoutHttpError::api(ApiError::store(error)))?;
    let (_, object) = store
        .object_with_portable_revision("stage_layout", "main")
        .map_err(|error| StageLayoutHttpError::api(ApiError::store(error)))?;
    let (mut body, expected) = match object {
        Some(object) => (object.body, object.revision),
        None => (
            serde_json::json!({"version": 2, "positions": {}, "positions3d": {}}),
            0,
        ),
    };
    let patched_order: HashMap<Uuid, usize> = store
        .objects("patched_fixture")
        .map_err(|error| StageLayoutHttpError::api(ApiError::store(error)))?
        .iter()
        .enumerate()
        .filter_map(|(index, object)| Uuid::parse_str(&object.id).ok().map(|id| (id, index)))
        .collect();
    let StageLayoutAction::MoveSelection {
        fixture_ids,
        axis,
        delta,
    } = &request.action;
    let moved = apply_selection_move(&mut body, fixture_ids, *axis, *delta, &patched_order)?;
    let outcome = if moved.is_empty() {
        StageLayoutActionOutcome {
            request_id: request.request_id.clone(),
            revision: expected,
            moved_fixture_ids: moved,
            replayed: false,
            changed: false,
        }
    } else {
        let action = active_show_object_action(
            operator_action_context(&session, light_application::ActionSource::Http)
                .with_request_id(&request.request_id),
            show_id,
            vec![
                put_active_show_object(
                    light_application::ActiveShowObjectKind::StageLayout,
                    "main",
                    expected,
                    body,
                )
                .map_err(StageLayoutHttpError::api)?,
            ],
        );
        let (result, _activation) =
            run_active_show_object_action_async(&state, _activation, action)
                .await
                .map_err(StageLayoutHttpError::api)?;
        let change = result
            .changes
            .first()
            .expect("one stage-layout mutation returns one change");
        emit(
            &state,
            "show_object_changed",
            serde_json::json!({
                "show_id": show_id,
                "kind": "stage_layout",
                "id": "main",
                "revision": change.object_revision
            }),
        );
        StageLayoutActionOutcome {
            request_id: request.request_id.clone(),
            revision: change.object_revision,
            moved_fixture_ids: moved,
            replayed: false,
            changed: true,
        }
    };
    state
        .stage_layout_replay
        .lock()
        .insert(key, request.action, outcome.clone());
    Ok(json_with_etag(outcome.revision, outcome))
}

fn validate_request(request: &StageLayoutActionRequest) -> Result<(), StageLayoutHttpError> {
    if request.request_id.is_empty() || request.request_id.len() > 128 {
        return Err(StageLayoutHttpError::bad_request(
            "request_id must be between 1 and 128 characters",
        ));
    }
    let StageLayoutAction::MoveSelection {
        fixture_ids, delta, ..
    } = &request.action;
    if fixture_ids.is_empty() {
        return Err(StageLayoutHttpError::bad_request(
            "fixture_ids must contain at least one fixture",
        ));
    }
    if fixture_ids.len() > 10_000 {
        return Err(StageLayoutHttpError::bad_request(
            "fixture_ids must not exceed 10000 entries",
        ));
    }
    if !delta.is_finite() {
        return Err(StageLayoutHttpError::bad_request("delta must be finite"));
    }
    Ok(())
}

/// Applies one uniform axis delta across the ordered selection, mutating only the touched
/// `positions3d` entries of the raw layout body so every other stored field survives verbatim.
/// A selected fixture resolves its base position exactly like the stage views do: the stored 3D
/// entry, else the migrated legacy 2D entry, else — for patched fixtures — the default grid slot
/// for its authoritative patch index. Selected ids outside the patch are skipped.
fn apply_selection_move(
    body: &mut serde_json::Value,
    fixture_ids: &[Uuid],
    axis: StagePositionAxis,
    delta: f64,
    patched_order: &HashMap<Uuid, usize>,
) -> Result<Vec<Uuid>, StageLayoutHttpError> {
    let layout = body
        .as_object_mut()
        .ok_or_else(|| StageLayoutHttpError::conflict("stored stage layout is not an object"))?;
    let legacy: HashMap<Uuid, serde_json::Value> = position_map(layout.get("positions"))?;
    let positions3d = layout
        .entry("positions3d")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let positions3d = positions3d.as_object_mut().ok_or_else(|| {
        StageLayoutHttpError::conflict("stored stage layout positions3d is not an object")
    })?;
    let stored3d: HashMap<Uuid, String> = positions3d
        .keys()
        .filter_map(|key| Uuid::parse_str(key).ok().map(|id| (id, key.clone())))
        .collect();
    let mut moved = Vec::new();
    if delta == 0.0 {
        return Ok(moved);
    }
    for fixture_id in fixture_ids {
        if moved.contains(fixture_id) {
            continue;
        }
        if let Some(key) = stored3d.get(fixture_id) {
            let entry = positions3d
                .get_mut(key)
                .and_then(serde_json::Value::as_object_mut)
                .ok_or_else(|| {
                    StageLayoutHttpError::conflict(
                        "stored stage layout positions3d entry is not an object",
                    )
                })?;
            let base = entry
                .get(axis_key(axis))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            entry.insert(axis_key(axis).into(), json_number(base + delta)?);
            moved.push(*fixture_id);
        } else if let Some(position) = legacy.get(fixture_id) {
            let mut migrated = migrate_legacy_position(position)?;
            let base = migrated[axis_key(axis)].as_f64().unwrap_or(0.0);
            migrated[axis_key(axis)] = json_number(base + delta)?;
            positions3d.insert(fixture_id.to_string(), migrated);
            moved.push(*fixture_id);
        } else if let Some(index) = patched_order.get(fixture_id) {
            let mut defaulted = default_stage_position(*index);
            let base = defaulted[axis_key(axis)].as_f64().unwrap_or(0.0);
            defaulted[axis_key(axis)] = json_number(base + delta)?;
            positions3d.insert(fixture_id.to_string(), defaulted);
            moved.push(*fixture_id);
        }
    }
    Ok(moved)
}

fn position_map(
    positions: Option<&serde_json::Value>,
) -> Result<HashMap<Uuid, serde_json::Value>, StageLayoutHttpError> {
    let Some(positions) = positions else {
        return Ok(HashMap::new());
    };
    if positions.is_null() {
        return Ok(HashMap::new());
    }
    let map = positions.as_object().ok_or_else(|| {
        StageLayoutHttpError::conflict("stored stage layout positions is not an object")
    })?;
    Ok(map
        .iter()
        .filter_map(|(key, value)| Uuid::parse_str(key).ok().map(|id| (id, value.clone())))
        .collect())
}

/// Mirror of the stage views' `defaultStagePosition` grid formula
/// (`apps/light-desktop/src/windows/stage3dScene/positions.ts`); `index` is the fixture's slot in
/// the authoritative Patch order (object-id ascending — the order every patch surface renders).
fn default_stage_position(index: usize) -> serde_json::Value {
    serde_json::json!({
        "x": -5.25 + ((index % 8) as f64) * 1.5,
        "y": 1.0 + ((index / 8) as f64) * 1.6,
        "z": 5.0,
        "rotationX": 0.0,
        "rotationY": 0.0,
        "rotationZ": 0.0,
    })
}

/// Mirror of the stage views' `migrateStagePosition` percent-to-meter formula
/// (`apps/light-desktop/src/windows/stage3dScene/positions.ts`).
fn migrate_legacy_position(
    position: &serde_json::Value,
) -> Result<serde_json::Value, StageLayoutHttpError> {
    let read = |key: &str| position.get(key).and_then(serde_json::Value::as_f64);
    let (Some(x), Some(y)) = (read("x"), read("y")) else {
        return Err(StageLayoutHttpError::conflict(
            "stored 2D stage position is missing numeric x/y",
        ));
    };
    let rotation = read("rotation").unwrap_or(0.0);
    Ok(serde_json::json!({
        "x": (x / 100.0 - 0.5) * 12.0,
        "y": (y / 100.0) * 8.0,
        "z": 5.0,
        "rotationX": 0.0,
        "rotationY": 0.0,
        "rotationZ": rotation,
    }))
}

const fn axis_key(axis: StagePositionAxis) -> &'static str {
    match axis {
        StagePositionAxis::X => "x",
        StagePositionAxis::Y => "y",
        StagePositionAxis::Z => "z",
        StagePositionAxis::RotationX => "rotationX",
        StagePositionAxis::RotationY => "rotationY",
        StagePositionAxis::RotationZ => "rotationZ",
    }
}

fn json_number(value: f64) -> Result<serde_json::Value, StageLayoutHttpError> {
    serde_json::Number::from_f64(value)
        .map(serde_json::Value::Number)
        .ok_or_else(|| StageLayoutHttpError::bad_request("position value must stay finite"))
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    desk_id: Uuid,
    session_id: Uuid,
    request_id: String,
}

struct ReplayEntry {
    action: StageLayoutAction,
    outcome: StageLayoutActionOutcome,
}

/// Session-scoped idempotency window for stage-layout edits, following the
/// `show_patch::ReplayCache` pattern with entry-count eviction only (entries are tiny).
#[derive(Default)]
pub(super) struct StageLayoutReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl StageLayoutReplayCache {
    fn get(
        &self,
        key: &ReplayKey,
        action: &StageLayoutAction,
    ) -> Result<Option<StageLayoutActionOutcome>, StageLayoutHttpError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.action != action {
            return Err(StageLayoutHttpError::conflict(
                "request_id was already used for a different stage-layout action",
            ));
        }
        let mut replay = entry.outcome.clone();
        replay.replayed = true;
        Ok(Some(replay))
    }

    fn insert(
        &mut self,
        key: ReplayKey,
        action: StageLayoutAction,
        outcome: StageLayoutActionOutcome,
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

fn json_with_etag<T: serde::Serialize>(revision: u64, body: T) -> Response {
    let mut response = Json(body).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&format!("\"{revision}\"")) {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
}

struct StageLayoutHttpError {
    status: StatusCode,
    body: StageLayoutErrorResponse,
}

impl StageLayoutHttpError {
    fn api(error: ApiError) -> Self {
        Self {
            status: error.status,
            body: StageLayoutErrorResponse {
                retryable: error.status == StatusCode::SERVICE_UNAVAILABLE,
                error: error.message,
            },
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: StageLayoutErrorResponse {
                error: message.into(),
                retryable: false,
            },
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            body: StageLayoutErrorResponse {
                error: message.into(),
                retryable: false,
            },
        }
    }
}

impl IntoResponse for StageLayoutHttpError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
