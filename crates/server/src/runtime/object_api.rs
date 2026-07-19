use super::object_normalization::normalize_object_body;
use super::*;

mod output_routes;

use output_routes::*;

pub(super) async fn list_objects(
    State(state): State<AppState>,
    Path((id, kind)): Path<(Uuid, String)>,
) -> Result<Json<Vec<light_show::VersionedObject>>, ApiError> {
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let mut objects = ShowStore::open(entry.path)
        .map_err(ApiError::store)?
        .objects(&kind)
        .map_err(ApiError::store)?;
    if kind == "group" {
        materialize_derived_group_memberships(&mut objects);
    }
    if kind == "preset" {
        materialize_preset_addresses(&mut objects)?;
    }
    Ok(Json(objects))
}
pub(super) async fn get_object(
    State(state): State<AppState>,
    Path((id, kind, object_id)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let mut objects = ShowStore::open(entry.path)
        .map_err(ApiError::store)?
        .objects(&kind)
        .map_err(ApiError::store)?;
    if kind == "group" {
        materialize_derived_group_memberships(&mut objects);
    }
    if kind == "preset" {
        materialize_preset_addresses(&mut objects)?;
    }
    let object = objects
        .into_iter()
        .find(|object| object.id == object_id)
        .ok_or_else(|| ApiError::not_found("show object"))?;
    Ok((
        [(header::ETAG, format!("\"{}\"", object.revision))],
        Json(object),
    )
        .into_response())
}

pub(super) fn materialize_derived_group_memberships(objects: &mut [light_show::VersionedObject]) {
    let groups = objects
        .iter()
        .filter_map(|object| {
            serde_json::from_value::<light_programmer::GroupDefinition>(object.body.clone())
                .ok()
                .map(|mut group| {
                    group.id = object.id.clone();
                    (group.id.clone(), group)
                })
        })
        .collect::<HashMap<_, _>>();
    for object in objects {
        let Ok(mut group) =
            serde_json::from_value::<light_programmer::GroupDefinition>(object.body.clone())
        else {
            continue;
        };
        group.id = object.id.clone();
        let Ok(fixtures) = light_programmer::resolve_group(&group.id, &groups) else {
            continue;
        };
        group.fixtures = fixtures;
        if let Ok(body) = serde_json::to_value(group) {
            object.body = body;
        }
    }
}
pub(super) fn materialize_preset_addresses(
    objects: &mut [light_show::VersionedObject],
) -> Result<(), ApiError> {
    for object in objects {
        let (_, preset) = decode_preset_object(object).map_err(ApiError::bad_request)?;
        object.body = serialize_preset_preserving_extensions(&object.body, &preset)
            .map_err(|error| ApiError::internal(error.to_string()))?;
    }
    Ok(())
}

fn validate_object_candidate(
    state: &AppState,
    entry: &ShowEntry,
    kind: &str,
    object_id: &str,
    body: &serde_json::Value,
    active: bool,
) -> Result<(), ApiError> {
    if !active && !matches!(kind, "patched_fixture" | "playback" | "playback_page") {
        return Ok(());
    }
    let candidate = load_engine_snapshot_with_override(entry, Some((kind, object_id, body)))
        .map_err(show_load_api_error)?;
    if active || matches!(kind, "playback" | "playback_page") {
        state.engine.validate_snapshot_for_runtime(&candidate)
    } else {
        candidate.validate()
    }
    .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn activate_object_change(
    state: &AppState,
    entry: &ShowEntry,
    kind: &str,
    body: &serde_json::Value,
) -> Result<(), ApiError> {
    let prepared = prepare_show_for_runtime(state, entry)?;
    state.engine.install_prepared_snapshot(prepared);
    if kind == "patched_fixture"
        && let Ok(fixture) = serde_json::from_value::<light_fixture::PatchedFixture>(body.clone())
    {
        state
            .media_cache
            .lock()
            .clear_fixture(&fixture.fixture_id.0.to_string());
        state.media_status.write().remove(&fixture.fixture_id);
    }
    Ok(())
}
pub(super) async fn put_object(
    State(state): State<AppState>,
    Path((id, kind, object_id)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    let session = authenticate(&state, &headers)?;
    let expected = parse_if_match(&headers)?;
    let show_id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let activation = state.activation_lock.clone().lock_owned().await;
    let active = state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id);
    if active && kind == "route" {
        let action = output_route_action(
            &session,
            show_id,
            object_id,
            expected,
            light_application::OutputRouteMutation::Put { body },
        );
        let (result, _activation) = run_output_route_action(&state, activation, action).await?;
        terminate_changed_route(&state, result.route_to_terminate.as_ref()).await;
        let change = result.change;
        emit(
            &state,
            "show_object_changed",
            serde_json::json!({
                "show_id": change.show_id,
                "kind": "route",
                "id": change.route_id,
                "revision": change.object_revision
            }),
        );
        return Ok((
            [(header::ETAG, format!("\"{}\"", change.object_revision))],
            Json(serde_json::json!({
                "revision":change.object_revision,
                "event_sequence":result.event_sequence
            })),
        )
            .into_response());
    }
    if active {
        let object_kind = light_application::ActiveShowObjectKind::from_storage_kind(&kind);
        if let Some(object_kind) = object_kind {
            if object_kind == light_application::ActiveShowObjectKind::Preset {
                light_programmer::PresetAddress::parse(&object_id)
                    .map_err(ApiError::bad_request)?;
            }
            let action = active_show_object_action(
                operator_action_context(&session, light_application::ActionSource::Http),
                show_id,
                vec![put_active_show_object(
                    object_kind,
                    object_id.clone(),
                    expected,
                    body,
                )],
            );
            let (result, _activation) =
                run_active_show_object_action_async(&state, activation, action).await?;
            let change = result
                .changes
                .first()
                .expect("one requested object mutation returns one change");
            emit(
                &state,
                "show_object_changed",
                serde_json::json!({
                    "show_id": show_id,
                    "kind": kind,
                    "id": object_id,
                    "revision": change.object_revision
                }),
            );
            return Ok((
                [(header::ETAG, format!("\"{}\"", change.object_revision))],
                Json(serde_json::json!({
                    "revision":change.object_revision,
                    "event_sequence":result.event_sequence
                })),
            )
                .into_response());
        }
    }
    let body = normalize_object_body(&state, &kind, &object_id, body)?;
    validate_object_candidate(&state, &entry, &kind, &object_id, &body, active)?;
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    backup_show(&state, &entry)?;
    let revision = store
        .put_object(&kind, &object_id, &body, expected)
        .map_err(ApiError::store)?;
    if active {
        activate_object_change(&state, &entry, &kind, &body).await?;
    }
    emit(
        &state,
        "show_object_changed",
        serde_json::json!({"show_id":show_id,"kind":kind,"id":object_id,"revision":revision}),
    );
    Ok((
        [(header::ETAG, format!("\"{revision}\""))],
        Json(serde_json::json!({
            "revision":revision,
            "event_sequence":null
        })),
    )
        .into_response())
}

pub(super) async fn delete_object(
    State(state): State<AppState>,
    Path((id, kind, object_id)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let session = authenticate(&state, &headers)?;
    if kind != "route" {
        return Err(ApiError::bad_request(
            "generic object deletion is currently limited to output routes",
        ));
    }
    let expected = parse_if_match(&headers)?;
    let show_id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let activation = state.activation_lock.clone().lock_owned().await;
    let active = state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id);
    if active {
        let action = output_route_action(
            &session,
            show_id,
            object_id,
            expected,
            light_application::OutputRouteMutation::Delete,
        );
        let (result, _activation) = run_output_route_action(&state, activation, action).await?;
        terminate_changed_route(&state, result.route_to_terminate.as_ref()).await;
        let change = result.change;
        emit(
            &state,
            "show_object_changed",
            serde_json::json!({
                "show_id": change.show_id,
                "kind": "route",
                "id": change.route_id,
                "revision": change.object_revision,
                "deleted": true
            }),
        );
        return Ok(StatusCode::NO_CONTENT);
    }
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    let _object = store
        .objects(&kind)
        .map_err(ApiError::store)?
        .into_iter()
        .find(|object| object.id == object_id)
        .ok_or_else(|| ApiError::not_found("show object"))?;
    backup_show(&state, &entry)?;
    store
        .mutate_objects_atomically(
            &[],
            &[AtomicObjectDelete {
                kind: &kind,
                id: &object_id,
                expected,
            }],
        )
        .map_err(ApiError::store)?;
    emit(
        &state,
        "show_object_changed",
        serde_json::json!({"show_id":show_id,"kind":kind,"id":object_id,"revision":expected + 1,"deleted":true}),
    );
    Ok(StatusCode::NO_CONTENT)
}
