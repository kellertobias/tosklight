//! Typed v2 intent routes for remaining production show-object writers.

use super::show_objects_v2::{active_entry, object_record, validate_request_id};
use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::show_objects as wire;
use std::collections::VecDeque;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/user-layouts/{id}/update", post(user_layout_action))
        .route("/api/v2/patch/layers/{id}/update", post(patch_layer_action))
        .route(
            "/api/v2/cue-lists/{id}/dynamics/record",
            post(dynamic_record_action),
        )
        .route("/api/v2/preload/record", post(preload_record_action))
}

async fn user_layout_action(
    State(state): State<AppState>,
    Path(object_id): Path<String>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::UserLayoutActionRequest>,
) -> Result<Json<wire::ShowObjectActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::UserLayout(request.action.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    validate_printable_id("user layout id", &object_id)?;
    if object_id != session.user.id.0.to_string() {
        return Err(ApiError::forbidden(
            "a user layout can only be updated by its owning user",
        ));
    }
    let _activation = state.active_show.acquire().await;
    let wire::UserLayoutAction::Update {
        expected_revision,
        patch,
    } = request.action;
    let mut body = load_body(&state, show_id, "user_layout", &object_id)?;
    let object = body
        .as_object_mut()
        .ok_or_else(|| ApiError::internal("stored user layout is not an object"))?;
    if let Some(desks) = patch.desks {
        object.insert("desks".into(), serde_json::Value::Array(desks));
    }
    if let Some(active_desk_id) = patch.active_desk_id {
        object.insert("activeDeskId".into(), active_desk_id.into());
    }
    if let Some(window_settings) = patch.window_settings {
        object.insert("windowSettings".into(), window_settings);
    }
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::UserLayout,
            object_id.clone(),
            expected_revision,
            body,
        )?],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, _activation, action).await?;
    let change = result
        .changes
        .first()
        .expect("one user-layout mutation returns one change");
    emit(
        &state,
        "show_object_changed",
        serde_json::json!({
            "show_id":show_id,
            "kind":"user_layout",
            "id":object_id,
            "revision":change.object_revision
        }),
    );
    let outcome = committed_outcome(
        &state,
        show_id,
        "user_layout",
        &object_id,
        request.request_id,
        Some(result.event_sequence),
    )?;
    state
        .replay
        .insert_show_object_intent(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn patch_layer_action(
    State(state): State<AppState>,
    Path(layer_id): Path<String>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::PatchLayerActionRequest>,
) -> Result<Json<wire::ShowObjectActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::PatchLayer(request.action.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let _activation = state.active_show.acquire().await;
    let wire::PatchLayerAction::Save {
        expected_revision,
        layer,
    } = request.action;
    validate_printable_id("layer id", &layer_id)?;
    if layer.name.trim().is_empty() {
        return Err(ApiError::bad_request("patch layer name must not be empty"));
    }
    let mut body = load_body(&state, show_id, "patch_layer", &layer_id)?;
    let object = body
        .as_object_mut()
        .ok_or_else(|| ApiError::internal("stored patch layer is not an object"))?;
    object.insert("id".into(), layer_id.clone().into());
    object.insert("name".into(), layer.name.into());
    object.insert("order".into(), layer.order.into());
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::PatchLayer,
            layer_id.clone(),
            expected_revision,
            body,
        )?],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, _activation, action).await?;
    let change = result
        .changes
        .first()
        .expect("one patch-layer mutation returns one change");
    emit(
        &state,
        "show_object_changed",
        serde_json::json!({
            "show_id":show_id,
            "kind":"patch_layer",
            "id":layer_id,
            "revision":change.object_revision
        }),
    );
    let outcome = committed_outcome(
        &state,
        show_id,
        "patch_layer",
        &layer_id,
        request.request_id,
        Some(result.event_sequence),
    )?;
    state
        .replay
        .insert_show_object_intent(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn dynamic_record_action(
    State(state): State<AppState>,
    Path(cue_list_id): Path<String>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::DynamicRecordActionRequest>,
) -> Result<Json<wire::ShowObjectActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::Dynamic(request.action.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    let wire::DynamicRecordAction::Append {
        expected_revision,
        speed,
        width,
        direction,
        fixture_ids,
        group_ids,
    } = request.action;
    if !speed.is_finite() || speed <= 0.0 {
        return Err(ApiError::bad_request(
            "dynamic speed must be finite and positive",
        ));
    }
    if !width.is_finite() || !(0.0..=100.0).contains(&width) {
        return Err(ApiError::bad_request(
            "dynamic width must be finite and between 0 and 100",
        ));
    }
    if fixture_ids.is_empty() && group_ids.is_empty() {
        return Err(ApiError::bad_request(
            "dynamic recording requires fixtures or groups",
        ));
    }
    let mut body = load_existing_body(&state, show_id, "cue_list", &cue_list_id)?;
    append_dynamic(&mut body, speed, width, direction, fixture_ids, group_ids)?;
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::CueList,
            cue_list_id.clone(),
            expected_revision,
            body,
        )?],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action).await?;
    emit(
        &state,
        "show_object_changed",
        serde_json::json!({
            "show_id": show_id,
            "kind": "cue_list",
            "id": cue_list_id,
            "revision": result.changes[0].object_revision
        }),
    );
    let outcome = committed_outcome(
        &state,
        show_id,
        "cue_list",
        &cue_list_id,
        request.request_id,
        Some(result.event_sequence),
    )?;
    state
        .replay
        .insert_show_object_intent(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn preload_record_action(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::PreloadRecordActionRequest>,
) -> Result<Json<wire::ShowObjectActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::Preload(request.action.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let (input, expected_revision) = preload_input(request.action);
    let stored =
        super::store_api::store_preload_intent(&state, &session, show_id, input, expected_revision)
            .await?;
    emit(
        &state,
        "preload_stored",
        serde_json::json!({
            "session_id":session.id,
            "target":stored.kind,
            "target_id":stored.object_id,
            "revision":stored.revision,
            "source":stored.source
        }),
    );
    let outcome = committed_outcome(
        &state,
        show_id,
        &stored.kind,
        &stored.object_id,
        request.request_id,
        stored.event_sequence,
    )?;
    state
        .replay
        .insert_show_object_intent(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

fn preload_input(action: wire::PreloadRecordAction) -> (PreloadStoreInput, u64) {
    match action {
        wire::PreloadRecordAction::Preset {
            target_id,
            expected_revision,
            name,
            mode,
            family,
        } => (
            PreloadStoreInput {
                target: "preset".into(),
                target_id,
                cue_number: None,
                name: Some(name),
                mode: Some(match mode {
                    wire::PreloadPresetMode::Merge => light_programmer::PresetStoreMode::Merge,
                    wire::PreloadPresetMode::Overwrite => {
                        light_programmer::PresetStoreMode::Overwrite
                    }
                    wire::PreloadPresetMode::AddMissingFixtures => {
                        light_programmer::PresetStoreMode::AddMissingFixtures
                    }
                }),
                family: Some(match family {
                    wire::PreloadPresetFamily::Mixed => light_programmer::PresetFamily::Mixed,
                    wire::PreloadPresetFamily::Intensity => {
                        light_programmer::PresetFamily::Intensity
                    }
                    wire::PreloadPresetFamily::Color => light_programmer::PresetFamily::Color,
                    wire::PreloadPresetFamily::Position => light_programmer::PresetFamily::Position,
                    wire::PreloadPresetFamily::Beam => light_programmer::PresetFamily::Beam,
                }),
            },
            expected_revision,
        ),
        wire::PreloadRecordAction::Cue {
            cue_list_id,
            expected_revision,
            cue_number,
            name,
        } => (
            PreloadStoreInput {
                target: "cue".into(),
                target_id: cue_list_id,
                cue_number: Some(cue_number),
                name,
                mode: None,
                family: None,
            },
            expected_revision,
        ),
    }
}

fn append_dynamic(
    body: &mut serde_json::Value,
    speed: f64,
    width: f64,
    direction: wire::DynamicDirection,
    fixture_ids: Vec<Uuid>,
    group_ids: Vec<String>,
) -> Result<(), ApiError> {
    let cues = body
        .get_mut("cues")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| ApiError::bad_request("the Cuelist needs at least one Cue"))?;
    let cue = cues
        .first_mut()
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| ApiError::bad_request("the Cuelist needs at least one Cue"))?;
    let phasers = cue
        .entry("phasers")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| ApiError::bad_request("Cue phasers must be an array"))?;
    let (phase_start_degrees, phase_end_degrees) = match direction {
        wire::DynamicDirection::Forward => (0, 360),
        wire::DynamicDirection::Reverse => (360, 0),
    };
    phasers.push(serde_json::json!({
        "fixture_ids": fixture_ids,
        "group_ids": group_ids,
        "attribute": "intensity",
        "phaser": {
            "mode": "relative",
            "steps": [
                {"position": 0, "value": 0, "curve_to_next": "sine"},
                {"position": 0.5, "value": 1, "curve_to_next": "sine"}
            ],
            "cycles_per_minute": speed,
            "phase_start_degrees": phase_start_degrees,
            "phase_end_degrees": phase_end_degrees,
            "width": width / 100.0
        }
    }));
    Ok(())
}

