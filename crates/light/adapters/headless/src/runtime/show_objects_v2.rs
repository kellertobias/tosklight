//! Authenticated active-show object snapshots and typed output-route intents.

use super::object_api::{
    exact_object_snapshot, materialize_derived_group_memberships,
    materialize_patched_fixture_definitions, materialize_preset_addresses, output_route_action,
    output_route_range_action, run_output_route_action, run_output_route_range_action,
    terminate_changed_route,
};
use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::{events as event_wire, show_objects as wire};
use std::collections::VecDeque;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/objects/{kind}", get(collection_snapshot))
        .route("/api/v2/objects/{kind}/{id}", get(exact_snapshot))
        .route(
            "/api/v2/output-routes/actions",
            post(output_route_action_v2),
        )
}

async fn collection_snapshot(
    State(state): State<AppState>,
    Path(kind): Path<String>,
    context: ShowContext,
    headers: HeaderMap,
) -> Result<Json<wire::ShowObjectCollectionSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (show_revision, mut objects) = store
        .objects_with_portable_revision(&kind)
        .map_err(ApiError::store)?;
    materialize_collection(&entry, &kind, &mut objects)?;
    Ok(Json(wire::ShowObjectCollectionSnapshot {
        show_id: show_id.0,
        show_revision: show_revision.value(),
        kind,
        objects: objects.into_iter().map(object_record).collect(),
    }))
}

async fn exact_snapshot(
    State(state): State<AppState>,
    Path((kind, object_id)): Path<(String, String)>,
    context: ShowContext,
    headers: HeaderMap,
) -> Result<Json<wire::ShowObjectExactSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (show_revision, mut object) = exact_object_snapshot(&store, &kind, &object_id)?;
    if kind == "patched_fixture"
        && let Some(object) = object.as_mut()
    {
        materialize_patched_fixture_definitions(&entry, std::slice::from_mut(object))?;
    }
    Ok(Json(wire::ShowObjectExactSnapshot {
        show_id: show_id.0,
        show_revision: show_revision.value(),
        kind,
        object_id,
        object: object.map(object_record),
    }))
}

