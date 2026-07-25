use std::collections::HashSet;

use crate::{ActiveShowObjectsChange, SelectiveShowImportChange};

use super::model::EventObject;

pub(super) fn active_show_routes(change: &ActiveShowObjectsChange) -> Vec<EventObject> {
    unique_routes(
        change.show_id,
        change
            .changes
            .iter()
            .map(|item| (item.kind.as_str(), item.object_id.as_str())),
    )
}

pub(super) fn selective_import_routes(change: &SelectiveShowImportChange) -> Vec<EventObject> {
    unique_routes(
        change.show_id,
        change
            .objects
            .iter()
            .map(|item| (item.key.kind(), item.key.id())),
    )
}

fn unique_routes<'a>(
    show_id: light_core::ShowId,
    objects: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<EventObject> {
    let objects = objects.into_iter();
    let mut routes = Vec::with_capacity(objects.size_hint().0.saturating_mul(2));
    let mut seen = HashSet::with_capacity(routes.capacity());
    for (kind, object_id) in objects {
        let kind_route = EventObject::show_storage_object_kind(show_id, kind);
        if seen.insert(kind_route.clone()) {
            routes.push(kind_route);
        }
        let object_route = EventObject::show_storage_object(show_id, kind, object_id);
        if seen.insert(object_route.clone()) {
            routes.push(object_route);
        }
    }
    routes
}
