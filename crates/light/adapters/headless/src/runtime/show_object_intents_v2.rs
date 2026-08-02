//! Typed v2 intent routes for remaining production show-object writers.

use super::show_objects_v2::{active_entry, object_record, validate_request_id};
use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::show_objects as wire;
use std::collections::VecDeque;

mod replay;

pub(super) use replay::*;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/dynamics/create", post(dynamic_create_action))
        .route("/api/v2/dynamics/{id}/move", post(dynamic_move_action))
        .route("/api/v2/dynamics/{id}/copy", post(dynamic_copy_action))
        .route("/api/v2/dynamics/{id}/delete", post(dynamic_delete_action))
        .route("/api/v2/dynamics/{id}/update", post(dynamic_update_action))
        .route(
            "/api/v2/dynamics/{id}/spatial-preview",
            post(dynamic_spatial_preview),
        )
        .route("/api/v2/user-layouts/{id}/update", post(user_layout_action))
        .route("/api/v2/patch/layers/{id}/update", post(patch_layer_action))
        .route("/api/v2/preload/record", post(preload_record_action))
}

async fn dynamic_create_action(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::DynamicCreateActionRequest>,
) -> Result<Json<wire::ShowObjectActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::DynamicCreate(request.definition.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let mut body = request.definition;
    let mut definition = decode_dynamic(body.clone())?;
    if definition.lanes.is_empty() {
        return Err(ApiError::bad_request(
            "a Dynamic is created only after its first valid lane is selected",
        ));
    }
    ensure_dynamic_pool_slot_free(&state, show_id, definition.pool_number, None)?;
    definition.id = Uuid::new_v4();
    definition.revision = 1;
    light_dynamics::validate_definition(&definition)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let id = definition.id.to_string();
    let object = body
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("Dynamic definition must be an object"))?;
    object.insert("id".into(), serde_json::Value::String(id.clone()));
    object.insert("revision".into(), serde_json::Value::from(1));
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::Dynamic,
            id.clone(),
            0,
            body,
        )?],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action).await?;
    let outcome = committed_outcome(
        &state,
        show_id,
        "dynamic",
        &id,
        request.request_id,
        Some(result.event_sequence),
    )?;
    emit_dynamic_object_event(&state, &outcome, "created");
    state
        .replay
        .insert_show_object_intent(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn dynamic_move_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::DynamicPoolActionRequest>,
) -> Result<Json<wire::ShowObjectActionOutcome>, ApiError> {
    dynamic_pool_action(state, id, context, headers, request, false).await
}

async fn dynamic_copy_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::DynamicPoolActionRequest>,
) -> Result<Json<wire::ShowObjectActionOutcome>, ApiError> {
    dynamic_pool_action(state, id, context, headers, request, true).await
}