// @tour add-a-capability:30 Keep HTTP at the adapter edge
// The route authenticates, validates replay identity, maps wire intent into an application action,
// and translates the typed outcome without owning show semantics.
async fn output_route_action_v2(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::OutputRouteActionRequest>,
) -> Result<Json<wire::OutputRouteActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let key = ReplayKey {
        session_id: session.id.0,
        show_id,
        request_id: request.request_id.clone(),
    };
    if let Some(outcome) = state
        .replay
        .lookup_show_object(&key, &request.action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    // A concurrent request can pass the optimistic replay lookup before the first mutation
    // commits. Recheck after acquiring the mutation serializer so that it observes the completed
    // entry instead of applying the same request twice.
    if let Some(outcome) = state
        .replay
        .lookup_show_object(&key, &request.action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let (changes, event_sequence) = match &request.action {
        wire::OutputRouteAction::CreateRange {
            range_id,
            route,
            logical_universe_end,
            destination_universe_end,
        } => {
            let first_route = light_show::LosslessBody::<light_output::OutputRoute>::decode(
                serde_json::to_value(route)
                    .map_err(|error| ApiError::internal(error.to_string()))?,
            )
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
            let action = output_route_range_action(
                &session,
                show_id,
                *range_id,
                first_route,
                *logical_universe_end,
                *destination_universe_end,
            );
            let (result, _activation) =
                run_output_route_range_action(&state, activation, action).await?;
            (result.changes, result.event_sequence)
        }
        _ => {
            let (route_id, expected, mutation) = route_mutation(&state, show_id, &request.action)?;
            let action = output_route_action(&session, show_id, route_id, expected, mutation);
            let (result, _activation) = run_output_route_action(&state, activation, action).await?;
            terminate_changed_route(&state, result.route_to_terminate.as_ref()).await;
            (vec![result.change], result.event_sequence)
        }
    };
    for change in &changes {
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
    }
    let outcome = wire::OutputRouteActionOutcome {
        request_id: request.request_id,
        replayed: false,
        changes: changes.iter().map(wire_route_change).collect(),
        event_sequence,
    };
    state
        .replay
        .insert_show_object(key, request.action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

fn route_mutation(
    state: &AppState,
    show_id: light_core::ShowId,
    action: &wire::OutputRouteAction,
) -> Result<(String, u64, light_application::OutputRouteMutation), ApiError> {
    match action {
        wire::OutputRouteAction::Create { route_id, route } => Ok((
            route_id.clone(),
            0,
            light_application::OutputRouteMutation::Put {
                body: light_show::LosslessBody::<light_output::OutputRoute>::decode(
                    serde_json::to_value(route)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                )
                .map_err(|error| ApiError::bad_request(error.to_string()))?,
            },
        )),
        wire::OutputRouteAction::Update {
            route_id,
            expected_revision,
            patch,
        } => {
            let entry = active_entry(state, show_id)?;
            let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
            let (_show_revision, object) = store
                .object_with_portable_revision("route", route_id)
                .map_err(ApiError::store)?;
            let object = object.ok_or_else(|| ApiError::not_found("output route"))?;
            let body = apply_route_patch(object.body, patch)?;
            Ok((
                route_id.clone(),
                *expected_revision,
                light_application::OutputRouteMutation::Put {
                    body: light_show::LosslessBody::decode(body)
                        .map_err(|error| ApiError::bad_request(error.to_string()))?,
                },
            ))
        }
        wire::OutputRouteAction::Delete {
            route_id,
            expected_revision,
        } => Ok((
            route_id.clone(),
            *expected_revision,
            light_application::OutputRouteMutation::Delete,
        )),
        wire::OutputRouteAction::CreateRange { .. } => Err(ApiError::internal(
            "output route range action reached the single-route mutation path",
        )),
    }
}

fn apply_route_patch(
    mut body: serde_json::Value,
    patch: &wire::OutputRoutePatch,
) -> Result<serde_json::Value, ApiError> {
    let object = body
        .as_object_mut()
        .ok_or_else(|| ApiError::internal("stored output route is not an object"))?;
    macro_rules! patch {
        ($field:ident) => {
            if let Some(value) = &patch.$field {
                object.insert(
                    stringify!($field).to_owned(),
                    serde_json::to_value(value)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                );
            }
        };
    }
    patch!(protocol);
    patch!(logical_universe);
    patch!(destination_universe);
    patch!(delivery_mode);
    patch!(destination);
    patch!(enabled);
    patch!(minimum_slots);
    Ok(body)
}

pub(super) fn active_entry(
    state: &AppState,
    show_id: light_core::ShowId,
) -> Result<ShowEntry, ApiError> {
    state
        .active_show
        .current()
        .clone()
        .filter(|entry| entry.id == show_id)
        .ok_or_else(|| ApiError::conflict("requested show is not active"))
}

fn materialize_collection(
    entry: &ShowEntry,
    kind: &str,
    objects: &mut [light_show::VersionedObject],
) -> Result<(), ApiError> {
    match kind {
        "group" => materialize_derived_group_memberships(objects),
        "preset" => materialize_preset_addresses(objects)?,
        "patched_fixture" => materialize_patched_fixture_definitions(entry, objects)?,
        _ => {}
    }
    Ok(())
}

pub(super) fn object_record(object: light_show::VersionedObject) -> wire::ShowObjectRecord {
    let validation_error = if object.kind == "dynamic" {
        dynamic_validation_error(&object.body)
    } else {
        None
    };
    wire::ShowObjectRecord {
        kind: object.kind,
        id: object.id,
        revision: object.revision,
        updated_at: object.updated_at,
        body: object.body,
        validation_error,
    }
}

pub(super) fn dynamic_validation_error(body: &serde_json::Value) -> Option<String> {
    serde_json::from_value::<light_dynamics::DynamicDefinition>(body.clone())
        .map_err(|error| format!("Malformed Dynamic definition: {error}"))
        .and_then(|definition| {
            light_dynamics::validate_definition(&definition)
                .map_err(|error| format!("Invalid Dynamic definition: {error}"))
        })
        .err()
}

fn wire_route_change(
    change: &light_application::OutputRouteChange,
) -> event_wire::OutputRouteChange {
    event_wire::OutputRouteChange {
        show_id: change.show_id.0,
        show_revision: change.show_revision.value(),
        route_id: change.route_id.clone(),
        object_revision: change.object_revision,
        route: change.route.as_ref().map(wire_route),
        deleted: change.deleted,
    }
}

#[cfg(test)]
mod validation_tests {
    use super::object_record;

    #[test]
    fn malformed_dynamic_record_remains_visible_with_actionable_validation() {
        let record = object_record(light_show::VersionedObject {
            kind: "dynamic".into(),
            id: "broken".into(),
            body: serde_json::json!({
                "id": uuid::Uuid::new_v4(),
                "pool_number": 12,
                "revision": 1,
                "name": "Needs repair",
                "lanes": "not-an-array"
            }),
            revision: 3,
            updated_at: "2026-07-26T00:00:00Z".into(),
        });

        assert_eq!(record.id, "broken");
        assert_eq!(record.body["name"], "Needs repair");
        assert!(
            record
                .validation_error
                .as_deref()
                .is_some_and(|error| error.starts_with("Malformed Dynamic definition:"))
        );
    }
}

fn wire_route(route: &light_output::OutputRoute) -> event_wire::OutputRoute {
    event_wire::OutputRoute {
        protocol: match route.protocol {
            light_output::Protocol::ArtNet => event_wire::OutputProtocol::ArtNet,
            light_output::Protocol::Sacn => event_wire::OutputProtocol::Sacn,
        },
        logical_universe: route.logical_universe,
        destination_universe: route.destination_universe,
        delivery_mode: match route.resolved_delivery_mode() {
            light_output::DeliveryMode::Broadcast => event_wire::OutputDeliveryMode::Broadcast,
            light_output::DeliveryMode::Multicast => event_wire::OutputDeliveryMode::Multicast,
            light_output::DeliveryMode::Unicast => event_wire::OutputDeliveryMode::Unicast,
        },
        destination: route.destination.map(|destination| destination.to_string()),
        enabled: route.enabled,
        minimum_slots: route.minimum_slots,
    }
}

pub(super) fn validate_request_id(request_id: &str) -> Result<(), ApiError> {
    if request_id.trim().is_empty() || request_id.len() > 128 {
        return Err(ApiError::bad_request(
            "request_id must contain 1-128 characters",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    session_id: Uuid,
    show_id: light_core::ShowId,
    request_id: String,
}

struct ReplayEntry {
    action: wire::OutputRouteAction,
    outcome: wire::OutputRouteActionOutcome,
}

#[derive(Default)]
pub(super) struct ShowObjectReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl ShowObjectReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        action: &wire::OutputRouteAction,
    ) -> Result<Option<wire::OutputRouteActionOutcome>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.action != action {
            return Err(ApiError::conflict(
                "request_id was already used for a different output-route action",
            ));
        }
        let mut outcome = entry.outcome.clone();
        outcome.replayed = true;
        Ok(Some(outcome))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        action: wire::OutputRouteAction,
        outcome: wire::OutputRouteActionOutcome,
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
