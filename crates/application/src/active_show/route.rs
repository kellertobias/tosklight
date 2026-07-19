use super::{MutateOutputRouteCommand, OutputRouteMutation};
use crate::{ActionError, ActionErrorKind, prepare_show_candidate};
use light_core::Revision;
use light_output::OutputRoute;
use light_show::{PortableShowDocument, PortableShowObject, PortableShowTransaction};
use serde_json::Value;

pub(super) struct PreparedRouteMutation {
    pub(super) transaction: PortableShowTransaction,
    pub(super) snapshot: light_engine::EngineSnapshot,
    pub(super) route: Option<OutputRoute>,
    pub(super) route_to_terminate: Option<OutputRoute>,
    pub(super) deleted: bool,
    pub(super) object_revision: Revision,
}

pub(super) fn prepare_route_mutation(
    document: &PortableShowDocument,
    command: &MutateOutputRouteCommand,
) -> Result<PreparedRouteMutation, ActionError> {
    validate_active_show(document, command)?;
    let existing = document.object("route", &command.route_id);
    let object_revision = next_object_revision(existing, command)?;

    let (transaction, route, route_to_terminate, deleted) = match &command.mutation {
        OutputRouteMutation::Put { body } => {
            let route = decode_normalized_route(body)?;
            let body = merge_route_fields(existing.map(PortableShowObject::body), body, &route)?;
            let route_to_terminate = existing
                .and_then(|object| decode_stored_route(object.body()).ok())
                .filter(|previous| route_requires_termination(previous, Some(&route)));
            let mut transaction = document.transaction();
            transaction.put("route", command.route_id.clone(), body);
            (transaction, Some(route), route_to_terminate, false)
        }
        OutputRouteMutation::Delete => {
            let previous = existing
                .ok_or_else(|| not_found("output route not found"))
                .and_then(|object| decode_stored_route(object.body()))?;
            let route_to_terminate =
                route_requires_termination(&previous, None).then_some(previous);
            let mut transaction = document.transaction();
            transaction.delete("route", command.route_id.clone());
            (transaction, None, route_to_terminate, true)
        }
    };

    let prepared = prepare_show_candidate(document, transaction)?;
    let (transaction, snapshot) = prepared.into_parts();
    Ok(PreparedRouteMutation {
        transaction,
        snapshot,
        route,
        route_to_terminate,
        deleted,
        object_revision,
    })
}

fn validate_active_show(
    document: &PortableShowDocument,
    command: &MutateOutputRouteCommand,
) -> Result<(), ActionError> {
    if document.id() == command.show_id {
        Ok(())
    } else {
        Err(not_found("requested show is not active"))
    }
}

fn next_object_revision(
    existing: Option<&PortableShowObject>,
    command: &MutateOutputRouteCommand,
) -> Result<Revision, ActionError> {
    let current = existing.map_or(0, PortableShowObject::revision);
    if matches!(command.mutation, OutputRouteMutation::Delete) && existing.is_none() {
        return Err(not_found("output route not found"));
    }
    if current == command.expected_object_revision {
        current.checked_add(1).ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Invalid,
                "output route revision cannot be incremented",
            )
            .at_revision(current)
        })
    } else {
        Err(
            ActionError::new(ActionErrorKind::Conflict, "stale output route revision")
                .at_revision(current),
        )
    }
}

fn decode_normalized_route(body: &Value) -> Result<OutputRoute, ActionError> {
    let mut route = serde_json::from_value::<OutputRoute>(body.clone()).map_err(invalid)?;
    if route.delivery_mode.is_none() {
        route.delivery_mode = Some(route.resolved_delivery_mode());
    }
    route.validate().map_err(invalid)?;
    Ok(route)
}

fn decode_stored_route(body: &Value) -> Result<OutputRoute, ActionError> {
    serde_json::from_value(body.clone()).map_err(invalid)
}

/// Applies the typed route projection to retained raw JSON. Existing extension fields survive,
/// and extension fields supplied by a newer client are accepted without becoming application
/// dependencies.
fn merge_route_fields(
    existing: Option<&Value>,
    request: &Value,
    route: &OutputRoute,
) -> Result<Value, ActionError> {
    let request = request
        .as_object()
        .ok_or_else(|| invalid("output route body must be an object"))?;
    let mut merged = existing
        .map(|body| {
            body.as_object()
                .cloned()
                .ok_or_else(|| invalid("stored output route body must be an object"))
        })
        .transpose()?
        .unwrap_or_default();
    let canonical = serde_json::to_value(route).map_err(invalid)?;
    let canonical = canonical
        .as_object()
        .ok_or_else(|| invalid("serialized output route must be an object"))?;

    for (key, value) in request {
        if !canonical.contains_key(key) {
            merged.insert(key.clone(), value.clone());
        }
    }
    for (key, value) in canonical {
        merged.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(merged))
}

fn route_requires_termination(previous: &OutputRoute, next: Option<&OutputRoute>) -> bool {
    previous.enabled
        && next.is_none_or(|next| {
            !next.enabled
                || previous.protocol != next.protocol
                || previous.destination_universe != next.destination_universe
                || previous.resolved_delivery_mode() != next.resolved_delivery_mode()
                || previous.destination != next.destination
        })
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}