async fn dynamic_pool_action(
    state: AppState,
    id: Uuid,
    context: ShowContext,
    headers: HeaderMap,
    request: wire::DynamicPoolActionRequest,
    copy: bool,
) -> Result<Json<wire::ShowObjectActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = if copy {
        ReplayAction::DynamicCopy(id, request.clone())
    } else {
        ReplayAction::DynamicMove(id, request.clone())
    };
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    ensure_dynamic_pool_slot_free(&state, show_id, request.pool_number, Some(id))?;
    let raw_body = load_body(&state, show_id, "dynamic", &id.to_string())?;
    let (object_revision, mut definition) = load_dynamic(&state, show_id, id)?;
    if object_revision != request.expected_revision {
        return Err(ApiError::conflict(format!(
            "Dynamic revision conflict: expected {}, current {object_revision}",
            request.expected_revision
        )));
    }
    let (target_id, expected_revision) = if copy {
        definition.id = Uuid::new_v4();
        definition.name = format!("{} Copy", definition.name);
        definition.revision = 1;
        (definition.id, 0)
    } else {
        definition.revision = definition.revision.saturating_add(1);
        (id, request.expected_revision)
    };
    definition.pool_number = request.pool_number;
    let body = if copy {
        let mut body = raw_body;
        let object = body
            .as_object_mut()
            .ok_or_else(|| ApiError::internal("stored Dynamic is not an object"))?;
        object.insert("id".into(), target_id.to_string().into());
        object.insert("pool_number".into(), request.pool_number.into());
        object.insert("revision".into(), 1.into());
        object.insert("name".into(), definition.name.clone().into());
        body
    } else {
        serde_json::to_value(definition).map_err(|error| ApiError::internal(error.to_string()))?
    };
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::Dynamic,
            target_id.to_string(),
            expected_revision,
            body,
        )?],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action).await?;
    let outcome = committed_outcome(
        &state,
        show_id,
        "dynamic",
        &target_id.to_string(),
        request.request_id,
        Some(result.event_sequence),
    )?;
    emit_dynamic_object_event(&state, &outcome, if copy { "copied" } else { "moved" });
    state
        .replay
        .insert_show_object_intent(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn dynamic_update_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::DynamicUpdateActionRequest>,
) -> Result<Json<wire::ShowObjectActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::DynamicUpdate(id, request.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let (object_revision, mut definition) = load_dynamic(&state, show_id, id)?;
    if object_revision != request.expected_revision {
        return Err(ApiError::conflict(format!(
            "Dynamic revision conflict: expected {}, current {object_revision}",
            request.expected_revision
        )));
    }
    if matches!(
        request.intent,
        wire::DynamicUpdateIntent::SetTargetBinding { .. }
    ) && state.output.is_dynamic_definition_running(id)
    {
        return Err(ApiError::conflict(
            "turn every running instance Off before changing Dynamic targets",
        ));
    }
    let snapshot = state.output.snapshot();
    apply_dynamic_update_intent(&mut definition, request.intent.clone(), &snapshot)?;
    definition.revision = definition.revision.saturating_add(1);
    light_dynamics::validate_definition(&definition)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::Dynamic,
            id.to_string(),
            request.expected_revision,
            serde_json::to_value(definition)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        )?],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action).await?;
    let outcome = committed_outcome(
        &state,
        show_id,
        "dynamic",
        &id.to_string(),
        request.request_id,
        Some(result.event_sequence),
    )?;
    emit_dynamic_object_event(&state, &outcome, "updated");
    state
        .replay
        .insert_show_object_intent(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn dynamic_spatial_preview(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<light_wire::v2::dynamics::DynamicSpatialPreviewRequest>,
) -> Result<Json<light_wire::v2::dynamics::DynamicSpatialPreviewResponse>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    let _activation = state.active_show.acquire().await;
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (show_revision, object) = store
        .object_with_portable_revision("dynamic", &id.to_string())
        .map_err(ApiError::store)?;
    if show_revision.value() != request.expected_show_revision {
        return Err(ApiError::conflict(format!(
            "active Show revision conflict: expected {}, current {}",
            request.expected_show_revision,
            show_revision.value()
        )));
    }
    let object = object.ok_or_else(|| ApiError::not_found("Dynamic does not exist"))?;
    if object.revision != request.expected_dynamic_revision {
        return Err(ApiError::conflict(format!(
            "Dynamic revision conflict: expected {}, current {}",
            request.expected_dynamic_revision, object.revision
        )));
    }
    let definition = decode_dynamic(object.body)?;
    let draft = decode_spatial_mapping(request.spatial_mapping.clone())?;
    let snapshot = state.output.snapshot();
    let context = dynamic_spatial_context(&snapshot, &definition)?;
    let ranked = light_dynamics::evaluate_dynamic_spatial_mapping(
        context.inherited_mapping.as_ref(),
        &draft,
        &context.targets,
        None,
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let ranks = ranked
        .ordered_fixture_ids
        .iter()
        .map(
            |fixture_id| light_wire::v2::group_management::GroupSpatialRankProjection {
                fixture_id: fixture_id.0,
                rank: ranked.rank_by_fixture[fixture_id],
            },
        )
        .collect();
    Ok(Json(
        light_wire::v2::dynamics::DynamicSpatialPreviewResponse {
            show_id: show_id.0,
            show_revision: show_revision.value(),
            dynamic_id: id,
            dynamic_revision: object.revision,
            target_binding: dynamic_target_binding_projection(&definition.target_binding),
            base: context.base,
            inherited_mapping: context
                .inherited_mapping
                .map(wire_group_spatial_mapping)
                .transpose()?,
            draft: request.spatial_mapping,
            source_order: context
                .source_order
                .iter()
                .map(|fixture_id| fixture_id.0)
                .collect(),
            ordered_fixture_ids: ranked
                .ordered_fixture_ids
                .iter()
                .map(|fixture_id| fixture_id.0)
                .collect(),
            ranks,
            rank_count: ranked.rank_count,
            warnings: ranked
                .warnings
                .into_iter()
                .map(wire_spatial_warning)
                .collect(),
        },
    ))
}

async fn dynamic_delete_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::DynamicDeleteActionRequest>,
) -> Result<Json<wire::ShowObjectActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::DynamicDelete(id, request.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (show_revision, object) = store
        .object_with_portable_revision("dynamic", &id.to_string())
        .map_err(ApiError::store)?;
    let object = object.ok_or_else(|| ApiError::not_found("Dynamic does not exist"))?;
    if object.revision != request.expected_revision {
        return Err(ApiError::conflict(format!(
            "Dynamic revision conflict: expected {}, current {}",
            request.expected_revision, object.revision
        )));
    }
    let definition = decode_dynamic(object.body.clone())?;
    let fallback = light_dynamics::DynamicDefinitionSnapshot {
        definition: Arc::new(definition.clone()),
    };
    let mut mutations = Vec::new();
    for kind in ["cue_list", "playback"] {
        let (_, objects) = store
            .objects_with_portable_revision(kind)
            .map_err(ApiError::store)?;
        for mut referenced in objects {
            if snapshot_deleted_dynamic_references(&mut referenced.body, id, &fallback) {
                mutations.push(put_active_show_object(
                    match kind {
                        "cue_list" => light_application::ActiveShowObjectKind::CueList,
                        "playback" => light_application::ActiveShowObjectKind::Playback,
                        _ => unreachable!(),
                    },
                    referenced.id,
                    referenced.revision,
                    referenced.body,
                )?);
            }
        }
    }
    mutations.push(delete_active_show_object(
        light_application::ActiveShowObjectKind::Dynamic,
        id.to_string(),
        request.expected_revision,
    ));
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        mutations,
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action).await?;
    let outcome = wire::ShowObjectActionOutcome {
        request_id: request.request_id,
        replayed: false,
        show_id: show_id.0,
        show_revision: show_revision.value().saturating_add(1),
        object: object_record(object),
        event_sequence: Some(result.event_sequence),
    };
    emit_dynamic_object_event(&state, &outcome, "deleted");
    state
        .replay
        .insert_show_object_intent(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
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

fn decode_dynamic(value: serde_json::Value) -> Result<light_dynamics::DynamicDefinition, ApiError> {
    serde_json::from_value(value)
        .map_err(|error| ApiError::bad_request(format!("invalid Dynamic definition: {error}")))
}

fn load_dynamic(
    state: &AppState,
    show_id: light_core::ShowId,
    id: Uuid,
) -> Result<(u64, light_dynamics::DynamicDefinition), ApiError> {
    let entry = active_entry(state, show_id)?;
    let (_, object) = ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .object_with_portable_revision("dynamic", &id.to_string())
        .map_err(ApiError::store)?;
    let object = object.ok_or_else(|| ApiError::not_found("Dynamic does not exist"))?;
    Ok((object.revision, decode_dynamic(object.body)?))
}

fn ensure_dynamic_pool_slot_free(
    state: &AppState,
    show_id: light_core::ShowId,
    pool_number: u16,
    except: Option<Uuid>,
) -> Result<(), ApiError> {
    if !(1..=9_999).contains(&pool_number) {
        return Err(ApiError::bad_request(
            "Dynamic pool number must be between 1 and 9999",
        ));
    }
    let entry = active_entry(state, show_id)?;
    let (_, objects) = ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .objects_with_portable_revision("dynamic")
        .map_err(ApiError::store)?;
    for object in objects {
        let definition = decode_dynamic(object.body)?;
        if definition.pool_number == pool_number && Some(definition.id) != except {
            return Err(ApiError::conflict(format!(
                "Dynamic pool slot {pool_number} is already occupied"
            )));
        }
    }
    Ok(())
}

fn apply_dynamic_update_intent(
    definition: &mut light_dynamics::DynamicDefinition,
    intent: wire::DynamicUpdateIntent,
    snapshot: &light_engine::EngineSnapshot,
) -> Result<(), ApiError> {
    use wire::DynamicUpdateIntent;
    match intent {
        DynamicUpdateIntent::SetName { name } => {
            if name.trim().is_empty() {
                return Err(ApiError::bad_request("Dynamic name must not be empty"));
            }
            definition.name = name;
        }
        DynamicUpdateIntent::SetColor { color } => definition.color = color,
        DynamicUpdateIntent::SetIcon { icon } => definition.icon = icon,
        DynamicUpdateIntent::SetTargetBinding { target_binding } => {
            definition.target_binding =
                serde_json::from_value(target_binding).map_err(|error| {
                    ApiError::bad_request(format!("invalid Dynamic target binding: {error}"))
                })?;
        }
        DynamicUpdateIntent::SetSpatialMapping { spatial_mapping } => {
            let mapping = decode_spatial_mapping(spatial_mapping)?;
            let context = dynamic_spatial_context(snapshot, definition)?;
            light_dynamics::evaluate_dynamic_spatial_mapping(
                context.inherited_mapping.as_ref(),
                &mapping,
                &context.targets,
                None,
            )
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
            definition.spatial_mapping = mapping;
        }
        DynamicUpdateIntent::AddLane { lane, index } => {
            let mut lane: light_dynamics::DynamicLane = decode_dynamic_part("lane", lane)?;
            seed_lane_phase(definition, &mut lane);
            let index = index.unwrap_or(definition.lanes.len());
            if index > definition.lanes.len() {
                return Err(ApiError::bad_request("Dynamic lane index is out of range"));
            }
            definition.lanes.insert(index, lane);
        }
        DynamicUpdateIntent::ReplaceLane { lane_id, lane } => {
            let mut lane: light_dynamics::DynamicLane = decode_dynamic_part("lane", lane)?;
            lane.id = lane_id;
            seed_lane_phase(definition, &mut lane);
            let stored = definition
                .lanes
                .iter_mut()
                .find(|candidate| candidate.id == lane_id)
                .ok_or_else(|| ApiError::not_found("Dynamic lane does not exist"))?;
            *stored = lane;
        }
        DynamicUpdateIntent::DeleteLane { lane_id } => {
            let before = definition.lanes.len();
            definition.lanes.retain(|lane| lane.id != lane_id);
            if definition.lanes.len() == before {
                return Err(ApiError::not_found("Dynamic lane does not exist"));
            }
        }
        DynamicUpdateIntent::MoveLane { lane_id, index } => {
            let Some(from) = definition.lanes.iter().position(|lane| lane.id == lane_id) else {
                return Err(ApiError::not_found("Dynamic lane does not exist"));
            };
            if index >= definition.lanes.len() {
                return Err(ApiError::bad_request("Dynamic lane index is out of range"));
            }
            let lane = definition.lanes.remove(from);
            definition.lanes.insert(index, lane);
        }
        DynamicUpdateIntent::SetPhase { phase } => {
            definition.phase = decode_dynamic_part("phase", phase)?;
        }
        DynamicUpdateIntent::SetPhaseMode { phase_mode } => {
            definition.phase_spread_mode = match phase_mode {
                light_wire::v2::dynamics::DynamicPhaseSpreadModeProjection::Uniform => {
                    light_dynamics::DynamicPhaseSpreadMode::Uniform
                }
                light_wire::v2::dynamics::DynamicPhaseSpreadModeProjection::PerLane => {
                    light_dynamics::DynamicPhaseSpreadMode::PerLane
                }
            };
            if definition.phase_spread_mode == light_dynamics::DynamicPhaseSpreadMode::PerLane {
                for lane in &mut definition.lanes {
                    if lane.phase.is_none() {
                        lane.phase = Some(definition.phase.clone());
                    }
                }
            }
        }
        DynamicUpdateIntent::SetSpeed { speed } => {
            definition.speed = decode_dynamic_part("speed", speed)?;
        }
        DynamicUpdateIntent::SetOverallSpeedMultiplier { multiplier } => {
            definition.overall_speed_multiplier = light_dynamics::Rational {
                numerator: multiplier.numerator,
                denominator: multiplier.denominator,
            };
        }
        DynamicUpdateIntent::SetRunMode { run_mode } => {
            definition.run_mode = match run_mode {
                light_wire::v2::dynamics::DynamicRunModeProjection::Loop => {
                    light_dynamics::DynamicRunMode::Loop
                }
                light_wire::v2::dynamics::DynamicRunModeProjection::OneShot => {
                    light_dynamics::DynamicRunMode::OneShot
                }
            };
        }
        DynamicUpdateIntent::SetActivation { activation } => {
            definition.default_activation = decode_dynamic_part("activation", activation)?;
        }
        DynamicUpdateIntent::SetActivationBoundary { boundary } => {
            definition.activation_boundary = match boundary {
                light_wire::v2::dynamics::DynamicActivationBoundaryProjection::Beat => {
                    light_dynamics::ActivationBoundary::Beat
                }
                light_wire::v2::dynamics::DynamicActivationBoundaryProjection::Bar => {
                    light_dynamics::ActivationBoundary::Bar
                }
            };
        }
        DynamicUpdateIntent::AddRandomGroup { group } => {
            definition
                .random_groups
                .push(decode_dynamic_part("Random group", group)?);
        }
        DynamicUpdateIntent::ReplaceRandomGroup { group_id, group } => {
            let mut group: light_dynamics::DynamicRandomGroup =
                decode_dynamic_part("Random group", group)?;
            group.id = group_id;
            let stored = definition
                .random_groups
                .iter_mut()
                .find(|candidate| candidate.id == group_id)
                .ok_or_else(|| ApiError::not_found("Dynamic Random group does not exist"))?;
            *stored = group;
        }
        DynamicUpdateIntent::DeleteRandomGroup { group_id } => {
            let before = definition.random_groups.len();
            definition
                .random_groups
                .retain(|group| group.id != group_id);
            if definition.random_groups.len() == before {
                return Err(ApiError::not_found("Dynamic Random group does not exist"));
            }
        }
    }
    Ok(())
}

struct DynamicSpatialContext {
    source_order: Vec<light_core::FixtureId>,
    targets: Vec<light_dynamics::SpatialTarget>,
    inherited_mapping: Option<light_dynamics::SpatialSelectionMapping>,
    base: light_wire::v2::dynamics::DynamicSpatialPreviewBaseProjection,
}

fn dynamic_spatial_context(
    snapshot: &light_engine::EngineSnapshot,
    definition: &light_dynamics::DynamicDefinition,
) -> Result<DynamicSpatialContext, ApiError> {
    use light_dynamics::DynamicTargetBinding;
    let positions = snapshot
        .dynamic_stage_positions
        .iter()
        .map(|(fixture_id, position)| {
            (
                *fixture_id,
                light_dynamics::Position3d {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                    z: f64::from(position.z),
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let target = |fixture_id: light_core::FixtureId| light_dynamics::SpatialTarget {
        fixture_id,
        position: positions.get(&fixture_id).copied(),
    };
    match &definition.target_binding {
        DynamicTargetBinding::LiveGroup { group_id } => {
            let groups = snapshot
                .groups
                .iter()
                .cloned()
                .map(|group| (group.id.clone(), group))
                .collect();
            let resolved = light_programmer::resolve_group_spatial(group_id, &groups, &positions)
                .map_err(ApiError::bad_request)?;
            let source_order = resolved.source_order;
            let targets = source_order.iter().copied().map(target).collect();
            Ok(DynamicSpatialContext {
                source_order,
                targets,
                inherited_mapping: resolved.effective_mapping,
                base: light_wire::v2::dynamics::DynamicSpatialPreviewBaseProjection::LiveGroup {
                    group_id: group_id.clone(),
                    mapping_provenance: wire_group_mapping_provenance(resolved.mapping_provenance),
                },
            })
        }
        DynamicTargetBinding::FrozenTargets { targets } => {
            let source_order = targets.clone();
            let targets = source_order.iter().copied().map(target).collect();
            Ok(DynamicSpatialContext {
                source_order,
                targets,
                inherited_mapping: None,
                base:
                    light_wire::v2::dynamics::DynamicSpatialPreviewBaseProjection::FrozenTargets {},
            })
        }
        DynamicTargetBinding::Targetless => Ok(DynamicSpatialContext {
            source_order: Vec::new(),
            targets: Vec::new(),
            inherited_mapping: None,
            base: light_wire::v2::dynamics::DynamicSpatialPreviewBaseProjection::Targetless {},
        }),
    }
}

fn decode_spatial_mapping(
    value: light_wire::v2::dynamics::DynamicSpatialMappingOverrideProjection,
) -> Result<light_dynamics::DynamicSpatialMappingOverride, ApiError> {
    serde_json::to_value(value)
        .map_err(|error| ApiError::internal(error.to_string()))
        .and_then(|value| {
            serde_json::from_value(value).map_err(|error| {
                ApiError::bad_request(format!("invalid Dynamic spatial mapping: {error}"))
            })
        })
}

fn dynamic_target_binding_projection(
    value: &light_dynamics::DynamicTargetBinding,
) -> light_wire::v2::dynamics::DynamicTargetBindingProjection {
    match value {
        light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
            light_wire::v2::dynamics::DynamicTargetBindingProjection::LiveGroup {
                group_id: group_id.clone(),
            }
        }
        light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => {
            light_wire::v2::dynamics::DynamicTargetBindingProjection::FrozenTargets {
                targets: targets.iter().map(|fixture_id| fixture_id.0).collect(),
            }
        }
        light_dynamics::DynamicTargetBinding::Targetless => {
            light_wire::v2::dynamics::DynamicTargetBindingProjection::Targetless
        }
    }
}

fn wire_group_spatial_mapping(
    value: light_dynamics::SpatialSelectionMapping,
) -> Result<light_wire::v2::group_management::GroupSpatialSelectionMapping, ApiError> {
    serde_json::to_value(value)
        .map_err(|error| ApiError::internal(error.to_string()))
        .and_then(|value| {
            serde_json::from_value(value).map_err(|error| ApiError::internal(error.to_string()))
        })
}

fn wire_group_mapping_provenance(
    value: light_programmer::GroupMappingProvenance,
) -> light_wire::v2::group_management::GroupMappingProvenanceProjection {
    use light_wire::v2::group_management::GroupMappingProvenanceProjection as Wire;
    match value {
        light_programmer::GroupMappingProvenance::None => Wire::None {},
        light_programmer::GroupMappingProvenance::Local { group_id } => Wire::Local { group_id },
        light_programmer::GroupMappingProvenance::Inherited { source_group_ids } => {
            Wire::Inherited { source_group_ids }
        }
        light_programmer::GroupMappingProvenance::MixedSourceMappings => {
            Wire::MixedSourceMappings {}
        }
    }
}

fn wire_spatial_warning(
    value: light_dynamics::SpatialMappingWarning,
) -> light_wire::v2::group_management::GroupSpatialWarningProjection {
    match value {
        light_dynamics::SpatialMappingWarning::MissingPosition { fixture_id } => {
            light_wire::v2::group_management::GroupSpatialWarningProjection::MissingPosition {
                fixture_id: fixture_id.0,
            }
        }
    }
}

fn seed_lane_phase(
    definition: &light_dynamics::DynamicDefinition,
    lane: &mut light_dynamics::DynamicLane,
) {
    if definition.phase_spread_mode == light_dynamics::DynamicPhaseSpreadMode::PerLane
        && lane.phase.is_none()
    {
        lane.phase = Some(definition.phase.clone());
    }
}

fn decode_dynamic_part<T: serde::de::DeserializeOwned>(
    label: &str,
    value: serde_json::Value,
) -> Result<T, ApiError> {
    serde_json::from_value(value)
        .map_err(|error| ApiError::bad_request(format!("invalid Dynamic {label}: {error}")))
}

pub(super) fn snapshot_deleted_dynamic_references(
    value: &mut serde_json::Value,
    dynamic_id: Uuid,
    fallback: &light_dynamics::DynamicDefinitionSnapshot,
) -> bool {
    let mut changed = false;
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                changed |= snapshot_deleted_dynamic_references(value, dynamic_id, fallback);
            }
        }
        serde_json::Value::Object(object) => {
            let matches = object
                .get("dynamic_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value == dynamic_id.to_string());
            if matches {
                object.insert("dynamic_id".into(), serde_json::Value::Null);
                if let Ok(fallback) = serde_json::to_value(fallback) {
                    object.insert("embedded_fallback".into(), fallback);
                }
                changed = true;
            }
            for value in object.values_mut() {
                changed |= snapshot_deleted_dynamic_references(value, dynamic_id, fallback);
            }
        }
        _ => {}
    }
    changed
}

fn emit_dynamic_object_event(
    state: &AppState,
    outcome: &wire::ShowObjectActionOutcome,
    operation: &str,
) {
    emit(
        state,
        "dynamic_object_changed",
        serde_json::json!({
            "show_id": outcome.show_id,
            "dynamic_id": outcome.object.id,
            "object_revision": outcome.object.revision,
            "operation": operation,
        }),
    );
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

fn validate_printable_id(label: &str, value: &str) -> Result<(), ApiError> {
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(ApiError::bad_request(format!(
            "{label} must contain 1-128 printable characters"
        )));
    }
    Ok(())
}