fn committed_outcome(
    state: &AppState,
    show_id: light_core::ShowId,
    kind: &str,
    object_id: &str,
    request_id: String,
    event_sequence: Option<u64>,
) -> Result<wire::ShowObjectActionOutcome, ApiError> {
    let entry = active_entry(state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (show_revision, object) = store
        .object_with_portable_revision(kind, object_id)
        .map_err(ApiError::store)?;
    Ok(wire::ShowObjectActionOutcome {
        request_id,
        replayed: false,
        show_id: show_id.0,
        show_revision: show_revision.value(),
        object: object_record(
            object.ok_or_else(|| ApiError::internal("committed show object is missing"))?,
        ),
        event_sequence,
    })
}

fn load_body(
    state: &AppState,
    show_id: light_core::ShowId,
    kind: &str,
    object_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let entry = active_entry(state, show_id)?;
    Ok(ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .object_with_portable_revision(kind, object_id)
        .map_err(ApiError::store)?
        .1
        .map_or_else(|| serde_json::json!({}), |object| object.body))
}

fn load_existing_body(
    state: &AppState,
    show_id: light_core::ShowId,
    kind: &str,
    object_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let entry = active_entry(state, show_id)?;
    ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .object_with_portable_revision(kind, object_id)
        .map_err(ApiError::store)?
        .1
        .map(|object| object.body)
        .ok_or_else(|| ApiError::not_found("show object"))
}

fn validate_printable_id(label: &str, value: &str) -> Result<(), ApiError> {
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(ApiError::bad_request(format!(
            "{label} must contain 1-128 printable characters"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ReplayAction {
    UserLayout(wire::UserLayoutAction),
    PatchLayer(wire::PatchLayerAction),
    Dynamic(wire::DynamicRecordAction),
    Preload(wire::PreloadRecordAction),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    session_id: Uuid,
    show_id: light_core::ShowId,
    request_id: String,
}

impl ReplayKey {
    fn new(session: &Session, show_id: light_core::ShowId, request_id: &str) -> Self {
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
pub(super) struct ShowObjectIntentReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl ShowObjectIntentReplayCache {
    pub(super) fn get(
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

    pub(super) fn insert(
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
